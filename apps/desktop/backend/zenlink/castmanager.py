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

import re

import json
import logging
import threading
import time
from typing import Callable

from zenlink import capability
from zenlink.apple import devices as apple_devices
from zenlink.audio_out import AppleAudioPump, NativeAudio
from zenlink.apple import models
from zenlink.connection import ConnectionModeManager
from zenlink.drivers import status as driver_status

log = logging.getLogger(__name__)

Listener = Callable[[str], None]


# Apple's screen-streaming service (dtremotedisplayd) only sends frames on iOS 27
# and later; older versions accept the request but the picture stays black.
# iOS 17-26 therefore run Hybrid Control (AirPlay picture + CoreDevice HID
# input, zenlink/apple/hybrid.py); iOS 16 and older have no CoreDevice tunnel.
APPLE_MIN_IOS = capability.NATIVE_MIN_IOS
AUDIO_ROUTES = ("pc", "device", "both")
VOLUME_STEPS = 20  # more than enough presses to go from full volume to 0
IPAD_SWAPPED_VOLUME = {270}  # iPad rotations where volume up/down swap (verified: 270)


def _ios_too_old(version: str | None) -> bool:
    """True when Apple Driver mode can't run at all (iOS 16 or older)."""
    return capability.expected("apple", version) == "unsupported"


def _old_ios_message(version: str) -> str:
    return (f"Apple Driver mode needs iOS {capability.HYBRID_MIN_IOS} or later; this device runs iOS {version}. "
            "Use ZenLink Studio over the cable or Wi-Fi (AirPlay): Settings > Connection & Drivers.")


def _no_native_video_message(version: str) -> str:
    return (f"iOS {version} doesn't send its screen to Apple Driver mode (that needs iOS {APPLE_MIN_IOS} or later). "
            "ZenLink uses Hybrid Control on this device: the picture comes over AirPlay. "
            "If it still fails, use ZenLink Studio: Settings > Connection & Drivers.")


def _friendly_error(error: Exception) -> str:
    text = str(error)
    low = text.lower()
    old = re.search(r"needs iOS [\d.]+ or later; this device runs iOS ([\d.]+)", text)
    if old:
        return _no_native_video_message(old.group(1))
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


def _receiver_name() -> str:
    from zenlink.airplay.receiver import receiver_name

    return receiver_name()


class CastManager:
    def __init__(self) -> None:
        self._lock = threading.Lock()
        self._listeners: set[Listener] = set()
        self.connection = ConnectionModeManager()
        self._session = None
        self._backend: str | None = None
        self._thread: threading.Thread | None = None
        self.hub = None  # MediaHub while a USB Direct session runs
        self._airplay_paused = False
        self._sound_lock = threading.RLock()
        self._presence: dict = {"service": True, "devices": [], "takenOver": False}
        self._drivers: dict = {"apple": {}, "libusb0": {}, "devices": []}
        self._drivers_at = 0.0
        self.driver_busy = False  # a consented driver change is running
        self.audio = NativeAudio()
        self.audio_enabled = True   # "Play sound on this PC"
        # Where the sound plays: "pc" (Windows only), "device" or "both".
        self.audio_route = "pc"
        self._device_quiet = False  # ZenLink turned the device volume down
        self._volume_lock = threading.Lock()
        self._volume_ready_at = 0.0
        # Unlock again when the device locks while mirroring (saved passcode).
        self.unlock_on_lock = False
        self._unlocked_at = 0.0
        self._unlocking = False
        self._pump = None
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
        info = {"backend": self._backend}
        if self._backend == "hybrid":
            st = session.stats()
            info.update(codec="H.264", width=st.get("width"), height=st.get("height"),
                        source="AirPlay (Wi-Fi) + touch over the Apple driver")
        elif self._backend == "usbdirect" and hasattr(session, "receiver"):
            st = session.stats()
            info.update(codec="H.264", width=st.get("width"), height=st.get("height"), source="AirPlay (Wi-Fi)")
        elif self._backend == "usbdirect":
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
            # Never while casting: the scan reads every driver package and
            # holds the interpreter for up to ~10 ms, which the video relay
            # and the sound output (same process) showed as a stutter every
            # 4 s. Drivers cannot change mid-cast anyway (changes are refused).
            if (self._session is None and not self.driver_busy
                    and time.monotonic() - self._drivers_at > 4.0):
                self.refresh_drivers(publish=True)
            if self._session is None and not self.driver_busy:
                self.refresh_devices()
                self._auto_airplay()
            time.sleep(2.0)

    def _auto_airplay(self) -> None:
        """ZenLink Studio over Wi-Fi listens all the time, like TikTok LIVE Studio:
        the device can pick ZenLinkStudio in Screen Mirroring whenever it wants.
        Pressing Stop pauses this until Start is pressed again."""
        if (self.connection.active == "usbdirect" and self.connection.link == "wireless"
                and self._session is None and not self._airplay_paused
                and self.state.get("phase") == "idle"):  # after an error, wait for Start
            self.start({})

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
                "os": None, "ready": True, "wifi": True, "airplay": True, "receiver": _receiver_name()}]}
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
        self._auto_wifi(presence.get("devices", []))
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
            out.update(hybrid=True, receiver=_receiver_name())
        return out

    def _driver_entry(self, udid: str) -> dict | None:
        serial = udid.replace("-", "").upper()
        for d in self._drivers.get("devices", []):
            if d["serial"].replace("-", "").upper() == serial:
                return d
        return None

    # ------------------------------------------------------------ casting

    def start(self, options: dict | None = None) -> dict:
        self._airplay_paused = False
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
        self._device_quiet = False

        if mode == "usbdirect" and target.get("airplay"):
            from zenlink.airplay.session import AirPlaySession
            from zenlink.usbdirect.hub import MediaHub

            self.hub = MediaHub(sparse_keyframes=True)
            session = AirPlaySession(self.hub, self._on_session_state)
            self.state["device"] = {"id": "airplay", "name": "AirPlay", "wifi": True}
        elif mode == "usbdirect":
            session = self._usbdirect_session(target)
            if session is None:
                return self.snapshot()
        else:
            from zenlink.apple import session as apple_session

            level = capability.expected("apple", target.get("os"))
            if level == "unsupported":
                self._set_state("error", error=_old_ios_message(str(target.get("os"))))
                return self.snapshot()
            options = {**(options or {}), "wifi": bool(target.get("wifi"))}
            if level == "hybrid":
                from zenlink.apple.hybrid import HybridSession
                from zenlink.usbdirect.hub import MediaHub

                self.hub = MediaHub(sparse_keyframes=True)
                session = HybridSession(target["id"], self.hub, self._on_session_state, options, self._on_control)
                mode = "hybrid"
            else:
                session = apple_session.AppleCastSession(target["id"], self._on_session_state, options,
                                                         self._on_rotation, self._on_locked)
        self.state["control"] = (session.control if mode == "hybrid"
                                 else capability.verified("native", video=False, control=False) if mode == "apple"
                                 else capability.verified("mirroring", video=False, control=False))
        self._session = session
        self._backend = mode
        self.state["backend"] = mode
        self._thread = threading.Thread(target=self._run, args=(session,), name=f"cast-{mode}", daemon=True)
        self._set_state("connecting", "Starting the AirPlay receiver and the touch link" if mode == "hybrid"
                        else ("Opening a secure Wi-Fi tunnel to the device" if target.get("wifi")
                              else "Opening a secure USB tunnel to the device") if mode == "apple"
                        else "Starting the AirPlay receiver" if target.get("airplay")
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
        return UsbDirectSession(entry["serial"], self.hub, self._on_session_state, audio_to_pc=self.audio_enabled)

    def stop(self) -> dict:
        session = self._session
        if session is not None:
            self._restore_device_volume()
        if session is not None:
            import traceback

            caller = " | ".join(line.strip().replace(chr(10), " ") for line in traceback.format_stack(limit=6)[:-1])
            log.info("cast: stop requested (%s) by %s", type(session).__name__, caller[-400:])
        if session is not None and hasattr(session, "receiver"):
            self._airplay_paused = True  # the user stopped the AirPlay receiver
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

    def _on_control(self, control: dict) -> None:
        """Hybrid reports what it actually opened (picture, control)."""
        if self.state.get("control") != control:
            self.state["control"] = control
            self._publish()

    def _on_locked(self) -> None:
        """The device locked while mirroring: wake it and enter the saved
        passcode. Locks reported while an unlock runs, or within 3 s after it,
        are ignored, so a wrong passcode cannot loop."""
        if not self.unlock_on_lock or self.state.get("phase") != "streaming":
            return
        if self._unlocking or time.monotonic() - self._unlocked_at < 3:
            return
        self._unlocking = True
        threading.Thread(target=self._unlock_now, name="auto-unlock", daemon=True).start()

    def _unlock_now(self) -> None:
        import urllib.request

        from zenlink import server

        time.sleep(0.8)  # let the lock finish first
        req = urllib.request.Request(f"http://127.0.0.1:{server.PORT}/api/keyboard/action",
                                     data=json.dumps({"action": "wake"}).encode(),
                                     headers={"Content-Type": "application/json"})
        try:
            with urllib.request.urlopen(req, timeout=15) as r:
                log.info("auto unlock after lock: %s", r.read()[:120])
        except OSError as error:
            log.info("auto unlock after lock failed: %s", error)
        finally:
            self._unlocked_at = time.monotonic()
            self._unlocking = False

    def _on_rotation(self, rotation: int | None) -> None:
        if self.state.get("rotation") != rotation:
            self.state["rotation"] = rotation
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
        if self.audio.running and (self._backend != "apple" or self._pump is not None):
            return
        # AirPlay over Wi-Fi delivers sound in bursts: keep ~120 ms queued.
        # The cable modes stay at the lowest latency.
        self.audio.set_jitter_ms(120 if hasattr(self._session, "receiver") else 0)
        if not self.audio.start():
            return
        if self._backend in ("usbdirect", "hybrid") and self.hub is not None:
            self.hub.audio_sink = self.audio.feed
        elif self._backend == "apple" and self._pump is None:
            from zenlink.apple import session as apple_session

            self._pump = AppleAudioPump(f"http://{apple_session.STREAM_HOST}:{apple_session.STREAM_PORT}/audio.bin", self.audio)
            self._pump.start()

    def _stop_sound(self) -> None:
        with self._sound_lock:
            if self._pump is not None:
                self._pump.stop()
                self._pump = None
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
        if route in AUDIO_ROUTES:
            self._apply_device_volume()
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
            # The picture is running now. Native control goes through the same
            # DisplayService session as the picture; ZenLink Studio has none.
            # Hybrid reports its own result (_on_control).
            control = self.state.get("control") or {}
            if control.get("level") == "native" and not control.get("video"):
                self.state["control"] = capability.verified("native", video=True, control=True)
            elif control.get("level") == "mirroring" and not control.get("video"):
                self.state["control"] = capability.verified("mirroring", video=True, control=False)
        self._set_state(phase, detail)
        if phase == "streaming":
            self._start_sound()
            # Presses sent while the stream is still settling are dropped, also
            # those of the app's own route push that arrives right now.
            self._volume_ready_at = time.monotonic() + 3.0
            self._apply_device_volume()

    # ------------------------------------------------------------ device volume

    def _apply_device_volume(self, delay: float = 0.0) -> None:
        """Runs in the background: each press takes ~0.4 s, and right after
        the stream starts the device needs a moment before it accepts buttons."""
        threading.Thread(target=self._apply_device_volume_now, args=(delay,), name="device-volume",
                         daemon=True).start()

    def _apply_device_volume_now(self, delay: float = 0.0) -> None:
        """Apple Driver mode sends a copy of the device's sound: the device keeps
        playing it. "Windows only" turns the device volume down to 0 (the copy
        sent to the PC does not follow the device volume); leaving it brings
        the volume back to half (the old level cannot be read)."""
        if self._backend != "apple" or self.state.get("phase") != "streaming":
            return
        with self._volume_lock:
            time.sleep(max(delay, self._volume_ready_at - time.monotonic(), 0.0))
            if self.state.get("phase") != "streaming":
                return
            quiet = self.audio_route == "pc"
            if quiet and not self._device_quiet:
                if self._press_volume("down", VOLUME_STEPS):
                    self._device_quiet = True
                    log.info("audio: device volume down (Windows only)")
            elif not quiet:
                self._restore_device_volume()

    def _restore_device_volume(self) -> None:
        if not self._device_quiet or self._backend != "apple":
            return
        if self._press_volume("up", VOLUME_STEPS // 2):
            log.info("audio: device volume back to half")
        self._device_quiet = False

    def _press_volume(self, direction: str, times: int) -> bool:
        """Volume buttons through the DisplayService session. iPadOS swaps the
        two buttons in some orientations (tested: 270)."""
        import urllib.request

        from zenlink.apple import session as apple_session

        family = str((self.state.get("device") or {}).get("family") or "")
        swap = family == "iPad" and self.state.get("rotation") in IPAD_SWAPPED_VOLUME
        name = "volume-down" if (direction == "down") != swap else "volume-up"
        url = f"http://{apple_session.STREAM_HOST}:{apple_session.STREAM_PORT}/button"
        body = json.dumps({"name": name, "state": "press"}).encode()

        def press() -> None:
            req = urllib.request.Request(url, data=body, headers={"Content-Type": "application/json"})
            with urllib.request.urlopen(req, timeout=5) as r:
                r.read()

        # The first press waits until the stream accepts buttons (up to ~15 s).
        for attempt in range(6):
            try:
                press()
                break
            except OSError as error:
                if attempt == 5 or self.state.get("phase") != "streaming":
                    log.info("audio: device volume %s failed: %s", direction, error)
                    return False
                time.sleep(2.0)
        try:
            for _ in range(times - 1):
                time.sleep(0.08)
                press()
            return True
        except OSError as error:
            log.info("audio: device volume %s failed: %s", direction, error)
            return False

    # ------------------------------------------------------------ connection mode

    def _auto_wifi(self, devices: list[dict]) -> None:
        """Like PhoneLink V1: a trusted device on the cable gets Wi-Fi set up once,
        so Apple Driver mode can later cast without the cable."""
        if self._session is not None:
            return
        tried = self.__dict__.setdefault("_wifi_auto_tried", {})  # udid -> monotonic time
        for d in devices:
            udid = d.get("id")
            on_cable = not d.get("wifi") and (d.get("ready") or d.get("need") == "wifi")
            if not udid or not on_cable or time.monotonic() - tried.get(udid, -1e9) < 60:
                continue
            tried[udid] = time.monotonic()
            if apple_devices.wifi_ready(udid):
                continue

            def run(udid=udid, name=d.get("name")) -> None:
                from zenlink.apple import wifi

                try:
                    result = wifi.prepare(udid)
                    log.info("wifi: automatic setup for %s: %s", name, result)
                except Exception as error:  # noqa: BLE001 - not trusted yet, locked, iOS < 17 ...
                    log.info("wifi: automatic setup for %s failed: %s", name, error)
                    return
                self.refresh_devices()

            threading.Thread(target=run, name="wifi-auto", daemon=True).start()

    def prepare_wifi(self) -> dict:
        """One-time Wi-Fi setup over the cable (Apple Driver mode only)."""
        if self.connection.active != "apple":
            return {"ok": False, "message": "Wi-Fi needs Apple Driver mode."}
        if self._session is not None:
            return {"ok": False, "message": "Stop casting first."}
        listed = apple_devices.connected_devices("wired").get("devices", [])
        usb = [d for d in listed if d.get("ready")]
        if not usb:
            if listed:
                return {"ok": False, "message": "Unlock the device and tap Trust, then try again."}
            # Already set up: the device is reachable without the cable.
            wifi_ok = [d for d in apple_devices.connected_devices("wireless").get("devices", [])
                       if d.get("wifi") and d.get("ready")]
            if wifi_ok:
                return {"ok": True, "message": "Wi-Fi is already set up.", "device": wifi_ok[0]["name"]}
            return {"ok": False, "message": "Connect the device with the cable, unlock it and tap Trust first."}
        # The device that has no Wi-Fi record yet first.
        usb.sort(key=lambda d: apple_devices.wifi_ready(d["id"]))
        target = usb[0]
        from zenlink.apple import wifi

        try:
            result = wifi.prepare(target["id"])
        except Exception as error:  # noqa: BLE001
            return {"ok": False, "message": f"Wi-Fi setup failed: {_friendly_error(error)}"}
        self.__dict__.setdefault("_wifi_auto_tried", {})[target["id"]] = time.monotonic()
        self.refresh_devices()
        return {**result, "device": target["name"]}

    def set_link(self, link: str) -> dict:
        if self._session is not None:
            self.stop()  # wired <-> wireless: end the current session first
        self._airplay_paused = False
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
