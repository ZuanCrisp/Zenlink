"""System tray icon and low-memory background mode.

* Left-click the icon: show / hide the window.
* Right-click menu: Show / Hide, Start / Stop casting, Quit.
* While the window is hidden, ZenLink trims its working set so it sits in the
  background with as little memory as Windows allows; casting and sound keep
  running.
"""

from __future__ import annotations

import ctypes
import logging
import threading

from zenlink.paths import APP_ROOT

log = logging.getLogger(__name__)


def trim_memory() -> None:
    """Return unused pages of this process to Windows."""
    try:
        kernel32 = ctypes.windll.kernel32
        kernel32.SetProcessWorkingSetSize(kernel32.GetCurrentProcess(), ctypes.c_size_t(-1), ctypes.c_size_t(-1))
    except Exception:  # noqa: BLE001
        pass


class Tray:
    def __init__(self, window_api, manager) -> None:
        self.window_api = window_api
        self.manager = manager
        self.icon = None

    def start(self) -> bool:
        try:
            import pystray
            from PIL import Image
        except Exception as error:  # noqa: BLE001
            log.info("tray unavailable: %s", error)
            return False

        image = Image.open(APP_ROOT / "assets" / "logo.png").convert("RGBA")
        image.thumbnail((64, 64))
        casting = lambda: self.manager.state.get("phase") in ("connecting", "activating", "streaming")  # noqa: E731
        menu = pystray.Menu(
            pystray.MenuItem(lambda _i: "Hide ZenLink Beta" if self.window_api.visible else "Show ZenLink Beta",
                             self._toggle, default=True),
            pystray.MenuItem(lambda _i: "Stop casting" if casting() else "Start casting", self._cast),
            pystray.Menu.SEPARATOR,
            pystray.MenuItem("Quit ZenLink Beta", self._quit),
        )
        self.icon = pystray.Icon("ZenLinkBeta", image, "ZenLink Beta", menu)

        def run() -> None:
            try:
                self.icon.run(setup=lambda icon: (setattr(icon, "visible", True), log.info("tray icon shown")))
            except Exception:  # noqa: BLE001 - e.g. a missing Pillow image plugin in the packaged app
                log.exception("tray icon failed")

        threading.Thread(target=run, name="tray", daemon=True).start()
        return True

    def stop(self) -> None:
        if self.icon is not None:
            try:
                self.icon.stop()
            except Exception:  # noqa: BLE001
                pass

    def _toggle(self, _icon=None, _item=None) -> None:
        if self.window_api.visible:
            self.window_api.hide_to_tray()
        else:
            self.window_api.show_from_tray()

    def _cast(self, _icon=None, _item=None) -> None:
        if self.manager.state.get("phase") in ("connecting", "activating", "streaming"):
            threading.Thread(target=self.manager.stop, daemon=True).start()
        else:
            threading.Thread(target=self.manager.start, daemon=True).start()

    def _quit(self, _icon=None, _item=None) -> None:
        self.window_api.quit()
