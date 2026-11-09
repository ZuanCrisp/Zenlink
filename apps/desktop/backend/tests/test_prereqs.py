"""Prerequisite checks: nothing warns about or installs what ZenLink Next does not use."""

import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]  # apps/desktop/backend/tests -> repository root


def _bat() -> str:
    return (ROOT / "Start ZenLink.bat").read_text(encoding="utf-8")


def _section(text: str, label: str) -> str:
    """Body of a batch label, up to the next label definition."""
    lines = text.splitlines()
    start = lines.index(label) + 1
    end = next((i for i in range(start, len(lines)) if lines[i].startswith(":")), len(lines))
    return "\n".join(lines[start:end])


class PrerequisiteTests(unittest.TestCase):
    def test_installer_has_no_webview2_check(self):
        # B-6: the Rust window does not use Microsoft Edge WebView2.
        iss = (ROOT / "ZenshiroDev" / "tooling" / "windows" / "packaging" / "ZenLink.iss").read_text(encoding="utf-8")
        self.assertNotIn("EdgeUpdate", iss)
        self.assertNotIn("WebView2 Runtime (included", iss)

    def test_start_script_never_installs_webview2(self):
        # v2.5.12: the classic WebView2 window is gone; only the Rust window starts.
        self.assertNotIn("EdgeUpdate", _bat())
        self.assertNotIn(":classic", _bat())

    def test_start_script_builds_latest_code_before_launch(self):
        # The user tests manually with Start ZenLink.bat: it must run the current code.
        bat = _bat()
        self.assertIn("call :build_next", _section(bat, ":launch"))
        build = _section(bat, ":build_next")
        self.assertIn(r"ZenshiroDev\tooling\dev\build-window.ps1", build)
        # The helper runs cargo with a progress title (elapsed / estimate).
        ps = (ROOT / "ZenshiroDev" / "tooling" / "dev" / "build-window.ps1").read_text(encoding="utf-8")
        self.assertRegex(ps, r"build.+--release.+-p.+zenlink-app")

    def test_start_script_loads_machine_only_settings(self):
        # A3: e.g. ANDROID_HOME for the user's SDK, kept out of the repository.
        launch = _section(_bat(), ":launch")
        self.assertIn('call "%~dp0Start ZenLink.local.bat"', launch)
        self.assertLess(launch.index("Start ZenLink.local.bat"), launch.index("call :build_next"))

    def test_android_fixture_is_small_and_binary(self):
        fx = ROOT / "apps" / "desktop" / "tests" / "fixtures" / "android" / "synthetic-h264-360x640.scrcpy"
        self.assertLess(fx.stat().st_size, 150_000)
        self.assertIn("*.scrcpy binary", (ROOT / ".gitattributes").read_text(encoding="utf-8"))

    def test_icon_font_has_windows_10_fallback(self):
        # B-12: "Segoe Fluent Icons" exists only on Windows 11.
        main = (ROOT / "apps" / "desktop" / "src" / "main.rs").read_text(encoding="utf-8")
        self.assertIn('"Segoe MDL2 Assets"', main)
        for f in (ROOT / "apps" / "desktop" / "ui").glob("*.slint"):
            self.assertNotIn('font-family: "Segoe Fluent Icons"', f.read_text(encoding="utf-8"), f.name)


if __name__ == "__main__":
    unittest.main()
