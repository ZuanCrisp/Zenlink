r"""The v2.5.1 beta must run next to an installed v2.5.0 and a v2.2.31 Portable.exe.

v2.2.31: ports 47321 / 47322 / AirPlay 7000, %LOCALAPPDATA%\ZenLink.
v2.5.0:  ports 47421 / 47422 / AirPlay 7100, %LOCALAPPDATA%\ZenLink-Next,
         installed as "ZenLink" with the v2.2.x AppId.
v2.5.1 beta: ports 47521 / 47522 / AirPlay 7200, %LOCALAPPDATA%\ZenLink-Beta,
         installed as "ZenLink Beta" with its own AppId.
"""

from __future__ import annotations

import importlib
import os
import unittest
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parents[1]
BS = "\\"
STABLE_APPID = "AppId={{6B0E8C1A-5D3F-4E0A-9C2B-2E7A1F5D9A10}"


class BetaIsolationTests(unittest.TestCase):
    def test_backend_port(self):
        from zenlink import server

        try:
            env = {k: v for k, v in os.environ.items() if k != "ZENLINK_PORT"}
            with mock.patch.dict(os.environ, env, clear=True):
                self.assertEqual(importlib.reload(server).PORT, 47521)
            with mock.patch.dict(os.environ, {"ZENLINK_PORT": "50111"}):
                self.assertEqual(importlib.reload(server).PORT, 50111)
        finally:
            importlib.reload(server)

    def test_stream_and_airplay_ports(self):
        from zenlink.airplay import receiver
        from zenlink.apple import session

        self.assertEqual(session.STREAM_PORT, 47522)
        self.assertEqual(receiver.AIRPLAY_PORT, 7200)
        # Two receivers with one mDNS name on one PC would clash.
        self.assertEqual(receiver.receiver_name(), "ZenLinkStudioBeta")

    def test_data_dir_name(self):
        from zenlink import paths

        self.assertEqual(paths.DATA_DIR_NAME, "ZenLink-Beta")

    def test_firewall_rule_name(self):
        # Same rule name is fine: add and delete are scoped to this program's path.
        from zenlink import firewall

        self.assertEqual(firewall.RULE, "ZenLink (AirPlay)")

    def test_rust_constants(self):
        lib = (ROOT / "rust/crates/zenlink-core/src/lib.rs").read_text(encoding="utf-8")
        self.assertIn("pub const PORT: u16 = 47521;", lib)
        self.assertIn('pub const DATA_DIR_NAME: &str = "ZenLink-Beta";', lib)
        self.assertIn('pub const LOG_FILE_NAME: &str = "zenlink-v251-beta.log";', lib)
        tray = (ROOT / "rust/crates/zenlink-app/src/tray.rs").read_text(encoding="utf-8")
        self.assertIn('w!("ZenLinkTrayV251Beta")', tray, "own tray class: single instance per build")
        self.assertIn('const RUN_VALUE: PCWSTR = w!("ZenLink Beta");', tray, "own autostart value")

    def test_installer_identity(self):
        iss = (ROOT / "packaging/ZenLink.iss").read_text(encoding="utf-8")
        lines = iss.splitlines()
        self.assertNotIn(STABLE_APPID, lines, "the beta Setup must not replace v2.5.0")
        self.assertIn("AppName=ZenLink Beta", lines)
        self.assertIn("DefaultDirName={autopf}" + BS + "ZenLink Beta", lines)
        self.assertIn("{localappdata}" + BS + "ZenLink-Beta", iss)
        self.assertNotIn("ZenLink-Next", iss)
        self.assertIn('ValueName: "ZenLink Beta"', iss)
        for old in ("av", "webview", "pythonnet", "PIL", "ui"):
            self.assertIn('Name: "{app}' + BS + old + '"', iss, "v2.2.x leftovers removed on upgrade")
        self.assertNotIn("taskkill", iss, "must not kill another ZenLink.exe by name")

    def test_uninstall_script_spares_other_builds(self):
        bat = (ROOT / "Uninstall-ZenLink.bat").read_text(encoding="utf-8")
        self.assertNotIn("taskkill", bat)
        self.assertNotIn("%LOCALAPPDATA%" + BS + 'ZenLink"', bat, "stable data folder")
        self.assertNotIn("ZenLink-Next", bat, "v2.5.0 data folder")
        self.assertIn("%LOCALAPPDATA%" + BS + "ZenLink-Beta", bat)


if __name__ == "__main__":
    unittest.main()
