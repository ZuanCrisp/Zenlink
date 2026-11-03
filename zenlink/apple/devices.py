"""Connected iPhones/iPads through Apple Mobile Device Service (usbmuxd).

Same approach as PhoneLink V1: list devices on 127.0.0.1:27015 and read
model / name / iOS version with a lockdown GetValue. No administrator
rights, no driver changes.
"""

from __future__ import annotations

import re
import subprocess
import threading
import winreg

from . import models, usbmuxd

_NO_WINDOW = 0x08000000
_INFO_KEYS = ("DeviceName", "DeviceClass", "ProductType", "ProductVersion")
_cache: dict[str, dict] = {}
_lock = threading.Lock()


def _read_info(device: dict) -> dict:
    udid = device["udid"]
    with _lock:
        cached = _cache.get(udid)
    if cached and cached.get("paired"):
        return cached
    info: dict = {"paired": False}
    try:
        with usbmuxd.open_lockdown(int(device["deviceId"])) as sock:
            usbmuxd.lockdown_request(sock, "QueryType")
            reply = usbmuxd.lockdown_request(sock, "GetValue")
        values = reply.get("Value") if isinstance(reply.get("Value"), dict) else {}
        info = {key: values.get(key) for key in _INFO_KEYS if key in values}
        info["paired"] = not reply.get("Error")
        if info.get("ProductType"):
            models.remember_product(udid.replace("-", ""), str(info["ProductType"]))
    except usbmuxd.MuxError as error:
        info["error"] = str(error)
    with _lock:
        _cache[udid] = info
    return info


def connected_devices() -> dict:
    """{'service': bool, 'devices': [...], 'takenOver': bool}"""
    try:
        listed = usbmuxd.list_devices()
    except usbmuxd.MuxError as error:
        return {"service": False, "error": str(error), "devices": [], "takenOver": _taken_over()}
    devices = []
    usb_ids = {d["udid"] for d in listed if str(d.get("connection", "USB")).upper() == "USB"}
    # A cable connection wins; a device only on Wi-Fi is listed with wifi=True.
    listed = [d for d in listed if str(d.get("connection", "USB")).upper() == "USB" or d["udid"] not in usb_ids]
    listed.sort(key=lambda d: str(d.get("connection", "USB")).upper() != "USB")
    for d in listed:
        wifi = str(d.get("connection", "USB")).upper() != "USB"
        info = _read_info(d)
        cls = str(info.get("DeviceClass") or "")
        family = "iPad" if cls == "iPad" else "iPhone" if cls == "iPhone" else "iOS device"
        devices.append({
            "id": d["udid"],
            "family": family,
            "name": models.model_name(d["udid"].replace("-", ""), family),
            "deviceName": info.get("DeviceName"),
            "os": info.get("ProductVersion"),
            "ready": bool(info.get("paired")),
            "wifi": wifi,
        })
    with _lock:  # forget unplugged devices so a re-plug re-reads trust
        for udid in list(_cache):
            if udid not in {d["udid"] for d in listed}:
                _cache.pop(udid, None)
    return {"service": True, "devices": devices, "takenOver": not devices and _taken_over()}


_APPLE = re.compile(r"^USB\\VID_05AC&PID_12[0-9A-F]{2}\\[^&\\]+$", re.IGNORECASE)


def _taken_over() -> bool:
    """An iPhone/iPad is plugged in but bound to a non-Apple (libusb) driver,
    e.g. after TikTok LIVE Studio's wired cast. Read-only check."""
    try:
        out = subprocess.run(["pnputil", "/enum-devices", "/connected"], capture_output=True,
                             text=True, timeout=10, creationflags=_NO_WINDOW).stdout
    except (OSError, subprocess.TimeoutExpired):
        return False
    for line in out.splitlines():
        label, _, value = line.partition(":")
        if label.strip().lower() == "instance id" and _APPLE.match(value.strip()):
            try:
                with winreg.OpenKey(winreg.HKEY_LOCAL_MACHINE, rf"SYSTEM\CurrentControlSet\Enum\{value.strip()}") as key:
                    if str(winreg.QueryValueEx(key, "Service")[0]).lower().startswith("libusb"):
                        return True
            except OSError:
                pass
    return False
