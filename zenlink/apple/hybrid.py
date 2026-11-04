"""Hybrid Control for iOS 17-26 (EXPERIMENTAL).

Apple's screen service (dtremotedisplayd) only sends frames on iOS 27+, but
on older iOS it still accepts the stream request, and the CoreDevice HID
services (universalhidservice, indigo) deliver touch, keyboard and buttons
while such a stream is open. Without an open stream iOS drops every touch
("authentication gate", documented in pymobiledevice3's hid_service).

So one session runs two independent paths at once:

* picture + sound: ZenLink's AirPlay receiver (the device mirrors to it over
  Wi-Fi), delivered through the same MediaHub / player as ZenLink Studio;
* control: a CoreDevice tunnel over the Apple driver (cable, or Wi-Fi when set
  up), a stream opened only to hold the HID gate (its RTP is discarded), and
  the HID services behind a loopback server with the same /touch, /key,
  /button API as Apple mode's stream server, so the UI, shortcuts, wake and
  lock work unchanged.

The two never share a USB driver: AirPlay uses the network and the control
path uses Apple's own driver. libusb0 is not involved.

If the control path can't be opened (Developer Mode off, no developer image,
services missing, tunnel refused) the picture keeps running and the session
reports "Mirroring only" with the reason. Nothing here is claimed to work on
a given iOS version until it has been confirmed on a real device.
"""

from __future__ import annotations

import asyncio
import contextlib
import contextvars
import json
import logging
import threading
from typing import Callable

from zenlink import capability

log = logging.getLogger(__name__)

# Lets Hybrid open the HID-gate stream on iOS < 27 without changing what the
# library does for anyone else (Apple mode keeps its iOS 27 check).
_GATE_STREAM = contextvars.ContextVar("zenlink_hid_gate_stream", default=False)
_gate_installed = False


def _install_gate_bypass() -> None:
    global _gate_installed
    if _gate_installed:
        return
    from pymobiledevice3.remote.core_device import display_service

    original = getattr(display_service, "require_video_stream_support", None)
    if original is None:  # older pymobiledevice3: no version gate to step around
        _gate_installed = True
        return

    def require(rsd):
        if _GATE_STREAM.get():
            return  # stream opened only for the HID gate; no frames expected
        original(rsd)

    display_service.require_video_stream_support = require
    _gate_installed = True


_ORIENTATION_TO_ROTATION = {1: 0, 2: 180, 3: 270, 4: 90}
INPUT_PATHS = ("touch", "key", "button")


class HybridSession:
    """Same contract as the other backends: run() blocks, stop() ends it."""

    def __init__(self, udid: str, hub, on_state: Callable[[str, str], None], options: dict,
                 on_control: Callable[[dict], None] | None = None) -> None:
        from zenlink.airplay.receiver import receiver_name

        self.udid = udid
        self.hub = hub
        self.on_state = on_state
        self.on_control = on_control or (lambda _c: None)
        self.options = options
        self.wifi = bool(options.get("wifi"))
        self.name = receiver_name()
        self.receiver = None
        self.control = capability.verified("hybrid", video=False, control=False,
                                           note="Opening touch control over the Apple driver…")
        self._loop: asyncio.AbstractEventLoop | None = None
        self._stop: asyncio.Event | None = None
        self._ready = threading.Event()
        self._rotation: int | None = 0
        self._uhs = None
        self._indigo = None
        self._kb_id = None
        self._queue: asyncio.Queue | None = None
        self._video = False

    # ------------------------------------------------------------ contract

    def run(self) -> None:
        asyncio.run(self._main())

    def stop(self) -> None:
        if self._ready.wait(5) and self._loop is not None and self._stop is not None:
            self._loop.call_soon_threadsafe(self._stop.set)

    def stats(self) -> dict:
        return dict(self.receiver.stats) if self.receiver else {}

    # ------------------------------------------------------------ session

    def _report(self, level: str, control: bool, note: str) -> None:
        self.control = capability.verified(level, video=self._video, control=control, note=note)
        self.on_control(self.control)

    def _airplay_event(self, kind: str, detail: str) -> None:
        from zenlink.airplay.session import HOW_TO

        if kind == "waiting":
            self.on_state("activating", HOW_TO.format(name=self.name))
        elif kind == "connected":
            self.on_state("activating", f"{detail} is connecting over AirPlay…")
        elif kind == "streaming":
            self._video = True
            self.on_control({**self.control, "video": True})
            self.on_state("streaming", "")

    async def _main(self) -> None:
        from zenlink.airplay.receiver import AirPlayReceiver
        from zenlink.server import quiet_resets

        self._loop = asyncio.get_running_loop()
        quiet_resets(self._loop)
        self._stop = asyncio.Event()
        self._queue = asyncio.Queue()
        self._ready.set()
        self.on_state("connecting", "Starting the AirPlay receiver and the touch link")
        self.receiver = AirPlayReceiver(self.name, self.hub, self._airplay_event, hevc=False)
        try:
            await self.receiver.start()
        except OSError as error:
            raise RuntimeError(
                f"ZenLink couldn't open the AirPlay port ({error}). Close other AirPlay receivers "
                "(TikTok LIVE Studio, UxPlay, LonelyScreen) and try again."
            ) from error
        runner = await self._start_input_server()
        control = asyncio.create_task(self._control())
        worker = asyncio.create_task(self._input_worker())
        try:
            await self._stop.wait()
        finally:
            for task in (control, worker):
                task.cancel()
                with contextlib.suppress(BaseException):
                    await task
            with contextlib.suppress(Exception):
                await runner.cleanup()
            await self.receiver.stop()
            from .session import _reset_userspace_tunnel

            await _reset_userspace_tunnel()

    # ------------------------------------------------------------ control path

    async def _control(self) -> None:
        """Open the CoreDevice tunnel, check what the device offers, hold the
        HID gate open and keep the orientation current. Failures leave the
        AirPlay picture running and say why control is off."""
        from .session import _reset_userspace_tunnel

        try:
            await _reset_userspace_tunnel()
            await self._control_once()
        except asyncio.CancelledError:
            raise
        except Exception as error:  # noqa: BLE001 - shown to the user, picture keeps running
            log.warning("hybrid: control path failed: %s", error, exc_info=True)
            self._report("mirroring", False, _control_error(error))

    async def _control_once(self) -> None:
        from pymobiledevice3.remote.rsd_tunnel import PreferredRsdTunnel
        from pymobiledevice3.services.mobile_image_mounter import auto_mount

        for attempt in range(2):
            if self.wifi:
                from .wifi import WifiRsdTunnel

                tunnel = WifiRsdTunnel(self.udid)
            else:
                tunnel = PreferredRsdTunnel(serial=self.udid, autopair=True)
            async with tunnel as rsd:
                missing = capability.missing_hybrid_services((rsd.peer_info or {}).get("Services", {}))
                if missing and not attempt:
                    # The HID and display services live on the developer disk image.
                    await auto_mount(rsd)
                    continue
                if missing:
                    self._report("mirroring", False,
                                 "This device doesn't offer touch control over the Apple driver "
                                 f"(missing: {', '.join(m.rsplit('.', 1)[-1] for m in missing)}). "
                                 "Turn on Developer Mode (Settings > Privacy & Security), restart the device "
                                 "and try again. The picture keeps working over AirPlay.")
                    await asyncio.Event().wait()  # keep the tunnel until the session ends
                await self._hold_gate(rsd)
                return

    async def _hold_gate(self, rsd) -> None:
        from pymobiledevice3.remote.core_device.display_service import DisplayService
        from pymobiledevice3.remote.core_device.hid_service import (
            TOUCH_AUTH_SETTLE_SECONDS,
            IndigoHIDService,
            UniversalHIDServiceService,
        )
        from pymobiledevice3.remote.core_device.screen_stream import open_media_receiver

        from . import keyboard

        keyboard.install_buttons()
        _install_gate_bypass()
        display = DisplayService(rsd)
        await display.__aenter__()
        transport = None
        drain = orientation = None
        try:
            transport, receiver_ip = open_media_receiver(display, (1 * 1024 * 1024,))
            token = _GATE_STREAM.set(True)
            try:
                await asyncio.wait_for(display.start_video_stream(
                    receiver_ip=receiver_ip, receiver_port=transport.port,
                    sender_ip=rsd.service.address[0]), timeout=12.0)
            except asyncio.TimeoutError as error:
                raise RuntimeError("The device's media service didn't answer. Restart the device and try again.") from error
            finally:
                _GATE_STREAM.reset(token)
            await asyncio.sleep(TOUCH_AUTH_SETTLE_SECONDS)
            drain = asyncio.create_task(_drain(transport))
            self._uhs = UniversalHIDServiceService(rsd)
            await self._uhs.connect()
            self._indigo = IndigoHIDService(rsd)
            await self._indigo.connect()
            orientation = asyncio.create_task(self._watch_orientation(rsd))
            self._report("hybrid", True, "Touch, keyboard and buttons go over the Apple driver. Experimental: "
                                         "not yet confirmed on every iOS version.")
            await asyncio.Event().wait()
        finally:
            for task in (orientation, drain):
                if task is not None:
                    task.cancel()
                    with contextlib.suppress(BaseException):
                        await task
            for svc in (self._uhs, self._indigo):
                if svc is not None:
                    with contextlib.suppress(Exception):
                        await svc.close()
            self._uhs = self._indigo = None
            self._kb_id = None
            with contextlib.suppress(Exception):
                await display.stop_media_stream(stop_all=True)
            if transport is not None:
                with contextlib.suppress(Exception):
                    transport.close()
            with contextlib.suppress(BaseException):
                await display.__aexit__(None, None, None)

    async def _watch_orientation(self, rsd) -> None:
        """Read-only SpringBoard orientation, so touches land right when the
        device is turned (AirPlay already shows the picture upright)."""
        from pymobiledevice3.services.springboard import SpringBoardServicesService

        sb = None
        while True:
            try:
                if sb is None:
                    sb = SpringBoardServicesService(rsd)
                value = int(await sb.get_interface_orientation())
                self._rotation = _ORIENTATION_TO_ROTATION.get(value, self._rotation)
                await asyncio.sleep(0.5)
            except asyncio.CancelledError:
                raise
            except Exception as error:  # noqa: BLE001
                log.debug("hybrid orientation poll: %s", error)
                sb = None
                await asyncio.sleep(2.0)

    # ------------------------------------------------------------ input API

    async def _start_input_server(self):
        from aiohttp import web

        from .session import STREAM_HOST, STREAM_PORT

        async def handle(request: web.Request) -> web.Response:
            if self._uhs is None or self._indigo is None:
                return web.json_response({"error": self.control.get("note") or "Touch control isn't ready yet."},
                                         status=409)
            self._queue.put_nowait((request.match_info["path"], await request.read()))
            return web.Response(text="ok")

        app = web.Application()
        app.router.add_post("/{path:(touch|key|button)}", handle)
        runner = web.AppRunner(app, access_log=None)
        await runner.setup()
        try:
            await web.TCPSite(runner, STREAM_HOST, STREAM_PORT).start()
        except OSError as error:
            await runner.cleanup()
            raise RuntimeError(f"ZenLink couldn't open its local control port {STREAM_PORT} ({error}).") from error
        return runner

    async def _input_worker(self) -> None:
        """One consumer keeps the order of touches and keys."""
        while True:
            path, body = await self._queue.get()
            try:
                await self._dispatch(path, body)
            except asyncio.CancelledError:
                raise
            except Exception:  # noqa: BLE001
                log.warning("hybrid: %s failed", path, exc_info=True)

    async def _dispatch(self, path: str, body: bytes) -> None:
        from pymobiledevice3.remote.core_device import screen_stream
        from pymobiledevice3.remote.core_device.hid_service import (
            HID_BUTTON_STATE_DOWN,
            HID_BUTTON_STATE_UP,
            TOUCHSCREEN_STATE_CONTACT,
            TOUCHSCREEN_STATE_RELEASE,
        )

        uhs, indigo = self._uhs, self._indigo
        if uhs is None or indigo is None:
            return
        data = json.loads(body or b"{}")
        if path == "touch":
            x, y = capability.upright_to_native(int(data["x"]), int(data["y"]), self._rotation)
            kind = str(data.get("type"))
            if kind in ("contact", "tap"):
                await uhs.send_touchscreen(TOUCHSCREEN_STATE_CONTACT, x, y)
            if kind == "tap":
                await asyncio.sleep(0.05)
            if kind in ("release", "tap"):
                await uhs.send_touchscreen(TOUCHSCREEN_STATE_RELEASE, x, y)
        elif path == "key":
            if self._kb_id is None:
                self._kb_id = await uhs.create_keyboard_service()
            await uhs.send_keyboard(self._kb_id, [int(u) for u in data.get("usages", [])])
        elif path == "button":
            named = screen_stream._NAMED_BUTTONS.get(str(data.get("name")))
            if named is None:
                return
            page, code, hold = named
            state = str(data.get("state", "press"))
            if state in ("press", "down"):
                await indigo.send_button(page, code, HID_BUTTON_STATE_DOWN)
            if state == "press":
                await asyncio.sleep(hold)
            if state in ("press", "up"):
                await indigo.send_button(page, code, HID_BUTTON_STATE_UP)


async def _drain(transport) -> None:
    with contextlib.suppress(asyncio.CancelledError, OSError):
        while True:
            await transport.recv()


def _control_error(error: Exception) -> str:
    low = str(error).lower()
    tail = " The picture keeps working over AirPlay."
    if "developer mode" in low or "developermode" in low:
        return "Touch control needs Developer Mode (Settings > Privacy & Security), then a restart." + tail
    if "pair" in low or "trust" in low:
        return "Unlock the device and tap Trust, then press Start again for touch control." + tail
    if "not found" in low or "no device" in low or "not connected" in low:
        return "Touch control needs the device on the cable (or Wi-Fi set up in Apple Driver mode)." + tail
    return f"Touch control couldn't start ({error})." + tail
