"""ZenLink audit tests: modes, links, AirPlay protocol, audio, API.

Run:  .venv\\Scripts\\python -m unittest discover -s tests -v
No device is needed: the iPhone/iPad side is simulated.
"""

from __future__ import annotations

import asyncio
import tempfile
import threading
import time
import unittest
from pathlib import Path
from unittest import mock

def tmp_settings() -> Path:
    return Path(tempfile.mkdtemp()) / "settings.json"


# ------------------------------------------------------------------ connection

class ConnectionModeTests(unittest.TestCase):
    def test_remember_and_switch(self):
        from zenlink.connection import ConnectionModeManager as M

        path = tmp_settings()
        m = M(path)
        self.assertTrue(m.snapshot()["needsChoice"])
        m.choose("apple", True)
        m.choose("usbdirect", True)  # switching while remembered saves the new mode
        snap = M(path).snapshot()
        self.assertEqual(snap["active"], "usbdirect")
        self.assertTrue(snap["remember"])

    def test_not_remembered_asks_again(self):
        from zenlink.connection import ConnectionModeManager as M

        path = tmp_settings()
        M(path).choose("apple", False)
        self.assertTrue(M(path).snapshot()["needsChoice"])

    def test_link_persists_and_survives_mode_change(self):
        from zenlink.connection import ConnectionModeManager as M

        path = tmp_settings()
        m = M(path)
        m.choose("apple", True)
        m.set_link("wireless")
        m.choose("usbdirect", True)
        self.assertEqual(M(path).snapshot()["link"], "wireless")
        with self.assertRaises(ValueError):
            m.set_link("bluetooth")


# ------------------------------------------------------------------ device lists

class DeviceListTests(unittest.TestCase):
    def setUp(self):
        from zenlink.apple import devices

        self.devices = devices
        listed = [
            {"udid": "A", "deviceId": 1, "connection": "USB"},
            {"udid": "A", "deviceId": 2, "connection": "Network"},
            {"udid": "B", "deviceId": 3, "connection": "USB"},
        ]
        self.patches = [
            mock.patch.object(devices.usbmuxd, "list_devices", return_value=listed),
            mock.patch.object(devices, "_read_info", return_value={"paired": True, "DeviceClass": "iPad"}),
            mock.patch.object(devices, "_taken_over", return_value=False),
        ]
        for p in self.patches:
            p.start()

    def tearDown(self):
        for p in self.patches:
            p.stop()

    def test_wired_lists_cable_only(self):
        got = self.devices.connected_devices("wired")["devices"]
        self.assertEqual([(d["id"], d["wifi"]) for d in got], [("A", False), ("B", False)])

    def test_wireless_prefers_wifi_and_flags_cable_only(self):
        got = {d["id"]: d for d in self.devices.connected_devices("wireless")["devices"]}
        self.assertTrue(got["A"]["wifi"])
        # A trusted device on the cable stays usable over the cable in Wi-Fi
        # mode (an iPhone could not connect after an iPad had Wi-Fi set up).
        self.assertTrue(got["B"]["ready"])
        self.assertFalse(got["B"]["wifi"])

# ------------------------------------------------------------------ cast manager

class CastManagerTests(unittest.TestCase):
    def make(self, mode: str, link: str):
        from zenlink.castmanager import CastManager
        from zenlink.connection import ConnectionModeManager

        with mock.patch("zenlink.castmanager.threading.Thread"):  # no background watcher
            m = CastManager()
        m.connection = ConnectionModeManager(tmp_settings())
        m.connection.choose(mode, False)
        m.connection.set_link(link)
        return m

    def test_studio_wireless_presents_airplay(self):
        m = self.make("usbdirect", "wireless")
        m.refresh_devices()
        dev = m._presence["devices"][0]
        self.assertTrue(dev["airplay"] and dev["ready"])
        self.assertEqual(dev["receiver"], "ZenLinkStudio")

    def test_wifi_needed_message(self):
        m = self.make("apple", "wireless")
        m._presence = {"devices": [{"id": "X", "name": "iPad", "need": "wifi", "ready": False, "wifi": False}]}
        with mock.patch.object(m, "refresh_devices"):
            m.start({})
        self.assertEqual(m.state["phase"], "error")
        self.assertIn("Wi-Fi", m.state["error"])


# ------------------------------------------------------------------ AirPlay protocol (simulated iPad)

# ------------------------------------------------------------------ video hub

class HubTests(unittest.IsolatedAsyncioTestCase):
    def frames(self, n):
        idr = (5).to_bytes(4, "big") + bytes([0x26, 1, 0xAF, 0, 0])      # HEVC IDR_W_RADL
        delta = (5).to_bytes(4, "big") + bytes([0x02, 1, 0xD0, 0, 0])    # HEVC TRAIL_R
        return [idr] + [delta] * (n - 1)

    async def test_airplay_hub_never_skips_and_replays_from_keyframe(self):
        from zenlink.usbdirect.hub import MediaHub

        hub = MediaHub(sparse_keyframes=True)
        hub.set_codec("hvc1.1.6.L150.B0", bytes(23), 2302, 1600, "HEVC")
        self.assertFalse(hub.codec()["keyframesOnRequest"])
        loop = asyncio.get_running_loop()
        slow = hub.open_video(loop)
        for f in self.frames(3000):          # one keyframe, then 2999 deltas (50 s)
            hub.push_video(f)
        await asyncio.sleep(0.05)
        self.assertFalse(slow.need_key)
        self.assertEqual(slow.queue.qsize(), 3000)   # nothing dropped
        late = hub.open_video(loop)                  # e.g. the window reloaded
        self.assertEqual(late.queue.qsize(), 3000)   # replayed from the keyframe
        for f in self.frames(2)[1:]:
            hub.push_video(f)                        # a live frame right after the replay
        await asyncio.sleep(0.05)
        self.assertEqual(late.queue.get_nowait()[4], 0)  # first message is the keyframe
        self.assertIn(late, hub._video, "a replay must never get the viewer cut off")

    async def test_cable_hub_still_skips_to_keyframe(self):
        from zenlink.usbdirect.hub import MediaHub

        hub = MediaHub()
        hub.set_codec("hvc1.1.6.L150.B0", bytes(23), 1920, 1080, "HEVC")
        v = hub.open_video(asyncio.get_running_loop())
        for f in self.frames(200):   # a viewer that never reads falls behind
            hub.push_video(f)
            await asyncio.sleep(0)   # frames are queued through the event loop
        self.assertTrue(v.need_key)


# ------------------------------------------------------------------ PC sound

class NativeAudioTests(unittest.TestCase):
    def test_jitter_buffer_target(self):
        from zenlink.audio_out import RATE, NativeAudio

        n = NativeAudio()
        n.set_jitter_ms(120)
        self.assertGreaterEqual(n._target() // 4, int(RATE * 0.12))
        n.set_jitter_ms(0)
        n.margin = 128
        self.assertLess(n._target() // 4, int(RATE * 0.02))

    def test_cable_margin_never_shrinks_into_underruns(self):
        # The margin used to shrink every 5 s down to 2.7 ms, so the sound
        # broke every so often by itself. Now it stays >= 10 ms.
        from zenlink import audio_out
        from zenlink.audio_out import MIN_MARGIN, NativeAudio

        n = NativeAudio()
        n.set_jitter_ms(0)
        self.assertGreaterEqual(n.margin, MIN_MARGIN)
        n._stream = object()  # feed() only queues while a stream is open
        n._started = True
        n._calm_since = audio_out.time.monotonic() - 3600
        n._buf = bytearray(4 * 48000)
        n._callback(bytearray(4 * 480), 480, None, None)
        self.assertGreaterEqual(n.margin, MIN_MARGIN)
        n._stream = None


    def test_large_output_blocks_raise_the_target(self):
        # MME / Bluetooth pull ~20+ ms per block: the buffer must hold more than one block.
        from zenlink.audio_out import BYTES_PER_FRAME, NativeAudio

        n = NativeAudio()
        n.set_jitter_ms(0)
        n.packet = 480
        n.block = 2048
        self.assertGreaterEqual(n._target() // BYTES_PER_FRAME, 480 + 2048)
        self.assertGreaterEqual(n._margin_cap(), 4096)


class DeviceOnlySoundTests(unittest.TestCase):
    """"Device only" must hand the sound back (HPA0): while the host asks for
    it (HPA1) iOS keeps its own speaker silent."""

    def _session(self):
        from zenlink.usbdirect import session as us
        from zenlink.usbdirect.hub import MediaHub

        s = us.UsbDirectSession("x", MediaHub(), lambda *_: None, audio_to_pc=True)
        s._ep_out, s._dev = object(), object()
        s._audio_clock = us.HostClock(1)
        s._device_audio_clock = 7
        s._audio_on = True
        sent = []
        s._write = lambda data, timeout=2000: sent.append(data)
        return s, sent

    def test_toggle_sends_hpa0_then_hpa1_once(self):
        from zenlink.usbdirect import wire

        s, sent = self._session()
        s.set_audio_to_pc(False)
        s.set_audio_to_pc(False)  # no repeat
        s.set_audio_to_pc(True)
        kinds = [wire.u32(p, 16) for p in sent]  # outgoing: length, asyn, clock, subtype
        self.assertEqual(kinds, [wire.HPA0, wire.HPA1])


class SkewMeterTests(unittest.TestCase):
    """The SKEW answer paces the device's audio, and video players sync to it."""

    def _feed(self, m, seconds, rate=1.0, jitter=0.0, start=(0.0, 0.0), seed=1):
        import random

        from zenlink.usbdirect.wire import CMTime

        rnd = random.Random(seed)
        dev0, loc0 = start
        t = 0.0
        while t < seconds:
            dev = dev0 + t
            loc = loc0 + t * rate + 0.02 + rnd.uniform(0, jitter)  # 20 ms base delay + jitter
            m.add(CMTime(int(dev * 48000), 48000), CMTime(int(loc * 1e9)))
            t += 0.02
        return dev0 + t, loc0 + t * rate

    def test_jittery_arrivals_give_nominal_rate(self):
        from zenlink.usbdirect.session import SkewMeter

        m = SkewMeter()
        self.assertEqual(m.rate(), 48000.0)  # too early: nominal
        self._feed(m, 30, jitter=0.03)
        self.assertAlmostEqual(m.rate(), 48000.0, delta=48000 * 0.0005)

    def test_measures_real_drift(self):
        from zenlink.usbdirect.session import SkewMeter

        m = SkewMeter()
        self._feed(m, 60, rate=1.0005, jitter=0.01)  # host clock 0.05 % faster
        self.assertAlmostEqual(m.rate(), 48024.0, delta=6)

    def test_clock_jump_starts_over_instead_of_giving_nonsense(self):
        # Seen on a device: 569543 Hz after the picture changed size.
        from zenlink.usbdirect.session import SkewMeter

        m = SkewMeter()
        self._feed(m, 20)
        self._feed(m, 20, start=(3.0, 25.0))  # device audio timeline restarted
        r = m.rate()
        self.assertGreaterEqual(r, 48000 * (1 - SkewMeter.LIMIT))
        self.assertLessEqual(r, 48000 * (1 + SkewMeter.LIMIT))
        self.assertAlmostEqual(r, 48000.0, delta=48000 * 0.0005)


class DriverScanTests(unittest.TestCase):
    def test_package_scan_is_cached_until_the_inf_folder_changes(self):
        from zenlink.drivers import status

        status._PACKAGES_CACHE.clear()
        first = status.libusb0_packages()
        self.assertIn("key", status._PACKAGES_CACHE)
        status._PACKAGES_CACHE["found"] = {"FAKE": "oem0.inf"}
        self.assertEqual(status.libusb0_packages(), {"FAKE": "oem0.inf"})  # served from the cache
        status._PACKAGES_CACHE["key"] = None  # folder "changed": read again
        self.assertEqual(status.libusb0_packages(), first)

    def test_concurrent_start_opens_one_stream(self):
        from zenlink.audio_out import NativeAudio

        n = NativeAudio()
        if not n.available:
            self.skipTest("no sound device")
        opened = []
        real = n._start_locked

        def counting():
            before = n._stream
            ok = real()
            if ok and before is None and n._stream is not None:
                opened.append(1)
            return ok

        n._start_locked = counting
        threads = [threading.Thread(target=n.start) for _ in range(8)]
        for t in threads:
            t.start()
        for t in threads:
            t.join()
        n.stop()
        self.assertLessEqual(len(opened), 1)


# ------------------------------------------------------------------ API

class ApiTests(unittest.IsolatedAsyncioTestCase):
    async def test_routes(self):
        from aiohttp.test_utils import TestClient, TestServer

        from zenlink.castmanager import CastManager
        from zenlink.connection import ConnectionModeManager
        from zenlink.server import build_app

        with mock.patch("zenlink.castmanager.threading.Thread"):
            m = CastManager()
        m.connection = ConnectionModeManager(tmp_settings())
        async with TestClient(TestServer(build_app(m))) as c:
            self.assertEqual((await c.get("/api/state")).status, 200)
            r = await (await c.post("/api/connection", json={"link": "wireless"})).json()
            self.assertEqual(r["connection"]["link"], "wireless")
            r = await c.post("/api/connection", json={"link": "nope"})
            self.assertEqual(r.status, 400)
            r = await (await c.get("/api/firewall")).json()
            self.assertIn("ok", r)
            self.assertEqual((await c.get("/")).status, 200)


class DriverSwitchTests(unittest.TestCase):
    SCRIPT = Path(__file__).resolve().parents[4] / "ZenshiroDev" / "tooling" / "windows" / "tools" / "usb_filter.ps1"

    def code_lines(self):
        return [l for l in self.SCRIPT.read_text(encoding="utf-8-sig").splitlines() if not l.strip().startswith("#")]

    def test_delete_key_is_never_piped(self):
        # v2.2 bug: "A parameter cannot be found that matches parameter name
        # 'DeleteKey'" - the dynamic parameter only binds with a direct cert path.
        for line in self.code_lines():
            if "-DeleteKey" in line:
                self.assertNotIn("|", line.split("-DeleteKey")[0], line)
                self.assertIn("-Path", line, line)

    def test_no_translated_pnputil_text_is_parsed(self):
        text = "\n".join(self.code_lines())
        self.assertNotIn("Published Name", text)
        self.assertNotIn("Provider Name", text)
        status_py = (Path(__file__).resolve().parents[1] / "zenlink" / "drivers" / "status.py").read_text(encoding="utf-8")
        self.assertNotIn("/enum-devices", status_py)
        self.assertNotIn("/enum-drivers", status_py)

    def test_libusb_packages_read_from_inf_files(self):
        import os
        import tempfile
        from unittest import mock

        from zenlink.drivers import status

        with tempfile.TemporaryDirectory() as root:
            inf = os.path.join(root, "INF")
            os.mkdir(inf)
            # libwdi writes UTF-16; a model line commented out must not count.
            Path(inf, "oem10.inf").write_text(
                '[Strings]\nDeviceID = "VID_05AC&PID_12A8"\n[x]\nAddService = libusb0, 2, s\n', encoding="utf-16")
            Path(inf, "oem11.inf").write_text(
                ";DeviceID = VID_05AC&PID_12AB\nAddService = libusb0, 2, s\n", encoding="utf-8")
            Path(inf, "oem12.inf").write_text(  # Apple's own package: not libusb
                "%d%=AppleUSB_CCGPDriverInstall, USB\VID_05ac&PID_12ab\n", encoding="utf-8")
            Path(inf, "oem13.inf").write_text(  # an interface (&MI_) package is not the parent
                "USB\VID_05AC&PID_12AB&MI_00\nAddService = libusb0, 2, s\n", encoding="utf-8")
            with mock.patch.dict(os.environ, {"WINDIR": root}):
                self.assertEqual(status.libusb0_packages(), {"12A8": "oem10.inf"})

    def test_device_presence_does_not_need_pnputil(self):
        from unittest import mock

        from zenlink.drivers import status

        with mock.patch("subprocess.run", side_effect=AssertionError("pnputil must not be used")):
            status._connected_instances()


if __name__ == "__main__":
    unittest.main()

