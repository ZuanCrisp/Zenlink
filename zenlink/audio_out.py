"""Native low-latency sound output (WASAPI through PortAudio / sounddevice).

The browser's audio path adds ~50 ms on this kind of PC (baseLatency +
outputLatency); a native WASAPI shared-mode stream measured 22 ms. PCM from
either capture backend (48 kHz stereo s16le) is fed into a small ring buffer
that the audio callback drains:

* playback starts once one packet plus a margin is queued;
* an underrun adds 2 ms of margin (up to 30 ms), 5 s without one takes 1 ms off;
* anything queued beyond twice the target is dropped, so sound never trails.
"""

from __future__ import annotations

import array
import logging
import threading
import time

log = logging.getLogger(__name__)

RATE = 48000
BYTES_PER_FRAME = 4  # s16 stereo


class NativeAudio:
    def __init__(self) -> None:
        self._lock = threading.Lock()
        self._buf = bytearray()
        self._stream = None
        self._started = False
        self.volume = 1.0
        self.muted = False
        self.packet = 0          # frames, largest recent packet
        self.margin = 128        # frames
        self._calm_since = time.monotonic()
        self.underruns = 0
        self.dropped_ms = 0.0
        self.available = self._probe()

    @staticmethod
    def _probe() -> bool:
        try:
            import sounddevice  # noqa: F401
            return True
        except Exception as error:  # noqa: BLE001
            log.info("native audio unavailable: %s", error)
            return False

    # ------------------------------------------------------------ control

    def start(self) -> bool:
        if not self.available:
            return False
        if self._stream is not None:
            return True
        import sounddevice as sd

        try:
            device = None
            for index, api in enumerate(sd.query_hostapis()):
                if "WASAPI" in api["name"]:
                    device = api["default_output_device"]
                    break
            self._stream = sd.RawOutputStream(
                samplerate=RATE, channels=2, dtype="int16", device=device,
                latency="low", callback=self._callback,
            )
            self._stream.start()
            log.info("native audio: %s, output latency %.1f ms",
                     sd.query_devices(self._stream.device)["name"], self._stream.latency * 1000)
            return True
        except Exception as error:  # noqa: BLE001
            log.warning("native audio could not start: %s", error)
            self._stream = None
            return False

    def stop(self) -> None:
        stream, self._stream = self._stream, None
        if stream is not None:
            try:
                stream.stop()
                stream.close()
            except Exception:  # noqa: BLE001
                pass
        with self._lock:
            self._buf.clear()
            self._started = False

    @property
    def running(self) -> bool:
        return self._stream is not None

    # ------------------------------------------------------------ data

    def _target(self) -> int:
        return min(4800, self.packet + self.margin) * BYTES_PER_FRAME

    def feed(self, pcm: bytes) -> None:
        if self._stream is None or not pcm:
            return
        frames = len(pcm) // BYTES_PER_FRAME
        with self._lock:
            self.packet = max(frames, int(self.packet * 0.995))
            target = self._target()
            ceiling = target * 2 + 128 * BYTES_PER_FRAME
            if len(self._buf) + len(pcm) > ceiling:
                drop = min(len(self._buf), len(self._buf) + len(pcm) - target)
                drop -= drop % BYTES_PER_FRAME
                del self._buf[:drop]
                self.dropped_ms += drop / BYTES_PER_FRAME / RATE * 1000
            self._buf += pcm

    def _callback(self, outdata, frames, _time, _status) -> None:
        need = frames * BYTES_PER_FRAME
        now = time.monotonic()
        with self._lock:
            if not self._started and len(self._buf) >= self._target():
                self._started = True
            if self._started and len(self._buf) >= need:
                data = bytes(self._buf[:need])
                del self._buf[:need]
            else:
                data = None
                if self._started:
                    self._started = False
                    self.underruns += 1
                    self.margin = min(1440, self.margin + 96)
                    self._calm_since = now
            if now - self._calm_since > 5 and self.margin > 128:
                self.margin -= 48
                self._calm_since = now
        if data is None or self.muted or self.volume <= 0:
            outdata[:] = bytes(need)
            return
        if self.volume < 0.999:
            samples = array.array("h", data)
            v = self.volume
            for i in range(len(samples)):
                samples[i] = int(samples[i] * v)
            data = samples.tobytes()
        outdata[:] = data

    def stats(self) -> dict:
        with self._lock:
            queued = len(self._buf) / BYTES_PER_FRAME / RATE * 1000
        output = (self._stream.latency * 1000) if self._stream is not None else 0.0
        return {
            "native": True, "running": self.running,
            "queueMs": round(queued), "outputMs": round(output), "latencyMs": round(queued + output),
            "targetMs": round(self._target() / BYTES_PER_FRAME / RATE * 1000),
            "underruns": self.underruns, "droppedMs": round(self.dropped_ms),
        }


class AppleAudioPump:
    """Apple mode: read the stream server's PCM (/audio.bin) into NativeAudio."""

    def __init__(self, url: str, sink: NativeAudio) -> None:
        self.url = url
        self.sink = sink
        self._stop = threading.Event()
        self._thread = threading.Thread(target=self._run, name="apple-audio", daemon=True)

    def start(self) -> None:
        self._thread.start()

    def stop(self) -> None:
        self._stop.set()

    def _run(self) -> None:
        import http.client
        import urllib.parse

        u = urllib.parse.urlparse(self.url)
        while not self._stop.is_set():
            conn = None
            try:
                conn = http.client.HTTPConnection(u.hostname, u.port, timeout=5)
                conn.request("GET", u.path)
                resp = conn.getresponse()
                if resp.status != 200:
                    raise OSError(f"audio {resp.status}")
                buf = bytearray()
                while not self._stop.is_set():
                    chunk = resp.read1(65536)
                    if not chunk:
                        break
                    buf += chunk
                    while len(buf) >= 4:
                        n = int.from_bytes(buf[:4], "big")
                        if len(buf) < 4 + n:
                            break
                        self.sink.feed(bytes(buf[4:4 + n]))
                        del buf[:4 + n]
            except Exception as error:  # noqa: BLE001
                log.debug("apple audio pump: %s", error)
            finally:
                if conn is not None:
                    conn.close()
            self._stop.wait(1.0)
