"""Wire format of Apple's USB screen-capture protocol (the one QuickTime uses).

Every message is ``[u32 length][u32 magic][...]`` little-endian. Values inside
messages use a small tagged dictionary format (keyv/strk/idxk/...), and media
arrives as serialized CMSampleBuffers. Protocol layout follows the reverse
engineering in danielpaulus/quicktime_video_hack (MIT).
"""

from __future__ import annotations

import struct
from dataclasses import dataclass, field


def fourcc(text: str) -> int:
    return int.from_bytes(text.encode("ascii"), "big")


# Top-level packet kinds
PING = fourcc("ping")
SYNC = fourcc("sync")
ASYN = fourcc("asyn")
RPLY = fourcc("rply")

# SYNC subtypes
CWPA = fourcc("cwpa")
CVRP = fourcc("cvrp")
CLOK = fourcc("clok")
TIME = fourcc("time")
AFMT = fourcc("afmt")
SKEW = fourcc("skew")
OG = fourcc("go! ")
STOP = fourcc("stop")

# ASYN subtypes
FEED = fourcc("feed")
EAT = fourcc("eat!")
SPRP = fourcc("sprp")
TJMP = fourcc("tjmp")
SRAT = fourcc("srat")
TBAS = fourcc("tbas")
RELS = fourcc("rels")
HPD1 = fourcc("hpd1")
HPA1 = fourcc("hpa1")
HPD0 = fourcc("hpd0")
HPA0 = fourcc("hpa0")
NEED = fourcc("need")

EMPTY_CF = 1

# Dictionary tags
KEYV = fourcc("keyv")
STRK = fourcc("strk")
IDXK = fourcc("idxk")
BULV = fourcc("bulv")
DICT = fourcc("dict")
DATV = fourcc("datv")
STRV = fourcc("strv")
NMBV = fourcc("nmbv")
FDSC = fourcc("fdsc")

# Sample buffer tags
SBUF = fourcc("sbuf")
OPTS = fourcc("opts")
STIA = fourcc("stia")
SDAT = fourcc("sdat")
SATT = fourcc("satt")
SARY = fourcc("sary")
SSIZ = fourcc("ssiz")
NSMP = fourcc("nsmp")

# Format description tags
MDIA = fourcc("mdia")
VDIM = fourcc("vdim")
CODC = fourcc("codc")
EXTN = fourcc("extn")
ASBD = fourcc("asbd")
MEDIA_VIDEO = fourcc("vide")
MEDIA_SOUND = fourcc("soun")


def u32(data: bytes, offset: int = 0) -> int:
    return struct.unpack_from("<I", data, offset)[0]


def u64(data: bytes, offset: int = 0) -> int:
    return struct.unpack_from("<Q", data, offset)[0]


def tag(length: int, magic: int) -> bytes:
    return struct.pack("<II", length, magic)


# --------------------------------------------------------------------------
# Numbers, CMTime


@dataclass
class NSNumber:
    kind: int  # 3 = u32, 4 = u64, 5 = u32 (signed), 6 = float64
    value: float | int

    @classmethod
    def parse(cls, data: bytes) -> "NSNumber":
        kind = data[0]
        if kind == 6:
            return cls(6, struct.unpack_from("<d", data, 1)[0])
        if kind == 4:
            return cls(4, u64(data, 1))
        if kind in (3, 5):
            return cls(kind, u32(data, 1))
        raise ValueError(f"unknown NSNumber kind {kind}")

    def to_bytes(self) -> bytes:
        if self.kind == 6:
            return bytes([6]) + struct.pack("<d", float(self.value))
        if self.kind == 4:
            return bytes([4]) + struct.pack("<Q", int(self.value))
        return bytes([self.kind]) + struct.pack("<I", int(self.value))


def f64(value: float) -> NSNumber:
    return NSNumber(6, value)


def n32(value: int) -> NSNumber:
    return NSNumber(3, value)


@dataclass
class CMTime:
    value: int = 0
    scale: int = 1_000_000_000
    flags: int = 0
    epoch: int = 0

    SIZE = 24

    @classmethod
    def parse(cls, data: bytes, offset: int = 0) -> "CMTime":
        return cls(*struct.unpack_from("<QIIQ", data, offset))

    def to_bytes(self) -> bytes:
        return struct.pack("<QIIQ", self.value, self.scale, self.flags, self.epoch)

    @property
    def seconds(self) -> float:
        return self.value / self.scale if self.scale else 0.0


# --------------------------------------------------------------------------
# Dictionaries


def _parse_value(data: bytes):
    length, magic = u32(data), u32(data, 4)
    body = data[8:length]
    if magic == STRV:
        return body.decode("utf-8", "replace")
    if magic == DATV:
        return bytes(body)
    if magic == BULV:
        return body[:1] == b"\x01"
    if magic == NMBV:
        return NSNumber.parse(body)
    if magic == DICT:
        try:
            return parse_string_dict(data[:length])
        except ValueError:
            return parse_index_dict(data[:length])
    if magic == FDSC:
        return FormatDescription.parse(data[:length])
    raise ValueError(f"unknown value tag {magic.to_bytes(4, 'big')!r}")


def _iter_pairs(data: bytes, outer: int):
    length, magic = u32(data), u32(data, 4)
    if magic != outer:
        raise ValueError("unexpected dictionary tag")
    pos = 8
    while pos < length:
        pair_len = u32(data, pos)
        if u32(data, pos + 4) != KEYV:
            raise ValueError("expected keyv")
        yield data[pos + 8 : pos + pair_len]
        pos += pair_len


def parse_string_dict(data: bytes, outer: int = DICT) -> dict:
    result = {}
    for pair in _iter_pairs(data, outer):
        key_len = u32(pair)
        if u32(pair, 4) != STRK:
            raise ValueError("not a string-key dictionary")
        result[pair[8:key_len].decode("utf-8", "replace")] = _parse_value(pair[key_len:])
    return result


def parse_index_dict(data: bytes, outer: int = DICT) -> dict:
    result = {}
    for pair in _iter_pairs(data, outer):
        key_len = u32(pair)
        if u32(pair, 4) != IDXK:
            raise ValueError("not an index-key dictionary")
        key = struct.unpack_from("<H", pair, 8)[0]
        result[key] = _parse_value(pair[key_len:])
    return result


def _serialize_value(value) -> bytes:
    if isinstance(value, bool):
        return tag(9, BULV) + (b"\x01" if value else b"\x00")
    if isinstance(value, NSNumber):
        body = value.to_bytes()
        return tag(len(body) + 8, NMBV) + body
    if isinstance(value, str):
        body = value.encode("utf-8")
        return tag(len(body) + 8, STRV) + body
    if isinstance(value, (bytes, bytearray)):
        return tag(len(value) + 8, DATV) + bytes(value)
    if isinstance(value, dict):
        return serialize_string_dict(value)
    raise TypeError(f"cannot serialize {type(value).__name__}")


def serialize_string_dict(entries: dict) -> bytes:
    pairs = bytearray()
    for key, value in entries.items():
        key_bytes = key.encode("utf-8")
        body = tag(len(key_bytes) + 8, STRK) + key_bytes + _serialize_value(value)
        pairs += tag(len(body) + 8, KEYV) + body
    return tag(len(pairs) + 8, DICT) + bytes(pairs)


# --------------------------------------------------------------------------
# Format descriptions and sample buffers


@dataclass
class AudioFormat:
    sample_rate: float = 48000.0
    format_id: int = fourcc("lpcm")
    flags: int = 12  # signed integer, packed
    bytes_per_packet: int = 4
    frames_per_packet: int = 1
    bytes_per_frame: int = 4
    channels: int = 2
    bits: int = 16

    @classmethod
    def parse(cls, data: bytes) -> "AudioFormat":
        rate, fmt, flags, bpp, fpp, bpf, ch, bits = struct.unpack_from("<dIIIIIII", data)
        return cls(rate, fmt, flags, bpp, fpp, bpf, ch, bits)

    def to_bytes(self) -> bytes:
        # 56 bytes: ASBD (40) followed by the rate twice, as the host sends it.
        return struct.pack(
            "<dIIIIIIIIdd",
            self.sample_rate,
            self.format_id,
            self.flags,
            self.bytes_per_packet,
            self.frames_per_packet,
            self.bytes_per_frame,
            self.channels,
            self.bits,
            0,
            self.sample_rate,
            self.sample_rate,
        )[:56]


CODEC_AVC = fourcc("avc1")
CODEC_HEVC = fourcc("hvc1")
CODEC_HEVC_ALT = fourcc("hev1")


@dataclass
class FormatDescription:
    """CMFormatDescription. Video carries the decoder configuration record:
    avcC for H.264 (what devices normally send on this path) or hvcC for HEVC.
    The codec is read from the description itself, never assumed."""

    media_type: int
    width: int = 0
    height: int = 0
    codec: int = 0
    config: bytes = b""
    audio: AudioFormat | None = None

    @classmethod
    def parse(cls, data: bytes) -> "FormatDescription":
        if u32(data, 4) != FDSC:
            raise ValueError("not an fdsc")
        pos = 8
        if u32(data, pos + 4) != MDIA:
            raise ValueError("fdsc without media type")
        media = u32(data, pos + 8)
        pos += u32(data, pos)
        if media == MEDIA_SOUND:
            return cls(media, audio=AudioFormat.parse(data[pos + 8 : pos + u32(data, pos)]))
        width = height = codec = 0
        config = b""
        end = min(u32(data), len(data))
        while pos + 8 <= end:
            length, magic = u32(data, pos), u32(data, pos + 4)
            if length < 8:
                raise ValueError("corrupt fdsc element")
            if magic == VDIM:
                width, height = struct.unpack_from("<II", data, pos + 8)
            elif magic == CODC:
                codec = u32(data, pos + 8)
            elif magic == EXTN:
                ext = parse_index_dict(data[pos : pos + length], EXTN)
                atoms = ext.get(49)  # SampleDescriptionExtensionAtoms
                if isinstance(atoms, dict):
                    # 105 is avcC; take whichever configuration record is there.
                    record = atoms.get(105)
                    if not isinstance(record, bytes):
                        record = next((v for v in atoms.values() if isinstance(v, bytes) and len(v) > 6), b"")
                    config = record
            pos += length
        return cls(media, width, height, codec, config)

    @property
    def is_hevc(self) -> bool:
        if self.codec in (CODEC_HEVC, CODEC_HEVC_ALT):
            return True
        # hvcC: version 1 and a 23-byte header; avcC: version 1, profile, compat, level.
        return self.codec == 0 and len(self.config) >= 23 and self.config[0] == 1 and (self.config[21] & 0x03) == 3 and self.config[22] < 16

    @property
    def codec_name(self) -> str:
        return "HEVC" if self.is_hevc else "H.264"

    @property
    def codec_string(self) -> str:
        """WebCodecs codec string built from the configuration record."""
        c = self.config
        if self.is_hevc:
            if len(c) < 13:
                return "hvc1.1.6.L150.B0"
            space = ("", "A", "B", "C")[c[1] >> 6]
            tier = "H" if c[1] & 0x20 else "L"
            profile = c[1] & 0x1F
            compat = int.from_bytes(c[2:6], "big")
            compat = int(f"{compat:032b}"[::-1], 2)  # reversed bit order per ISO/IEC 14496-15
            constraints = c[6:12].rstrip(bytes(1))
            tail = "".join(f".{b:X}" for b in constraints) or ".B0"
            return f"hvc1.{space}{profile}.{compat:X}.{tier}{c[12]}{tail}"
        if len(c) >= 4:
            return "avc1." + c[1:4].hex()
        return "avc1.640033"


@dataclass
class SampleBuffer:
    media_type: int
    pts: CMTime = field(default_factory=CMTime)
    data: bytes = b""
    sizes: list[int] = field(default_factory=list)
    format: FormatDescription | None = None
    num_samples: int = 0

    @classmethod
    def parse(cls, data: bytes, media_type: int) -> "SampleBuffer":
        if u32(data, 4) != SBUF:
            raise ValueError("not an sbuf")
        buf = cls(media_type)
        end = min(u32(data), len(data))
        pos = 8
        while pos < end:
            length, magic = u32(data, pos), u32(data, pos + 4)
            if length < 8:
                raise ValueError("corrupt sbuf element")
            if magic == OPTS:
                buf.pts = CMTime.parse(data, pos + 8)
            elif magic == SDAT:
                buf.data = bytes(data[pos + 8 : pos + length])
            elif magic == NSMP:
                buf.num_samples = u32(data, pos + 8)
            elif magic == SSIZ:
                count = (length - 8) // 4
                buf.sizes = list(struct.unpack_from(f"<{count}I", data, pos + 8))
            elif magic == FDSC:
                buf.format = FormatDescription.parse(data[pos : pos + length])
            pos += length
        return buf


# --------------------------------------------------------------------------
# Outgoing packets


def ping() -> bytes:
    """Reply to the device's PING."""
    return struct.pack("<IIQ", 16, PING, 0x0000000100000000)


def ping_active() -> bytes:
    """Host-initiated PING, sent when the device stays silent after activation."""
    return struct.pack("<IIQ", 16, PING, 0x0000000100000001)


def reply_clock(correlation: int, clock_ref: int) -> bytes:
    return struct.pack("<IIQIQ", 28, RPLY, correlation, 0, clock_ref)


def reply_empty(correlation: int) -> bytes:
    return struct.pack("<IIQQ", 24, RPLY, correlation, 0)


def reply_time(correlation: int, time: CMTime) -> bytes:
    return struct.pack("<IIQI", 44, RPLY, correlation, 0) + time.to_bytes()


def reply_skew(correlation: int, skew: float) -> bytes:
    return struct.pack("<IIQId", 28, RPLY, correlation, 0, skew)


def reply_dict(correlation: int, entries: dict) -> bytes:
    body = serialize_string_dict(entries)
    return struct.pack("<IIQI", len(body) + 20, RPLY, correlation, 0) + body


def asyn(subtype: int, clock_ref: int, body: bytes = b"") -> bytes:
    return struct.pack("<IIQI", len(body) + 20, ASYN, clock_ref, subtype) + body


def need(clock_ref: int) -> bytes:
    return asyn(NEED, clock_ref)


def hpd1(width: int, height: int) -> bytes:
    """Ask for video; DisplaySize is the receiver's preferred picture size."""
    return asyn(
        HPD1,
        EMPTY_CF,
        serialize_string_dict(
            {
                "Valeria": True,
                "HEVCDecoderSupports444": True,
                "DisplaySize": {"Width": f64(width), "Height": f64(height)},
            }
        ),
    )


def hpa1(clock_ref: int) -> bytes:
    """Ask for audio as 48 kHz stereo 16-bit PCM."""
    return asyn(
        HPA1,
        clock_ref,
        serialize_string_dict(
            {
                "BufferAheadInterval": f64(0.073),
                "deviceUID": "ZenLink",
                "ScreenLatency": f64(0.04),
                "formats": AudioFormat().to_bytes(),
                "EDIDAC3Support": n32(0),
                "deviceName": "ZenLink",
            }
        ),
    )
