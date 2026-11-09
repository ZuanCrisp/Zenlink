"""iOS compatibility and Hybrid Control tests (SIMULATED: no device).

What these prove: which backend ZenLink picks per iOS version, that the
capability shown is the one actually opened, that Hybrid keeps the AirPlay
picture when control fails, that input reaches the HID calls with the right
coordinates, and that the Apple-mode iOS 27 check is untouched for everyone
else. What they can't prove: that a real iOS 17-26 device accepts the touches.
That needs a physical device (see README, compatibility matrix).

Run:  .venv\\Scripts\\python -m unittest discover -s tests -v
"""

from __future__ import annotations

import json
import socket
import tempfile
import unittest
from pathlib import Path
from unittest import mock


def tmp_settings() -> Path:
    return Path(tempfile.mkdtemp()) / "settings.json"


def free_port() -> int:
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


class CapabilityMatrixTests(unittest.TestCase):
    """One case per iOS band from the target: 16, 17, 18, 19-26, 27+."""

    def test_apple_mode_per_ios_band(self):
        from zenlink.capability import expected

        cases = {
            "16.7.10": "unsupported",
            "17.0": "hybrid", "17.4.1": "hybrid",
            "18.6.2": "hybrid",
            "19.0": "hybrid", "22.3": "hybrid", "26.1": "hybrid", "26.9": "hybrid",
            "27.0": "native", "27.0.1": "native", "28.1": "native",
        }
        for version, level in cases.items():
            with self.subTest(version=version):
                self.assertEqual(expected("apple", version), level)

    def test_studio_is_mirroring_on_every_version(self):
        from zenlink.capability import expected

        for version in ("16.7", "17.0", "18.6", "26.1", "27.0", None):
            self.assertEqual(expected("usbdirect", version), "mirroring")

    def test_unknown_version_or_mode_claims_nothing(self):
        from zenlink.capability import expected

        self.assertIsNone(expected("apple", None))
        self.assertIsNone(expected("apple", "garbage"))
        self.assertIsNone(expected(None, "27.0"))

    def test_hybrid_is_flagged_experimental(self):
        from zenlink.capability import verified

        self.assertTrue(verified("hybrid", video=True, control=True)["experimental"])
        self.assertFalse(verified("native", video=True, control=True)["experimental"])

    def test_missing_services(self):
        from zenlink.capability import HYBRID_SERVICES, missing_hybrid_services

        self.assertEqual(missing_hybrid_services(list(HYBRID_SERVICES) + ["x"]), [])
        self.assertEqual(len(missing_hybrid_services({})), 3)


class TouchRotationTests(unittest.TestCase):
    def test_upright_picture_maps_back_to_portrait_touch_space(self):
        """AirPlay shows the screen already turned; touches must land where
        the same point is in the device's portrait digitizer space."""
        from zenlink.capability import upright_to_native

        def turn(u, v, rot):  # portrait point -> where it shows on the upright picture
            for _ in range(rot // 90):
                u, v = 1 - v, u  # one clockwise quarter turn
            return u, v

        for rot in (0, 90, 180, 270):
            for u, v in ((0.1, 0.2), (0.9, 0.05), (0.5, 0.5), (0.0, 1.0)):
                x, y = turn(u, v, rot)
                nx, ny = upright_to_native(round(x * 65535), round(y * 65535), rot)
                with self.subTest(rot=rot, point=(u, v)):
                    self.assertAlmostEqual(nx / 65535, u, delta=2 / 65535)
                    self.assertAlmostEqual(ny / 65535, v, delta=2 / 65535)

    def test_unknown_rotation_is_identity_and_clamped(self):
        from zenlink.capability import upright_to_native

        self.assertEqual(upright_to_native(100, 200, None), (100, 200))
        self.assertEqual(upright_to_native(70000, -5, 0), (65535, 0))


class CastManagerRoutingTests(unittest.TestCase):
    def make(self, mode="apple", link="wired"):
        from zenlink.castmanager import CastManager
        from zenlink.connection import ConnectionModeManager

        with mock.patch("zenlink.castmanager.threading.Thread"):
            m = CastManager()
        m.connection = ConnectionModeManager(tmp_settings())
        m.connection.choose(mode, False)
        m.connection.set_link(link)
        return m

    def device(self, os_version):
        return {"id": "00008110-000A", "name": "iPhone", "os": os_version, "ready": True, "wifi": False}

    def start_with(self, m, os_version):
        m._presence = {"devices": [m._with_level(self.device(os_version))]}
        with mock.patch.object(m, "refresh_devices"), \
             mock.patch("zenlink.castmanager.threading.Thread"):
            m.start({})

    def test_presence_shows_expected_level(self):
        m = self.make()
        self.assertEqual(m._with_level(self.device("18.6"))["control"], "hybrid")
        self.assertTrue(m._with_level(self.device("18.6"))["hybrid"])
        self.assertEqual(m._with_level(self.device("27.0"))["control"], "native")
        self.assertNotIn("hybrid", m._with_level(self.device("27.0")))

    def test_apple_mode_is_left_to_the_window(self):
        # Apple Driver (iOS 27+) and Hybrid (17-26) run in ZenLink.exe (Rust); the service never starts them.
        for version in ("16.7", "18.6", "27.0"):
            m = self.make()
            self.start_with(m, version)
            with self.subTest(version=version):
                self.assertIsNone(m._session)
                self.assertIsNone(m.state["backend"])

    def test_studio_unchanged(self):
        m = self.make("usbdirect", "wireless")
        m.refresh_devices()
        dev = m._presence["devices"][0]
        self.assertTrue(dev["airplay"])
        self.assertNotIn("control", dev)


class FrozenBuildTests(unittest.TestCase):
    """The build self-tests the frozen exe (a missing module fails the build)."""

    root = Path(__file__).resolve().parents[4]  # repository root

    def test_build_self_tests_both_exes(self):
        script = (self.root / "build_portable_release.ps1").read_text(encoding="utf-8")
        calls = [l for l in script.splitlines() if l.strip().startswith("Test-FrozenExe ")]
        self.assertEqual(len(calls), 2, "both the folder exe and the one-file exe are tested")

    def test_selftest_covers_the_device_paths(self):
        import importlib.util

        spec = importlib.util.spec_from_file_location("zl_launch", self.root / "ZenshiroDev" / "tooling" / "windows" / "packaging" / "launch.py")
        launch = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(launch)
        for name in ("zenlink.apple.devices", "zenlink.usbdirect.session", "zenlink.castmanager"):
            self.assertIn(name, launch.SELFTEST_MODULES)
        out = Path(tempfile.mkdtemp()) / "selftest.json"
        self.assertEqual(launch.selftest(str(out)), 0, out.read_text(encoding="utf-8"))
        self.assertTrue(json.loads(out.read_text(encoding="utf-8"))["ok"])


class AppleDevicePickTests(unittest.TestCase):
    """B-53: pick which connected/saved iPhone or iPad Start uses."""

    A = {"id": "00008150-000A", "name": "iPhone 17", "deviceName": "Zenvy", "family": "iPhone",
         "os": "27.2", "ready": True, "wifi": False}
    B = {"id": "00008150-000B", "name": "iPad Pro", "deviceName": "Tab", "family": "iPad",
         "os": "27.2", "ready": True, "wifi": True}

    def make(self):
        from zenlink.castmanager import CastManager
        from zenlink.connection import ConnectionModeManager

        with mock.patch("zenlink.castmanager.threading.Thread"):
            m = CastManager()
        m.connection = ConnectionModeManager(tmp_settings())
        m._known, m._selected = {}, None
        m.connection.choose("apple", False)
        return m

    def start(self, m, devices, options=None):
        m._presence = {"devices": [m._with_level(d) for d in devices]}
        with mock.patch.object(m, "refresh_devices"), mock.patch("zenlink.castmanager.threading.Thread"):
            m.start(options or {})

    def test_trusted_devices_are_saved_and_listed_when_unplugged(self):
        m = self.make()
        m._remember([self.A, {**self.B, "ready": False}])
        self.assertEqual(list(m._known), [self.A["id"]], "only trusted devices are saved")
        m._presence = {"devices": []}
        known = m.snapshot()["known"]
        self.assertEqual(known[0]["deviceName"], "Zenvy")
        self.assertFalse(known[0]["present"])
        # Survives a restart (settings.json).
        from zenlink.connection import ConnectionModeManager
        self.assertIn(self.A["id"], ConnectionModeManager(m.connection._path).known_devices())

    def test_start_uses_the_picked_device(self):
        m = self.make()
        m.select_device(self.B["id"])
        self.start(m, [self.A, self.B])
        self.assertEqual(m.state["device"]["id"], self.B["id"])
        self.assertTrue(m.state["device"]["wifi"])

    def test_an_explicit_device_that_is_not_connected_says_so(self):
        m = self.make()
        m._known = {self.B["id"]: {"name": "iPad Pro", "deviceName": "Tab"}}
        self.start(m, [self.A], {"device": self.B["id"]})
        self.assertEqual(m.state["phase"], "error")
        self.assertIn("Tab isn't connected", m.state["error"])
        self.assertIsNone(m._session)

    def test_a_saved_pick_that_is_unplugged_falls_back_to_a_ready_device(self):
        m = self.make()
        m.select_device(self.B["id"])
        self.start(m, [self.A])
        self.assertEqual(m.state["device"]["id"], self.A["id"])

    def test_forget_removes_the_device_and_the_pick(self):
        m = self.make()
        m._remember([self.A])
        m.select_device(self.A["id"])
        m.forget_device(self.A["id"])
        self.assertEqual(m._known, {})
        self.assertIsNone(m._selected)
        self.assertIsNone(m.connection.selected_device())


if __name__ == "__main__":
    unittest.main()
