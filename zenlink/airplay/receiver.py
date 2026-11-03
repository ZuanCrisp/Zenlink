"""AirPlay screen-mirroring receiver (Wi-Fi) for ZenLink Studio.

The iPhone/iPad opens Control Center > Screen Mirroring > "ZenLink". The
protocol follows UxPlay / shairplay (GPL-3.0), the AirPlay 1 mirroring
protocol an Apple TV 3 speaks:

  mDNS     _airplay._tcp + _raop._tcp adverts (zeroconf)
  RTSP     GET /info, pair-setup / pair-verify (legacy Ed25519 + X25519),
           fp-setup (FairPlay, fairplay.py), SETUP, RECORD, GET/SET_PARAMETER,
           /feedback heartbeats, TEARDOWN
  video    TCP stream of 128-byte headers + payloads; type 1 carries the
           avcC configuration, type 0 an AES-CTR encrypted, length-prefixed
           H.264 access unit
  audio    RTP over UDP, AES-CBC encrypted AAC-ELD 44.1 kHz stereo

Video goes to the USB Direct MediaHub (same player, same /cast/* routes);
audio is decoded with PyAV, resampled to 48 kHz and pushed to the hub too.
"""

from __future__ import annotations

import asyncio
import hashlib
import json
import logging
import os
import plistlib
import secrets
import socket
import struct
import time
from typing import Callable

from cryptography.hazmat.primitives import serialization
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey, Ed25519PublicKey
from cryptography.hazmat.primitives.asymmetric.x25519 import X25519PrivateKey, X25519PublicKey
from Crypto.Cipher import AES

from zenlink.paths import DATA_DIR

from .fairplay import FairPlay

log = logging.getLogger(__name__)

AIRPLAY_PORT = 7000
MODEL = "AppleTV3,2"
SOURCE_VERSION = "220.68"
# Apple TV 3 mirroring with legacy pairing; bit 42 (SupportsScreenMultiCodec)
# lets iOS send HEVC. ZenLink passes the device's stream through untouched
# (no re-encoding): HEVC at the device's own resolution by default.
FEATURES = (0x5A7FFEE6, 0x400)
FEATURES_H264 = (0x5A7FFEE6, 0x0)
# The screen ZenLink offers iOS. iOS mirrors at its own native size up to
# this, so offering 4K means "send me everything you've got".
DISPLAY = (3840, 2160)
PI = "2e388006-13ba-4041-9a67-25dd4a43d536"
AAC_ELD_44K = bytes.fromhex("f8e85000")


def _sha512_16(*parts: bytes) -> bytes:
    h = hashlib.sha512()
    for p in parts:
        h.update(p)
    return h.digest()[:16]


def _identity() -> dict:
    """Stable receiver identity: Ed25519 key and a locally administered MAC."""
    path = DATA_DIR / "airplay.json"
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
        if len(bytes.fromhex(data["seed"])) == 32 and len(data["mac"]) == 17:
            return data
    except (OSError, ValueError, KeyError):
        pass
    mac = bytearray(secrets.token_bytes(6))
    mac[0] = (mac[0] | 0x02) & 0xFE
    data = {"seed": secrets.token_hex(32), "mac": ":".join(f"{b:02X}" for b in mac)}
    try:
        path.write_text(json.dumps(data), encoding="utf-8")
    except OSError:
        pass
    return data


def _lan_ipv4() -> list[str]:
    import ifaddr

    out = []
    for adapter in ifaddr.get_adapters():
        name = f"{adapter.nice_name} {adapter.name}".lower()
        if any(s in name for s in ("vethernet", "virtual", "vmware", "hyper-v", "loopback", "wsl", "tap")):
            continue
        for ip in adapter.ips:
            if isinstance(ip.ip, str) and not ip.ip.startswith(("127.", "169.254.")):
                out.append(ip.ip)
    return out


class _Conn:
    """Per-RTSP-connection state."""

    def __init__(self, peer: str) -> None:
        self.peer = peer
        self.fairplay = FairPlay()
        self.ecdh_secret: bytes | None = None
        self.ecdh_ours: X25519PrivateKey | None = None
        self.ecdh_theirs: bytes = b""
        self.ed_theirs: bytes = b""
        self.aeskey: bytes | None = None
        self.aesiv: bytes | None = None


class AirPlayReceiver:
    def __init__(self, name: str, hub, on_event: Callable[[str, str], None], hevc: bool = True) -> None:
        self.features = FEATURES if hevc else FEATURES_H264
        self.name = name
        self.hub = hub
        self.on_event = on_event  # ("waiting"|"connected"|"streaming"|"stopped", detail)
        ident = _identity()
        self._ed = Ed25519PrivateKey.from_private_bytes(bytes.fromhex(ident["seed"]))
        self._pk = self._ed.public_key().public_bytes(serialization.Encoding.Raw, serialization.PublicFormat.Raw)
        self.mac = ident["mac"]
        self.display = DISPLAY
        self._servers: list[asyncio.AbstractServer] = []
        self._zc = None
        self._infos = []
        self._tasks: set[asyncio.Task] = set()
        self._mirror_server: asyncio.AbstractServer | None = None
        self._video_writers: set[asyncio.StreamWriter] = set()
        self._rtsp_writers: dict[str, asyncio.StreamWriter] = {}
        self._audio_transport = None
        self._control_transport = None
        self._timing_transport = None
        self.client: str | None = None
        self.stats = {"videoBytes": 0, "frames": 0, "audioPackets": 0, "width": 0, "height": 0}

    # ------------------------------------------------------------ lifecycle

    async def start(self) -> None:
        server = await asyncio.start_server(self._handle_rtsp, "0.0.0.0", AIRPLAY_PORT)
        self._servers.append(server)
        await asyncio.to_thread(self._advertise)
        log.info("airplay: receiver \"%s\" on port %d (%s)", self.name, AIRPLAY_PORT, ", ".join(_lan_ipv4()))
        self.on_event("waiting", "")

    async def stop(self) -> None:
        for task in list(self._tasks):
            task.cancel()
        self._close_streams()
        for server in self._servers:
            server.close()
        self._servers.clear()
        if self._zc is not None:
            zc, self._zc = self._zc, None
            await asyncio.to_thread(self._unadvertise, zc)

    def _close_streams(self) -> None:
        if self._mirror_server is not None:
            self._mirror_server.close()
            self._mirror_server = None
        # Closing the listening server leaves accepted connections open: cut the
        # live video connections too, or a replaced device keeps feeding frames
        # into the same picture (two streams mixed -> decode errors, drops).
        for w in list(self._video_writers):
            w.close()
        self._video_writers.clear()
        for name in ("_audio_transport", "_control_transport", "_timing_transport"):
            t = getattr(self, name)
            if t is not None:
                t.close()
                setattr(self, name, None)

    # ------------------------------------------------------------ mDNS

    def _txt_airplay(self) -> dict:
        return {
            "deviceid": self.mac, "features": f"0x{self.features[0]:X},0x{self.features[1]:X}", "flags": "0x4",
            "model": MODEL, "pk": self._pk.hex(), "pi": PI, "srcvers": SOURCE_VERSION, "vv": "2",
        }

    def _txt_raop(self) -> dict:
        return {
            "ch": "2", "cn": "0,1,2,3", "da": "true", "et": "0,3,5", "vv": "2",
            "ft": f"0x{self.features[0]:X},0x{self.features[1]:X}", "am": MODEL, "md": "0,1,2", "rhd": "5.6.0.0",
            "pw": "false", "sf": "0x4", "sr": "44100", "ss": "16", "sv": "false", "tp": "UDP",
            "txtvers": "1", "vs": SOURCE_VERSION, "vn": "65537", "pk": self._pk.hex(),
        }

    @staticmethod
    def _txt_bytes(txt: dict) -> bytes:
        out = b""
        for k, v in txt.items():
            item = f"{k}={v}".encode()
            out += bytes([len(item)]) + item
        return out

    def _advertise(self) -> None:
        from zeroconf import IPVersion, ServiceInfo, Zeroconf

        ips = _lan_ipv4()
        addresses = [socket.inet_aton(ip) for ip in ips]
        host = f"ZenLink-{self.mac.replace(':', '')[-6:]}.local."
        self._zc = Zeroconf(interfaces=ips or None, ip_version=IPVersion.V4Only)
        raop_name = self.mac.replace(":", "") + "@" + self.name
        self._infos = [
            ServiceInfo("_airplay._tcp.local.", f"{self.name}._airplay._tcp.local.", addresses=addresses,
                        port=AIRPLAY_PORT, properties=self._txt_airplay(), server=host),
            ServiceInfo("_raop._tcp.local.", f"{raop_name}._raop._tcp.local.", addresses=addresses,
                        port=AIRPLAY_PORT, properties=self._txt_raop(), server=host),
        ]
        for info in self._infos:
            self._zc.register_service(info, allow_name_change=True)

    def _unadvertise(self, zc) -> None:
        try:
            for info in self._infos:
                zc.unregister_service(info)
        finally:
            zc.close()

    # ------------------------------------------------------------ RTSP

    async def _handle_rtsp(self, reader: asyncio.StreamReader, writer: asyncio.StreamWriter) -> None:
        peer = writer.get_extra_info("peername")[0]
        conn = _Conn(peer)
        self._rtsp_writers[peer] = writer
        log.info("airplay: connection from %s", peer)
        try:
            while True:
                head = await reader.readuntil(b"\r\n\r\n")
                lines = head.decode("latin-1").split("\r\n")
                method, url, _proto = (lines[0].split(" ") + ["", ""])[:3]
                headers = {}
                for line in lines[1:]:
                    if ":" in line:
                        k, v = line.split(":", 1)
                        headers[k.strip().lower()] = v.strip()
                body = await reader.readexactly(int(headers.get("content-length", "0") or 0))
                status, extra, data = await self._dispatch(conn, method, url, headers, body)
                if not (method == "POST" and url == "/feedback"):
                    log.info("airplay: %s %s (%d bytes) -> %s (%d bytes)", method, url, len(body), status, len(data))
                reply = [f"RTSP/1.0 {status}", f"CSeq: {headers.get('cseq', '0')}", "Server: AirTunes/" + SOURCE_VERSION]
                for k, v in extra.items():
                    reply.append(f"{k}: {v}")
                reply.append(f"Content-Length: {len(data)}")
                writer.write(("\r\n".join(reply) + "\r\n\r\n").encode() + data)
                await writer.drain()
                if method == "TEARDOWN" and self._mirror_server is None and self._audio_transport is None:
                    break
        except (asyncio.IncompleteReadError, ConnectionError, asyncio.LimitOverrunError):
            pass
        except Exception:  # noqa: BLE001
            log.exception("airplay: RTSP handler failed")
        finally:
            writer.close()
            if self._rtsp_writers.get(peer) is writer:
                self._rtsp_writers.pop(peer, None)
            log.info("airplay: %s disconnected", peer)
            if self.client == peer:
                self.client = None
                self._close_streams()
                self.on_event("waiting", "")

    async def _dispatch(self, conn: _Conn, method: str, url: str, headers: dict, body: bytes):
        ok = "200 OK"
        bplist = {"Content-Type": "application/x-apple-binary-plist"}
        octet = {"Content-Type": "application/octet-stream"}
        if method == "GET" and url == "/info":
            return ok, bplist, self._info(headers, body)
        if method == "POST" and url == "/pair-setup":
            return ok, octet, self._pk
        if method == "POST" and url == "/pair-verify":
            return ok, octet, self._pair_verify(conn, body)
        if method == "POST" and url == "/fp-setup":
            try:
                if len(body) == 16:
                    return ok, octet, conn.fairplay.setup(body)
                return ok, octet, conn.fairplay.handshake(body)
            except (ValueError, IndexError):
                return "501 Not Implemented", {}, b""
        if method == "OPTIONS":
            return ok, {"Public": "SETUP, RECORD, FLUSH, TEARDOWN, OPTIONS, GET_PARAMETER, SET_PARAMETER"}, b""
        if method == "SETUP":
            return ok, bplist, await self._setup(conn, body)
        if method == "GET_PARAMETER":
            if b"volume" in body:
                return ok, {"Content-Type": "text/parameters"}, b"volume: 0.000000\r\n"
            return ok, {}, b""
        if method == "RECORD":
            return ok, {"Audio-Latency": "11025", "Audio-Jack-Status": "connected; type=analog"}, b""
        if method == "TEARDOWN":
            req = self._plist(body)
            streams = [s.get("type") for s in req.get("streams", [])] if isinstance(req, dict) else []
            if 110 in streams or not streams:
                if self._mirror_server is not None:
                    self._mirror_server.close()
                    self._mirror_server = None
            if 96 in streams or not streams:
                for name in ("_audio_transport", "_control_transport"):
                    t = getattr(self, name)
                    if t is not None:
                        t.close()
                        setattr(self, name, None)
            if not streams:
                self.on_event("waiting", "")
            return ok, {}, b""
        # SET_PARAMETER, FLUSH, POST /feedback, POST /audioMode, POST /command ...
        return ok, {}, b""

    @staticmethod
    def _plist(body: bytes):
        try:
            return plistlib.loads(body) if body else {}
        except Exception:  # noqa: BLE001
            return {}

    def _info(self, headers: dict, body: bytes) -> bytes:
        res: dict = {}
        req = self._plist(body) if "binary-plist" in headers.get("content-type", "") else {}
        qualifier = (req.get("qualifier") or [None])[0] if isinstance(req, dict) else None
        if qualifier == "txtAirPlay":
            res["txtAirPlay"] = self._txt_bytes(self._txt_airplay())
        elif qualifier == "txtRAOP":
            res["txtRAOP"] = self._txt_bytes(self._txt_raop())
        if headers.get("content-type"):
            return plistlib.dumps(res, fmt=plistlib.FMT_BINARY)
        w, h = self.display
        res.update({
            "deviceID": self.mac, "macAddress": self.mac, "pk": self._pk,
            "features": (self.features[1] << 32) | self.features[0], "name": self.name, "pi": PI, "vv": 2,
            "statusFlags": 68, "keepAliveLowPower": True, "sourceVersion": SOURCE_VERSION,
            "keepAliveSendStatsAsBody": True, "model": MODEL, "initialVolume": 0.0,
            "audioLatencies": [
                {"type": 100, "audioType": "default", "inputLatencyMicros": 0, "outputLatencyMicros": False},
                {"type": 101, "audioType": "default", "inputLatencyMicros": 0, "outputLatencyMicros": False},
            ],
            "audioFormats": [
                {"type": 100, "audioInputFormats": 0x3FFFFFC, "audioOutputFormats": 0x3FFFFFC},
                {"type": 101, "audioInputFormats": 0x3FFFFFC, "audioOutputFormats": 0x3FFFFFC},
            ],
            "displays": [{
                "uuid": "e0ff8a27-6738-3d56-8a16-cc53aacee925", "widthPhysical": 0, "heightPhysical": 0,
                "width": w, "height": h, "widthPixels": w, "heightPixels": h, "rotation": False,
                "refreshRate": 1.0 / 60, "maxFPS": 60, "overscanned": False, "features": 14,
            }],
        })
        return plistlib.dumps(res, fmt=plistlib.FMT_BINARY)

    def _pair_verify(self, conn: _Conn, body: bytes) -> bytes:
        if len(body) < 4:
            return b""
        if body[0] == 1 and len(body) == 4 + 32 + 32:
            conn.ecdh_theirs = body[4:36]
            conn.ed_theirs = body[36:68]
            conn.ecdh_ours = X25519PrivateKey.generate()
            ours = conn.ecdh_ours.public_key().public_bytes(serialization.Encoding.Raw, serialization.PublicFormat.Raw)
            conn.ecdh_secret = conn.ecdh_ours.exchange(X25519PublicKey.from_public_bytes(conn.ecdh_theirs))
            signature = self._ed.sign(ours + conn.ecdh_theirs)
            key = _sha512_16(b"Pair-Verify-AES-Key", conn.ecdh_secret)
            iv = _sha512_16(b"Pair-Verify-AES-IV", conn.ecdh_secret)
            enc = AES.new(key, AES.MODE_CTR, nonce=b"", initial_value=iv).encrypt(signature)
            return ours + enc
        if body[0] == 0 and len(body) == 4 + 64 and conn.ecdh_secret is not None:
            key = _sha512_16(b"Pair-Verify-AES-Key", conn.ecdh_secret)
            iv = _sha512_16(b"Pair-Verify-AES-IV", conn.ecdh_secret)
            ctr = AES.new(key, AES.MODE_CTR, nonce=b"", initial_value=iv)
            ctr.encrypt(bytes(64))  # the first 64 bytes of keystream signed our reply
            signature = ctr.encrypt(body[4:])  # CTR: encrypt == decrypt
            ours = conn.ecdh_ours.public_key().public_bytes(serialization.Encoding.Raw, serialization.PublicFormat.Raw)
            try:
                Ed25519PublicKey.from_public_bytes(conn.ed_theirs).verify(signature, conn.ecdh_theirs + ours)
            except Exception:  # noqa: BLE001
                log.warning("airplay: pair-verify signature mismatch (continuing)")
        return b""

    async def _setup(self, conn: _Conn, body: bytes) -> bytes:
        req = self._plist(body)
        res: dict = {}
        if isinstance(req, dict) and "ekey" in req and "eiv" in req:
            aeskey = conn.fairplay.decrypt(bytes(req["ekey"]))
            if conn.ecdh_secret is not None:
                aeskey = hashlib.sha512(aeskey + conn.ecdh_secret).digest()[:16]
            conn.aeskey, conn.aesiv = aeskey, bytes(req["eiv"])
            if self.client and self.client != conn.peer:
                log.info("airplay: %s replaces %s", conn.peer, self.client)
                old = self._rtsp_writers.pop(self.client, None)
                if old is not None:
                    old.close()  # the previous device's session ends for real
            self._close_streams()
            self.client = conn.peer
            client_name = req.get("name") or req.get("model") or "iPhone/iPad"
            log.info("airplay: %s (%s) connected", client_name, req.get("model"))
            self.on_event("connected", str(client_name))
            loop = asyncio.get_running_loop()
            self._timing_transport, _ = await loop.create_datagram_endpoint(asyncio.DatagramProtocol, local_addr=("0.0.0.0", 0))
            res["timingPort"] = self._timing_transport.get_extra_info("sockname")[1]
            res["eventPort"] = 0
            timing_port = int(req.get("timingPort") or 0)
            if timing_port:
                self._spawn(self._ntp_requests(conn.peer, timing_port))
        streams = []
        for stream in (req.get("streams") or []) if isinstance(req, dict) else []:
            kind = stream.get("type")
            if kind == 110 and conn.aeskey:
                # iOS sends this as an 8-byte integer, which plistlib reads as
                # *signed*: IDs >= 2**63 came out negative and the video key was
                # wrong (no picture in about half the sessions). It is unsigned.
                stream_id = int(stream.get("streamConnectionID") or 0) & 0xFFFFFFFFFFFFFFFF
                log.info("airplay: video stream id %d", stream_id)
                port = await self._start_mirror(conn, stream_id)
                streams.append({"dataPort": port, "type": 110})
            elif kind == 96 and conn.aeskey:
                data_port, control_port = await self._start_audio(conn, stream)
                streams.append({"dataPort": data_port, "controlPort": control_port, "type": 96})
        if streams:
            res["streams"] = streams
        return plistlib.dumps(res, fmt=plistlib.FMT_BINARY)

    def _spawn(self, coro) -> None:
        task = asyncio.ensure_future(coro)
        self._tasks.add(task)
        task.add_done_callback(self._tasks.discard)

    async def _ntp_requests(self, peer: str, port: int) -> None:
        """Ask the sender for its clock now and then, as UxPlay does (keeps it happy)."""
        try:
            while self._timing_transport is not None:
                now = time.time() + 2208988800
                ts = struct.pack(">II", int(now), int((now % 1) * (1 << 32)))
                self._timing_transport.sendto(bytes([0x80, 0xD2, 0x00, 0x07]) + bytes(20) + ts, (peer, port))
                await asyncio.sleep(3)
        except (OSError, AttributeError):
            pass

    # ------------------------------------------------------------ video

    async def _start_mirror(self, conn: _Conn, stream_id: int) -> int:
        key = _sha512_16(f"AirPlayStreamKey{stream_id}".encode(), conn.aeskey)
        iv = _sha512_16(f"AirPlayStreamIV{stream_id}".encode(), conn.aeskey)
        if self._mirror_server is not None:
            self._mirror_server.close()
        count = {"n": 0}

        async def handle(reader: asyncio.StreamReader, writer: asyncio.StreamWriter) -> None:
            # Each video connection starts its own keystream: one shared cipher
            # went out of step when iOS opened the connection again (garbage
            # frames, picture never appeared while sound played).
            count["n"] += 1
            log.info("airplay: video connection #%d from %s", count["n"], writer.get_extra_info("peername"))
            self._video_writers.add(writer)
            try:
                await self._mirror_loop(reader, _VideoDecryptor(key, iv))
            finally:
                self._video_writers.discard(writer)
                writer.close()

        self._mirror_server = await asyncio.start_server(handle, "0.0.0.0", 0)
        return self._mirror_server.sockets[0].getsockname()[1]

    async def _mirror_loop(self, reader: asyncio.StreamReader, cipher) -> None:
        streaming = False
        win = {"t0": time.monotonic(), "frames": 0, "bytes": 0, "gap": 0.0, "last": 0.0}
        try:
            while True:
                header = await reader.readexactly(128)
                now = time.monotonic()
                if header[4] == 0:
                    if win["last"]:
                        win["gap"] = max(win["gap"], now - win["last"])
                    win["last"] = now
                if now - win["t0"] >= 5:
                    secs = now - win["t0"]
                    log.info("airplay: received %.1f fps, %.1f Mbps, longest gap %d ms",
                             win["frames"] / secs, win["bytes"] * 8 / secs / 1e6, win["gap"] * 1000)
                    win.update(t0=now, frames=0, bytes=0, gap=0.0)
                size = struct.unpack_from("<I", header, 0)[0]
                kind = header[4]
                payload = await reader.readexactly(size) if size else b""
                if kind == 0:  # encrypted, length-prefixed H.264/HEVC access unit
                    data = cipher.decrypt(payload)
                    if data is None:
                        continue
                    win["frames"] += 1
                    win["bytes"] += len(data)
                    self.stats["videoBytes"] += len(data)
                    self.stats["frames"] += 1
                    self.hub.push_video(data)
                    if not streaming and self.hub.codec():
                        streaming = True
                        self.on_event("streaming", "")
                elif kind == 1 and payload:  # decoder configuration and the picture size
                    width = int(struct.unpack_from("<f", header, 56)[0])
                    height = int(struct.unpack_from("<f", header, 60)[0])
                    self.stats["width"], self.stats["height"] = width, height
                    if payload[4:8] == b"hvc1":
                        config = _hvcc_from_sample_entry(bytes(payload))
                        if config is None:
                            log.warning("airplay: HEVC configuration not understood: %s", bytes(payload[:160]).hex())
                            continue
                        name = "HEVC"
                    else:
                        config, name = bytes(payload), "H.264"
                    codec = _codec_string(config, name)
                    self.stats["codec"] = name
                    log.info("airplay: video %sx%s %s %s", width, height, name, codec)
                    self.hub.set_codec(codec, config, width, height, name)
                # kind 2 / 5: heartbeat and streaming report
        except (asyncio.IncompleteReadError, ConnectionError):
            pass
        except Exception:  # noqa: BLE001
            log.exception("airplay: mirror stream failed")

    # ------------------------------------------------------------ audio

    async def _start_audio(self, conn: _Conn, stream: dict) -> tuple[int, int]:
        loop = asyncio.get_running_loop()
        receiver = self
        log.info("airplay: audio stream ct=%s spf=%s format=%s", stream.get("ct"), stream.get("spf"), stream.get("audioFormat"))
        decoder = _AudioDecoder(conn.aeskey, conn.aesiv, int(stream.get("ct") or 8))
        reorder = _Reorder()

        class Data(asyncio.DatagramProtocol):
            def datagram_received(self, data: bytes, addr) -> None:
                if len(data) < 12:
                    return
                receiver.stats["audioPackets"] += 1
                for packet in reorder.push(data):
                    pcm = decoder.decode(packet)
                    if pcm:
                        receiver.hub.push_audio(pcm)

        for name in ("_audio_transport", "_control_transport"):
            t = getattr(self, name)
            if t is not None:
                t.close()
        self._audio_transport, _ = await loop.create_datagram_endpoint(Data, local_addr=("0.0.0.0", 0))
        self._control_transport, _ = await loop.create_datagram_endpoint(asyncio.DatagramProtocol, local_addr=("0.0.0.0", 0))
        return (self._audio_transport.get_extra_info("sockname")[1],
                self._control_transport.get_extra_info("sockname")[1])


def _valid_au(data: bytes) -> bool:
    """A decrypted access unit is a chain of 4-byte length-prefixed NAL units
    that ends exactly at the end of the buffer."""
    pos, n = 0, len(data)
    if n < 5:
        return False
    while pos + 4 <= n:
        size = int.from_bytes(data[pos:pos + 4], "big")
        if size <= 0 or pos + 4 + size > n or data[pos + 4] & 0x80:  # forbidden_zero_bit
            return False
        pos += 4 + size
    return pos == n


class _VideoDecryptor:
    """AES-CTR over the whole video connection, with a guard: a frame that
    doesn't decrypt to valid NAL units is retried with a fresh keystream (iOS
    restarted it); if that is valid, the stream re-synchronises there.
    Garbage is never passed on to the decoder."""

    def __init__(self, key: bytes, iv: bytes) -> None:
        self.key, self.iv = key, iv
        self.cipher = self._fresh()
        self.bad = 0

    def _fresh(self):
        return AES.new(self.key, AES.MODE_CTR, nonce=b"", initial_value=self.iv)

    def decrypt(self, payload: bytes) -> bytes | None:
        data = self.cipher.decrypt(payload)
        if _valid_au(data):
            return data
        fresh = self._fresh()
        retry = fresh.decrypt(payload)
        if _valid_au(retry):
            log.warning("airplay: video keystream restarted by the device; re-synchronised")
            self.cipher = fresh
            return retry
        self.bad += 1
        if self.bad in (1, 30, 300):
            log.warning("airplay: %d video frame(s) didn't decrypt to valid data; skipped", self.bad)
        return None


def _codec_string(config: bytes, name: str) -> str:
    """WebCodecs codec string from the configuration record of a known codec."""
    if name == "H.264":
        return "avc1." + config[1:4].hex() if len(config) >= 4 else "avc1.640033"
    from zenlink.usbdirect.wire import CODEC_HEVC, FormatDescription

    return FormatDescription(media_type=0, codec=CODEC_HEVC, config=config).codec_string


def _hvcc_from_sample_entry(entry: bytes) -> bytes | None:
    """The HEVC codec packet is an 'hvc1' sample entry; its 'hvcC' box is the
    HEVCDecoderConfigurationRecord WebCodecs wants as the description."""
    pos = entry.find(b"hvcC")
    if pos < 4:
        return None
    size = int.from_bytes(entry[pos - 4:pos], "big")
    if size < 31 or pos - 4 + size > len(entry):
        return None
    return entry[pos + 4:pos - 4 + size]


class _Reorder:
    """Wi-Fi delivers RTP audio slightly out of order; release it in sequence.
    Holds at most `depth` packets (~10 ms each) before skipping a lost one."""

    def __init__(self, depth: int = 6) -> None:
        self.depth = depth
        self.next: int | None = None
        self.pending: dict[int, bytes] = {}

    def push(self, packet: bytes) -> list[bytes]:
        seq = struct.unpack_from(">H", packet, 2)[0]
        if self.next is None:
            self.next = seq
        if ((seq - self.next) & 0xFFFF) > 0x8000:  # older than what was already played
            return []
        self.pending[seq] = packet
        out = []
        while True:
            if self.next in self.pending:
                out.append(self.pending.pop(self.next))
                self.next = (self.next + 1) & 0xFFFF
            elif len(self.pending) > self.depth:
                self.next = (self.next + 1) & 0xFFFF  # give up on a lost packet
            else:
                return out


class _AudioDecoder:
    """RTP -> AES-CBC decrypt -> AAC-ELD (PyAV) -> s16le stereo 48 kHz."""

    def __init__(self, key: bytes, iv: bytes, ct: int) -> None:
        import av

        self._key, self._iv = key, iv
        self._ok = ct == 8
        if not self._ok:
            log.warning("airplay: audio type ct=%s not supported (only AAC-ELD)", ct)
            return
        cookie = bytearray(AAC_ELD_44K)
        cookie[2] |= 0x10  # ELD frameLengthFlag = 1 (480 samples), as in apple/audio.py
        self._av = av
        self._ctx = av.codec.CodecContext.create("aac", "r")
        self._ctx.extradata = bytes(cookie)
        self._ctx.sample_rate = 44100
        self._ctx.layout = "stereo"
        self._ctx.open()
        self._resampler = av.AudioResampler(format="s16", layout="stereo", rate=48000)
        self._last_seq = None
        self.decoded = 0
        self.failed = 0

    def decode(self, packet: bytes) -> bytes:
        if not self._ok:
            return b""
        seq = struct.unpack_from(">H", packet, 2)[0]
        if seq == self._last_seq:
            return b""
        self._last_seq = seq
        payload = packet[12:]
        n = len(payload) // 16 * 16
        if n:
            payload = AES.new(self._key, AES.MODE_CBC, self._iv).decrypt(payload[:n]) + payload[n:]
        if not payload or payload[0] not in (0x8C, 0x8D, 0x8E, 0x80, 0x81, 0x82, 0x20):
            self.failed += 1
            if self.failed in (1, 50, 500):
                log.warning("airplay: audio packet not AAC-ELD after decryption (first byte %02x, %d so far)",
                            payload[0] if payload else 0, self.failed)
            return b""
        out = bytearray()
        try:
            for frame in self._ctx.decode(self._av.Packet(payload)):
                for pcm in self._resampler.resample(frame):
                    out += bytes(pcm.planes[0])[: pcm.samples * 4]
        except Exception as error:  # noqa: BLE001
            self.failed += 1
            if self.failed in (1, 50, 500):
                log.warning("airplay: audio decode failed: %s (%d so far)", error, self.failed)
            return b""
        if out and not self.decoded:
            log.info("airplay: sound is playing (AAC-ELD 44.1 kHz -> 48 kHz)")
        self.decoded += 1
        return bytes(out)


def receiver_name() -> str:
    return "ZenLinkStudio"


__all__ = ["AirPlayReceiver", "receiver_name", "AIRPLAY_PORT"]
