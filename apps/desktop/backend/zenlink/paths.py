r"""Filesystem locations.

APP_ROOT holds the app's read-only files. In the frozen build they all sit in
one folder (ui/, assets/, tools/, drivers/). In a source checkout APP_ROOT is
the repository root and the files live in the monorepo folders:
apps/desktop/backend/ui, ZenshiroDev/resources/assets,
ZenshiroDev/tooling/windows/tools, ZenshiroDev/resources/windows/drivers.
Use BACKEND_DIR, UI_DIR, ASSETS_DIR, TOOLS_DIR and DRIVERS_DIR, not APP_ROOT.

DATA_DIR holds what ZenLink writes (settings, logs, WebView2 profile, the
encrypted passcode): `data/` in the repository when run from source, the exe
folder for the portable build (a `portable.txt` next to ZenLink.exe), and
%LOCALAPPDATA%\ZenLink-Next for the installed build (Program Files is read-only).
The stable v2.2.31 uses %LOCALAPPDATA%\ZenLink; the beta never touches it.
"""

from __future__ import annotations

import os
import sys
from pathlib import Path

FROZEN = bool(getattr(sys, "frozen", False))
# v2.5 data folder name (the stable build uses "ZenLink").
DATA_DIR_NAME = "ZenLink-Next"
# Files a source checkout kept in the repository root before data/ existed.
LEGACY_DATA = ("settings.json", "devices.json", "airplay.json", "secrets.json", "allow-airplay.cmd", ".webview")

if FROZEN:
    EXE_DIR = Path(sys.executable).resolve().parent
    # Bundled files: next to the exe (folder build) or the one-file build's
    # temporary unpack folder (sys._MEIPASS); the two are equal in a folder build.
    APP_ROOT = Path(getattr(sys, "_MEIPASS", EXE_DIR)).resolve()
    if (EXE_DIR / "portable.txt").exists():
        DATA_DIR = EXE_DIR
    else:
        DATA_DIR = Path(os.environ.get("LOCALAPPDATA") or Path.home() / "AppData" / "Local") / DATA_DIR_NAME
    BACKEND_DIR = APP_ROOT
    UI_DIR = APP_ROOT / "ui"
    TOOLS_DIR = APP_ROOT / "tools"
    DRIVERS_DIR = APP_ROOT / "drivers"
else:
    # apps/desktop/backend/zenlink/paths.py -> repository root
    APP_ROOT = Path(__file__).resolve().parents[4]
    # ZENLINK_DATA_DIR keeps dev data outside the checkout (Start ZenLink.local.bat).
    DATA_DIR = Path(os.environ.get("ZENLINK_DATA_DIR") or APP_ROOT / "data").resolve()
    BACKEND_DIR = APP_ROOT / "apps" / "desktop" / "backend"
    UI_DIR = BACKEND_DIR / "ui"
    TOOLS_DIR = APP_ROOT / "ZenshiroDev" / "tooling" / "windows" / "tools"
    DRIVERS_DIR = APP_ROOT / "ZenshiroDev" / "resources" / "windows" / "drivers"
ASSETS_DIR = APP_ROOT / "assets" if FROZEN else APP_ROOT / "ZenshiroDev" / "resources" / "assets"


def migrate_legacy_data(old: Path, new: Path) -> list[str]:
    """Move LEGACY_DATA from `old` to `new`; never overwrites a file in `new`."""
    moved: list[str] = []
    if old.resolve() == new.resolve():
        return moved
    for name in LEGACY_DATA:
        src, dst = old / name, new / name
        if src.exists() and not dst.exists():
            try:
                src.replace(dst)
                moved.append(name)
            except OSError:
                pass
    return moved


DATA_DIR.mkdir(parents=True, exist_ok=True)
if not FROZEN:
    migrate_legacy_data(APP_ROOT, DATA_DIR)

# All logs (backend, crashes, and the window's own log from the Rust app):
# %LOCALAPPDATA%\ZenLink-Next\logs, also for a source checkout, so a bug
# report always points at one folder (Settings > About > Open log folder).
if FROZEN and DATA_DIR == EXE_DIR:
    LOG_DIR = DATA_DIR / "logs"
else:
    LOG_DIR = Path(os.environ.get("LOCALAPPDATA") or Path.home() / "AppData" / "Local") / DATA_DIR_NAME / "logs"
try:
    LOG_DIR.mkdir(parents=True, exist_ok=True)
except OSError:
    LOG_DIR = DATA_DIR
