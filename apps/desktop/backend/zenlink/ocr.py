"""Copy text from the device's screen (Alt+C) with Windows' built-in OCR.

Each capture is recognized by ZenshiroDev/tooling/windows/tools/ocr.ps1 (WinRT OcrEngine). Captures of a
scrolled page are stitched: the longest run of lines that ends the stored
text and starts the new capture is recognized once. Alt+R clears the stitch.
"""

from __future__ import annotations

import base64
import logging
import os
import re
import subprocess
import tempfile
import threading

from zenlink.paths import TOOLS_DIR

log = logging.getLogger(__name__)

SCRIPT = TOOLS_DIR / "ocr.ps1"
_NO_WINDOW = 0x08000000


def _norm(line: str) -> str:
    return re.sub(r"\s+", " ", line).strip().lower()


def merge(old: list[str], new: list[str]) -> tuple[list[str], int]:
    """Append `new` to `old`, skipping lines already at the end of `old`."""
    if not old:
        return list(new), len(new)
    a = [_norm(x) for x in old]
    b = [_norm(x) for x in new]
    best = 0
    for k in range(min(len(a), len(b)), 0, -1):
        if a[-k:] == b[:k]:
            best = k
            break
    if best == 0 and b and all(x in a for x in b):
        return old, 0  # same screen again
    added = new[best:]
    return old + added, len(added)


def recognize_png(png: bytes) -> list[str]:
    fd, path = tempfile.mkstemp(suffix=".png", prefix="zenlink-ocr-")
    try:
        with os.fdopen(fd, "wb") as fh:
            fh.write(png)
        proc = subprocess.run(
            ["powershell.exe", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", str(SCRIPT), "-ImagePath", path],
            capture_output=True, timeout=30, creationflags=_NO_WINDOW,
        )
        if proc.returncode != 0:
            raise RuntimeError(proc.stderr.decode("utf-8", "replace").strip().splitlines()[-1:] or ["OCR failed"])
        text = proc.stdout.decode("utf-8", "replace")
        return [line.rstrip() for line in text.splitlines() if line.strip()]
    finally:
        try:
            os.remove(path)
        except OSError:
            pass


class OcrSession:
    def __init__(self) -> None:
        self._lock = threading.Lock()
        self.lines: list[str] = []

    def add_capture(self, data_url: str) -> dict:
        if "," in data_url:
            data_url = data_url.split(",", 1)[1]
        lines = recognize_png(base64.b64decode(data_url))
        with self._lock:
            self.lines, added = merge(self.lines, lines)
            text = "\n".join(self.lines)
        log.info("ocr: %d lines read, %d new, %d stitched", len(lines), added, len(self.lines))
        return {"ok": True, "text": text, "read": len(lines), "added": added, "total": len(self.lines)}

    def clear(self) -> dict:
        with self._lock:
            self.lines = []
        return {"ok": True}
