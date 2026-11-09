"""Cast manager: one capture session at a time, on the backend chosen by the
Connection Mode Manager, plus device presence and driver diagnostics for
the UI.

Backends share one contract: ``run()`` blocks until the stream ends,
``stop()`` ends it from another thread, and state changes arrive through
``on_state(phase, detail)``. Both deliver video/audio to the UI through the
same /cast/* HTTP API (served from an in-process MediaHub). Apple Driver mode,
Hybrid and Studio over Wi-Fi (AirPlay) run in ZenLink.exe (Rust).
"""

from __future__ import annotations

import json
import os
import logging
import threading
import time
from typing import Callable

from zenlink import capability
from zenlink.apple import devices as apple_devices
from zenlink.audio_out import NativeAudio
from zenlink.apple import models
from zenlink.connection import ConnectionModeManager
from zenlink.drivers import status as driver_status

log = logging.getLogger(__name__)

Listener = Callable[[str], None]


# Apple's screen-streaming service (dtremotedisplayd) only sends frames on iOS 27
# and later; older versions accept the request but the picture stays black.
# iOS 17-26 therefore run Hybrid Control (AirPlay picture + CoreDevice HID
# input, in the Rust window); iOS 16 and older have no CoreDevice tunnel.
AUDIO_ROUTES = ("pc", "device", "both")


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


# Why a listed device can't start in the chosen mode (castmanager._need).
_NEED_MESSAGES = {
    "restore": "This device uses the ZenLink Studio driver, so Apple mode can't see it. Press Restore Apple driver "
               "in Settings > Connection & Drivers, or switch to ZenLink Studio.",
    "bind": "ZenLink Studio needs its driver on this device. Press Install ZenLink Studio driver in Settings > "
            "Connection & Drivers, or switch to Apple Driver mode.",
    "package": "ZenLink Studio uses libusb-win32, which isn't on this PC. Install TikTok LIVE Studio's iOS driver "
               "or libusb-win32 once, or switch to Apple Driver mode.",
    "removefilter": "A libusb0 filter is stacked on Apple's driver. Press Remove libusb0 filter in Settings > "
                    "Connection & Drivers first.",
    "mode": "Choose a connection mode first.",
}


# The AirPlay name devices pick in Screen Mirroring (the Rust receiver, airplay.rs).
RECEIVER_NAME = "ZenLinkStudio"


class CastManager:
    def __init__(self) -> None:
        self._lock = threading.Lock()
        self._listeners: set[Listener] = set()
        self.connection = ConnectionModeManager()
        self._session = None
        self._backend: str | None = None
        self._thread: threading.Thread | None = None
        self.hub = None  # MediaHub while a USB Direct session runs
        self._sound_lock = threading.RLock()
        self._presence: dict = {"service": True, "devices": [], "takenOver": False}
        self._drivers: dict = {"apple": {}, "libusb0": {}, "devices": []}
        self._drivers_at = 0.0
        self.driver_busy = False  # a consented driver change is running
        self.audio = NativeAudio()
        self.audio_enabled = True   # "Play sound on this PC"
        # Where the sound plays: "pc" (Windows only), "device" or "both".
        self.audio_route = "pc"
        self.state = {"phase": "idle", "detail": "", "device": None, "error": None, "rotation": None, "backend": None,
                      "control": None}
        # iPhones/iPads trusted before (listed even when not connected) and
        # the one the user picked; Start uses the pick when it is connected.
        self._known: dict = self.connection.known_devices()
        self._selected: str | None = self.connection.selected_device()
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
            "known": self._known_list(),
            "selected": self._selected,
            "service": self._presence.get("service", True),
            "takenOver": self._presence.get("takenOver", False),
            "connection": self.connection.snapshot(),
            "drivers": self._drivers,
            "capture": self.capture_info(),
            "nativeAudio": self.audio.available,
        }

    def _known_list(self) -> list[dict]:
        present = {d.get("id") for d in self._presence.get("devices", [])}
        return [{"id": udid, **info, "present": udid in present} for udid, info in self._known.items()]

    def _remember(self, devices: list[dict]) -> None:
        for d in devices:
            udid = d.get("id")
            if not d.get("ready") or not udid or udid == "airplay":
                continue
            old = self._known.get(udid) or {}
            if old.get("deviceName") == d.get("deviceName") and old.get("name") == d.get("name"):
                continue
            try:
                self.connection.remember_device(d)
            except OSError:
                log.debug("could not save the device list", exc_info=True)
            self._known = self.connection.known_devices()

    def select_device(self, udid: str | None) -> dict:
        self._selected = udid or None
        self.connection.select_device(self._selected)
        self._publish()
        return self.snapshot()

    def forget_device(self, udid: str) -> dict:
        self.connection.forget_device(udid)
        self._known = self.connection.known_devices()
        if self._selected == udid:
            self._selected = None
        self._publish()
        return self.snapshot()

    def _pick(self, found: list[dict], wanted: str | None) -> dict:
        """The device to start: the picked one when it is listed, otherwise
        one that can start now (another may wait for Trust or a driver)."""
        if wanted:
            hit = next((d for d in found if d.get("id") == wanted), None)
            if hit is not None:
                return hit
        return next((d for d in found if d.get("ready")), found[0])

    def capture_info(self) -> dict | None:
        session = self._session
        if session is None:
            return None
        return {"backend": self._backend, **getattr(session, "info", {})}

    def _set_state(self, phase: str, detail: str = "", error: str | None = None) -> None:
        self.state.update(phase=phase, detail=detail, error=error)
        self._publish()

    # ------------------------------------------------------------ presence & drivers

    def _watch(self) -> None:
        while True:
            # Never while casting: the scan reads every driver package and
            # holds the interpreter for up to ~10 ms, which the video relay
            # and the sound output (same process) showed as a stutter every
            # 4 s. Drivers cannot change mid-cast anyway (changes are refused).
            if (self._session is None and not self.driver_busy
                    and time.monotonic() - self._drivers_at > 4.0):
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
        if mode == "usbdirect" and self.connection.link == "wireless":
            # Studio over Wi-Fi: ZenLink is an AirPlay receiver; the device comes to us.
            presence = {"service": True, "takenOver": False, "devices": [{
                "id": "airplay", "family": "iOS device", "name": "AirPlay (Wi-Fi)", "deviceName": None,
                "os": None, "ready": True, "wifi": True, "airplay": True, "receiver": RECEIVER_NAME}]}
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
        presence = apple_devices.connected_devices(self.connection.link)
        presence = {**presence, "devices": [self._with_level(d) for d in presence.get("devices", [])]}
        self._remember(presence.get("devices", []))
        if not presence.get("devices"):
            # Apple's service can't see a device whose driver is wrong for this
            # mode; say what to fix instead of "No device connected".
            blocked = [d for d in self._windows_presence()["devices"] if d.get("need")]
            if blocked:
                presence = {**presence, "devices": blocked, "takenOver": False}
        if presence != self._presence:
            self._presence = presence
            self._publish()

    def _with_level(self, d: dict) -> dict:
        """Add what ZenLink will try for this device (not yet verified)."""
        level = capability.expected(self.connection.active, d.get("os"))
        out = {**d, "control": level}
        if level == "hybrid":
            out.update(hybrid=True, receiver=RECEIVER_NAME)
        return out

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
        wanted = str((options or {}).get("device") or "") or None
        if wanted and wanted != self._selected:
            self.select_device(wanted)
        if wanted and not any(d.get("id") == wanted for d in found) and not (
                mode == "usbdirect" and self.connection.link == "wireless"):
            info = self._known.get(wanted) or {}
            name = info.get("deviceName") or info.get("name") or "That device"
            how = ("Keep it unlocked and on the same Wi-Fi as this PC, or plug in the cable."
                   if self.connection.link == "wireless" else "Plug it in with the cable, unlock it and tap Trust.")
            self._set_state("error", error=f"{name} isn't connected. {how}")
            return self.snapshot()
        if not found:
            msg = ("Your iPhone/iPad is using another app's USB driver (TikTok LIVE Studio). Switch to USB Direct "
                   "or repair the driver in Settings > Connection & Drivers." if self._presence.get("takenOver")
                   else "Connect an iPhone or iPad with a USB cable.")
            self._set_state("error", error=msg)
            return self.snapshot()
        # The picked device, else one that can start now.
        target = self._pick(found, wanted or self._selected)
        need = target.get("need")
        if need in _NEED_MESSAGES:
            self._set_state("error", error=_NEED_MESSAGES[need])
            return self.snapshot()
        if target.get("need") == "wifi":
            self._set_state("error", error="The device isn't on Wi-Fi yet. Press Set up Wi-Fi in Settings with the "
                            "cable connected, keep both on the same network, or switch to Wired.")
            return self.snapshot()
        if not target.get("ready"):
            self._set_state("error", error="Unlock the device and tap Trust, then try again.")
            return self.snapshot()
        self.state["device"] = {"id": target["id"], "name": target["name"], "wifi": bool(target.get("wifi")),
                                "family": target.get("family")}
        self.state["rotation"] = None

        if mode != "usbdirect" or target.get("airplay"):
            # Apple Driver mode, Hybrid and Studio over Wi-Fi run in the Rust window.
            return self.snapshot()
        session = self._usbdirect_session(target)
        if session is None:
            return self.snapshot()
        self.state["control"] = capability.verified("mirroring", video=False, control=False)
        self._session = session
        self._backend = mode
        self.state["backend"] = mode
        self._thread = threading.Thread(target=self._run, args=(session,), name=f"cast-{mode}", daemon=True)
        self._set_state("connecting", "Opening the device through libusb0")
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
        return UsbDirectSession(entry["serial"], self.hub, self._on_session_state, audio_to_pc=self.audio_enabled)

    def stop(self) -> dict:
        session = self._session
        if session is not None:
            import traceback

            caller = " | ".join(line.strip().replace(chr(10), " ") for line in traceback.format_stack(limit=6)[:-1])
            log.info("cast: stop requested (%s) by %s", type(session).__name__, caller[-400:])
        if session is not None:
            session.stop()
            if self._thread is not None:
                self._thread.join(10)
        return self.snapshot()

    def _run(self, session) -> None:
        try:
            session.run()
            log.info("cast: session %s ended", type(session).__name__)
            self._set_state("idle", "Casting stopped")
        except Exception as error:
            log.exception("cast session failed")
            self._set_state("error", error=_friendly_error(error))
        finally:
            self._stop_sound()
            self._session = None
            self._backend = None
            self.state["backend"] = None
            self.state["control"] = None
            if self.hub is not None:
                self.hub.close()
            self.hub = None
            self._publish()

    # ------------------------------------------------------------ native sound

    def _start_sound(self) -> None:
        # Called from the cast session and from every open window at once.
        # Without the lock two calls could both pass the check and start two
        # Apple sound feeds: the same sound fed twice -> overflow, crackle.
        with self._sound_lock:
            self._start_sound_locked()

    def _start_sound_locked(self) -> None:
        if not self.audio_enabled or self._session is None:
            return
        if self.audio.running:
            return
        self.audio.set_jitter_ms(0)  # the cable: lowest latency
        if not self.audio.start():
            return
        if self._backend == "usbdirect" and self.hub is not None:
            self.hub.audio_sink = self.audio.feed

    def _stop_sound(self) -> None:
        with self._sound_lock:
            if self.hub is not None:
                self.hub.audio_sink = None
            self.audio.stop()

    def set_audio(self, enabled=None, volume=None, muted=None, route=None) -> dict:
        if route in AUDIO_ROUTES:
            self.audio_route = route
            enabled = route != "device"
        if volume is not None:
            self.audio.volume = max(0.0, min(1.0, float(volume)))
        if muted is not None:
            self.audio.muted = bool(muted)
        if enabled is not None:
            self.audio_enabled = bool(enabled)
            # ZenLink Studio over the cable: the device plays its own sound
            # only while ZenLink does not ask for it.
            mover = getattr(self._session, "set_audio_to_pc", None)
            if mover is not None:
                mover(self.audio_enabled)
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
        if phase == "streaming":
            # The picture is running now; ZenLink Studio has no control.
            control = self.state.get("control") or {}
            if control.get("level") == "mirroring" and not control.get("video"):
                self.state["control"] = capability.verified("mirroring", video=True, control=False)
        self._set_state(phase, detail)
        if phase == "streaming":
            self._start_sound()

    # ------------------------------------------------------------ connection mode

    def set_link(self, link: str) -> dict:
        if self._session is not None:
            self.stop()  # wired <-> wireless: end the current session first
        self.connection.set_link(link)
        self.refresh_devices()
        self._publish()
        return self.snapshot()

    def choose_mode(self, mode: str, remember: bool) -> dict:
        before = self.connection.active
        self.connection.choose(mode, remember)
        if before is not None and self.connection.pending is not None and self._session is not None:
            # Never hot-swap: end the running session cleanly; the new mode
            # applies after a restart.
            self.stop()
        self._publish()
        return self.snapshot()
