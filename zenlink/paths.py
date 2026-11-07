r"""Filesystem locations.

APP_ROOT holds the app's read-only files (ui/, assets/, tools/, drivers/).
DATA_DIR holds what ZenLink writes (settings, logs, WebView2 profile, the
encrypted passcode): the source folder when run from source, the exe folder
for the portable build (a `portable.txt` next to ZenLink.exe), and
%LOCALAPPDATA%\ZenLink-Beta for the installed build (Program Files is read-only).
The stable v2.2.31 uses %LOCALAPPDATA%\ZenLink; the beta never touches it.
"""

from __future__ import annotations

import os
import sys
from pathlib import Path

FROZEN = bool(getattr(sys, "frozen", False))
# v2.5 data folder name (the stable build uses "ZenLink").
DATA_DIR_NAME = "ZenLink-Beta"

if FROZEN:
    EXE_DIR = Path(sys.executable).resolve().parent
    # Bundled files: next to the exe (folder build) or the one-file build's
    # temporary unpack folder (sys._MEIPASS); the two are equal in a folder build.
    APP_ROOT = Path(getattr(sys, "_MEIPASS", EXE_DIR)).resolve()
    if (EXE_DIR / "portable.txt").exists():
        DATA_DIR = EXE_DIR
    else:
        DATA_DIR = Path(os.environ.get("LOCALAPPDATA") or Path.home() / "AppData" / "Local") / DATA_DIR_NAME
else:
    APP_ROOT = Path(__file__).resolve().parents[1]
    DATA_DIR = APP_ROOT

DATA_DIR.mkdir(parents=True, exist_ok=True)

# All logs (backend, crashes, and the window's own log from the Rust app):
# %LOCALAPPDATA%\ZenLink-Beta\logs, also for a source checkout, so a bug
# report always points at one folder (Settings > About > Open log folder).
if FROZEN and DATA_DIR == EXE_DIR:
    LOG_DIR = DATA_DIR / "logs"
else:
    LOG_DIR = Path(os.environ.get("LOCALAPPDATA") or Path.home() / "AppData" / "Local") / DATA_DIR_NAME / "logs"
try:
    LOG_DIR.mkdir(parents=True, exist_ok=True)
except OSError:
    LOG_DIR = DATA_DIR
UI_DIR = APP_ROOT / "ui"
