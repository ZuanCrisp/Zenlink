"""Prerequisite checks: nothing warns about or installs what ZenLink Next does not use."""

import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


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
        iss = (ROOT / "packaging" / "ZenLink.iss").read_text(encoding="utf-8")
        self.assertNotIn("EdgeUpdate", iss)
        self.assertNotIn("WebView2 Runtime (included", iss)

    def test_start_script_checks_webview2_only_for_classic_window(self):
        bat = _bat()
        self.assertNotIn("EdgeUpdate", _section(bat, ":check_env"), "default setup must not install WebView2")
        self.assertIn("call :check_webview2", _section(bat, ":classic"))
        self.assertNotIn("check_webview2", _section(bat, ":launch"))

    def test_start_script_builds_latest_code_before_launch(self):
        # The user tests manually with Start ZenLink.bat: it must run the current code.
        bat = _bat()
        self.assertIn("call :build_next", _section(bat, ":launch"))
        build = _section(bat, ":build_next")
        self.assertIn(r"rust\tools\build-window.ps1", build)
        # The helper runs cargo with a progress title (elapsed / estimate).
        ps = (ROOT / "rust" / "tools" / "build-window.ps1").read_text(encoding="utf-8")
        self.assertRegex(ps, r"build.+--release.+-p.+zenlink-app")

    def test_start_script_loads_machine_only_settings(self):
        # A3: e.g. ANDROID_HOME for the user's SDK, kept out of the repository.
        launch = _section(_bat(), ":launch")
        self.assertIn('call "%~dp0Start ZenLink.local.bat"', launch)
        self.assertLess(launch.index("Start ZenLink.local.bat"), launch.index("call :build_next"))

    def test_android_fixture_is_small_and_binary(self):
        fx = ROOT / "tests" / "fixtures" / "android" / "synthetic-h264-360x640.scrcpy"
        self.assertLess(fx.stat().st_size, 150_000)
        self.assertIn("*.scrcpy binary", (ROOT / ".gitattributes").read_text(encoding="utf-8"))

    def test_icon_font_has_windows_10_fallback(self):
        # B-12: "Segoe Fluent Icons" exists only on Windows 11.
        main = (ROOT / "rust" / "crates" / "zenlink-app" / "src" / "main.rs").read_text(encoding="utf-8")
        self.assertIn('"Segoe MDL2 Assets"', main)
        for f in (ROOT / "rust" / "crates" / "zenlink-app" / "ui").glob("*.slint"):
            self.assertNotIn('font-family: "Segoe Fluent Icons"', f.read_text(encoding="utf-8"), f.name)


if __name__ == "__main__":
    unittest.main()
