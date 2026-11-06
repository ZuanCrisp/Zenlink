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

import asyncio
import json
import socket
import tempfile
import time
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

    def test_ios_17_to_26_start_hybrid(self):
        for version in ("17.0", "18.6", "26.1"):
            m = self.make()
            self.start_with(m, version)
            with self.subTest(version=version):
                self.assertEqual(m.state["backend"], "hybrid")
                self.assertEqual(type(m._session).__name__, "HybridSession")
                self.assertIsNotNone(m.hub)
                self.assertEqual(m.state["control"]["level"], "hybrid")
                self.assertFalse(m.state["control"]["control"], "control is not claimed before it opens")
                self.assertEqual(m.capture_info()["source"], "AirPlay (Wi-Fi) + touch over the Apple driver")

    def test_ios_27_keeps_native_apple_mode(self):
        m = self.make()
        self.start_with(m, "27.0")
        self.assertEqual(m.state["backend"], "apple")
        self.assertEqual(type(m._session).__name__, "AppleCastSession")
        self.assertIsNone(m.hub)
        self.assertEqual(m.state["control"]["level"], "native")

    def test_ios_16_is_refused_with_studio_advice(self):
        m = self.make()
        self.start_with(m, "16.7")
        self.assertEqual(m.state["phase"], "error")
        self.assertIn("ZenLink Studio", m.state["error"])
        self.assertIsNone(m._session)

    def test_studio_unchanged(self):
        m = self.make("usbdirect", "wireless")
        m.refresh_devices()
        dev = m._presence["devices"][0]
        self.assertTrue(dev["airplay"])
        self.assertNotIn("control", dev)

    def test_library_message_points_to_hybrid(self):
        from zenlink.castmanager import _friendly_error

        msg = _friendly_error(Exception("Screen streaming needs iOS 27.0 or later; this device runs iOS 26.1"))
        self.assertIn("iOS 26.1", msg)
        self.assertIn("Hybrid Control", msg)


class GateBypassTests(unittest.TestCase):
    def test_bypass_only_inside_the_hybrid_gate(self):
        """Apple mode keeps the library's iOS 27 check; only Hybrid's own
        HID-gate stream skips it."""
        from pymobiledevice3.exceptions import DeviceVersionNotSupportedError
        from pymobiledevice3.remote.core_device import display_service

        from zenlink.apple import hybrid

        hybrid._install_gate_bypass()
        rsd = mock.Mock(product_version="18.6")
        with self.assertRaises(DeviceVersionNotSupportedError):
            display_service.require_video_stream_support(rsd)
        token = hybrid._GATE_STREAM.set(True)
        try:
            display_service.require_video_stream_support(rsd)  # no raise
        finally:
            hybrid._GATE_STREAM.reset(token)
        with self.assertRaises(DeviceVersionNotSupportedError):
            display_service.require_video_stream_support(rsd)


class FakeHid:
    def __init__(self):
        self.calls = []

    async def send_touchscreen(self, state, x, y, **_):
        self.calls.append(("touch", state, x, y))

    async def create_keyboard_service(self):
        self.calls.append(("kbd-create",))
        return 77

    async def send_keyboard(self, sid, usages):
        self.calls.append(("key", sid, list(usages)))

    async def send_button(self, page, code, state):
        self.calls.append(("button", page, code, state))

    async def close(self):
        pass


class HybridSessionTests(unittest.TestCase):
    """Real HybridSession with a real AirPlay receiver and input server on
    loopback; the CoreDevice side is faked."""

    def run_session(self, control_coro):
        from zenlink.airplay import receiver
        from zenlink.apple import hybrid, session

        states, controls = [], []
        port = free_port()
        patches = [
            mock.patch.object(receiver, "AIRPLAY_PORT", 0),
            mock.patch("zenlink.airplay.receiver.AirPlayReceiver._advertise"),
            mock.patch("zenlink.airplay.receiver.AirPlayReceiver._unadvertise"),
            mock.patch.object(session, "STREAM_PORT", port),
            mock.patch.object(hybrid.HybridSession, "_control_once", control_coro),
        ]
        for p in patches:
            p.start()
        self.addCleanup(lambda: [p.stop() for p in patches])
        s = hybrid.HybridSession("UDID", mock.Mock(), lambda ph, d: states.append((ph, d)), {},
                                 controls.append)
        import threading

        t = threading.Thread(target=s.run, daemon=True)
        t.start()
        return s, t, port, states, controls

    def post(self, port, path, payload):
        import urllib.error
        import urllib.request

        req = urllib.request.Request(f"http://127.0.0.1:{port}/{path}", json.dumps(payload).encode(),
                                     {"Content-Type": "application/json"})
        try:
            with urllib.request.urlopen(req, timeout=3) as r:
                return r.status, r.read().decode()
        except urllib.error.HTTPError as e:
            return e.code, e.read().decode()

    def wait(self, cond, timeout=5):
        end = time.time() + timeout
        while time.time() < end:
            if cond():
                return True
            time.sleep(0.03)
        return False

    def test_control_failure_keeps_picture_and_says_why(self):
        async def failing(self):
            raise RuntimeError("Developer Mode is disabled")

        s, t, port, states, controls = self.run_session(failing)
        self.assertTrue(self.wait(lambda: controls and controls[-1]["level"] == "mirroring"))
        self.assertIn("Developer Mode", controls[-1]["note"])
        self.assertFalse(controls[-1]["control"])
        self.assertTrue(self.wait(lambda: any(ph == "activating" and "Screen Mirroring" in d for ph, d in states)),
                        "AirPlay guide is still shown")
        code, body = self.post(port, "touch", {"type": "tap", "x": 1, "y": 1})
        self.assertEqual(code, 409)
        self.assertIn("Developer Mode", body)
        s.stop()
        t.join(5)
        self.assertFalse(t.is_alive())

    def test_input_reaches_hid_with_rotation(self):
        hid = FakeHid()

        async def ready(self):
            self._uhs = self._indigo = hid
            self._rotation = 90
            self._report("hybrid", True, "ok")
            await asyncio.Event().wait()

        s, t, port, _states, controls = self.run_session(ready)
        self.assertTrue(self.wait(lambda: controls and controls[-1]["control"]))
        self.assertEqual(self.post(port, "touch", {"type": "contact", "x": 0, "y": 65535})[0], 200)
        self.assertEqual(self.post(port, "touch", {"type": "release", "x": 0, "y": 65535})[0], 200)
        self.assertEqual(self.post(port, "key", {"usages": [4]})[0], 200)
        self.assertEqual(self.post(port, "button", {"name": "home", "state": "press"})[0], 200)
        self.assertTrue(self.wait(lambda: len(hid.calls) >= 6))
        touches = [c for c in hid.calls if c[0] == "touch"]
        # bottom-left of a picture turned 90° clockwise is the bottom-right... of
        # portrait space: (1, 1) -> u=1, v=1
        self.assertEqual(touches[0][2:], (65535, 65535))
        self.assertEqual([c[0] for c in hid.calls[2:]], ["kbd-create", "key", "button", "button"])
        s.stop()
        t.join(5)
        self.assertFalse(t.is_alive())


class HybridServerRouteTests(unittest.IsolatedAsyncioTestCase):
    async def test_input_is_forwarded_only_for_hybrid(self):
        from aiohttp.test_utils import TestClient, TestServer

        from zenlink.castmanager import CastManager
        from zenlink.connection import ConnectionModeManager
        from zenlink.server import build_app

        with mock.patch("zenlink.castmanager.threading.Thread"):
            m = CastManager()
        m.connection = ConnectionModeManager(tmp_settings())
        m.hub = mock.Mock()
        async with TestClient(TestServer(build_app(m))) as c:
            m.state["backend"] = "usbdirect"
            r = await c.post("/cast/touch", json={"type": "tap", "x": 1, "y": 1})
            self.assertEqual(r.status, 409)  # Studio: picture only
            m.state["backend"] = "hybrid"
            with mock.patch("zenlink.server.UPSTREAM", f"http://127.0.0.1:{free_port()}"):
                r = await c.post("/cast/touch", json={"type": "tap", "x": 1, "y": 1})
            self.assertEqual(r.status, 503)  # forwarded to the HID server (not running here)


class FrozenBuildTests(unittest.TestCase):
    """The exe once failed Apple mode with "No package metadata was found
    for pyimg4": pyimg4 reads its own version at import and the build left
    its dist-info out. The build now copies the metadata and self-tests."""

    root = Path(__file__).resolve().parents[1]

    def test_build_copies_dependency_metadata_and_self_tests(self):
        script = (self.root / "build_portable_release.ps1").read_text(encoding="utf-8")
        self.assertIn('"--recursive-copy-metadata", "pymobiledevice3"', script)
        # pytun_pmd3 loads wintun.dll when pymobiledevice3's tunnel module is imported.
        self.assertIn('"--collect-all", "pytun_pmd3"', script)
        calls = [l for l in script.splitlines() if l.strip().startswith("Test-FrozenExe ")]
        self.assertEqual(len(calls), 2, "both the folder exe and the one-file exe are tested")

    def test_selftest_covers_the_device_paths(self):
        import importlib.util

        spec = importlib.util.spec_from_file_location("zl_launch", self.root / "packaging" / "launch.py")
        launch = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(launch)
        for name in ("pyimg4", "pymobiledevice3.services.mobile_image_mounter", "zenlink.apple.hybrid",
                     "zenlink.apple.session", "zenlink.usbdirect.session", "zenlink.airplay.receiver"):
            self.assertIn(name, launch.SELFTEST_MODULES)
        out = Path(tempfile.mkdtemp()) / "selftest.json"
        self.assertEqual(launch.selftest(str(out)), 0, out.read_text(encoding="utf-8"))
        self.assertTrue(json.loads(out.read_text(encoding="utf-8"))["ok"])


if __name__ == "__main__":
    unittest.main()
