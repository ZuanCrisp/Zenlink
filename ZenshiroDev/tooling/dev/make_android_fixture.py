"""Make a synthetic scrcpy video stream for tests (no device needed).

The output has exactly the bytes a scrcpy v5.0.1 server sends on the video
socket of a reverse tunnel: the 64-byte device meta, the codec id, a session
packet, a config packet (SPS/PPS) and H.264 media packets (Annex B), PTS in
microseconds. The picture is generated (moving colour bars and a frame
counter), so the file holds no third-party content.

    .venv\\Scripts\\python ZenshiroDev\\tooling\\dev\\make_android_fixture.py out.scrcpy [--seconds 4] [--fps 30]

Needs PyAV (dev venv only; ZenLink itself does not use it).
"""

import argparse
import struct
from fractions import Fraction

import av

FLAG_CONFIG = 1 << 62
FLAG_KEY = 1 << 61
CODEC_H264 = 0x68323634


# BT.601 limited-range Y, U, V of the colour bars.
BARS = [(235, 128, 128), (210, 16, 146), (170, 166, 16), (145, 54, 34),
        (106, 202, 222), (81, 90, 240), (41, 240, 110), (16, 128, 128)]


def frame_planes(i: int, w: int, h: int):
    """Y, U, V planes (tightly packed) of frame `i`: moving bars, a moving
    block and a binary frame counter, so dropped or repeated frames show."""
    bw = w // len(BARS)
    shift = (i * 4) % w
    row_y, row_u, row_v = bytearray(w), bytearray(w // 2), bytearray(w // 2)
    for x in range(w):
        y, u, v = BARS[((x - shift) % w) // bw % len(BARS)]
        row_y[x] = y
        if x % 2 == 0:
            row_u[x // 2], row_v[x // 2] = u, v
    top = h * 2 // 3
    Y = bytearray(bytes(row_y) * top + bytes([16]) * (w * (h - top)))
    U = bytearray(bytes(row_u) * (top // 2) + bytes([128]) * ((w // 2) * ((h - top) // 2 + (h - top) % 2)))
    V = bytearray(bytes(row_v) * (top // 2) + bytes([128]) * ((w // 2) * ((h - top) // 2 + (h - top) % 2)))
    U, V = U[: (w // 2) * (h // 2)], V[: (w // 2) * (h // 2)]

    def box(x0, y0, size_w, size_h, luma):
        for yy in range(y0, y0 + size_h):
            Y[yy * w + x0 : yy * w + x0 + size_w] = bytes([luma]) * size_w

    box((i * 6) % (w - 40), top + 8, 40, 40, 200)
    for bit in range(10):
        box(w - 20 - bit * 18, h - 30, 14, 16, 235 if (i >> bit) & 1 else 50)
    return bytes(Y), bytes(U), bytes(V)


def to_frame(planes, w: int, h: int):
    f = av.VideoFrame(w, h, "yuv420p")
    for plane, data, pw in zip(f.planes, planes, (w, w // 2, w // 2)):
        ls = plane.line_size
        if ls == pw:
            plane.update(data)
        else:
            rows = len(data) // pw
            buf = bytearray(ls * rows)
            for r in range(rows):
                buf[r * ls : r * ls + pw] = data[r * pw : (r + 1) * pw]
            plane.update(bytes(buf))
    return f


def split_annexb(data: bytes):
    """Yield (start, end) of each NAL including its start code."""
    i, n, starts = 0, len(data), []
    while i + 3 <= n:
        if data[i] == 0 and data[i + 1] == 0 and data[i + 2] == 1:
            s = i - 1 if i > 0 and data[i - 1] == 0 else i
            starts.append(s)
            i += 3
        else:
            i += 1
    for k, s in enumerate(starts):
        yield data[s : starts[k + 1] if k + 1 < len(starts) else n]


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("out")
    ap.add_argument("--seconds", type=float, default=4.0)
    ap.add_argument("--fps", type=int, default=30)
    ap.add_argument("--width", type=int, default=360)
    ap.add_argument("--height", type=int, default=640)
    ap.add_argument("--name", default="Synthetic test stream")
    a = ap.parse_args()

    enc = av.CodecContext.create("libx264", "w")
    enc.width, enc.height = a.width, a.height
    enc.pix_fmt = "yuv420p"
    enc.time_base = Fraction(1, a.fps)
    enc.framerate = Fraction(a.fps, 1)
    enc.gop_size = a.fps * 2
    enc.max_b_frames = 0
    enc.bit_rate = 400_000
    enc.options = {"preset": "veryfast", "tune": "zerolatency", "profile": "baseline", "x264-params": "annexb=1"}

    out = bytearray()
    meta = a.name.encode("utf-8")[:63]
    out += meta + b"\0" * (64 - len(meta))
    out += struct.pack(">I", CODEC_H264)
    out += struct.pack(">III", 0x80000000, a.width, a.height)

    sent_config = False
    total = int(a.seconds * a.fps)

    def emit(packets):
        nonlocal sent_config
        for p in packets:
            data = bytes(p)
            nals = list(split_annexb(data))
            ps = b"".join(x for x in nals if (x[x.index(b"\x00\x01") + 2] & 0x1F) in (7, 8))
            body = b"".join(x for x in nals if (x[x.index(b"\x00\x01") + 2] & 0x1F) not in (6, 7, 8))
            if not ps and not sent_config and enc.extradata:
                # libx264 keeps SPS/PPS in the extradata instead of the first packet.
                ex = list(split_annexb(bytes(enc.extradata)))
                ps = b"".join(x for x in ex if (x[x.index(b"\x00\x01") + 2] & 0x1F) in (7, 8))
            if ps and not sent_config:
                out.extend(struct.pack(">QI", FLAG_CONFIG, len(ps)) + ps)
                sent_config = True
            if not body:
                continue
            pts_us = int(p.pts * 1_000_000 / a.fps)
            flags = pts_us | (FLAG_KEY if p.is_keyframe else 0)
            out.extend(struct.pack(">QI", flags, len(body)) + body)

    for i in range(total):
        f = to_frame(frame_planes(i, a.width, a.height), a.width, a.height)
        f.pts = i
        emit(enc.encode(f))
    emit(enc.encode(None))

    with open(a.out, "wb") as fh:
        fh.write(out)
    print(f"{a.out}: {len(out)} bytes, {total} frames, {a.width}x{a.height} @ {a.fps} fps")


if __name__ == "__main__":
    main()
