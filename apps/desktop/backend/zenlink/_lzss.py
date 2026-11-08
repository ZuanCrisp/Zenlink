"""Pure-Python stand-ins for the native `lzss` (pylzss) and `lzfse` modules.

pyimg4 imports both at import time, and pymobiledevice3 reaches pyimg4 through
mobile_image_mounter -> restore.tss -> restore.img4. Mounting the developer
image never compresses or decompresses anything with them, but loading their
unsigned .pyd files is enough for Windows Smart App Control to block the whole
iPhone connection (GitHub issue #2: "DLL load failed while importing lzss").
`install()` puts these modules in sys.modules first, so the native ones are
never loaded, and the release build leaves them out.

LZSS is Apple's kernelcache variant (xnu lzss.c: N=4096, F=18, threshold 2,
window pre-filled with spaces), byte-compatible with pylzss.
"""

from __future__ import annotations

import sys
import types

_N = 4096
_F = 18
_THRESHOLD = 2


def decompress(data: bytes) -> bytes:
    ring = bytearray(b" " * _N)
    r = _N - _F
    out = bytearray()
    flags = 0
    i, n = 0, len(data)
    while i < n:
        flags >>= 1
        if not flags & 0x100:
            flags = data[i] | 0xFF00
            i += 1
            if i >= n:
                break
        if flags & 1:
            c = data[i]
            i += 1
            out.append(c)
            ring[r] = c
            r = (r + 1) & (_N - 1)
        else:
            if i + 1 >= n:
                break
            pos = data[i] | ((data[i + 1] & 0xF0) << 4)
            length = (data[i + 1] & 0x0F) + _THRESHOLD + 1
            i += 2
            for k in range(length):
                c = ring[(pos + k) & (_N - 1)]
                out.append(c)
                ring[r] = c
                r = (r + 1) & (_N - 1)
    return bytes(out)


def compress(data: bytes) -> bytes:
    # Greedy matcher over the last N-F input bytes (never the pre-filled
    # spaces). shortcut: slow on big inputs (pure Python); ZenLink never
    # compresses, it only exists so pyimg4 keeps its full API.
    out = bytearray()
    heads: dict[bytes, list[int]] = {}
    p, n = 0, len(data)
    while p < n:
        flag_at = len(out)
        out.append(0)
        for bit in range(8):
            if p >= n:
                break
            best_len, best_pos = 0, 0
            if p + _THRESHOLD < n:
                for q in reversed(heads.get(data[p : p + 3], ())):
                    if p - q > _N - _F:
                        break
                    m = 0
                    while m < _F and p + m < n and data[q + m] == data[p + m]:
                        m += 1
                    if m > best_len:
                        best_len, best_pos = m, q
                        if m == _F:
                            break
            if best_len > _THRESHOLD:
                ring = (best_pos + _N - _F) & (_N - 1)
                out += bytes((ring & 0xFF, ((ring >> 4) & 0xF0) | (best_len - _THRESHOLD - 1)))
                step = best_len
            else:
                out[flag_at] |= 1 << bit
                out.append(data[p])
                step = 1
            for s in range(p, p + step):
                heads.setdefault(data[s : s + 3], []).append(s)
            p += step
    return bytes(out)


def _lzfse_missing(*_args, **_kwargs):
    raise RuntimeError("LZFSE is not bundled with ZenLink (not needed to mount the developer image)")


def install() -> None:
    """Make `import lzss` / `import lzfse` resolve to these pure-Python modules."""
    if "lzss" not in sys.modules:
        mod = types.ModuleType("lzss", __doc__)
        mod.compress, mod.decompress = compress, decompress
        sys.modules["lzss"] = mod
    if "lzfse" not in sys.modules:
        mod = types.ModuleType("lzfse", "ZenLink stub: LZFSE is not bundled.")
        mod.compress = mod.decompress = _lzfse_missing
        sys.modules["lzfse"] = mod
