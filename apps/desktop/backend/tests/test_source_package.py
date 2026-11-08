"""The Manual (source) package downloads what it does not carry, at the same
versions as the release build (Start ZenLink.bat, the in-app adb installer,
ZenshiroDev/tooling/windows/tools/get-adb.ps1, requirements.lock)."""

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]  # apps/desktop/backend/tests -> repository root


def _bat_pin(name: str) -> str:
    text = (ROOT / "Start ZenLink.bat").read_text(encoding="utf-8")
    m = re.search(rf'^set "{name}=(.+)"\r?$', text, re.M)
    assert m, name
    return m.group(1)


def test_adb_pin_is_the_same_everywhere():
    rev, sha = _bat_pin("ZL_ADB_REV"), _bat_pin("ZL_ADB_SHA256")
    assert re.fullmatch(r"\d+\.\d+\.\d+", rev)
    assert re.fullmatch(r"[0-9a-f]{64}", sha)
    script = (ROOT / "ZenshiroDev" / "tooling" / "windows" / "tools" / "get-adb.ps1").read_text(encoding="utf-8")
    assert f'[string]$Revision = "{rev}"' in script
    assert f'[string]$Sha256 = "{sha}"' in script
    notices = (ROOT / "ZenshiroDev" / "docs" / "THIRD_PARTY.md").read_text(encoding="utf-8")
    assert f"platform-tools_r{rev}-win.zip" in notices
    assert sha in notices
    # ZenLink.exe downloads the same zip on request (adb is not bundled).
    android = (ROOT / "apps" / "desktop" / "src" / "android.rs").read_text(encoding="utf-8")
    assert f"platform-tools_r{rev}-win.zip" in android
    assert sha in android


def test_bat_pins_python_and_does_not_prompt_for_adb():
    text = (ROOT / "Start ZenLink.bat").read_text(encoding="utf-8")
    # adb comes from the in-app "Install adb" button; a Y/N prompt here blocked the start.
    assert "get-adb.ps1\" -Revision" not in text
    assert "Download it from Google now?" not in text
    assert r'-c "%~dp0apps\desktop\backend\requirements.lock"' in text


def test_lock_pins_every_requirement():
    def name(line: str) -> str:
        return re.split(r"[<>=!~\[; ]", line, maxsplit=1)[0].strip().lower().replace("_", "-")

    def lines(p: Path):
        return [
            s.strip()
            for s in p.read_text(encoding="utf-8").splitlines()
            if s.strip() and not s.lstrip().startswith("#")
        ]

    locked = {}
    for s in lines(ROOT / "apps" / "desktop" / "backend" / "requirements.lock"):
        assert "==" in s, s
        locked[name(s)] = s
    for s in lines(ROOT / "apps" / "desktop" / "backend" / "requirements.txt"):
        assert name(s) in locked, f"{s} is missing from requirements.lock"


def test_source_zip_leaves_out_what_is_downloaded_or_built_in():
    attrs = (ROOT / ".gitattributes").read_text(encoding="utf-8")
    assert "ZenshiroDev/resources/third_party/scrcpy/scrcpy-server-v5.0.1 export-ignore" in attrs
    ignore = (ROOT / ".gitignore").read_text(encoding="utf-8")
    assert "ZenshiroDev/resources/third_party/platform-tools/" in ignore
