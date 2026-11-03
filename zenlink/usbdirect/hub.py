"""Media hub: the USB Direct backend's video/audio, served with the same
HTTP contract as the Apple backend's stream server, so the player, audio
path and UI are shared:

  GET /cast/codec       {"codec": <WebCodecs string>, "description": <base64>, ...}
  GET /cast/stream.bin  [u32 BE length][u8 type][access unit] ...
                        type 0 = keyframe, 1 = delta, 3 = new codec config (JSON)
  GET /cast/audio.bin   [u32 BE length][s16le stereo 48 kHz PCM] ...

The capture thread publishes; each HTTP viewer gets its own bounded queue.
A late viewer is primed with the current GOP (last keyframe onwards) so it
can start decoding at once. A viewer that falls behind is dropped to the
next keyframe instead of growing memory.
"""

from __future__ import annotations

import asyncio
import base64
import json
import logging
import threading
import time

from . import h264

log = logging.getLogger(__name__)

_MAX_GOP = 600          # frames kept for priming (~10 s at 60 fps)
_MAX_QUEUE = 45         # per-viewer backlog (~0.75 s) before it skips to a keyframe


def _frame(kind: int, payload: bytes) -> bytes:
    return (len(payload) + 1).to_bytes(4, "big") + bytes([kind]) + payload


def is_keyframe(avcc: bytes, hevc: bool) -> bool:
    """IDR/IRAP present in a length-prefixed access unit."""
    pos = 0
    n = len(avcc)
    while pos + 5 <= n:
        size = int.from_bytes(avcc[pos:pos + 4], "big")
        header = avcc[pos + 4]
        if hevc:
            if 16 <= (header >> 1) & 0x3F <= 21:
                return True
        elif header & 0x1F == 5:
            return True
        if size <= 0:
            break
        pos += 4 + size
    return False


def strip_sei(avcc: bytes, hevc: bool) -> bytes:
    """Drop SEI NAL units from a length-prefixed access unit. Apple's capture
    puts private SEI in every frame; TikTok's qcast erases it before decoding,
    and decoders may reject it (each rejection costs a resync)."""
    out = []
    pos, n, dropped = 0, len(avcc), False
    while pos + 5 <= n:
        size = int.from_bytes(avcc[pos:pos + 4], "big")
        end = pos + 4 + size
        if size <= 0 or end > n:
            return avcc  # not well-formed: leave it alone
        header = avcc[pos + 4]
        sei = ((header >> 1) & 0x3F) in (39, 40) if hevc else (header & 0x1F) == 6
        if sei:
            dropped = True
        else:
            out.append(avcc[pos:end])
        pos = end
    return b"".join(out) if dropped else avcc


class _Viewer:
    def __init__(self, loop: asyncio.AbstractEventLoop) -> None:
        self.loop = loop
        self.queue: asyncio.Queue = asyncio.Queue()
        self.need_key = False

    def put(self, item) -> None:
        self.loop.call_soon_threadsafe(self.queue.put_nowait, item)


class MediaHub:
    def __init__(self) -> None:
        self._lock = threading.Lock()
        self._codec: dict | None = None
        self._hevc = False
        self._gop: list[bytes] = []
        self._video: set[_Viewer] = set()
        self._audio: set[_Viewer] = set()
        self._closed = False
        self.audio_sink = None   # NativeAudio.feed while native sound plays
        self.stats = {"frames": 0, "bytes": 0, "audioBytes": 0, "keyframes": 0, "started": time.monotonic()}

    # ------------------------------------------------------------ producer

    def set_codec(self, codec: str, config: bytes, width: int, height: int, name: str) -> None:
        if name == "H.264":
            # Declare "no reordering" so the decoder outputs each frame at once
            # (see h264.py); the stream itself is unchanged.
            try:
                size = int.from_bytes(config[6:8], "big")
                log.info("usb direct: device SPS %s", h264.sps_info(config[8:8 + size]))
            except (ValueError, IndexError) as error:
                log.info("usb direct: device SPS not parsed: %s", error)
            config = h264.rewrite_avcc(config)
        info = {
            "codec": codec,
            "description": base64.b64encode(config).decode("ascii"),
            "width": width,
            "height": height,
            "codecName": name,
            "source": "usbdirect",
        }
        with self._lock:
            if info == self._codec:
                return
            changed = self._codec is not None
            self._codec = info
            self._hevc = name == "HEVC"
            self._gop = []
            viewers = list(self._video)
        if changed:
            msg = _frame(3, json.dumps(info).encode())
            for v in viewers:
                v.need_key = True
                v.put(msg)

    def push_video(self, avcc: bytes) -> None:
        with self._lock:
            if self._codec is None or self._closed:
                return
            avcc = strip_sei(avcc, self._hevc)
            if not avcc:
                return
            if not self._hevc:
                avcc = h264.rewrite_access_unit(avcc)
            key = is_keyframe(avcc, self._hevc)
            msg = _frame(0 if key else 1, avcc)
            if key:
                self._gop = [msg]
                self.stats["keyframes"] += 1
            elif self._gop and len(self._gop) < _MAX_GOP:
                self._gop.append(msg)
            self.stats["frames"] += 1
            self.stats["bytes"] += len(avcc)
            viewers = list(self._video)
        for v in viewers:
            if v.need_key:
                if not key:
                    continue
                v.need_key = False
            if v.queue.qsize() > _MAX_QUEUE:
                v.need_key = True  # behind: wait for the next keyframe
                continue
            v.put(msg)

    def push_audio(self, pcm: bytes) -> None:
        if not pcm:
            return
        sink = self.audio_sink
        if sink is not None:
            sink(pcm)
        msg = len(pcm).to_bytes(4, "big") + pcm
        with self._lock:
            self.stats["audioBytes"] += len(pcm)
            viewers = list(self._audio)
        for v in viewers:
            if v.queue.qsize() < 400:
                v.put(msg)

    def max_backlog(self) -> int:
        with self._lock:
            return max((v.queue.qsize() for v in self._video), default=0)

    def close(self) -> None:
        with self._lock:
            self._closed = True
            viewers = list(self._video) + list(self._audio)
        for v in viewers:
            v.put(None)

    # ------------------------------------------------------------ viewers

    def codec(self) -> dict | None:
        return self._codec

    def open_video(self, loop) -> _Viewer:
        viewer = _Viewer(loop)
        with self._lock:
            if self._closed:
                viewer.queue.put_nowait(None)
                return viewer
            for msg in self._gop:
                viewer.queue.put_nowait(msg)
            viewer.need_key = not self._gop
            self._video.add(viewer)
        return viewer

    def open_audio(self, loop) -> _Viewer:
        viewer = _Viewer(loop)
        with self._lock:
            if self._closed:
                viewer.queue.put_nowait(None)
            self._audio.add(viewer)
        return viewer

    def close_viewer(self, viewer: _Viewer) -> None:
        with self._lock:
            self._video.discard(viewer)
            self._audio.discard(viewer)
