"""Entry point for the standalone (PyInstaller) build.

``ZenLink.exe --selftest <file>`` imports every module the device paths load
on demand (devices, sound, USB, drivers) and writes
the result to <file> as JSON, then exits. The build runs it on the finished
exe, so a package that's missing from the bundle fails the build instead of a user's cast.
"""

import multiprocessing
import sys

SELFTEST_MODULES = (
    "zenlink.apple.devices",
    "zenlink.usbdirect.session",
    "zenlink.castmanager",
    "zenlink.server",
    # Sound (PortAudio loads at import), USB Direct.
    "sounddevice",
    "zenlink.audio_out",
    "usb.core",
    "usb.backend.libusb0",
    "zenlink.firewall",
    "zenlink.drivers.status",
    "zenlink.drivers.manager",
    "zenlink.__main__",
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
    # zlaac.dll (sound decoder) must load and decode inside the frozen exe.
    try:
        from zenlink.aac import AacDecoder, avcodec_version

        avcodec_version()
        AacDecoder(bytes.fromhex("f8e85000"), 44100).close()
        results["zenlink.aac (zlaac.dll)"] = "ok"
    except BaseException:  # noqa: BLE001
        results["zenlink.aac (zlaac.dll)"] = traceback.format_exc(limit=3)
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
