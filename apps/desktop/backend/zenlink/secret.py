"""Small secrets (the device passcode for Alt+U) encrypted with Windows DPAPI:
only the same Windows user on the same PC can decrypt them."""

from __future__ import annotations

import base64
import ctypes
import json
from ctypes import wintypes

from zenlink.paths import DATA_DIR

FILE = DATA_DIR / "secrets.json"


class _Blob(ctypes.Structure):
    _fields_ = [("cbData", wintypes.DWORD), ("pbData", ctypes.POINTER(ctypes.c_char))]


def _blob(data: bytes) -> _Blob:
    buf = ctypes.create_string_buffer(data, len(data))
    return _Blob(len(data), ctypes.cast(buf, ctypes.POINTER(ctypes.c_char)))


def _crypt(data: bytes, protect: bool) -> bytes:
    src, out = _blob(data), _Blob()
    fn = ctypes.windll.crypt32.CryptProtectData if protect else ctypes.windll.crypt32.CryptUnprotectData
    if not fn(ctypes.byref(src), None, None, None, None, 0x1, ctypes.byref(out)):  # UI_FORBIDDEN
        raise OSError("DPAPI failed")
    try:
        return ctypes.string_at(out.pbData, out.cbData)
    finally:
        ctypes.windll.kernel32.LocalFree(out.pbData)


def _load() -> dict:
    try:
        return json.loads(FILE.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return {}


def set_secret(name: str, value: str) -> None:
    data = _load()
    if value:
        data[name] = base64.b64encode(_crypt(value.encode(), True)).decode()
    else:
        data.pop(name, None)
    FILE.write_text(json.dumps(data), encoding="utf-8")


def get_secret(name: str) -> str:
    raw = _load().get(name)
    if not raw:
        return ""
    try:
        return _crypt(base64.b64decode(raw), False).decode()
    except OSError:
        return ""


def has_secret(name: str) -> bool:
    return bool(_load().get(name))
