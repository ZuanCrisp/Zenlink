"""Windows Firewall rule so AirPlay (ZenLink Studio over Wi-Fi) and Apple Driver over Wi-Fi
(ZenLink.exe browses mDNS for the device) can be reached.

The iPhone/iPad must reach ZenLink on the local network: mDNS queries (UDP
5353), the AirPlay port (TCP 7000) and the stream ports. Windows blocks
those for a program without a rule, and on a "Public" network it often
never asks. ZenLink adds one inbound allow rule per program, limited to the
local subnet, only when the user presses the button and approves the UAC
prompt (the same "repair" TikTok LIVE Studio offers).
"""

from __future__ import annotations

import ctypes
import subprocess
import sys
from pathlib import Path

RULE = "ZenLink (AirPlay)"
_NO_WINDOW = 0x08000000


def programs() -> list[str]:
    """Executables that actually own ZenLink's sockets."""
    if getattr(sys, "frozen", False):
        service = Path(sys.executable).resolve()
        window = service.parent / "ZenLink.exe"  # Apple Driver (Rust) runs in the window's process
        return [str(service)] + ([str(window)] if window.exists() else [])
    base = Path(getattr(sys, "_base_executable", sys.executable)).resolve().parent
    out = [str(base / name) for name in ("pythonw.exe", "python.exe") if (base / name).exists()]
    return out or [str(Path(sys.executable).resolve())]


def has_rule() -> bool:
    try:
        r = subprocess.run(["netsh", "advfirewall", "firewall", "show", "rule", f"name={RULE}", "verbose"],
                           capture_output=True, text=True, creationflags=_NO_WINDOW, timeout=10)
    except (OSError, subprocess.SubprocessError):
        return False
    if r.returncode != 0:
        return False
    text = r.stdout.lower()
    return all(p.lower() in text for p in programs())


def request_rule() -> bool:
    """Ask Windows (UAC) to add the rule. Returns True if the elevated helper ran."""
    from zenlink.paths import DATA_DIR

    # Delete only this program's old rule: an installed ZenLink and a portable
    # copy share the rule name, and each keeps its own rule.
    lines = ["@echo off"]
    for program in programs():
        lines.append(f'netsh advfirewall firewall delete rule name="{RULE}" program="{program}" >nul 2>&1')
        lines.append(f'netsh advfirewall firewall add rule name="{RULE}" dir=in action=allow '
                     f'program="{program}" enable=yes profile=any remoteip=localsubnet >nul')
    script = DATA_DIR / "allow-airplay.cmd"
    script.write_bytes(("\r\n".join(lines) + "\r\n").encode("ascii"))
    # ShellExecuteW returns > 32 when the elevated process started (UAC accepted).
    rc = ctypes.windll.shell32.ShellExecuteW(None, "runas", "cmd.exe", f'/c "{script}"', None, 0)
    return rc > 32
