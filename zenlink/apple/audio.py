"""AAC-ELD decoding on Windows for the CoreDevice audio stream.

pymobiledevice3 decodes the device's AAC-ELD with macOS AudioToolbox only.
FFmpeg's AAC decoder (zlaac.dll, zenlink/aac.py) decodes it here, provided
the codec config says 480-sample frames (the library's cookie leaves
frameLengthFlag at 0) and the layout is set explicitly. Verified on a
captured iPad stream (with PyAV, same decoder): every AU decodes.
"""

from __future__ import annotations

import sys

from zenlink.aac import AacDecoder


class AacEldDecoder:
    def __init__(self, magic_cookie: bytes) -> None:
        cookie = bytearray(magic_cookie)
        if len(cookie) >= 3:
            cookie[2] |= 0x10  # ELD frameLengthFlag = 1 (480 samples)
        self._dec = AacDecoder(bytes(cookie), 48000, channels=2, out_rate=48000)

    def decode(self, au: bytes) -> bytes:
        return self._dec.decode(au)


def install(screen_stream_server_cls) -> None:
    if sys.platform == "darwin":
        return
    import pymobiledevice3.remote.core_device.screen_stream as screen_stream

    screen_stream.AACELDDecoder = AacEldDecoder
    screen_stream_server_cls._missing_audio_deps = staticmethod(lambda: [])
