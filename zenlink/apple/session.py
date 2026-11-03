"""Cast through Apple's own services, the way PhoneLink V1 does.

USB tunnel to the device (CoreDevice over the Apple driver), developer disk
image mounted on first use, then pymobiledevice3's ScreenStreamServer on a
loopback port: HEVC video (/codec, /stream.bin), PCM audio (/audio.bin) and
touch / keyboard / buttons (/touch, /key, /button). ZenLink's own server
proxies those paths under /cast/.
"""

from __future__ import annotations

import asyncio
import contextlib
import logging
import threading
from typing import Callable

from . import audio, offer

log = logging.getLogger(__name__)

STREAM_HOST = "127.0.0.1"
STREAM_PORT = 47322
DISPLAY_SERVICE = "com.apple.coredevice.displayservice"

# SpringBoard interface orientation -> clockwise rotation (deg) the PC applies
# to the portrait capture buffer to show it upright.
_ORIENTATION_TO_ROTATION = {1: 0, 2: 180, 3: 270, 4: 90}


async def _reset_userspace_tunnel() -> None:
    """Close a userspace tunnel a previous session left behind."""
    try:
        from pymobiledevice3.remote import tunnel_service, userspace_tunnel

        active = getattr(userspace_tunnel, "_active_tunnel", None)
        if active is not None and hasattr(active, "aclose"):
            with contextlib.suppress(Exception):
                await asyncio.wait_for(active.aclose(), timeout=1.5)
        userspace_tunnel._active_tunnel = None
        userspace_tunnel.USERSPACE_ACTIVE = False
        tunnel_service.USE_USERSPACE_TUNNEL = False
        lock = getattr(userspace_tunnel, "_lifecycle_locks", {}).get(asyncio.get_running_loop())
        if lock is not None and lock.locked():
            with contextlib.suppress(Exception):
                lock.release()
    except Exception as error:
        log.debug("tunnel reset: %s", error)


class AppleCastSession:
    def __init__(self, udid: str, on_state: Callable[[str, str], None], options: dict,
                 on_rotation: Callable[[int | None], None] | None = None) -> None:
        self.udid = udid
        self.on_state = on_state
        self.on_rotation = on_rotation or (lambda _r: None)
        self.options = options
        self.wifi = bool(options.get("wifi"))
        self._loop: asyncio.AbstractEventLoop | None = None
        self._task: asyncio.Task | None = None
        self._server = None

    # ------------------------------------------------------------ control

    def run(self) -> None:
        """Blocking; returns when stopped or when the stream ends."""
        asyncio.run(self._main())

    def stop(self) -> None:
        loop, task = self._loop, self._task
        if loop is not None and task is not None and loop.is_running():
            loop.call_soon_threadsafe(task.cancel)

    # ------------------------------------------------------------ session

    async def _main(self) -> None:
        self._loop = asyncio.get_running_loop()
        self._task = asyncio.current_task()
        try:
            await self._serve()
        except asyncio.CancelledError:
            pass
        finally:
            await _reset_userspace_tunnel()

    async def _serve(self) -> None:
        from pymobiledevice3.remote.core_device.screen_stream import ScreenStreamServer
        from pymobiledevice3.remote.rsd_tunnel import PreferredRsdTunnel
        from pymobiledevice3.services.mobile_image_mounter import auto_mount

        audio.install(ScreenStreamServer)
        from . import keyboard

        keyboard.install_buttons()
        offer.install(str(self.options.get("quality") or "best"))
        await _reset_userspace_tunnel()

        for attempt in range(3):
            self.on_state("connecting", "Opening a secure Wi-Fi tunnel to the device" if self.wifi
                          else "Opening a secure USB tunnel to the device")
            if self.wifi:
                from .wifi import WifiRsdTunnel

                tunnel = WifiRsdTunnel(self.udid)
            else:
                tunnel = PreferredRsdTunnel(serial=self.udid, autopair=True)
            try:
                async with tunnel as rsd:
                    if DISPLAY_SERVICE not in (rsd.peer_info or {}).get("Services", {}):
                        if attempt:
                            raise RuntimeError(
                                "The device offers no screen service. Turn on Developer Mode "
                                "(Settings > Privacy & Security), restart the device and try again."
                            )
                        self.on_state("activating", "Preparing the device (first time downloads a small image)")
                        await auto_mount(rsd)
                        continue
                    await self._stream(rsd, ScreenStreamServer)
                    return
            except asyncio.CancelledError:
                raise
            except Exception as error:
                if "already active" in str(error) and attempt < 2:
                    await _reset_userspace_tunnel()
                    await asyncio.sleep(0.3)
                    continue
                raise

    async def _stream(self, rsd, ScreenStreamServer) -> None:
        from pymobiledevice3.remote.core_device.display_service import is_media_in_use_error

        server = ScreenStreamServer(
            rsd,
            bind=STREAM_HOST,
            http_port=STREAM_PORT,
            audio_default_on=False,
            # Off, like upstream pane-iphone-mirror-windows.
            rctl_enabled=bool(self.options.get("rateControl", False)),
        )

        async def eager_video_only() -> None:
            try:
                await server._ensure_fresh_stream(force=False)
            except Exception as error:
                if is_media_in_use_error(error):
                    raise
                log.warning("eager video start failed (retried on /codec)", exc_info=True)

        server._eager_stream_start = eager_video_only
        self._server = server
        orientation = asyncio.create_task(self._watch_orientation(rsd))
        self.on_state("streaming", "")
        try:
            await server.serve()
        finally:
            orientation.cancel()
            with contextlib.suppress(BaseException):
                await orientation
            self.on_rotation(None)
            self._server = None

    async def _watch_orientation(self, rsd) -> None:
        """Poll SpringBoard's interface orientation (read-only, never rotates
        the device) so the picture follows the device like V1."""
        from pymobiledevice3.services.springboard import SpringBoardServicesService

        sb = None
        last = None
        while True:
            try:
                if sb is None:
                    sb = SpringBoardServicesService(rsd)
                value = int(await sb.get_interface_orientation())
                rotation = _ORIENTATION_TO_ROTATION.get(value, last)
                if rotation != last:
                    last = rotation
                    self.on_rotation(rotation)
                await asyncio.sleep(0.5)
            except asyncio.CancelledError:
                raise
            except Exception as error:
                log.debug("orientation poll: %s", error)
                sb = None
                await asyncio.sleep(2.0)
