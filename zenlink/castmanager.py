"""Cast manager: one capture session at a time, on the backend chosen by the
Connection Mode Manager, plus device presence and driver diagnostics for
the UI.

Backends share one contract: ``run()`` blocks until the stream ends,
``stop()`` ends it from another thread, and state changes arrive through
``on_state(phase, detail)``. Both deliver video/audio to the UI through the
same /cast/* HTTP API (Apple: proxied to pymobiledevice3's stream server;
USB Direct: served from an in-process MediaHub), so the player is shared.
"""

from __future__ import annotations

import json
import logging
import threading
import time
from typing import Callable

from zenlink.apple import devices as apple_devices
from zenlink.audio_out import AppleAudioPump, NativeAudio
from zenlink.apple import models
from zenlink.connection import ConnectionModeManager
from zenlink.drivers import status as driver_status

log = logging.getLogger(__name__)

Listener = Callable[[str], None]


def _friendly_error(error: Exception) -> str:
    text = str(error)
    low = text.lower()
    if "developer mode" in low or "libusb0" in low or "settings >" in low:
        return text
    if "pair" in low or "trust" in low:
        return "Unlock the device and tap Trust, then try again."
    if "not connected" in low or "not found" in low or "no device" in low:
        return "The device was disconnected. Plug it back in and try again."
    if "in use" in low or "busy" in low or "access" in low:
        return "Another app is using the device's screen (Xcode, QuickTime, TikTok LIVE Studio). Close it and try again."
    return text


class CastManager:
    def __init__(self) -> None:
        self._lock = threading.Lock()
        self._listeners: set[Listener] = set()
        self.connection = ConnectionModeManager()
        self._session = None
        self._backend: str | None = None
        self._thread: threading.Thread | None = None
        self.hub = None  # MediaHub while a USB Direct session runs
        self._presence: dict = {"service": True, "devices": [], "takenOver": False}
        self._drivers: dict = {"apple": {}, "libusb0": {}, "devices": []}
        self._drivers_at = 0.0
        self.driver_busy = False  # a consented driver change is running
        self.audio = NativeAudio()
        self.audio_enabled = True   # "Play sound on this PC"
        self._pump = None
        self.state = {"phase": "idle", "detail": "", "device": None, "error": None, "rotation": None, "backend": None}
        threading.Thread(target=self._watch, name="device-watch", daemon=True).start()

    # ------------------------------------------------------------ viewers

    def add_listener(self, listener: Listener) -> None:
        with self._lock:
            self._listeners.add(listener)
        listener(json.dumps({"type": "state", **self.snapshot()}))

    def remove_listener(self, listener: Listener) -> None:
        with self._lock:
            self._listeners.discard(listener)

    def _publish(self) -> None:
        message = json.dumps({"type": "state", **self.snapshot()})
        with self._lock:
            listeners = list(self._listeners)
        for listener in listeners:
            try:
                listener(message)
            except Exception:
                log.debug("listener failed", exc_info=True)

    def snapshot(self) -> dict:
        return {
            **self.state,
            "devices": self._presence.get("devices", []),
            "service": self._presence.get("service", True),
            "takenOver": self._presence.get("takenOver", False),
            "connection": self.connection.snapshot(),
            "drivers": self._drivers,
            "capture": self.capture_info(),
            "nativeAudio": self.audio.available,
        }

    def capture_info(self) -> dict | None:
        session = self._session
        if session is None:
            return None
        info = {"backend": self._backend}
        if self._backend == "usbdirect":
            info.update(getattr(session, "info", {}))
        else:
            info.update(codec="HEVC", source="CoreDevice DisplayService")
        return info

    def _set_state(self, phase: str, detail: str = "", error: str | None = None) -> None:
        self.state.update(phase=phase, detail=detail, error=error)
        self._publish()

    # ------------------------------------------------------------ presence & drivers

    def _watch(self) -> None:
        while True:
            if not self.driver_busy and time.monotonic() - self._drivers_at > 4.0:
                self.refresh_drivers(publish=True)
            if self._session is None and not self.driver_busy:
                self.refresh_devices()
            time.sleep(2.0)

    def refresh_drivers(self, publish: bool = False) -> dict:
        snap = driver_status.snapshot()
        self._drivers_at = time.monotonic()
        if snap != self._drivers:
            self._drivers = snap
            if publish:
                self._publish()
        return snap

    def _need(self, d: dict) -> str | None:
        """What stands between this device and the chosen mode (None = ready)."""
        mode = self.connection.active
        if mode is None:
            return "mode"
        if d.get("unsafeFilter"):
            return "removefilter"
        if mode == "usbdirect":
            if d.get("mode") == "replaced":
                return None
            # A missing per-model package is built during the install.
            return "bind" if self._drivers.get("libusb0", {}).get("available") else "package"
        if d.get("mode") == "replaced":
            return "restore"
        return None

    def _windows_presence(self) -> dict:
        """Devices from Windows' own device list only (no Apple service, no USB I/O)."""
        devices = []
        for d in self._drivers.get("devices", []):
            serial = d["serial"]
            udid = f"{serial[:8]}-{serial[8:]}" if len(serial) == 24 else serial
            need = self._need(d)
            devices.append({"id": udid, "family": d["family"], "name": models.model_name(serial, d["family"]),
                            "deviceName": None, "os": None, "need": need,
                            "ready": self.connection.active == "usbdirect" and need is None})
        return {"service": True, "devices": devices, "takenOver": False}

    def refresh_devices(self) -> None:
        mode = self.connection.active
        if mode is None:
            # Before a mode is chosen nothing runs: Windows detection only.
            presence = self._windows_presence()
            if presence != self._presence:
                self._presence = presence
                self._publish()
            return
        if mode == "usbdirect":
            presence = self._windows_presence()
            if presence != self._presence:
                self._presence = presence
                self._publish()
            return
        presence = apple_devices.connected_devices()
        if not presence.get("devices"):
            # Apple's service can't see a device whose driver is wrong for this
            # mode; say what to fix instead of "No device connected".
            blocked = [d for d in self._windows_presence()["devices"] if d.get("need")]
            if blocked:
                presence = {**presence, "devices": blocked, "takenOver": False}
        if presence != self._presence:
            self._presence = presence
            self._publish()

    def _driver_entry(self, udid: str) -> dict | None:
        serial = udid.replace("-", "").upper()
        for d in self._drivers.get("devices", []):
            if d["serial"].replace("-", "").upper() == serial:
                return d
        return None

    # ------------------------------------------------------------ casting

    def start(self, options: dict | None = None) -> dict:
        if self._session is not None or self.driver_busy:
            return self.snapshot()
        mode = self.connection.active
        if mode is None:
            self._set_state("error", error="Choose a connection mode first.")
            return self.snapshot()
        self.refresh_devices()
        found = self._presence.get("devices", [])
        if not found:
            msg = ("Your iPhone/iPad is using another app's USB driver (TikTok LIVE Studio). Switch to USB Direct "
                   "or repair the driver in Settings > Connection & Drivers." if self._presence.get("takenOver")
                   else "Connect an iPhone or iPad with a USB cable.")
            self._set_state("error", error=msg)
            return self.snapshot()
        target = found[0]
        if not target.get("ready"):
            self._set_state("error", error="Unlock the device and tap Trust, then try again.")
            return self.snapshot()
        self.state["device"] = {"id": target["id"], "name": target["name"], "wifi": bool(target.get("wifi"))}
        self.state["rotation"] = None

        if mode == "usbdirect":
            session = self._usbdirect_session(target)
            if session is None:
                return self.snapshot()
        else:
            from zenlink.apple import session as apple_session

            options = {**(options or {}), "wifi": bool(target.get("wifi"))}
            session = apple_session.AppleCastSession(target["id"], self._on_session_state, options, self._on_rotation)
        self._session = session
        self._backend = mode
        self.state["backend"] = mode
        self._thread = threading.Thread(target=self._run, args=(session,), name=f"cast-{mode}", daemon=True)
        self._set_state("connecting", ("Opening a secure Wi-Fi tunnel to the device" if target.get("wifi")
                                       else "Opening a secure USB tunnel to the device") if mode == "apple"
                        else "Opening the device through libusb0")
        self._thread.start()
        return self.snapshot()

    def _usbdirect_session(self, target: dict):
        self.refresh_drivers()
        lib = self._drivers.get("libusb0", {})
        if not lib.get("available"):
            self._set_state("error", error="USB Direct needs the libusb0 driver, which isn't installed on this PC. "
                                           "Open Settings > Connection & Drivers.")
            return None
        entry = self._driver_entry(target["id"])
        if entry is None or not entry.get("usbDirectReady"):
            if entry is not None and entry.get("unsafeFilter"):
                msg = ("A libusb0 filter is stacked on this device's Apple driver. That combination can crash "
                       "Windows, so ZenLink won't use it. Open Settings > Connection & Drivers and press "
                       "Remove libusb0 filter.")
            else:
                msg = ("The USB Direct driver isn't installed for this device yet. Open Settings > Connection & "
                       "Drivers and press Install USB Direct driver, then reconnect the cable.")
            self._set_state("error", error=msg)
            return None
        from zenlink.usbdirect.hub import MediaHub
        from zenlink.usbdirect.session import UsbDirectSession

        self.hub = MediaHub()
        return UsbDirectSession(entry["serial"], self.hub, self._on_session_state)

    def stop(self) -> dict:
        session = self._session
        if session is not None:
            session.stop()
            if self._thread is not None:
                self._thread.join(10)
        return self.snapshot()

    def _run(self, session) -> None:
        try:
            session.run()
            self._set_state("idle", "Casting stopped")
        except Exception as error:
            log.exception("cast session failed")
            self._set_state("error", error=_friendly_error(error))
        finally:
            self._stop_sound()
            self._session = None
            self._backend = None
            self.state["backend"] = None
            if self.hub is not None:
                self.hub.close()
            self.hub = None
            self._publish()

    def _on_rotation(self, rotation: int | None) -> None:
        if self.state.get("rotation") != rotation:
            self.state["rotation"] = rotation
            self._publish()

    # ------------------------------------------------------------ native sound

    def _start_sound(self) -> None:
        if not self.audio_enabled or self._session is None or self.audio.running:
            return
        if not self.audio.start():
            return
        if self._backend == "usbdirect" and self.hub is not None:
            self.hub.audio_sink = self.audio.feed
        elif self._backend == "apple":
            from zenlink.apple import session as apple_session

            self._pump = AppleAudioPump(f"http://{apple_session.STREAM_HOST}:{apple_session.STREAM_PORT}/audio.bin", self.audio)
            self._pump.start()

    def _stop_sound(self) -> None:
        if self._pump is not None:
            self._pump.stop()
            self._pump = None
        if self.hub is not None:
            self.hub.audio_sink = None
        self.audio.stop()

    def set_audio(self, enabled=None, volume=None, muted=None) -> dict:
        if volume is not None:
            self.audio.volume = max(0.0, min(1.0, float(volume)))
        if muted is not None:
            self.audio.muted = bool(muted)
        if enabled is not None:
            self.audio_enabled = bool(enabled)
            if not self.audio_enabled:
                self._stop_sound()
            elif self.state.get("phase") == "streaming":
                self._start_sound()
        return self.audio.stats()

    def _on_session_state(self, phase: str, detail: str) -> None:
        # USB Direct reports handshake progress; the UI's checklist knows
        # connecting / activating / streaming.
        if phase in ("waiting", "handshake"):
            phase = "activating"
        self._set_state(phase, detail)
        if phase == "streaming":
            self._start_sound()

    # ------------------------------------------------------------ connection mode

    def prepare_wifi(self) -> dict:
        """One-time Wi-Fi setup over the cable (Apple Driver mode only)."""
        if self.connection.active != "apple":
            return {"ok": False, "message": "Wi-Fi needs Apple Driver mode."}
        if self._session is not None:
            return {"ok": False, "message": "Stop casting first."}
        self.refresh_devices()
        usb = [d for d in self._presence.get("devices", []) if not d.get("wifi") and d.get("ready")]
        if not usb:
            return {"ok": False, "message": "Connect the device with the cable, unlock it and tap Trust first."}
        from zenlink.apple import wifi

        try:
            result = wifi.prepare(usb[0]["id"])
        except Exception as error:  # noqa: BLE001
            return {"ok": False, "message": f"Wi-Fi setup failed: {_friendly_error(error)}"}
        return {**result, "device": usb[0]["name"]}

    def choose_mode(self, mode: str, remember: bool) -> dict:
        before = self.connection.active
        self.connection.choose(mode, remember)
        if before is not None and self.connection.pending is not None and self._session is not None:
            # Never hot-swap: end the running session cleanly; the new mode
            # applies after a restart.
            self.stop()
        self._publish()
        return self.snapshot()
