"""The Manual (source) package downloads what it does not carry, at the same
versions as the release build (Start ZenLink.bat, tools/get-adb.ps1,
requirements.lock)."""

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def _bat_pin(name: str) -> str:
    text = (ROOT / "Start ZenLink.bat").read_text(encoding="utf-8")
    m = re.search(rf'^set "{name}=(.+)"\r?$', text, re.M)
    assert m, name
    return m.group(1)


def test_adb_pin_is_the_same_everywhere():
    rev, sha = _bat_pin("ZL_ADB_REV"), _bat_pin("ZL_ADB_SHA256")
    assert re.fullmatch(r"\d+\.\d+\.\d+", rev)
    assert re.fullmatch(r"[0-9a-f]{64}", sha)
    script = (ROOT / "tools" / "get-adb.ps1").read_text(encoding="utf-8")
    assert f'[string]$Revision = "{rev}"' in script
    assert f'[string]$Sha256 = "{sha}"' in script
    notices = (ROOT / "docs" / "THIRD_PARTY.md").read_text(encoding="utf-8")
    assert f"platform-tools_r{rev}-win.zip" in notices
    assert sha in notices


def test_bat_passes_the_pin_to_the_download():
    text = (ROOT / "Start ZenLink.bat").read_text(encoding="utf-8")
    assert '-Revision "%ZL_ADB_REV%" -Sha256 "%ZL_ADB_SHA256%"' in text
    assert '-c "%~dp0requirements.lock"' in text


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
    for s in lines(ROOT / "requirements.lock"):
        assert "==" in s, s
        locked[name(s)] = s
    for s in lines(ROOT / "requirements.txt"):
        assert name(s) in locked, f"{s} is missing from requirements.lock"


def test_source_zip_leaves_out_what_is_downloaded_or_built_in():
    attrs = (ROOT / ".gitattributes").read_text(encoding="utf-8")
    assert "third_party/scrcpy/scrcpy-server-v5.0.1 export-ignore" in attrs
    ignore = (ROOT / ".gitignore").read_text(encoding="utf-8")
    assert "third_party/platform-tools/" in ignore
