"""What ZenLink can do with a device: picture, control, or both.

Two steps, so the UI never claims more than was checked:

* ``expected(...)``: before casting, from the chosen mode and the iOS version
  the device reports. Only says which backend will be tried.
* ``verified(...)``: during a session, from the device's own service list
  (CoreDevice RSD) and whether the input path actually opened.

Levels shown to the user:

* ``native``      - Apple Driver mode, iOS 27+: video and control through
                    CoreDevice DisplayService (tested on real devices).
* ``hybrid``      - Apple Driver mode, iOS 17-26: video and sound over AirPlay,
                    touch / keyboard / buttons through CoreDevice HID over the
                    Apple driver. EXPERIMENTAL: not yet confirmed on a real
                    device by ZenLink's own tests.
* ``mirroring``   - picture and sound only (ZenLink Studio, or Hybrid whose
                    input path could not be opened).
* ``unsupported`` - Apple Driver mode on iOS 16 or older (no CoreDevice
                    tunnel): use ZenLink Studio.
"""

from __future__ import annotations

NATIVE_MIN_IOS = 27   # dtremotedisplayd sends frames from iOS 27
HYBRID_MIN_IOS = 17   # CoreDevice (RSD tunnel, personalized DDI) from iOS 17

# CoreDevice services each path needs (names from the device's RSD list).
DISPLAY_SERVICE = "com.apple.coredevice.displayservice"
UHID_SERVICE = "com.apple.coredevice.hid.universalhidservice"
INDIGO_SERVICE = "com.apple.coredevice.hid.indigo"
HYBRID_SERVICES = (DISPLAY_SERVICE, UHID_SERVICE, INDIGO_SERVICE)

LEVELS = ("native", "hybrid", "mirroring", "unsupported")
EXPERIMENTAL = {"hybrid"}


def major(version: str | None) -> int | None:
    try:
        return int(str(version).split(".")[0])
    except (TypeError, ValueError):
        return None


def expected(mode: str | None, version: str | None) -> str | None:
    """Level ZenLink will try for this device; None when it can't tell yet."""
    if mode == "usbdirect":
        return "mirroring"
    if mode != "apple":
        return None
    v = major(version)
    if v is None:
        return None
    if v >= NATIVE_MIN_IOS:
        return "native"
    if v >= HYBRID_MIN_IOS:
        return "hybrid"
    return "unsupported"


def missing_hybrid_services(services) -> list[str]:
    """CoreDevice services Hybrid control needs that the device doesn't offer."""
    names = set(services or ())
    return [s for s in HYBRID_SERVICES if s not in names]


def verified(level: str, *, video: bool, control: bool, note: str = "") -> dict:
    """Session result for the UI: what was actually opened, not assumed."""
    return {
        "level": level,
        "video": bool(video),
        "control": bool(control),
        "experimental": level in EXPERIMENTAL,
        "note": note,
    }


def upright_to_native(x: int, y: int, rotation: int | None) -> tuple[int, int]:
    """Map a touch on an upright picture (AirPlay mirrors the screen already
    turned) to the device's portrait touch space, both 0..65535.

    ``rotation`` is the clockwise turn that makes the portrait buffer upright
    (the same value Apple mode draws with); the mapping is the one
    ui/player.js applies for a rotated Apple-mode picture.
    """
    u, v = x / 65535, y / 65535
    if rotation == 90:
        u, v = v, 1 - u
    elif rotation == 180:
        u, v = 1 - u, 1 - v
    elif rotation == 270:
        u, v = 1 - v, u
    clamp = lambda n: max(0, min(65535, round(n * 65535)))  # noqa: E731
    return clamp(u), clamp(v)
