"""Low-latency H.264 for the decoder: declare "no frame reordering" in the SPS.

Apple's QuickTime capture stream is decoded in display order (no B-frames),
but its SPS doesn't say so (no VUI bitstream_restriction). Without that, a
hardware H.264 decoder must assume reordering and holds up to a full DPB of
frames before output: measured ~170-245 ms in ZenLink, and frames coming out
in bursts. This module rewrites the SPS so the decoder outputs every frame as
soon as it is decoded:

  vui.bitstream_restriction_flag = 1
  max_num_reorder_frames         = 0
  max_dec_frame_buffering        = max(1, max_num_ref_frames)

Only the SPS changes; slices and PPS are untouched. A stream that does use
B-frames (pic_order_cnt_type 0 with reordering) is detected at runtime by
the caller and left alone.
"""

from __future__ import annotations

_HIGH_PROFILES = {100, 110, 122, 244, 44, 83, 86, 118, 128, 138, 139, 134, 135}


# ---------------------------------------------------------------- bits

def _unescape(nal: bytes) -> bytes:
    out = bytearray()
    zeros = 0
    for b in nal:
        if zeros >= 2 and b == 3:
            zeros = 0
            continue
        out.append(b)
        zeros = zeros + 1 if b == 0 else 0
    return bytes(out)


def _escape(rbsp: bytes) -> bytes:
    out = bytearray()
    zeros = 0
    for b in rbsp:
        if zeros >= 2 and b <= 3:
            out.append(3)
            zeros = 0
        out.append(b)
        zeros = zeros + 1 if b == 0 else 0
    return bytes(out)


class _Reader:
    def __init__(self, data: bytes) -> None:
        self.data = data
        self.pos = 0

    def u(self, n: int) -> int:
        v = 0
        for _ in range(n):
            byte = self.data[self.pos >> 3]
            v = (v << 1) | ((byte >> (7 - (self.pos & 7))) & 1)
            self.pos += 1
        return v

    def ue(self) -> int:
        zeros = 0
        while self.u(1) == 0:
            zeros += 1
            if zeros > 31:
                raise ValueError("bad exp-golomb")
        return (1 << zeros) - 1 + (self.u(zeros) if zeros else 0)

    def se(self) -> int:
        k = self.ue()
        return (k + 1) // 2 if k & 1 else -(k // 2)


class _Writer:
    def __init__(self) -> None:
        self.bits: list[int] = []

    def u(self, n: int, v: int) -> None:
        for i in range(n - 1, -1, -1):
            self.bits.append((v >> i) & 1)

    def ue(self, v: int) -> None:
        v += 1
        n = v.bit_length()
        self.u(n - 1, 0)
        self.u(n, v)

    def copy(self, reader: _Reader, start: int, end: int) -> None:
        for p in range(start, end):
            self.bits.append((reader.data[p >> 3] >> (7 - (p & 7))) & 1)

    def trailing(self) -> bytes:
        self.bits.append(1)
        while len(self.bits) % 8:
            self.bits.append(0)
        out = bytearray()
        for i in range(0, len(self.bits), 8):
            v = 0
            for bit in self.bits[i:i + 8]:
                v = (v << 1) | bit
            out.append(v)
        return bytes(out)


def _scaling_list(r: _Reader, size: int) -> None:
    last = nxt = 8
    for _ in range(size):
        if nxt != 0:
            nxt = (last + r.se() + 256) % 256
        last = nxt if nxt != 0 else last


def _hrd(r: _Reader) -> None:
    cnt = r.ue() + 1
    r.u(4)
    r.u(4)
    for _ in range(cnt):
        r.ue()
        r.ue()
        r.u(1)
    r.u(5)
    r.u(5)
    r.u(5)
    r.u(5)


# ---------------------------------------------------------------- SPS

def sps_info(nal: bytes) -> dict:
    """poc type, ref frames and current reorder setting of an SPS NAL."""
    info, _ = _parse(nal)
    return info


def _parse(nal: bytes):
    if not nal or (nal[0] & 0x1F) != 7:
        raise ValueError("not an SPS")
    r = _Reader(_unescape(nal[1:]))
    profile = r.u(8)
    r.u(8)
    r.u(8)
    r.ue()
    if profile in _HIGH_PROFILES:
        chroma = r.ue()
        if chroma == 3:
            r.u(1)
        r.ue()
        r.ue()
        r.u(1)
        if r.u(1):
            for i in range(12 if chroma == 3 else 8):
                if r.u(1):
                    _scaling_list(r, 16 if i < 6 else 64)
    r.ue()
    poc_type = r.ue()
    if poc_type == 0:
        r.ue()
    elif poc_type == 1:
        r.u(1)
        r.se()
        r.se()
        for _ in range(r.ue()):
            r.se()
    max_refs = r.ue()
    r.u(1)
    r.ue()
    r.ue()
    if not r.u(1):
        r.u(1)
    r.u(1)
    if r.u(1):
        r.ue()
        r.ue()
        r.ue()
        r.ue()
    marks = {"vui_flag": r.pos}
    info = {"profile": profile, "poc_type": poc_type, "max_refs": max_refs, "vui": False,
            "restriction": False, "reorder": None, "dec_buffering": None}
    if r.u(1):
        info["vui"] = True
        if r.u(1):
            if r.u(8) == 255:
                r.u(16)
                r.u(16)
        if r.u(1):
            r.u(1)
        if r.u(1):
            r.u(3)
            r.u(1)
            if r.u(1):
                r.u(8)
                r.u(8)
                r.u(8)
        if r.u(1):
            r.ue()
            r.ue()
        if r.u(1):
            r.u(32)
            r.u(32)
            r.u(1)
        nal_hrd = r.u(1)
        if nal_hrd:
            _hrd(r)
        vcl_hrd = r.u(1)
        if vcl_hrd:
            _hrd(r)
        if nal_hrd or vcl_hrd:
            r.u(1)
        r.u(1)
        marks["restriction_flag"] = r.pos
        if r.u(1):
            info["restriction"] = True
            r.u(1)
            r.ue()
            r.ue()
            r.ue()
            r.ue()
            info["reorder"] = r.ue()
            info["dec_buffering"] = r.ue()
    return info, (r, marks)


def low_latency_sps(nal: bytes) -> bytes:
    """Return the SPS with max_num_reorder_frames = 0 (unchanged if already)."""
    info, (r, marks) = _parse(nal)
    if info["restriction"] and info["reorder"] == 0:
        return nal
    buffering = max(1, info["max_refs"])
    w = _Writer()
    if not info["vui"]:
        w.copy(r, 0, marks["vui_flag"])
        w.u(1, 1)          # vui_parameters_present_flag
        w.u(1, 0)          # aspect_ratio_info_present_flag
        w.u(1, 0)          # overscan_info_present_flag
        w.u(1, 0)          # video_signal_type_present_flag
        w.u(1, 0)          # chroma_loc_info_present_flag
        w.u(1, 0)          # timing_info_present_flag
        w.u(1, 0)          # nal_hrd_parameters_present_flag
        w.u(1, 0)          # vcl_hrd_parameters_present_flag
        w.u(1, 0)          # pic_struct_present_flag
    else:
        w.copy(r, 0, marks["restriction_flag"])
    w.u(1, 1)              # bitstream_restriction_flag
    w.u(1, 1)              # motion_vectors_over_pic_boundaries_flag
    w.ue(2)                # max_bytes_per_pic_denom (spec default)
    w.ue(1)                # max_bits_per_mb_denom (spec default)
    w.ue(16)               # log2_max_mv_length_horizontal
    w.ue(16)               # log2_max_mv_length_vertical
    w.ue(0)                # max_num_reorder_frames
    w.ue(buffering)        # max_dec_frame_buffering
    return bytes([nal[0]]) + _escape(w.trailing())


def rewrite_avcc(avcc: bytes) -> bytes:
    """avcC decoder configuration record with every SPS made low-latency."""
    if len(avcc) < 7 or avcc[0] != 1:
        return avcc
    out = bytearray(avcc[:5])
    pos = 5
    n_sps = avcc[pos] & 0x1F
    out.append(avcc[pos])
    pos += 1
    for _ in range(n_sps):
        size = int.from_bytes(avcc[pos:pos + 2], "big")
        sps = avcc[pos + 2:pos + 2 + size]
        pos += 2 + size
        try:
            sps = low_latency_sps(sps)
        except (ValueError, IndexError):
            pass
        out += len(sps).to_bytes(2, "big") + sps
    out += avcc[pos:]  # PPS count, PPS and any high-profile extension
    return bytes(out)


def rewrite_access_unit(au: bytes) -> bytes:
    """Rewrite in-band SPS NAL units in a length-prefixed access unit."""
    pos, n, parts, changed = 0, len(au), [], False
    while pos + 5 <= n:
        size = int.from_bytes(au[pos:pos + 4], "big")
        end = pos + 4 + size
        if size <= 0 or end > n:
            return au
        nal = au[pos + 4:end]
        if (nal[0] & 0x1F) == 7:
            try:
                new = low_latency_sps(nal)
                if new != nal:
                    nal, changed = new, True
            except (ValueError, IndexError):
                pass
        parts.append(len(nal).to_bytes(4, "big") + nal)
        pos = end
    return b"".join(parts) if changed else au
