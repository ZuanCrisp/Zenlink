"""ZenLink desktop entry point: local server + native window."""

from __future__ import annotations

import ctypes
import logging
import os
import sys
import threading

from zenlink.castmanager import CastManager
from zenlink.paths import APP_ROOT, DATA_DIR
from zenlink.server import HOST, PORT, serve_forever
from zenlink.window import WindowApi

# Hardware video decode and sound without a click first.
os.environ.setdefault(
    "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS",
    # No DirectComposition video overlay: an overlay plane looks smooth on screen
    # but reaches DWM-based capture (OBS Window/Display Capture) unevenly.
    "--enable-features=PlatformHEVCDecoderSupport --autoplay-policy=no-user-gesture-required"
    " --disable-direct-composition-video-overlays",
)


def _dark_title_bar(window) -> None:
    """Dark mode + rounded corners for the frameless window (Windows 11 DWM)."""
    try:
        hwnd = window.native.Handle.ToInt32()
        corner = ctypes.c_int(2)  # DWMWCP_ROUND
        ctypes.windll.dwmapi.DwmSetWindowAttribute(hwnd, 33, ctypes.byref(corner), ctypes.sizeof(corner))
        value = ctypes.c_int(1)
        ctypes.windll.dwmapi.DwmSetWindowAttribute(hwnd, 20, ctypes.byref(value), ctypes.sizeof(value))
    except Exception:
        pass



def _wait_for_parent() -> None:
    """Headless backend: run until the process given by --parent-pid exits
    (the native ZenLink app), or forever when there is none."""
    pid = None
    if "--parent-pid" in sys.argv:
        try:
            pid = int(sys.argv[sys.argv.index("--parent-pid") + 1])
        except (IndexError, ValueError):
            pid = None
    if not pid:
        threading.Event().wait()
        return
    synchronize, infinite = 0x00100000, 0xFFFFFFFF
    handle = ctypes.windll.kernel32.OpenProcess(synchronize, False, pid)
    if not handle:
        return
    ctypes.windll.kernel32.WaitForSingleObject(handle, infinite)
    ctypes.windll.kernel32.CloseHandle(handle)

ICON = APP_ROOT / "assets" / "zenlink.ico"


def _app_id() -> None:
    """Own taskbar group (and icon) instead of python's."""
    try:
        ctypes.windll.shell32.SetCurrentProcessExplicitAppUserModelID("ZenLink.Next")
    except Exception:
        pass


def _set_icon(window) -> None:
    """Window + taskbar icon from assets/zenlink.ico (WinForms has no icon option)."""
    try:
        hwnd = window.native.Handle.ToInt32()
        user32 = ctypes.windll.user32
        user32.LoadImageW.restype = ctypes.c_void_p
        for kind, size in ((1, 256), (0, 32)):  # ICON_BIG, ICON_SMALL
            hicon = user32.LoadImageW(None, str(ICON), 1, size, size, 0x10)  # IMAGE_ICON, LR_LOADFROMFILE
            if hicon:
                user32.SendMessageW(hwnd, 0x0080, kind, ctypes.c_void_p(hicon))  # WM_SETICON
    except Exception:
        pass


def _already_running() -> bool:
    """A second launch brings the running ZenLink to the front and exits,
    instead of waiting invisibly for a port the first one holds."""
    import json
    import urllib.request

    try:
        with urllib.request.urlopen(f"http://{HOST}:{PORT}/api/state", timeout=1.5) as r:
            json.loads(r.read())
    except Exception:  # noqa: BLE001 - nothing (or not ZenLink) is listening
        return False
    try:
        req = urllib.request.Request(f"http://{HOST}:{PORT}/api/app/show", data=b"{}", method="POST",
                                     headers={"Content-Type": "application/json"})
        urllib.request.urlopen(req, timeout=3).read()
    except Exception:  # noqa: BLE001
        pass
    logging.getLogger(__name__).info("ZenLink is already running: brought it to the front")
    return True


def main() -> None:
    logging.basicConfig(
        level=logging.INFO,
        format="%(asctime)s %(levelname)s %(name)s: %(message)s",
        handlers=[logging.FileHandler(DATA_DIR / "zenlink.log", encoding="utf-8")],
        force=True,  # a library import may have configured logging already (empty log otherwise)
    )
    import faulthandler

    try:  # a native crash (WebView2, audio driver, decoder) leaves a trace instead of nothing
        faulthandler.enable(open(DATA_DIR / "crash.log", "a", encoding="utf-8"))  # noqa: SIM115
    except OSError:
        pass
    from zenlink import restart

    restart.wait_for_port(HOST, PORT)
    if _already_running():
        return
    manager = CastManager()
    ready = threading.Event()
    threading.Thread(target=serve_forever, args=(manager, ready), name="server", daemon=True).start()
    if not ready.wait(10):
        msg = (f"ZenLink could not start: port {PORT} is already in use.\n\n"
               "ZenLink may already be running (check the system tray), or another program uses the port. "
               "Close it and open ZenLink again.")
        try:
            import ctypes

            ctypes.windll.user32.MessageBoxW(None, msg, "ZenLink Next", 0x10)
        except Exception:  # noqa: BLE001
            pass
        sys.exit(msg)

    if "--headless" in sys.argv:
        if sys.stdout:
            print(f"ZenLink running at http://{HOST}:{PORT}/", flush=True)
        _wait_for_parent()
        manager.stop()
        return

    import webview

    _app_id()

    api = WindowApi()
    window = webview.create_window(
        "ZenLink Next",
        f"http://{HOST}:{PORT}/",
        js_api=api,
        width=1180,
        height=760,
        min_size=(640, 420),
        frameless=True,
        easy_drag=False,
        background_color="#f5f6fa",
    )
    api.bind(window)
    restart_requested = threading.Event()

    def restart_hook() -> None:
        restart_requested.set()
        window.destroy()  # closes WebView2 cleanly; webview.start() returns

    manager.restart_hook = restart_hook
    manager.show_hook = api.show_from_tray

    from zenlink.tray import Tray

    tray = Tray(api, manager)
    api.tray_available = tray.start()
    window.events.shown += lambda: (_dark_title_bar(window), _set_icon(window))
    try:
        webview.start(gui="edgechromium", private_mode=False, storage_path=str(DATA_DIR / ".webview"))
    finally:
        tray.stop()
        manager.stop()
        if restart_requested.is_set():
            from zenlink import restart

            restart.spawn_new_instance()


if __name__ == "__main__":
    main()
