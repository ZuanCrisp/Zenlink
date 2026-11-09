"""ZenLink backend entry point: the local server the Rust window (ZenLink.exe) talks to."""

from __future__ import annotations

import ctypes
import logging
import os
import sys
import threading

from zenlink.castmanager import CastManager
from zenlink.paths import APP_ROOT, BACKEND_DIR, LOG_DIR
from zenlink.server import HOST, PORT, serve_forever

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


def _open_rust_window() -> None:
    """ZenLinkService.exe was started on its own: start ZenLink.exe instead."""
    import subprocess

    exe = APP_ROOT / "ZenLink.exe"
    if exe.is_file():
        subprocess.Popen([str(exe)], close_fds=True)  # noqa: S603
        return
    try:
        ctypes.windll.user32.MessageBoxW(None, "Open ZenLink.exe to start ZenLink.", "ZenLink", 0x40)
    except Exception:  # noqa: BLE001
        pass


def _version() -> str:
    try:
        from importlib.metadata import version

        return version("zenlink")
    except Exception:  # noqa: BLE001 - not installed as a package (source checkout, frozen build)
        try:
            import tomllib

            with open(BACKEND_DIR / "pyproject.toml", "rb") as fh:
                return tomllib.load(fh)["project"]["version"]
        except Exception:  # noqa: BLE001
            return "?"


def main() -> None:
    from logging.handlers import RotatingFileHandler

    # logs/zenlink-service.log, 2 MB per file, the last two kept (.1, .2).
    logging.basicConfig(
        level=logging.INFO,
        format="%(asctime)s %(levelname)s %(name)s: %(message)s",
        handlers=[RotatingFileHandler(LOG_DIR / "zenlink-service.log", maxBytes=2 << 20, backupCount=2,
                                      encoding="utf-8")],
        force=True,  # a library import may have configured logging already (empty log otherwise)
    )
    logging.getLogger("zenlink").info("ZenLink backend %s started (pid %d, Python %s, %s)", _version(), os.getpid(),
                                      sys.version.split()[0], sys.platform)
    import faulthandler

    try:  # a native crash (audio driver, decoder) leaves a trace instead of nothing
        faulthandler.enable(open(LOG_DIR / "crash.log", "a", encoding="utf-8"))  # noqa: SIM115
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

            ctypes.windll.user32.MessageBoxW(None, msg, "ZenLink", 0x10)
        except Exception:  # noqa: BLE001
            pass
        sys.exit(msg)

    if "--headless" in sys.argv:
        if sys.stdout:
            print(f"ZenLink running at http://{HOST}:{PORT}/", flush=True)
        _wait_for_parent()
        manager.stop()
        return

    # A started ZenLinkService.exe (or `python -m zenlink`) opens the Rust window, which starts
    # its own backend; the classic WebView2 window is gone (v2.5.12).
    manager.stop()
    _open_rust_window()


if __name__ == "__main__":
    main()
