"""ZenLink audit tests: modes, links, AirPlay protocol, audio, API.

Run:  .venv\\Scripts\\python -m unittest discover -s tests -v
No device is needed: the iPhone/iPad side is simulated.
"""

from __future__ import annotations

import asyncio
import hashlib
import plistlib
import struct
import tempfile
import threading
import time
import unittest
from pathlib import Path
from unittest import mock

from Crypto.Cipher import AES
from cryptography.hazmat.primitives import serialization as ser
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey, Ed25519PublicKey
from cryptography.hazmat.primitives.asymmetric.x25519 import X25519PrivateKey, X25519PublicKey


def raw(key) -> bytes:
    return key.public_key().public_bytes(ser.Encoding.Raw, ser.PublicFormat.Raw)


def sha16(*parts: bytes) -> bytes:
    h = hashlib.sha512()
    for p in parts:
        h.update(p)
    return h.digest()[:16]


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
            mock.patch.object(devices, "wifi_ready", side_effect=lambda u: u == "A"),
            mock.patch("zenlink.apple.lan.scan_async"),
            mock.patch("zenlink.apple.lan.devices", return_value=[]),
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
        self.assertEqual(got["B"].get("need"), "wifi")
        self.assertFalse(got["B"]["ready"])

    def test_lan_found_device_without_setup_needs_wifi(self):
        lan_dev = [{"udid": "C", "ip": "192.168.1.9", "name": "iPhone", "productType": "iPhone18,3",
                    "version": "27.2", "deviceClass": "iPhone", "paired": True}]
        with mock.patch("zenlink.apple.lan.devices", return_value=lan_dev):
            got = {d["id"]: d for d in self.devices.connected_devices("wireless")["devices"]}
        self.assertEqual(got["C"].get("need"), "wifi")  # no RemotePairing record for C


# ------------------------------------------------------------------ cast manager

class AppleSoundFeedTests(unittest.TestCase):
    def test_simultaneous_starts_make_one_feed(self):
        """Window + session asking for sound at once used to start two Apple
        sound feeds: the same sound fed twice (overflow, crackling)."""
        from zenlink.castmanager import CastManager

        with mock.patch("zenlink.castmanager.threading.Thread"):
            m = CastManager()
        m._session, m._backend, m.audio_enabled = object(), "apple", True
        pumps = []

        class FakePump:
            def __init__(self, *a):
                pumps.append(self)
                time.sleep(0.05)  # widen the race window

            def start(self):
                pass

            def stop(self):
                pass

        with mock.patch("zenlink.castmanager.AppleAudioPump", FakePump),              mock.patch.object(m.audio, "start", return_value=True),              mock.patch.object(type(m.audio), "running", new_callable=mock.PropertyMock, return_value=True):
            threads = [threading.Thread(target=m._start_sound) for _ in range(8)]
            for t in threads:
                t.start()
            for t in threads:
                t.join()
        self.assertEqual(len(pumps), 1)


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

    def test_auto_airplay_and_pause(self):
        m = self.make("usbdirect", "wireless")
        m.refresh_devices()
        with mock.patch.object(m, "start") as start:
            m._auto_airplay()
            start.assert_called_once()
        m._airplay_paused = True
        with mock.patch.object(m, "start") as start:
            m._auto_airplay()
            start.assert_not_called()
        m.state["phase"] = "error"
        m._airplay_paused = False
        with mock.patch.object(m, "start") as start:
            m._auto_airplay()
            start.assert_not_called()  # no restart loop after an error

    def test_no_auto_airplay_in_other_modes(self):
        for mode, link in (("apple", "wired"), ("apple", "wireless"), ("usbdirect", "wired")):
            m = self.make(mode, link)
            with mock.patch.object(m, "start") as start:
                m._auto_airplay()
                start.assert_not_called()

    def test_airplay_session_lifecycle(self):
        m = self.make("usbdirect", "wireless")
        m.refresh_devices()
        with mock.patch("zenlink.airplay.receiver.AIRPLAY_PORT", 0), \
             mock.patch("zenlink.airplay.receiver.AirPlayReceiver._advertise"), \
             mock.patch("zenlink.airplay.receiver.AirPlayReceiver._unadvertise"):
            from zenlink.airplay import receiver

            receiver.AIRPLAY_PORT = 0
            m.start({})
            deadline = time.time() + 5
            while m.state["phase"] != "activating" and time.time() < deadline:
                time.sleep(0.05)
            self.assertEqual(m.state["phase"], "activating")
            self.assertIn("Screen Mirroring", m.state["detail"])
            m.stop()
            self.assertIsNone(m._session)
            self.assertTrue(m._airplay_paused)
            receiver.AIRPLAY_PORT = 7000

    def test_wifi_needed_message(self):
        m = self.make("apple", "wireless")
        m._presence = {"devices": [{"id": "X", "name": "iPad", "need": "wifi", "ready": False, "wifi": False}]}
        with mock.patch.object(m, "refresh_devices"):
            m.start({})
        self.assertEqual(m.state["phase"], "error")
        self.assertIn("Wi-Fi", m.state["error"])


# ------------------------------------------------------------------ AirPlay protocol (simulated iPad)

class FakeHub:
    def __init__(self):
        self.codecs, self.frames, self.audio = [], [], []

    def set_codec(self, codec, config, w, h, name):
        self.codecs.append((codec, config, w, h, name))

    def push_video(self, data):
        self.frames.append(data)

    def push_audio(self, pcm):
        self.audio.append(pcm)

    def codec(self):
        return self.codecs[-1] if self.codecs else None


class AirPlayProtocolTests(unittest.IsolatedAsyncioTestCase):
    async def asyncSetUp(self):
        from zenlink.airplay import receiver

        self.receiver_mod = receiver
        self.events = []
        self.hub = FakeHub()
        self.patches = [mock.patch.object(receiver, "AIRPLAY_PORT", 0),
                        mock.patch.object(receiver.AirPlayReceiver, "_advertise"),
                        mock.patch.object(receiver.AirPlayReceiver, "_unadvertise")]
        for p in self.patches:
            p.start()
        self.rx = receiver.AirPlayReceiver("ZenLinkStudio", self.hub, lambda e, d: self.events.append((e, d)))
        server = await asyncio.start_server(self.rx._handle_rtsp, "127.0.0.1", 0)
        self.rx._servers.append(server)
        self.port = server.sockets[0].getsockname()[1]
        self.reader, self.writer = await asyncio.open_connection("127.0.0.1", self.port)
        self.cseq = 0

    async def asyncTearDown(self):
        self.writer.close()
        await self.rx.stop()
        for p in self.patches:
            p.stop()

    async def rtsp(self, method, url, body=b"", ctype=None):
        self.cseq += 1
        head = f"{method} {url} RTSP/1.0\r\nCSeq: {self.cseq}\r\nContent-Length: {len(body)}\r\n"
        if ctype:
            head += f"Content-Type: {ctype}\r\n"
        self.writer.write(head.encode() + b"\r\n" + body)
        await self.writer.drain()
        resp = await self.reader.readuntil(b"\r\n\r\n")
        lines = resp.decode().split("\r\n")
        length = int(next(l for l in lines if l.lower().startswith("content-length")).split(":")[1])
        data = await self.reader.readexactly(length)
        return lines[0], data

    async def handshake(self):
        x = X25519PrivateKey.generate()
        ed = Ed25519PrivateKey.generate()
        status, data = await self.rtsp("POST", "/pair-verify", b"\x01\0\0\0" + raw(x) + raw(ed))
        self.assertIn("200", status)
        server_x = data[:32]
        secret = x.exchange(X25519PublicKey.from_public_bytes(server_x))
        ctr = AES.new(sha16(b"Pair-Verify-AES-Key", secret), AES.MODE_CTR, nonce=b"",
                      initial_value=sha16(b"Pair-Verify-AES-IV", secret))
        sig = ctr.encrypt(data[32:])
        Ed25519PublicKey.from_public_bytes(self.rx._pk).verify(sig, server_x + raw(x))
        status, _ = await self.rtsp("POST", "/pair-verify", b"\0\0\0\0" + ctr.encrypt(ed.sign(raw(x) + server_x)))
        self.assertIn("200", status)
        status, data = await self.rtsp("POST", "/fp-setup", bytes([0x46, 0x50, 0x4C, 0x59, 3, 1, 1, 0, 0, 0, 0, 4, 2, 0, 2, 0xBB]))
        self.assertEqual(len(data), 142)
        msg3 = bytes([0x46, 0x50, 0x4C, 0x59, 3]) + bytes(159)
        status, data = await self.rtsp("POST", "/fp-setup", msg3)
        self.assertEqual(data[-20:], msg3[144:164])
        return secret, msg3

    async def test_info(self):
        status, data = await self.rtsp("GET", "/info")
        info = plistlib.loads(data)
        self.assertIn("200", status)
        self.assertEqual(info["name"], "ZenLinkStudio")
        self.assertEqual((info["displays"][0]["width"], info["displays"][0]["height"]), (3840, 2160))
        self.assertTrue(info["features"] >> 42 & 1, "HEVC passthrough (SupportsScreenMultiCodec) is advertised")
        status, data = await self.rtsp("GET", "/info", plistlib.dumps({"qualifier": ["txtAirPlay"]}, fmt=plistlib.FMT_BINARY),
                                       "application/x-apple-binary-plist")
        self.assertIn(b"deviceid=", plistlib.loads(data)["txtAirPlay"])

    async def test_full_mirror_session(self):
        from zenlink.airplay.fairplay import FairPlay

        secret, msg3 = await self.handshake()
        ekey, eiv = bytes(range(72)), bytes(range(16))
        fp = FairPlay()
        fp.keymsg = msg3
        aeskey = hashlib.sha512(fp.decrypt(ekey) + secret).digest()[:16]

        setup = {"ekey": ekey, "eiv": eiv, "name": "iPad Test", "model": "iPad15,7", "deviceID": "AA", "timingPort": 0}
        status, data = await self.rtsp("SETUP", "rtsp://x/1", plistlib.dumps(setup, fmt=plistlib.FMT_BINARY))
        self.assertIn("timingPort", plistlib.loads(data))
        self.assertIn(("connected", "iPad Test"), self.events)

        stream_id = 0xF1E2D3C4B5A69788  # >= 2**63: sent as an 8-byte (signed on the wire) integer, like iOS
        status, data = await self.rtsp("SETUP", "rtsp://x/1", plistlib.dumps(
            {"streams": [{"type": 110, "streamConnectionID": stream_id - (1 << 64)}]}, fmt=plistlib.FMT_BINARY))
        port = plistlib.loads(data)["streams"][0]["dataPort"]

        # avcC config packet (type 1) then two encrypted frames (type 0)
        _r, w = await asyncio.open_connection("127.0.0.1", port)
        avcc = bytes.fromhex("0164002affe1000a6764002aac2b40780227e501000468ee3cb0")

        def header(size, kind, width=1554.0, height=1080.0):
            h = bytearray(128)
            struct.pack_into("<I", h, 0, size)
            h[4] = kind
            struct.pack_into("<f", h, 56, width)
            struct.pack_into("<f", h, 60, height)
            return bytes(h)

        w.write(header(len(avcc), 1) + avcc)
        cipher = AES.new(sha16(f"AirPlayStreamKey{stream_id}".encode(), aeskey), AES.MODE_CTR, nonce=b"",
                         initial_value=sha16(f"AirPlayStreamIV{stream_id}".encode(), aeskey))
        frames = [(5).to_bytes(4, "big") + bytes([0x65, 1, 2, 3, 4]),          # IDR, 9 bytes (odd size)
                  (20).to_bytes(4, "big") + bytes([0x41]) + bytes(range(19))]   # P frame
        for f in frames:
            w.write(header(len(f), 0) + cipher.encrypt(f))
        await w.drain()
        for _ in range(100):
            if len(self.hub.frames) == 2:
                break
            await asyncio.sleep(0.02)
        self.assertEqual(self.hub.codecs[0][4], "H.264")
        self.assertEqual(self.hub.codecs[0][0], "avc1.64002a")
        from zenlink.airplay.receiver import _codec_string
        # an avcC that looks like an hvcC to a byte-pattern guess stays H.264
        tricky = bytes([1, 0x42, 0xC0, 0x33]) + bytes(17) + bytes([0x03, 0x01]) + bytes(8)
        self.assertTrue(_codec_string(tricky, "H.264").startswith("avc1.42c033"))
        self.assertEqual(self.hub.frames, frames, "decrypted frames must match across packet boundaries")
        self.assertIn(("streaming", ""), self.events)
        w.close()

        # audio stream setup returns ports and does not crash
        status, data = await self.rtsp("SETUP", "rtsp://x/1", plistlib.dumps(
            {"streams": [{"type": 96, "ct": 8, "spf": 480, "audioFormat": 0x1000000, "controlPort": 0}]},
            fmt=plistlib.FMT_BINARY))
        stream = plistlib.loads(data)["streams"][0]
        self.assertEqual(stream["type"], 96)
        self.assertTrue(stream["dataPort"] and stream["controlPort"])

        status, _ = await self.rtsp("TEARDOWN", "rtsp://x/1", plistlib.dumps({}, fmt=plistlib.FMT_BINARY))
        self.assertIn("200", status)


    async def test_new_device_cuts_off_the_old_one(self):
        secret, msg3 = await self.handshake()
        setup = {"ekey": bytes(72), "eiv": bytes(16), "name": "A", "timingPort": 0}
        await self.rtsp("SETUP", "rtsp://x/1", plistlib.dumps(setup, fmt=plistlib.FMT_BINARY))
        _s, data = await self.rtsp("SETUP", "rtsp://x/1", plistlib.dumps(
            {"streams": [{"type": 110, "streamConnectionID": 1}]}, fmt=plistlib.FMT_BINARY))
        port = plistlib.loads(data)["streams"][0]["dataPort"]
        vr, vw = await asyncio.open_connection("127.0.0.1", port)
        await asyncio.sleep(0.05)
        self.assertEqual(len(self.rx._video_writers), 1)
        # a second device (another RTSP connection) takes over
        r2, w2 = await asyncio.open_connection("127.0.0.1", self.port)
        old = (self.reader, self.writer)
        self.reader, self.writer = r2, w2
        self.rx.client = "someone-else"  # make the takeover visible to the receiver
        self.rx._rtsp_writers["someone-else"] = old[1]
        await self.handshake()
        await self.rtsp("SETUP", "rtsp://x/2", plistlib.dumps({**setup, "name": "B"}, fmt=plistlib.FMT_BINARY))
        await asyncio.sleep(0.05)
        self.assertEqual(len(self.rx._video_writers), 0, "old device's video connection must be closed")
        self.assertEqual(await asyncio.wait_for(vr.read(1), 1), b"", "old video socket gets EOF")
        old[1].close()
        vw.close()


class AirPlayPartsTests(unittest.TestCase):
    def test_hevc_config_from_sample_entry(self):
        from zenlink.airplay.receiver import _hvcc_from_sample_entry
        from zenlink.usbdirect.wire import FormatDescription

        cfg = bytes([1, 1, 0x60, 0, 0, 0, 0x90, 0, 0, 0, 0, 0, 153, 0xF0, 0, 0xFC, 0xFD, 0xF8, 0xF8, 0, 0, 0x0F, 3]) + bytes(10)
        entry = b"\0\0\0\0hvc1" + bytes(70) + (8 + len(cfg)).to_bytes(4, "big") + b"hvcC" + cfg
        self.assertEqual(_hvcc_from_sample_entry(entry), cfg)
        self.assertTrue(FormatDescription(media_type=0, config=cfg).codec_string.startswith("hvc1.1.6.L153"))
        self.assertIsNone(_hvcc_from_sample_entry(b"\0\0\0\0hvc1" + bytes(80)))

    def test_audio_decoder_survives_garbage(self):
        import os

        from zenlink.airplay.receiver import _AudioDecoder

        d = _AudioDecoder(bytes(16), bytes(16), 8)
        for i in range(100):
            d.decode(bytes([0x80, 0x60]) + struct.pack(">H", i) + bytes(8) + os.urandom(160))
        self.assertGreater(d.failed, 0)

    def test_video_decryptor_resyncs_and_never_passes_garbage(self):
        from zenlink.airplay.receiver import _VideoDecryptor, _valid_au

        key, iv = bytes(range(16)), bytes(range(16, 32))
        enc = lambda: AES.new(key, AES.MODE_CTR, nonce=b"", initial_value=iv)
        frames = [(3).to_bytes(4, "big") + bytes([0x65, 1, 2]), (5).to_bytes(4, "big") + bytes([0x41, 9, 9, 9, 9])]
        self.assertTrue(all(_valid_au(f) for f in frames))
        d = _VideoDecryptor(key, iv)
        sender = enc()
        self.assertEqual([d.decrypt(sender.encrypt(f)) for f in frames], frames)
        # the device restarts its keystream (new connection, same key)
        sender = enc()
        self.assertEqual([d.decrypt(sender.encrypt(f)) for f in frames], frames)
        # random bytes are skipped, not passed to the decoder
        import os
        self.assertIsNone(d.decrypt(os.urandom(64)))

    def test_audio_reorder(self):
        from zenlink.airplay.receiver import _Reorder

        pk = lambda n: bytes([0x80, 0x60]) + struct.pack(">H", n) + bytes(8)
        seqs = lambda pkts: [struct.unpack_from(">H", x, 2)[0] for x in pkts]
        r, out = _Reorder(depth=3), []
        for n in (10, 12, 11, 13, 15, 16, 17, 18, 19, 14):
            out += seqs(r.push(pk(n)))
        self.assertEqual(out, [10, 11, 12, 13, 15, 16, 17, 18, 19])  # 14 was lost, then late
        r, out = _Reorder(), []
        for n in (65535, 0, 1):
            out += seqs(r.push(pk(n)))
        self.assertEqual(out, [65535, 0, 1])  # sequence wrap

    def test_playfair_dll(self):
        from zenlink.airplay.fairplay import FairPlay

        fp = FairPlay()
        fp.handshake(bytes([0x46, 0x50, 0x4C, 0x59, 3]) + bytes(159))
        self.assertEqual(len(fp.decrypt(bytes(72))), 16)


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


if __name__ == "__main__":
    unittest.main()
