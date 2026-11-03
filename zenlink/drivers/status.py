"""Read-only view of the USB driver stack for connected iPhones/iPads.

Nothing here changes the system: it reads the Windows device list
(`pnputil /enum-devices /connected`), the device registry keys and the
service registry. Safe to poll while a capture session owns the device.

Terms used throughout ZenLink:

* device driver - the function driver of a device node (``Service``), e.g.
  ``usbccgp`` (Windows USB composite parent, used by Apple's INF),
  ``WINUSB`` (Apple's usbmux interface), or ``libusb0`` when another app
  (TikTok LIVE Studio's iOS driver) replaced Apple's parent driver;
* filter driver - an extra driver stacked on a device node (``UpperFilters``
  / ``LowerFilters``); Apple adds ``AppleLowerFilter``; USB Direct adds the
  per-device ``libusb0`` upper filter on the composite parent;
* user-mode library - ``libusb0.dll`` (talks to libusb0.sys);
* capture backend - ZenLink's Apple (CoreDevice DisplayService) or USB
  Direct (QuickTime capture interface) session.

USB Direct state of a device:

* ``replaced`` - parent bound to ``libusb0`` as its device driver, the way
  TikTok LIVE Studio's iOS driver does it: USB Direct works; Apple Mobile
  Device Service can't see the device until Apple's driver is restored;
* ``apple``    - Apple drivers only: Apple mode works;
* ``coexist``  - parent on ``usbccgp`` with a ``libusb0`` upper filter.
  UNSAFE: on an iPad 11th gen this left the device stuck in configuration 1
  (Apple's usbmux unusable) and selecting the capture configuration through
  it crashed Windows. ZenLink never uses this state and offers to remove
  the filter;
* ``unknown``  - anything else (no driver, problem code, other driver).
"""

from __future__ import annotations

import os
import re
import subprocess
import winreg

_NO_WINDOW = 0x08000000
_PARENT = re.compile(r"^USB\\VID_05AC&PID_(12[0-9A-F]{2})\\([^&\\]+)$", re.IGNORECASE)
_CHILD = re.compile(r"^USB\\VID_05AC&PID_(12[0-9A-F]{2})&MI_([0-9A-F]{2})\\", re.IGNORECASE)
_FAMILY_BY_PID = {"12A8": "iPhone", "12AB": "iPad"}
_SYSTEM32 = os.path.join(os.environ.get("WINDIR", r"C:\Windows"), "System32")


def _value(key, name):
    try:
        return winreg.QueryValueEx(key, name)[0]
    except OSError:
        return None


def _list(value) -> list[str]:
    if not value:
        return []
    if isinstance(value, str):
        return [value]
    return [str(v) for v in value if v]


def _enum_key(instance: str) -> dict:
    try:
        with winreg.OpenKey(winreg.HKEY_LOCAL_MACHINE, rf"SYSTEM\CurrentControlSet\Enum\{instance}") as key:
            return {
                "service": str(_value(key, "Service") or ""),
                "upper": _list(_value(key, "UpperFilters")),
                "lower": _list(_value(key, "LowerFilters")),
                "desc": str(_value(key, "FriendlyName") or _value(key, "DeviceDesc") or "").split(";")[-1],
                "driver": str(_value(key, "Driver") or ""),
            }
    except OSError:
        return {"service": "", "upper": [], "lower": [], "desc": "", "driver": ""}


def _connected_instances() -> list[tuple[str, str]]:
    """(instance id, status) of connected Apple USB device nodes."""
    try:
        out = subprocess.run(
            ["pnputil", "/enum-devices", "/connected"],
            capture_output=True, text=True, timeout=10, creationflags=_NO_WINDOW,
        ).stdout
    except (OSError, subprocess.TimeoutExpired):
        return []
    found: list[tuple[str, str]] = []
    instance = None
    for line in out.splitlines():
        label, _, value = line.partition(":")
        label, value = label.strip().lower(), value.strip()
        if label == "instance id":
            instance = value if value.upper().startswith("USB\\VID_05AC&PID_12") else None
            if instance:
                found.append((instance, ""))
        elif label == "status" and instance and found and found[-1][0] == instance:
            found[-1] = (instance, value)
    return found


def _service_state(name: str) -> dict:
    """Installed / start type of a service from the registry; running from sc."""
    info = {"installed": False, "running": False}
    try:
        with winreg.OpenKey(winreg.HKEY_LOCAL_MACHINE, rf"SYSTEM\CurrentControlSet\Services\{name}") as key:
            info["installed"] = True
            info["image"] = str(_value(key, "ImagePath") or "")
    except OSError:
        return info
    try:
        out = subprocess.run(["sc", "query", name], capture_output=True, text=True, timeout=5,
                             creationflags=_NO_WINDOW).stdout
        info["running"] = "RUNNING" in out
    except (OSError, subprocess.TimeoutExpired):
        pass
    return info


from zenlink.paths import APP_ROOT

BUNDLED = str(APP_ROOT / "drivers" / "libusb-win32")
ZADIG_URL = "https://zadig.akeo.ie/"  # official Zadig (GPL-3.0, pbatard/libwdi)


def libusb0_status() -> dict:
    service = _service_state("libusb0")
    sys_path = os.path.join(_SYSTEM32, "drivers", "libusb0.sys")
    dll_path = os.path.join(_SYSTEM32, "libusb0.dll")
    status = {
        "service": service["installed"],
        "sys": os.path.exists(sys_path),
        "dll": os.path.exists(dll_path),
        "dllPath": dll_path,
    }
    status["bundled"] = all(os.path.exists(os.path.join(BUNDLED, f))
                            for f in (r"amd64\libusb0.sys", r"amd64\libusb0.dll", r"x86\libusb0_x86.dll"))
    status["installed"] = status["service"] and status["sys"] and status["dll"]
    # Installing the USB Direct driver works from either source.
    status["available"] = status["installed"] or status["bundled"]
    status["zadig"] = True  # the official download page is always offered
    return status


def libusb0_packages() -> dict[str, str]:
    """libusb-win32 device-driver packages in the driver store, by USB PID.

    TikTok LIVE Studio's installer (libwdi) and Zadig generate one package per
    device model (e.g. ``VID_05AC&PID_12A8`` for iPhone), so an iPad needs its
    own. Read-only: parses ``pnputil /enum-drivers`` and the INF files."""
    try:
        out = subprocess.run(["pnputil", "/enum-drivers"], capture_output=True, text=True, timeout=15,
                             creationflags=_NO_WINDOW).stdout
    except (OSError, subprocess.TimeoutExpired):
        return {}
    found: dict[str, str] = {}
    for block in re.split(r"\n\s*\n", out):
        if "libusb-win32" not in block:
            continue
        m = re.search(r"Published Name:\s+(\S+\.inf)", block, re.IGNORECASE)
        if not m:
            continue
        path = os.path.join(os.environ.get("WINDIR", r"C:\Windows"), "INF", m.group(1))
        try:
            with open(path, "rb") as fh:
                raw = fh.read()
        except OSError:
            continue
        # INF files are ANSI/UTF-8 or UTF-16 (libwdi writes UTF-16).
        utf16 = raw[:2] in (b"\xff\xfe", b"\xfe\xff") or raw[1:2] == b"\x00"
        text = raw.decode("utf-16" if utf16 else "utf-8", errors="ignore")
        for pid in re.findall(r"VID_05AC&PID_([0-9A-F]{4})", text, re.IGNORECASE):
            found.setdefault(pid.upper(), m.group(1))
    return found


def apple_status() -> dict:
    amds = _service_state("Apple Mobile Device Service")
    return {"service": amds["installed"], "running": amds["running"]}


def device_states(packages: dict[str, str] | None = None) -> list[dict]:
    """One entry per connected Apple composite device (the parent node)."""
    packages = libusb0_packages() if packages is None else packages
    nodes = _connected_instances()
    children: dict[str, list[dict]] = {}
    for instance, status in nodes:
        m = _CHILD.match(instance)
        if m:
            info = _enum_key(instance)
            children.setdefault(m.group(1).upper(), []).append(
                {"interface": int(m.group(2), 16), "service": info["service"], "name": info["desc"],
                 "upper": info["upper"], "status": status}
            )
    devices = []
    for instance, status in nodes:
        m = _PARENT.match(instance)
        if not m:
            continue
        pid, serial = m.group(1).upper(), m.group(2)
        info = _enum_key(instance)
        service = info["service"].lower()
        upper = [f.lower() for f in info["upper"]]
        if service == "libusb0":
            mode = "replaced"
        elif service == "usbccgp" and "libusb0" in upper:
            mode = "coexist"
        elif service == "usbccgp":
            mode = "apple"
        else:
            mode = "unknown"
        kids = sorted(children.get(pid, []), key=lambda c: c["interface"])
        stale_child_filters = [c["interface"] for c in kids if "libusb0" in [f.lower() for f in c["upper"]]]
        devices.append({
            "instance": instance,
            "serial": serial,
            "pid": pid,
            "family": _FAMILY_BY_PID.get(pid) or ("iPad" if "ipad" in info["desc"].lower() else "iPhone"),
            "status": status,
            "service": info["service"],
            "upperFilters": info["upper"],
            "lowerFilters": info["lower"],
            "mode": mode,
            "interfaces": kids,
            "staleChildFilters": stale_child_filters,
            "usbDirectReady": mode == "replaced",
            "appleReady": mode == "apple",
            "unsafeFilter": "libusb0" in upper or bool(stale_child_filters),
            "libusbPackage": packages.get(pid),
        })
    return devices


def snapshot() -> dict:
    packages = libusb0_packages()
    lib = libusb0_status()
    lib["packages"] = packages
    return {"apple": apple_status(), "libusb0": lib, "devices": device_states(packages)}
