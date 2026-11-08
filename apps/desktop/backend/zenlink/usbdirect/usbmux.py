"""usbmux over raw USB: talk to services on the device (lockdownd on port
62078) through its mux interface (class 0xFF, subclass 0xFE), the way
usbmuxd does it on macOS and TikTok LIVE Studio's qcast does it on Windows.

Wire format (big-endian): a mux header — protocol, length, and for protocol
version 2 a 0xFEEDFACE magic plus tx/rx sequence numbers — followed by a
payload. TCP-like streams (protocol 6) carry a 20-byte TCP header; the device
expects SYN / SYN-ACK / ACK, sequence numbers, and ACKs for received data.
"""

from __future__ import annotations

import logging
import queue
import random
import struct
import threading

import usb.core

log = logging.getLogger(__name__)

PROTO_VERSION = 0
PROTO_CONTROL = 1
PROTO_SETUP = 2
PROTO_TCP = 6

TH_FIN, TH_SYN, TH_RST, TH_PUSH, TH_ACK = 0x01, 0x02, 0x04, 0x08, 0x10
MAGIC = 0xFEEDFACE
DEVICE_MAGIC = 0xFACEFACE
RX_WINDOW = 0x20000
LOCKDOWN_PORT = 62078


def _timeout(error: usb.core.USBError) -> bool:
    return isinstance(error, usb.core.USBTimeoutError) or "timeout" in str(error).lower()


class MuxError(RuntimeError):
    pass


class MuxStream:
    """One TCP-like connection to a device port. Blocking send/recv."""

    def __init__(self, mux: "UsbMux", sport: int, dport: int) -> None:
        self.mux = mux
        self.sport = sport
        self.dport = dport
        self.tx_seq = 0
        self.tx_ack = 0
        self.connected = threading.Event()
        self.closed = False
        self._rx: "queue.Queue[bytes | None]" = queue.Queue()
        self._buf = bytearray()

    def _segment(self, flags: int, data: bytes = b"") -> None:
        tcp = struct.pack(
            ">HHIIBBHHH",
            self.sport, self.dport, self.tx_seq, self.tx_ack,
            5 << 4, flags, RX_WINDOW >> 8, 0, 0,
        )
        self.mux._send(PROTO_TCP, tcp + data)

    def sendall(self, data: bytes) -> None:
        if self.closed:
            raise MuxError("connection closed by the device")
        for i in range(0, len(data), 16 * 1024):
            chunk = data[i : i + 16 * 1024]
            self._segment(TH_ACK, chunk)
            self.tx_seq = (self.tx_seq + len(chunk)) & 0xFFFFFFFF

    def recv_exact(self, n: int, timeout: float = 10.0) -> bytes:
        while len(self._buf) < n:
            try:
                item = self._rx.get(timeout=timeout)
            except queue.Empty:
                raise MuxError("timed out waiting for the device") from None
            if item is None:
                raise MuxError("connection closed by the device")
            self._buf += item
        out = bytes(self._buf[:n])
        del self._buf[:n]
        return out

    def recv_some(self, timeout: float = 10.0) -> bytes:
        if self._buf:
            out = bytes(self._buf)
            self._buf.clear()
            return out
        try:
            item = self._rx.get(timeout=timeout)
        except queue.Empty:
            raise MuxError("timed out waiting for the device") from None
        if item is None:
            raise MuxError("connection closed by the device")
        return item

    def close(self) -> None:
        if not self.closed:
            self.closed = True
            try:
                self._segment(TH_RST)
            except Exception:
                pass
            self.mux._streams.pop(self.sport, None)

    # called from the mux reader thread
    def _on_segment(self, flags: int, seq: int, data: bytes) -> None:
        if flags & TH_RST:
            self.closed = True
            self.connected.set()
            self._rx.put(None)
            return
        if not self.connected.is_set():
            if flags & TH_SYN and flags & TH_ACK:
                self.tx_seq = (self.tx_seq + 1) & 0xFFFFFFFF
                self.tx_ack = (seq + 1) & 0xFFFFFFFF
                self._segment(TH_ACK)
                self.connected.set()
            return
        if data:
            self.tx_ack = (seq + len(data)) & 0xFFFFFFFF
            self._segment(TH_ACK)
            self._rx.put(data)


class UsbMux:
    """usbmux protocol on an already-claimed mux interface."""

    def __init__(self, ep_in, ep_out) -> None:
        self.ep_in = ep_in
        self.ep_out = ep_out
        self.version = 0
        self.tx_seq = 0
        self.rx_seq = 0xFFFF
        self._streams: dict[int, MuxStream] = {}
        self._lock = threading.Lock()
        self._stop = threading.Event()
        self._version_ok = threading.Event()
        self._reader = threading.Thread(target=self._read_loop, name="usbmux", daemon=True)

    # ------------------------------------------------------------ framing

    def _send(self, proto: int, payload: bytes) -> None:
        with self._lock:
            if self.version >= 2:
                if proto == PROTO_SETUP:
                    self.tx_seq, self.rx_seq = 0, 0xFFFF
                header = struct.pack(">IIIHH", proto, 16 + len(payload), MAGIC, self.tx_seq, self.rx_seq)
                self.tx_seq = (self.tx_seq + 1) & 0xFFFF
            else:
                header = struct.pack(">II", proto, 8 + len(payload))
            packet = header + payload
            log.debug("mux tx proto=%d len=%d head=%s", proto, len(packet), packet[:40].hex())
            self.ep_out.write(packet, timeout=1000)
            if len(packet) % self.ep_out.wMaxPacketSize == 0:
                self.ep_out.write(b"", timeout=1000)  # zero-length packet ends the transfer

    def start(self, timeout: float = 5.0) -> None:
        self._reader.start()
        self._send(PROTO_VERSION, struct.pack(">III", 2, 0, 0))
        if not self._version_ok.wait(timeout):
            raise MuxError("the device did not answer the usbmux handshake")
        self._send(PROTO_SETUP, b"\x07")

    def stop(self) -> None:
        self._stop.set()
        for stream in list(self._streams.values()):
            stream.close()

    def connect(self, port: int = LOCKDOWN_PORT, timeout: float = 5.0) -> MuxStream:
        sport = random.randint(0x4000, 0xFFF0)
        stream = MuxStream(self, sport, port)
        self._streams[sport] = stream
        stream._segment(TH_SYN)
        if not stream.connected.wait(timeout) or stream.closed:
            self._streams.pop(sport, None)
            raise MuxError(f"the device refused a connection to port {port}")
        return stream

    # ------------------------------------------------------------ reader

    def _read_loop(self) -> None:
        buf = bytearray()
        while not self._stop.is_set():
            try:
                buf += bytes(self.ep_in.read(16 * 1024, timeout=200))
            except usb.core.USBError as error:
                if _timeout(error):
                    continue
                log.debug("usbmux read stopped: %s", error)
                break
            while True:
                if len(buf) < 8:
                    break
                proto, length = struct.unpack_from(">II", buf)
                if length < 8 or len(buf) < length:
                    break
                packet = bytes(buf[:length])
                del buf[:length]
                self._dispatch(proto, packet)
        for stream in list(self._streams.values()):
            stream.closed = True
            stream.connected.set()
            stream._rx.put(None)

    def _dispatch(self, proto: int, packet: bytes) -> None:
        log.debug("mux rx proto=%d len=%d head=%s", proto, len(packet), packet[:40].hex())
        if proto == PROTO_VERSION:
            major = struct.unpack_from(">I", packet, 8)[0] if len(packet) >= 12 else 1
            self.version = 2 if major >= 2 else 1
            self._version_ok.set()
            return
        if self.version >= 2:
            # The host sends 0xFEEDFACE; the device answers with 0xFACEFACE.
            if len(packet) < 16 or struct.unpack_from(">I", packet, 8)[0] not in (MAGIC, DEVICE_MAGIC):
                return
            # Acknowledge the device's packet counter (its tx_seq).
            self.rx_seq = struct.unpack_from(">H", packet, 12)[0]
            body = packet[16:]
        else:
            body = packet[8:]
        if proto != PROTO_TCP or len(body) < 20:
            return
        sport, dport, seq, _ack, off, flags = struct.unpack_from(">HHIIBB", body)
        data = body[(off >> 4) * 4 :]
        stream = self._streams.get(dport)
        if stream is not None:
            stream._on_segment(flags, seq, data)
