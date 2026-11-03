"""Connection Mode Manager: which capture backend this run uses.

Modes:
  apple      Apple drivers + CoreDevice DisplayService (HEVC, touch/keyboard)
  usbdirect  USB Direct: QuickTime capture interface through libusb0

Until a mode is chosen for this run nothing connects: Windows may still
list the device, but no capture backend is selected and Start is refused.

* "Remember my choice" stores the mode in settings.json and the next start
  skips the chooser.
* Without it the choice applies to this run only and the chooser shows again
  on the next start.
* Changing the mode while a mode is active never hot-swaps: the new mode is
  saved as pending and applies after ZenLink restarts (and the cable is
  reconnected when the driver stack changed).
"""

from __future__ import annotations

import json
import logging
import threading

from zenlink.paths import DATA_DIR

log = logging.getLogger(__name__)

MODES = ("apple", "usbdirect")
SETTINGS = DATA_DIR / "settings.json"


class ConnectionModeManager:
    def __init__(self, path=SETTINGS) -> None:
        self._path = path
        self._lock = threading.Lock()
        data = self._load()
        saved = data.get("connectionMode")
        self.remember = bool(data.get("rememberConnectionMode")) and saved in MODES
        self.saved = saved if self.remember else None
        # A remembered mode applies straight away; otherwise wait for a choice.
        self.active: str | None = self.saved
        self.pending: str | None = None

    def _load(self) -> dict:
        try:
            with open(self._path, encoding="utf-8") as fh:
                data = json.load(fh)
            return data if isinstance(data, dict) else {}
        except (OSError, ValueError):
            return {}

    def _save(self, mode: str | None, remember: bool) -> None:
        data = self._load()
        if remember and mode in MODES:
            data["connectionMode"] = mode
            data["rememberConnectionMode"] = True
        else:
            data.pop("connectionMode", None)
            data["rememberConnectionMode"] = False
        tmp = self._path.with_suffix(".tmp")
        with open(tmp, "w", encoding="utf-8") as fh:
            json.dump(data, fh, indent=2)
        tmp.replace(self._path)

    def choose(self, mode: str, remember: bool) -> dict:
        """Apply the first choice of this run, or queue a change for restart."""
        if mode not in MODES:
            raise ValueError(f"Unknown connection mode: {mode}")
        with self._lock:
            self._save(mode, remember)
            self.remember = bool(remember)
            self.saved = mode if remember else None
            if self.active is None:
                self.active = mode
                self.pending = None
            elif mode != self.active:
                self.pending = mode
            else:
                self.pending = None
            log.info("connection mode: active=%s pending=%s remember=%s", self.active, self.pending, self.remember)
            return self.snapshot()

    def snapshot(self) -> dict:
        return {
            "active": self.active,
            "pending": self.pending,
            "saved": self.saved,
            "remember": self.remember,
            "needsChoice": self.active is None,
            "restartRequired": self.pending is not None,
        }
