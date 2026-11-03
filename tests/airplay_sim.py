"""Simulated iPhone/iPad AirPlay sender, for testing ZenLink Studio over Wi-Fi
without a device.

It speaks the same protocol as iOS Screen Mirroring (pair-verify, fp-setup,
SETUP, the 128-byte-header mirror stream with AES-CTR, /feedback) and sends a
real H.264 picture: a moving test pattern with a frame counter.

  .venv\\Scripts\\python tests\\airplay_sim.py --seconds 20 --size 2302x1600 --fps 60
"""

from __future__ import annotations

import argparse
import asyncio
import hashlib
import plistlib
import struct
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import av  # noqa: E402
from Crypto.Cipher import AES  # noqa: E402
from cryptography.hazmat.primitives import serialization as ser  # noqa: E402
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey  # noqa: E402
from cryptography.hazmat.primitives.asymmetric.x25519 import X25519PrivateKey, X25519PublicKey  # noqa: E402

from zenlink.airplay.fairplay import FairPlay  # noqa: E402


def raw(key) -> bytes:
    return key.public_key().public_bytes(ser.Encoding.Raw, ser.PublicFormat.Raw)


def sha16(*parts: bytes) -> bytes:
    h = hashlib.sha512()
    for p in parts:
        h.update(p)
    return h.digest()[:16]


class Rtsp:
    def __init__(self, reader, writer):
        self.r, self.w, self.cseq = reader, writer, 0

    async def call(self, method, url, body=b"", ctype=None):
        self.cseq += 1
        head = f"{method} {url} RTSP/1.0\r\nCSeq: {self.cseq}\r\nContent-Length: {len(body)}\r\n"
        if ctype:
            head += f"Content-Type: {ctype}\r\n"
        self.w.write(head.encode() + b"\r\n" + body)
        await self.w.drain()
        resp = (await self.r.readuntil(b"\r\n\r\n")).decode()
        n = int(next(l for l in resp.split("\r\n") if l.lower().startswith("content-length")).split(":")[1])
        return resp.split("\r\n")[0], await self.r.readexactly(n)


def annexb_to_nals(data: bytes) -> list[bytes]:
    out, i, n = [], 0, len(data)
    starts = []
    while i < n - 3:
        if data[i:i + 3] == b"\0\0\1":
            starts.append(i + 3)
            i += 3
        else:
            i += 1
    for k, s in enumerate(starts):
        e = starts[k + 1] - 3 if k + 1 < len(starts) else n
        nal = data[s:e]
        while nal.endswith(b"\0"):
            nal = nal[:-1]
        if nal:
            out.append(nal)
    return out


def avcc_config(sps: bytes, pps: bytes) -> bytes:
    return (bytes([1, sps[1], sps[2], sps[3], 0xFF, 0xE1]) + len(sps).to_bytes(2, "big") + sps
            + bytes([1]) + len(pps).to_bytes(2, "big") + pps)


def header(size: int, kind: int, w: float = 0, h: float = 0) -> bytes:
    hd = bytearray(128)
    struct.pack_into("<I", hd, 0, size)
    hd[4] = kind
    for off in (16, 40, 56):
        struct.pack_into("<f", hd, off, w)
    for off in (20, 44, 60):
        struct.pack_into("<f", hd, off, h)
    return bytes(hd)


async def main(args) -> None:
    w, h = (int(x) for x in args.size.split("x"))
    reader, writer = await asyncio.open_connection(args.host, args.port)
    rtsp = Rtsp(reader, writer)

    x = X25519PrivateKey.generate()
    ed = Ed25519PrivateKey.generate()
    _, data = await rtsp.call("POST", "/pair-verify", b"\x01\0\0\0" + raw(x) + raw(ed))
    server_x = data[:32]
    secret = x.exchange(X25519PublicKey.from_public_bytes(server_x))
    ctr = AES.new(sha16(b"Pair-Verify-AES-Key", secret), AES.MODE_CTR, nonce=b"",
                  initial_value=sha16(b"Pair-Verify-AES-IV", secret))
    ctr.encrypt(data[32:])
    await rtsp.call("POST", "/pair-verify", b"\0\0\0\0" + ctr.encrypt(ed.sign(raw(x) + server_x)))
    await rtsp.call("POST", "/fp-setup", bytes([0x46, 0x50, 0x4C, 0x59, 3, 1, 1, 0, 0, 0, 0, 4, 2, 0, 2, 0xBB]))
    msg3 = bytes([0x46, 0x50, 0x4C, 0x59, 3]) + bytes(159)
    await rtsp.call("POST", "/fp-setup", msg3)

    ekey, eiv = bytes(range(72)), bytes(range(16))
    fp = FairPlay()
    fp.keymsg = msg3
    aeskey = hashlib.sha512(fp.decrypt(ekey) + secret).digest()[:16]
    setup = {"ekey": ekey, "eiv": eiv, "name": "Simulated iPad", "model": "iPad15,7", "deviceID": "AA:BB", "timingPort": 0}
    await rtsp.call("SETUP", "rtsp://sim/1", plistlib.dumps(setup, fmt=plistlib.FMT_BINARY))
    sid = args.sid
    wire_id = sid - (1 << 64) if sid >= 1 << 63 else sid  # 8-byte integer on the wire, like iOS
    _, data = await rtsp.call("SETUP", "rtsp://sim/1", plistlib.dumps(
        {"streams": [{"type": 110, "streamConnectionID": wire_id}]}, fmt=plistlib.FMT_BINARY))
    port = plistlib.loads(data)["streams"][0]["dataPort"]
    cipher = AES.new(sha16(f"AirPlayStreamKey{sid}".encode(), aeskey), AES.MODE_CTR, nonce=b"",
                     initial_value=sha16(f"AirPlayStreamIV{sid}".encode(), aeskey))

    enc = av.CodecContext.create(args.encoder, "w")
    enc.width, enc.height, enc.pix_fmt = w, h, "yuv420p"
    from fractions import Fraction

    enc.time_base = Fraction(1, args.fps)
    enc.framerate = args.fps
    enc.bit_rate = args.mbps * 1_000_000
    enc.gop_size = 100000  # like iOS: one keyframe at the start
    if args.encoder == "libx264":
        enc.options = {"preset": "ultrafast", "tune": "zerolatency", "profile": "high", "bframes": "0"}
    enc.open()

    _mr, mw = await asyncio.open_connection(args.host, port)
    sent_config = False
    t0 = time.perf_counter()
    bars = bytes((60 if (x // 40) % 2 == 0 else 180) for x in range(2 * w))
    last_feedback = 0.0
    sent = 0
    for i in range(args.seconds * args.fps):
        # moving bars + a moving white square: motion everywhere, like scrolling
        shift = (i * 12) % w
        row = bars[shift:shift + w]
        sq = (i * 20) % (w - 200)
        row_sq = row[:sq] + bytes([235]) * 200 + row[sq + 200:]
        frame = av.VideoFrame(w, h, "yuv420p")
        y = frame.planes[0]
        pad = bytes(y.line_size - w)
        y.update((row + pad) * 200 + (row_sq + pad) * 200 + (row + pad) * (h - 400))
        for pl in frame.planes[1:]:
            pl.update(bytes([128]) * pl.buffer_size)
        frame.pts = i
        for packet in enc.encode(frame):
            nals = annexb_to_nals(bytes(packet))
            sps = next((n for n in nals if n[0] & 0x1F == 7), None)
            pps = next((n for n in nals if n[0] & 0x1F == 8), None)
            if sps and pps and not sent_config:
                cfg = avcc_config(sps, pps)
                mw.write(header(len(cfg), 1, w, h) + cfg)
                sent_config = True
            body = b"".join(len(n).to_bytes(4, "big") + n for n in nals if n[0] & 0x1F not in (7, 8, 9))
            if body:
                mw.write(header(len(body), 0) + cipher.encrypt(body))
                sent += 1
        await mw.drain()
        now = time.perf_counter()
        if now - last_feedback > 2:
            await rtsp.call("POST", "/feedback")
            last_feedback = now
        due = t0 + (i + 1) / args.fps
        if due > now:
            await asyncio.sleep(due - now)
    print(f"sent {sent} frames in {time.perf_counter() - t0:.1f} s")
    await rtsp.call("TEARDOWN", "rtsp://sim/1", plistlib.dumps({}, fmt=plistlib.FMT_BINARY))
    mw.close()
    writer.close()


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("--host", default="127.0.0.1")
    ap.add_argument("--port", type=int, default=7000)
    ap.add_argument("--seconds", type=int, default=20)
    ap.add_argument("--size", default="2302x1600")
    ap.add_argument("--fps", type=int, default=60)
    ap.add_argument("--mbps", type=int, default=8)
    ap.add_argument("--encoder", default="libx264")
    ap.add_argument("--sid", type=lambda v: int(v, 0), default=0xF1E2D3C4B5A69788)
    asyncio.run(main(ap.parse_args()))
