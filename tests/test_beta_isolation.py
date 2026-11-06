r"""v2.5 Beta must run next to the stable v2.2.31 without sharing anything.

The stable build uses port 47321 (backend), 47322 (local stream), 7000
(AirPlay), %LOCALAPPDATA%\ZenLink and the "ZenLink (AirPlay)" firewall rule.
"""

from __future__ import annotations

import importlib
import os
import unittest
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parents[1]
BS = "\\"


class BetaIsolationTests(unittest.TestCase):
    def test_backend_port(self):
        from zenlink import server

        try:
            env = {k: v for k, v in os.environ.items() if k != "ZENLINK_PORT"}
            with mock.patch.dict(os.environ, env, clear=True):
                self.assertEqual(importlib.reload(server).PORT, 47421)
            with mock.patch.dict(os.environ, {"ZENLINK_PORT": "50111"}):
                self.assertEqual(importlib.reload(server).PORT, 50111)
        finally:
            importlib.reload(server)

    def test_stream_and_airplay_ports(self):
        from zenlink.airplay import receiver
        from zenlink.apple import session

        self.assertEqual(session.STREAM_PORT, 47422)
        self.assertEqual(receiver.AIRPLAY_PORT, 7100)
        self.assertEqual(receiver.receiver_name(), "ZenLinkStudioBeta")

    def test_data_dir_name(self):
        from zenlink import paths

        self.assertEqual(paths.DATA_DIR_NAME, "ZenLink-Beta")

    def test_firewall_rule_name(self):
        from zenlink import firewall

        self.assertEqual(firewall.RULE, "ZenLink Beta (AirPlay)")

    def test_rust_constants(self):
        lib = (ROOT / "rust/crates/zenlink-core/src/lib.rs").read_text(encoding="utf-8")
        self.assertIn("pub const PORT: u16 = 47421;", lib)
        self.assertIn('pub const DATA_DIR_NAME: &str = "ZenLink-Beta";', lib)

    def test_installer_identity(self):
        iss = (ROOT / "packaging/ZenLink.iss").read_text(encoding="utf-8")
        lines = iss.splitlines()
        self.assertNotIn("6B0E8C1A-5D3F-4E0A-9C2B-2E7A1F5D9A10", iss, "stable AppId")
        self.assertIn("AppName=ZenLink Beta", lines)
        self.assertIn("DefaultDirName={autopf}" + BS + "ZenLink Beta", lines)
        self.assertIn("{localappdata}" + BS + "ZenLink-Beta", iss)
        self.assertNotIn("{localappdata}" + BS + 'ZenLink"', iss, "stable data folder")
        self.assertNotIn("taskkill", iss, "must not kill the stable ZenLink.exe by name")
        self.assertNotIn('"ZenLink (AirPlay)"', iss.replace('""', '"'), "stable firewall rule")

    def test_uninstall_script_spares_stable(self):
        bat = (ROOT / "Uninstall-ZenLink.bat").read_text(encoding="utf-8")
        self.assertNotIn("taskkill", bat)
        self.assertNotIn("%LOCALAPPDATA%" + BS + 'ZenLink"', bat, "stable data folder")
        self.assertIn("%LOCALAPPDATA%" + BS + "ZenLink-Beta", bat)


if __name__ == "__main__":
    unittest.main()
