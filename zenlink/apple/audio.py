"""AAC-ELD decoding on Windows for the CoreDevice audio stream.

pymobiledevice3 decodes the device's AAC-ELD with macOS AudioToolbox only.
FFmpeg (PyAV) decodes it here, provided the codec config says 480-sample
frames (the library's cookie leaves frameLengthFlag at 0) and the layout
is set explicitly. Verified on a captured iPad stream: every AU decodes.
"""

from __future__ import annotations

import sys


class FfmpegAacEldDecoder:
    def __init__(self, magic_cookie: bytes) -> None:
        import av

        self._av = av
        cookie = bytearray(magic_cookie)
        if len(cookie) >= 3:
            cookie[2] |= 0x10  # ELD frameLengthFlag = 1 (480 samples)
        self._ctx = av.codec.CodecContext.create("aac", "r")
        self._ctx.extradata = bytes(cookie)
        self._ctx.sample_rate = 48000
        self._ctx.layout = "stereo"
        self._ctx.open()
        self._resampler = av.AudioResampler(format="s16", layout="stereo", rate=48000)

    def decode(self, au: bytes) -> bytes:
        out = bytearray()
        for frame in self._ctx.decode(self._av.Packet(au)):
            for pcm in self._resampler.resample(frame):
                out += bytes(pcm.planes[0])[: pcm.samples * 4]
        return bytes(out)


def install(screen_stream_server_cls) -> None:
    if sys.platform == "darwin":
        return
    import pymobiledevice3.remote.core_device.screen_stream as screen_stream

    screen_stream.AACELDDecoder = FfmpegAacEldDecoder
    screen_stream_server_cls._missing_audio_deps = staticmethod(lambda: [])
