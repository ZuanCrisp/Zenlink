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
    "--enable-features=PlatformHEVCDecoderSupport --autoplay-policy=no-user-gesture-required",
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


ICON = APP_ROOT / "assets" / "zenlink.ico"


def _app_id() -> None:
    """Own taskbar group (and icon) instead of python's."""
    try:
        ctypes.windll.shell32.SetCurrentProcessExplicitAppUserModelID("ZenLink")
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


def main() -> None:
    logging.basicConfig(
        level=logging.INFO,
        format="%(asctime)s %(levelname)s %(name)s: %(message)s",
        handlers=[logging.FileHandler(DATA_DIR / "zenlink.log", encoding="utf-8")],
        force=True,  # a library import may have configured logging already (empty log otherwise)
    )
    from zenlink import restart

    restart.wait_for_port(HOST, PORT)
    manager = CastManager()
    ready = threading.Event()
    threading.Thread(target=serve_forever, args=(manager, ready), name="server", daemon=True).start()
    if not ready.wait(10):
        msg = (f"ZenLink could not start: port {PORT} is already in use.\n\n"
               "ZenLink may already be running (check the system tray), or another program uses the port. "
               "Close it and open ZenLink again.")
        try:
            import ctypes

            ctypes.windll.user32.MessageBoxW(None, msg, "ZenLink", 0x10)
        except Exception:  # noqa: BLE001
            pass
        sys.exit(msg)

    if "--headless" in sys.argv:
        if sys.stdout:
            print(f"ZenLink running at http://{HOST}:{PORT}/")
        threading.Event().wait()

    import webview

    _app_id()

    api = WindowApi()
    window = webview.create_window(
        "ZenLink",
        f"http://{HOST}:{PORT}/",
        js_api=api,
        width=1180,
        height=760,
        min_size=(640, 420),
        frameless=True,
        easy_drag=False,
        background_color="#070708",
    )
    api.bind(window)
    restart_requested = threading.Event()

    def restart_hook() -> None:
        restart_requested.set()
        window.destroy()  # closes WebView2 cleanly; webview.start() returns

    manager.restart_hook = restart_hook

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
