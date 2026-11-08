"""Consented driver switches between Apple mode and USB Direct.

The app process itself never writes driver state. A change runs only when
the user pressed a button in Settings > Connection & Drivers and confirmed
it; then `tools/usb_filter.ps1` runs elevated through UAC (Windows asks the
user), backs up the device's driver state, applies the change, verifies the
device restarted, and rolls back on failure.
"""

from __future__ import annotations

import json
import logging
import os
import subprocess
import tempfile
import time
import uuid

from zenlink.paths import TOOLS_DIR

from . import status

log = logging.getLogger(__name__)

SCRIPT = TOOLS_DIR / "usb_filter.ps1"
ACTIONS = ("bind", "restore", "removefilter")
_NO_WINDOW = 0x08000000


class DriverActionError(RuntimeError):
    pass


def run_action(action: str, instance: str) -> dict:
    """Run one elevated action on one device. Blocks until it finishes."""
    if action not in ACTIONS:
        raise DriverActionError(f"Unknown action: {action}")
    known = {d["instance"] for d in status.device_states()}
    if instance not in known:
        raise DriverActionError("That device is not connected anymore. Plug it in and try again.")
    result_path = os.path.join(tempfile.gettempdir(), f"zenlink-driver-{uuid.uuid4().hex}.json")
    args = (
        f"-NoProfile -ExecutionPolicy Bypass -File \"{SCRIPT}\" "
        f"-Action {action} -InstanceId \"{instance}\" -ResultPath \"{result_path}\""
    )
    # Start-Process -Verb RunAs shows the UAC prompt; declining throws.
    launcher = (
        "try { $p = Start-Process -FilePath powershell.exe -Verb RunAs -WindowStyle Normal -Wait -PassThru "
        f"-ArgumentList '{args.replace(chr(39), chr(39) * 2)}'; exit $p.ExitCode }} catch {{ exit 1223 }}"
    )
    log.info("driver action %s on %s (elevated)", action, instance)
    started = time.monotonic()
    proc = subprocess.run(
        ["powershell.exe", "-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", launcher],
        capture_output=True, text=True, timeout=180, creationflags=_NO_WINDOW,
    )
    if proc.returncode == 1223 and not os.path.exists(result_path):
        raise DriverActionError("Administrator permission was not given. Nothing was changed.")
    try:
        with open(result_path, encoding="utf-8-sig") as fh:
            result = json.load(fh)
    except (OSError, ValueError):
        raise DriverActionError(
            f"The driver tool did not report a result (exit {proc.returncode}). Nothing is known to have changed."
        ) from None
    finally:
        try:
            os.remove(result_path)
        except OSError:
            pass
    result["seconds"] = round(time.monotonic() - started, 1)
    log.info("driver action result: %s", {k: result.get(k) for k in ("ok", "message", "rolledBack", "backup")})
    return result
