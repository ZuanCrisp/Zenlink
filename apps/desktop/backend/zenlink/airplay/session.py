"""ZenLink Studio over Wi-Fi: an AirPlay mirroring session for CastManager."""

from __future__ import annotations

import asyncio
import logging
import threading
from typing import Callable

from .receiver import AirPlayReceiver, receiver_name

log = logging.getLogger(__name__)

HOW_TO = ("On your iPhone/iPad open Control Center, tap Screen Mirroring and choose “{name}”. "
          "Both must be on the same Wi-Fi network.")


class AirPlaySession:
    def __init__(self, hub, on_state: Callable[[str, str], None], hevc: bool = False) -> None:
        self.hevc = hevc
        self.hub = hub
        self.on_state = on_state
        self.name = receiver_name()
        self._loop: asyncio.AbstractEventLoop | None = None
        self._stop: asyncio.Event | None = None
        self._ready = threading.Event()
        self.receiver: AirPlayReceiver | None = None

    def _event(self, kind: str, detail: str) -> None:
        if kind == "waiting":
            self.on_state("activating", HOW_TO.format(name=self.name))
        elif kind == "connected":
            self.on_state("activating", f"{detail} is connecting over AirPlay…")
        elif kind == "streaming":
            self.on_state("streaming", "")

    def run(self) -> None:
        asyncio.run(self._main())

    async def _main(self) -> None:
        self._loop = asyncio.get_running_loop()
        from zenlink.server import quiet_resets

        quiet_resets(self._loop)
        self._stop = asyncio.Event()
        self._ready.set()
        self.on_state("connecting", "Starting the AirPlay receiver")
        self.receiver = AirPlayReceiver(self.name, self.hub, self._event, hevc=self.hevc)
        try:
            await self.receiver.start()
        except OSError as error:
            raise RuntimeError(
                f"ZenLink couldn't open the AirPlay port ({error}). Close other AirPlay receivers "
                "(TikTok LIVE Studio, UxPlay, LonelyScreen) and try again."
            ) from error
        try:
            await self._stop.wait()
        finally:
            await self.receiver.stop()

    def stop(self) -> None:
        if self._ready.wait(5) and self._loop is not None and self._stop is not None:
            self._loop.call_soon_threadsafe(self._stop.set)

    def stats(self) -> dict:
        return dict(self.receiver.stats) if self.receiver else {}
