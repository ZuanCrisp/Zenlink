"""Minimal lockdownd client over a usbmux stream.

Wired casting needs the device to see this PC as a trusted host with an open
lockdown session (TikTok LIVE Studio's qcast does the same: StartSession with
the pair record, or a pair request that shows "Trust This Computer?").
This client reads the existing Windows pair record that iTunes / Apple Devices
created when the device was trusted, opens the session, and keeps it alive.
"""

from __future__ import annotations

import logging
import os
import plistlib
import ssl
import struct
import tempfile
from pathlib import Path

from .usbmux import MuxStream

log = logging.getLogger(__name__)

LOCKDOWN_DIR = Path(os.environ.get("ProgramData", r"C:\ProgramData")) / "Apple" / "Lockdown"
LABEL = "ZenLink"


class LockdownError(RuntimeError):
    pass


def pair_record_path(udid: str) -> Path:
    udid = udid.strip("\x00")
    dashed = f"{udid[:8]}-{udid[8:]}" if len(udid) == 24 and "-" not in udid else udid
    return LOCKDOWN_DIR / f"{dashed}.plist"


def load_pair_record(udid: str) -> dict | None:
    try:
        record = plistlib.loads(pair_record_path(udid).read_bytes())
    except (OSError, plistlib.InvalidFileException):
        return None
    if "SystemBUID" not in record:
        try:
            system = plistlib.loads((LOCKDOWN_DIR / "SystemConfiguration.plist").read_bytes())
            record["SystemBUID"] = system.get("SystemBUID")
        except (OSError, plistlib.InvalidFileException):
            pass
    return record


class _TlsOverStream:
    """TLS on top of a MuxStream using memory BIOs."""

    def __init__(self, stream: MuxStream, cert_pem: bytes, key_pem: bytes) -> None:
        self.stream = stream
        ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
        ctx.check_hostname = False
        ctx.verify_mode = ssl.CERT_NONE
        ctx.set_ciphers("ALL:!aNULL:!eNULL:@SECLEVEL=0")
        with tempfile.TemporaryDirectory() as tmp:
            cert = Path(tmp) / "host.pem"
            cert.write_bytes(cert_pem + b"\n" + key_pem)
            ctx.load_cert_chain(cert)
        self.incoming = ssl.MemoryBIO()
        self.outgoing = ssl.MemoryBIO()
        self.tls = ctx.wrap_bio(self.incoming, self.outgoing, server_side=False)
        self._pump(self.tls.do_handshake)

    def _flush(self) -> None:
        data = self.outgoing.read()
        if data:
            self.stream.sendall(data)

    def _pump(self, op, *args):
        while True:
            try:
                result = op(*args)
                self._flush()
                return result
            except ssl.SSLWantReadError:
                self._flush()
                self.incoming.write(self.stream.recv_some())

    def sendall(self, data: bytes) -> None:
        self._pump(self.tls.write, data)

    def recv_exact(self, n: int) -> bytes:
        out = bytearray()
        while len(out) < n:
            out += self._pump(self.tls.read, n - len(out))
        return bytes(out)


class LockdownClient:
    def __init__(self, stream: MuxStream) -> None:
        self.stream = stream
        self.io = stream
        self.session_id: str | None = None

    def request(self, message: dict) -> dict:
        body = plistlib.dumps({"Label": LABEL, **message})
        self.io.sendall(struct.pack(">I", len(body)) + body)
        length = struct.unpack(">I", self.io.recv_exact(4))[0]
        reply = plistlib.loads(self.io.recv_exact(length))
        if "Error" in reply:
            raise LockdownError(f"{message.get('Request')}: {reply['Error']}")
        return reply

    def query_type(self) -> str:
        return self.request({"Request": "QueryType"}).get("Type", "")

    def get_value(self, key: str | None = None, domain: str | None = None):
        msg = {"Request": "GetValue"}
        if key:
            msg["Key"] = key
        if domain:
            msg["Domain"] = domain
        return self.request(msg).get("Value")

    def start_session(self, record: dict) -> None:
        reply = self.request({
            "Request": "StartSession",
            "HostID": record["HostID"],
            "SystemBUID": record.get("SystemBUID", ""),
        })
        self.session_id = reply.get("SessionID")
        if reply.get("EnableSessionSSL"):
            self.io = _TlsOverStream(self.stream, record["HostCertificate"], record["HostPrivateKey"])

    def stop_session(self) -> None:
        if self.session_id:
            try:
                self.request({"Request": "StopSession", "SessionID": self.session_id})
            except Exception:
                pass
            self.session_id = None
