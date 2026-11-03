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
        # start()/stop() run on several threads (cast session, UI settings). Two
        # concurrent starts used to open two streams; the replaced one was freed
        # while PortAudio still called it -> access violation (app closed).
        self._control = threading.RLock()
        self._retired: list = []  # closed streams kept alive: PortAudio may still be finishing a callback
        self._buf = bytearray()
        self._stream = None
        self._started = False
        self.volume = 1.0
        self.muted = False
        self.packet = 0          # frames, largest recent packet
        self.margin = 128        # frames
        self.floor = 0           # frames: minimum margin (Wi-Fi jitter buffer)
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
        # Open the stream on a fresh thread: WASAPI needs to set up COM itself,
        # and the caller's thread (a cast session, the UI server) may already
        # carry COM state from other libraries. In the packaged app that made
        # WASAPI fail and fall back to MME (91 ms instead of ~23 ms).
        with self._control:
            result: list[bool] = []
            t = threading.Thread(target=lambda: result.append(self._start_locked()), name="audio-open", daemon=True)
            t.start()
            t.join(10)
            return bool(result and result[0])

    def _start_locked(self) -> bool:
        if not self.available:
            return False
        if self._stream is not None:
            return True
        import sounddevice as sd

        # WASAPI first (lowest latency). auto_convert lets Windows resample
        # when the speaker runs at 44.1 kHz instead of the device's 48 kHz;
        # without it the stream fails with "Invalid sample rate".
        attempts = []
        for api in sd.query_hostapis():
            if "WASAPI" in api["name"] and api["default_output_device"] >= 0:
                wasapi = api["default_output_device"]
                try:
                    attempts.append((wasapi, sd.WasapiSettings(auto_convert=True)))
                except TypeError:  # sounddevice < 0.5
                    pass
                attempts.append((wasapi, None))
                break
        attempts.append((None, None))  # system default output (MME)
        last = None
        for device, extra in attempts:
            try:
                stream = sd.RawOutputStream(
                    samplerate=RATE, channels=2, dtype="int16", device=device,
                    latency="low", callback=self._callback, extra_settings=extra,
                )
                stream.start()
            except Exception as error:  # noqa: BLE001
                log.info("native audio: %s (%s) not usable: %s", device, "auto-convert" if extra else "plain", error)
                last = error
                continue
            self._stream = stream
            api = sd.query_hostapis(sd.query_devices(stream.device)["hostapi"])["name"]
            log.info("native audio: %s via %s, output latency %.1f ms",
                     sd.query_devices(stream.device)["name"], api, stream.latency * 1000)
            return True
        log.warning("native audio could not start: %s", last)
        return False

    def stop(self) -> None:
        with self._control:
            stream, self._stream = self._stream, None
            self._retired.append(stream) if stream is not None else None
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

    def set_jitter_ms(self, ms: float) -> None:
        """Keep at least `ms` of sound queued. 0 for the cable (lowest latency);
        AirPlay over Wi-Fi needs ~100+ ms because packets arrive in bursts."""
        with self._lock:
            self.floor = int(RATE * ms / 1000)
            self.margin = max(self.margin, self.floor)

    def _target(self) -> int:
        cap = max(4800, self.floor + 2400)
        return min(cap, self.packet + max(self.margin, self.floor)) * BYTES_PER_FRAME

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
                    self.margin = min(max(1440, self.floor + 2400), self.margin + (480 if self.floor else 96))
                    self._calm_since = now
            if now - self._calm_since > 5 and self.margin > max(128, self.floor):
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
