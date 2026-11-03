"""Video quality presets on top of pymobiledevice3's own offer builder.

The CoreDevice display service negotiates the stream from a "mediaBlob" the
host sends. Measured on an iPad 11th gen (iOS 27):

* the encoder picks its own rate, ~4-6 Mbps at most, whatever the host offers;
* bitrate tiers (f9) are network *caps*: lowering them works (3 Mbps cap ->
  ~2.3 Mbps, 1 Mbps -> ~1.0 Mbps); raising them above Apple's table does not
  raise quality and at 150 Mbps the device fell back to ~0.3 Mbps;
* customVideoWidth/Height (f4/f5) are ignored (same SPS, same size).

So "Best graphics" uses the library's builder untouched, byte-for-byte what
upstream pane-iphone-mirror-windows and Xcode send, and the lighter presets
only lower the caps.
"""

from __future__ import annotations

from pymobiledevice3.remote.core_device import display_service
from pymobiledevice3.remote.core_device import media_stream_offer as mso

# Bitrate cap per preset (bps); None = Apple's own table.
PRESETS = {"best": None, "normal": 4_000_000, "performance": 2_000_000}

_ORIGINAL = mso.build_negotiator_offer_video


def _capped_tiers(cap: int) -> tuple:
    return tuple(
        (kind, min(bps, cap) if kind == 0 else bps, buf) for kind, bps, buf in mso._DEFAULT_VIDEO_BITRATE_TIERS
    )


def install(mode: str) -> None:
    """Select the offer for the next stream start."""
    cap = PRESETS.get(mode)
    if not cap:
        display_service.build_negotiator_offer_video = _ORIGINAL
        mso.build_media_blob_video.__kwdefaults__["bitrate_tiers"] = mso._DEFAULT_VIDEO_BITRATE_TIERS
        return
    mso.build_media_blob_video.__kwdefaults__["bitrate_tiers"] = _capped_tiers(cap)
    display_service.build_negotiator_offer_video = _ORIGINAL
