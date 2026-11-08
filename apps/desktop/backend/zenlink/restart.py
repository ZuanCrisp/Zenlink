"""Start a fresh app instance once this one has gone.

The new process is started directly (no cmd /c timeout: it fails without a
console) and waits in __main__ until the old instance has released its port.
"""

from __future__ import annotations

import os
import socket
import subprocess
import sys
import time

from zenlink.paths import BACKEND_DIR

WAIT_ENV = "ZENLINK_WAIT_FOR_PORT"


def spawn_new_instance() -> None:
    exe = sys.executable
    pyw = os.path.join(os.path.dirname(exe), "pythonw.exe")
    if os.path.exists(pyw) and "--headless" not in sys.argv:
        exe = pyw
    module = (__package__ or "zenlink").split(".")[0]
    env = dict(os.environ, **{WAIT_ENV: "1"})
    argv = [sys.executable, *sys.argv[1:]] if getattr(sys, "frozen", False) else [exe, "-m", module, *sys.argv[1:]]
    # CREATE_NO_WINDOW gives the new instance a hidden console that its own
    # children inherit. Not DETACHED_PROCESS: Windows then ignores
    # CREATE_NO_WINDOW, and the venv's python.exe launcher starts the real
    # interpreter without a console, so Windows opens a visible one for it.
    subprocess.Popen(
        argv,
        cwd=str(BACKEND_DIR), env=env,
        creationflags=0x08000000 | 0x00000200,  # no window, new process group
        close_fds=True,
    )


def wait_for_port(host: str, port: int, timeout: float = 15.0) -> None:
    """Called at startup after a restart: wait for the old instance to exit."""
    if not os.environ.pop(WAIT_ENV, None):
        return
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        with socket.socket() as s:
            try:
                s.bind((host, port))
                return
            except OSError:
                time.sleep(0.25)
