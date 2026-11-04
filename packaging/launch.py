"""Entry point for the standalone (PyInstaller) build.

``ZenLink.exe --selftest <file>`` imports every module the device paths load
on demand (tunnel, developer image, display / HID, AirPlay, USB) and writes
the result to <file> as JSON, then exits. The build runs it on the finished
exe, so a package that's missing from the bundle (or its metadata, like
pyimg4 reading its own version) fails the build instead of a user's cast.
"""

import multiprocessing
import sys

SELFTEST_MODULES = (
    "pyimg4",
    "pymobiledevice3.services.mobile_image_mounter",
    "pymobiledevice3.remote.rsd_tunnel",
    "pymobiledevice3.remote.userspace_tunnel",
    "pymobiledevice3.remote.tunnel_service",
    "pymobiledevice3.remote.core_device.display_service",
    "pymobiledevice3.remote.core_device.hid_service",
    "pymobiledevice3.remote.core_device.screen_stream",
    "pymobiledevice3.services.springboard",
    "pymobiledevice3.lockdown",
    "zenlink.apple.session",
    "zenlink.apple.hybrid",
    "zenlink.apple.wifi",
    "zenlink.apple.devices",
    "zenlink.airplay.receiver",
    "zenlink.usbdirect.session",
    "zenlink.castmanager",
    "zenlink.server",
)


def selftest(out_path: str) -> int:
    import importlib
    import json
    import traceback

    results = {}
    for name in SELFTEST_MODULES:
        try:
            importlib.import_module(name)
            results[name] = "ok"
        except BaseException:  # noqa: BLE001 - report everything
            results[name] = traceback.format_exc(limit=3)
    ok = all(v == "ok" for v in results.values())
    with open(out_path, "w", encoding="utf-8") as fh:
        json.dump({"ok": ok, "modules": results}, fh, indent=1)
    return 0 if ok else 1


if __name__ == "__main__":
    multiprocessing.freeze_support()
    if len(sys.argv) >= 3 and sys.argv[1] == "--selftest":
        sys.exit(selftest(sys.argv[2]))
    from zenlink.__main__ import main

    main()
