r"""Filesystem locations.

APP_ROOT holds the app's read-only files (ui/, assets/, tools/, drivers/).
DATA_DIR holds what ZenLink writes (settings, logs, WebView2 profile, the
encrypted passcode): the source folder when run from source, the exe folder
for the portable build (a `portable.txt` next to ZenLink.exe), and
%LOCALAPPDATA%\ZenLink for the installed build (Program Files is read-only).
"""

from __future__ import annotations

import os
import sys
from pathlib import Path

FROZEN = bool(getattr(sys, "frozen", False))

if FROZEN:
    EXE_DIR = Path(sys.executable).resolve().parent
    # Bundled files: next to the exe (folder build) or the one-file build's
    # temporary unpack folder (sys._MEIPASS); the two are equal in a folder build.
    APP_ROOT = Path(getattr(sys, "_MEIPASS", EXE_DIR)).resolve()
    if (EXE_DIR / "portable.txt").exists():
        DATA_DIR = EXE_DIR
    else:
        DATA_DIR = Path(os.environ.get("LOCALAPPDATA") or Path.home() / "AppData" / "Local") / "ZenLink"
else:
    APP_ROOT = Path(__file__).resolve().parents[1]
    DATA_DIR = APP_ROOT

DATA_DIR.mkdir(parents=True, exist_ok=True)
UI_DIR = APP_ROOT / "ui"
