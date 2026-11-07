"""Builds the Microsoft Store package release/ZenLink-v<ver>.msix.

Run after build_portable_release.ps1: it packs the installer layout
(dist/ZenLinkService: ZenLink.exe, ZenLinkService.exe, adb, ...) with
packaging/msix/AppxManifest.xml and logos made from assets/logo.png.

The package is unsigned: the Microsoft Store signs it. For a local test
install it must be signed with a certificate whose subject equals the
manifest Publisher (only with the user's consent: that installs a
certificate).

    .venv\\Scripts\\python packaging\\build_msix.py
"""

from __future__ import annotations

import re
import shutil
import subprocess
import sys
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / "dist" / "ZenLinkService"
LAYOUT = ROOT / "build" / "msix"
MAKEAPPX = Path(r"C:\Program Files (x86)\Windows Kits\10\bin\10.0.26100.0\x64\makeappx.exe")
# Helper scripts for the source/portable folder; the package has its own
# Start menu entry and uninstall.
SKIP = {"Start ZenLink.bat", "run.bat", "Create-Desktop-Shortcut.bat", "Uninstall-ZenLink.bat", "portable.txt"}
LOGOS = {
    "StoreLogo.png": (50, 50),
    "Square44x44Logo.png": (44, 44),
    "Square44x44Logo.targetsize-256_altform-unplated.png": (256, 256),
    "Square44x44Logo.targetsize-48_altform-unplated.png": (48, 48),
    "Square44x44Logo.targetsize-32_altform-unplated.png": (32, 32),
    "Square44x44Logo.targetsize-16_altform-unplated.png": (16, 16),
    "Square150x150Logo.png": (150, 150),
    "Wide310x150Logo.png": (310, 150),
}


def version() -> str:
    text = (ROOT / "pyproject.toml").read_text(encoding="utf-8")
    v = re.search(r'^version\s*=\s*"(\d+)\.(\d+)\.(\d+)"', text, re.M)
    if not v:
        sys.exit("pyproject.toml: a plain X.Y.Z version is needed for the Store")
    # The Store needs four parts and the last one 0.
    return f"{v[1]}.{v[2]}.{v[3]}.0"


def logo(size: tuple[int, int], dest: Path) -> None:
    src = Image.open(ROOT / "assets" / "logo.png").convert("RGBA")
    w, h = size
    side = min(w, h)
    # Small sizes keep the whole mark; tiles get some padding.
    inner = side if side <= 48 else int(side * 0.8)
    mark = src.resize((inner, inner), Image.LANCZOS)
    canvas = Image.new("RGBA", size, (0, 0, 0, 0))
    canvas.paste(mark, ((w - inner) // 2, (h - inner) // 2), mark)
    canvas.save(dest)


def main() -> None:
    if not (SOURCE / "ZenLink.exe").exists() or not (SOURCE / "ZenLinkService.exe").exists():
        sys.exit(f"{SOURCE} is not built: run build_portable_release.ps1 first")
    if not (SOURCE / "third_party" / "platform-tools" / "adb.exe").exists():
        sys.exit("adb is missing from the layout")
    ver = version()
    if LAYOUT.exists():
        shutil.rmtree(LAYOUT)
    shutil.copytree(SOURCE, LAYOUT, ignore=lambda d, names: [n for n in names if Path(d) == SOURCE and n in SKIP])
    # The current ZenLink.exe (it knows the Store edition: updater::is_store).
    subprocess.run(["cargo", "build", "--release", "--manifest-path", str(ROOT / "rust" / "Cargo.toml"), "-p", "zenlink-app"], check=True)
    shutil.copy2(ROOT / "rust" / "target" / "release" / "ZenLink.exe", LAYOUT / "ZenLink.exe")
    manifest = (ROOT / "packaging" / "msix" / "AppxManifest.xml").read_text(encoding="utf-8")
    (LAYOUT / "AppxManifest.xml").write_text(manifest.replace("{VERSION}", ver), encoding="utf-8")
    assets = LAYOUT / "Assets"
    assets.mkdir(exist_ok=True)
    for name, size in LOGOS.items():
        logo(size, assets / name)
    out = ROOT / "release" / f"ZenLink-v{ver.rsplit('.', 1)[0]}.msix"
    out.parent.mkdir(exist_ok=True)
    subprocess.run([str(MAKEAPPX), "pack", "/o", "/h", "SHA256", "/d", str(LAYOUT), "/p", str(out)], check=True)
    print(f"{out} {out.stat().st_size / 1e6:.1f} MB (version {ver})")


if __name__ == "__main__":
    main()
