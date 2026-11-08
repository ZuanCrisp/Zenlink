"""Files to an iPhone/iPad (v2.5.1, zenlink/apple/files.py). No device needed."""

from __future__ import annotations

import asyncio
import tempfile
import unittest
from pathlib import Path
from unittest import mock


class NameTests(unittest.TestCase):
    def test_safe_names(self):
        from zenlink.apple.files import safe_name

        self.assertEqual(safe_name("Foto liburan.jpg"), "Foto liburan.jpg")
        self.assertEqual(safe_name("a:b*c?.txt"), "a_b_c_.txt")
        self.assertEqual(safe_name("../x"), ".._x")
        self.assertIsNone(safe_name(".."))
        self.assertIsNone(safe_name("   "))
        long = safe_name("é" * 150 + ".mp4")
        self.assertTrue(len(long.encode()) <= 200 and long.endswith(".mp4"))

    def test_numbered(self):
        from zenlink.apple.files import numbered

        self.assertEqual(numbered("a.jpg", 2), "a (2).jpg")
        self.assertEqual(numbered("notes", 1), "notes (1)")
        self.assertEqual(numbered(".bashrc", 1), ".bashrc (1)")

    def test_only_file_sharing_apps(self):
        from zenlink.apple.files import sharing_apps

        apps = {
            "org.videolan.vlc-ios": {"UIFileSharingEnabled": True, "CFBundleDisplayName": "VLC"},
            "com.example.plain": {"CFBundleDisplayName": "Plain"},
            "com.readdle.Documents": {"UIFileSharingEnabled": True, "CFBundleName": "Documents"},
        }
        self.assertEqual(sharing_apps(apps), [
            {"bundle": "com.readdle.Documents", "name": "Documents"},
            {"bundle": "org.videolan.vlc-ios", "name": "VLC"},
        ])


class SendTests(unittest.IsolatedAsyncioTestCase):
    async def test_streams_into_documents_under_a_free_name(self):
        from zenlink.apple import files

        written = bytearray()

        class Afc:
            async def exists(self, path):
                return path == "/Documents/clip.mp4"

            async def fopen(self, path, mode):
                self.path = path
                return 7

            async def fwrite(self, handle, data):
                written.extend(data)

            async def fclose(self, handle):
                pass

            async def close(self):
                pass

        afc = Afc()

        class Lockdown:
            async def close(self):
                pass

        local = Path(tempfile.mkdtemp()) / "clip.mp4"
        local.write_bytes(b"x" * (files.CHUNK + 5))
        with mock.patch.object(files, "_lockdown", mock.AsyncMock(return_value=Lockdown())), \
                mock.patch("pymobiledevice3.services.house_arrest.HouseArrestService.create",
                           mock.AsyncMock(return_value=afc)):
            name = await files.send("udid", "org.videolan.vlc-ios", "VLC", local)
        self.assertEqual(name, "clip (1).mp4")
        self.assertEqual(afc.path, "/Documents/clip (1).mp4")
        self.assertEqual(len(written), files.CHUNK + 5)
        self.assertEqual(files.job["state"], "done")
        self.assertEqual(files.job["sent"], files.CHUNK + 5)


class RouteTests(unittest.IsolatedAsyncioTestCase):
    async def test_needs_apple_driver_mode_and_json(self):
        from aiohttp.test_utils import TestClient, TestServer

        from zenlink.castmanager import CastManager
        from zenlink.connection import ConnectionModeManager
        from zenlink.apple import files
        from zenlink.server import build_app

        files.job.update(state="idle")
        with mock.patch("zenlink.castmanager.threading.Thread"):
            m = CastManager()
        m.connection = ConnectionModeManager(Path(tempfile.mkdtemp()) / "settings.json")
        async with TestClient(TestServer(build_app(m))) as c:
            r = await c.get("/api/files/apps")
            self.assertEqual(r.status, 409)
            r = await c.post("/api/files/send", data='{"path": "C:/x", "bundle": "a.b"}')
            self.assertEqual(r.status, 415)  # text/plain from a web page is refused
            r = await c.post("/api/files/send", json={"path": "C:/x", "bundle": "a.b"})
            self.assertEqual(r.status, 409)
            self.assertEqual((await (await c.get("/api/files/job")).json())["state"], "idle")


if __name__ == "__main__":
    unittest.main()
