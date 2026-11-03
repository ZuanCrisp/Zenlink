"""USB Direct capture backend: Apple's QuickTime screen-capture USB protocol.

Flow (ZenLink's own implementation; protocol notes from
danielpaulus/quicktime_video_hack, MIT):

1. open the device through libusb0.dll. That needs the per-device libusb0
   upper filter on the composite parent (Apple's drivers stay on their
   interfaces) or a parent bound to libusb0. WinUSB/libusbK can't select a
   non-default configuration, so they are not used;
2. if the device doesn't offer the capture configuration yet, send the vendor
   request (0x40, 0x52, wIndex 2) and wait for it to re-enumerate;
3. select the capture configuration, claim the vendor interface (class 0xFF,
   subclass 0x2A) and clear both endpoint halts;
4. answer the device's handshake (PING, CWPA/AFMT/CVRP/CLOK/TIME/SKEW/GO!)
   and receive FEED (video CMSampleBuffers) and EAT! (48 kHz PCM);
5. on stop: close the streams (HPA0/HPD0), switch the device back with the
   disable request (wIndex 0) and restore its normal configuration, so Apple
   services and the next session find it in its normal state.

With libusb0 as the device driver Apple Mobile Device Service is out of the
picture, so ZenLink proves trust itself the way TikTok LIVE Studio's qcast
does ("QMirror_Trust"): in the capture configuration it opens usbmux on the
mux interface, starts a TLS lockdown session with the Windows pair record,
stops it, and releases the interface. Without that the device stays silent.

The codec is whatever the device's format description says (normally H.264
on this path); nothing assumes HEVC.
"""

from __future__ import annotations

import logging
import os
import queue
import struct
import threading
import time
from typing import Callable

import usb.backend.libusb0
import usb.core
import usb.util

from zenlink.drivers import status as driver_status

from zenlink.apple import models

from . import lockdown, usbmux, wire
from .hub import MediaHub

log = logging.getLogger(__name__)

APPLE_VID = 0x05AC
QT_SUBCLASS = 0x2A
MUX_SUBCLASS = 0xFE
VENDOR_CLASS = 0xFF
READ_SIZE = 0x10000  # one libusb0 request per call (qcast uses 16 KiB from C++; Python needs fewer calls)
READ_TIMEOUT_MS = 100  # qcast recv_thread_func: 100 ms
_backend = None


class UsbDirectError(RuntimeError):
    pass


def backend():
    """libusb-win32 user-mode library from System32 (pairs with libusb0.sys)."""
    global _backend
    if _backend is None:
        # libusb0.dll is installed to System32 together with the driver.
        dll = driver_status.libusb0_status()["dllPath"]
        if not os.path.exists(dll):
            raise UsbDirectError("libusb0.dll is not installed. Open Settings > Connection & Drivers.")
        _backend = usb.backend.libusb0.get_backend(find_library=lambda _name: dll)
        if _backend is None:
            raise UsbDirectError("libusb0.dll could not be loaded.")
    return _backend


def _configs(dev) -> tuple[int | None, int | None]:
    mux = qt = None
    for cfg in dev:
        kinds = {(i.bInterfaceClass, i.bInterfaceSubClass) for i in cfg}
        if (VENDOR_CLASS, QT_SUBCLASS) in kinds:
            qt = cfg.bConfigurationValue
        elif (VENDOR_CLASS, MUX_SUBCLASS) in kinds and mux is None:
            mux = cfg.bConfigurationValue
    return mux, qt


def _serial(dev) -> str:
    try:
        # libusb0 may pad the serial string with spaces or NULs.
        return (usb.util.get_string(dev, dev.iSerialNumber) or "").strip("\x00 \t")
    except (usb.core.USBError, ValueError, NotImplementedError):
        return ""


def find(serial: str):
    want = serial.replace("-", "").upper()
    for dev in usb.core.find(find_all=True, idVendor=APPLE_VID, backend=backend()) or []:
        if _serial(dev).replace("-", "").upper() == want:
            return dev
    return None


def _is_timeout(error: usb.core.USBError) -> bool:
    return isinstance(error, usb.core.USBTimeoutError) or "timeout" in str(error).lower() or error.errno == 116


class HostClock:
    """A CMClock running on host time, in nanoseconds."""

    def __init__(self, clock_id: int) -> None:
        self.id = clock_id
        self._start = time.perf_counter_ns()

    def now(self) -> wire.CMTime:
        return wire.CMTime(time.perf_counter_ns() - self._start, 1_000_000_000, 1, 0)


class UsbDirectSession:
    """One capture session for one device. run() blocks; stop() ends it."""

    def __init__(self, serial: str, hub: MediaHub, on_state: Callable[[str, str], None],
                 display_size: tuple[int, int] = (3840, 2160)) -> None:
        self.serial = serial
        self.hub = hub
        self.on_state = on_state
        self.display_size = display_size
        self._stop = threading.Event()
        self._released = threading.Event()
        self._write_lock = threading.Lock()
        self._dev = None
        self._iface = None
        self._ep_in = None
        self._ep_out = None
        self._mux_cfg: int | None = None
        self._need = b""
        self._clock: HostClock | None = None
        self._audio_clock: HostClock | None = None
        self._device_audio_clock = 0
        self._audio_first: tuple[wire.CMTime, wire.CMTime] | None = None
        self._audio_last: tuple[wire.CMTime, wire.CMTime] | None = None
        self._pinged = False
        self._streaming = False
        self._mux = None
        self._mux_iface = None
        self._newly_activated = False
        self.info: dict = {"codec": None, "width": 0, "height": 0, "audio": None}

    # ------------------------------------------------------------ control

    def stop(self) -> None:
        self._stop.set()

    def run(self) -> None:
        try:
            self._activate()
            self._loop()
        finally:
            self._close_stream()
            self._deactivate()
            self.hub.close()

    # ------------------------------------------------------------ USB setup

    def _check_driver_stack(self) -> None:
        """Only the TikTok-style stack is safe: libusb0 as the parent's device
        driver. A libusb0 filter on Apple's composite driver left an iPad stuck
        and crashed Windows when the capture configuration was selected."""
        want = self.serial.replace("-", "").upper()
        entry = next((d for d in driver_status.device_states() if d["serial"].upper() == want), None)
        if entry is None:
            raise UsbDirectError("The device is not connected.")
        if entry["mode"] != "replaced" or entry["unsafeFilter"]:
            raise UsbDirectError(
                "USB Direct needs the libusb0 driver installed as the device's driver (no filters). "
                "Open Settings > Connection & Drivers."
            )

    def _activate(self, force: bool = False) -> None:
        """TikTok's qcast order (from its own log of a working session):
        normal mode (4 configurations) -> capture request -> the device
        reconnects with 5 -> trust over usbmux -> capture interface -> PING.
        A device still left in capture mode by an earlier attempt stays
        silent, so it is first switched back to normal and then enabled once."""
        self._check_driver_stack()
        dev = find(self.serial)
        if dev is None:
            raise UsbDirectError(
                "ZenLink can't open the device through libusb0. Open Settings > Connection & Drivers "
                "and install the USB Direct driver, then reconnect the cable."
            )
        self._mux_cfg, qt = _configs(dev)
        self.on_state("activating", "Switching the device to screen capture")
        adopt = False
        if qt is not None:
            log.info("usb direct: device is still in capture mode; switching it back to normal first")
            self._capture_request(dev, enable=False)
            try:
                dev = self._wait_reconnect(want_capture=False, patience=6.0)
            except UsbDirectError:
                # It kept its capture configuration: use it as it is (claim,
                # trust, capture module) instead of failing.
                log.info("usb direct: device stayed in capture mode; adopting it")
                adopt = True
                dev = find(self.serial)
                if dev is None:
                    raise UsbDirectError("The device disappeared. Plug it in again and retry.") from None
            self._mux_cfg, qt = _configs(dev)
        if not adopt:
            log.info("usb direct: requesting capture mode (%d configurations)", dev.bNumConfigurations)
            self._capture_request(dev, enable=True)
            dev = self._wait_reconnect(want_capture=True)
            self._mux_cfg, qt = _configs(dev)
        if qt is None:
            raise UsbDirectError("The device did not offer its screen-capture configuration. Unlock it and try again.")
        log.info("usb direct: capture mode on (%d configurations, capture config %s)", dev.bNumConfigurations, qt)

        # qcast QMirror_Init/Trust: capture configuration, lockdown session over
        # usbmux, then the mux interface is released and the handle closed.
        try:
            if self._active_config(dev) != qt:
                dev.set_configuration(qt)
            self._prove_trust(dev, dev.get_active_configuration())
        except usb.core.USBError as error:
            raise UsbDirectError(f"Trust check over USB failed ({error}). Unplug the device and retry.") from error
        usb.util.dispose_resources(dev)

        # qcast quicktime_libusb0_impl::start (decompiled): open a fresh handle,
        # SET_CONFIGURATION(capture), claim the capture interface, clear the IN
        # and OUT halts, then start reading 16 KiB with a 100 ms timeout.
        dev = find(self.serial)
        if dev is None:
            raise UsbDirectError("The device disappeared. Plug it in again and retry.")
        try:
            dev.set_configuration(qt)
        except usb.core.USBError as error:
            if self._active_config(dev) != qt:
                raise UsbDirectError(
                    f"Windows refused to select the capture configuration ({error}). Unplug the device, "
                    "plug it in again and retry."
                ) from error
            log.info("usb direct: capture configuration already active (%s)", error)
        cfg = dev.get_active_configuration()
        iface = next((i for i in cfg if i.bInterfaceClass == VENDOR_CLASS and i.bInterfaceSubClass == QT_SUBCLASS), None)
        if iface is None:
            raise UsbDirectError("The capture interface is missing from the capture configuration.")
        try:
            usb.util.claim_interface(dev, iface.bInterfaceNumber)
        except usb.core.USBError as error:
            raise UsbDirectError(f"Windows refused to open the capture interface ({error}). Unplug the device and retry.") from error
        is_in = lambda e: usb.util.endpoint_direction(e.bEndpointAddress) == usb.util.ENDPOINT_IN  # noqa: E731
        ep_in = usb.util.find_descriptor(iface, custom_match=is_in)
        ep_out = usb.util.find_descriptor(iface, custom_match=lambda e: not is_in(e))
        if ep_in is None or ep_out is None:
            raise UsbDirectError("The capture interface has no bulk endpoints.")
        for ep in (ep_in, ep_out):
            try:
                dev.clear_halt(ep)
            except usb.core.USBError as error:
                log.info("usb direct: clear halt 0x%02x: %s", ep.bEndpointAddress, error)
        self._dev, self._iface, self._ep_in, self._ep_out = dev, iface, ep_in, ep_out
        log.info("usb direct: capture module started: config %s, interface %s, in=0x%02x out=0x%02x",
                 qt, iface.bInterfaceNumber, ep_in.bEndpointAddress, ep_out.bEndpointAddress)

    @staticmethod
    def _active_config(dev):
        try:
            return dev.get_active_configuration().bConfigurationValue
        except usb.core.USBError:
            return None

    def _prove_trust(self, dev, cfg) -> None:
        """Lockdown StartSession (TLS) then StopSession over usbmux on the
        capture configuration's mux interface, then release it."""
        mux_iface = next((i for i in cfg if i.bInterfaceClass == VENDOR_CLASS and i.bInterfaceSubClass == MUX_SUBCLASS), None)
        if mux_iface is None:
            log.warning("usb direct: no usbmux interface in the capture configuration")
            return
        self.on_state("activating", "Checking that this PC is trusted")
        udid = self.serial
        mux = None
        keep = False
        try:
            usb.util.claim_interface(dev, mux_iface.bInterfaceNumber)
            ep_in = usb.util.find_descriptor(mux_iface, custom_match=lambda e: e.bEndpointAddress & 0x80)
            ep_out = usb.util.find_descriptor(mux_iface, custom_match=lambda e: not e.bEndpointAddress & 0x80)
            mux = usbmux.UsbMux(ep_in, ep_out)
            mux.start()
            stream = mux.connect(usbmux.LOCKDOWN_PORT)
            client = lockdown.LockdownClient(stream)
            client.query_type()
            try:
                product = client.get_value("ProductType")
                if product:
                    models.remember_product(udid, str(product))
            except lockdown.LockdownError:
                pass
            record = lockdown.load_pair_record(udid)
            if record is None:
                raise UsbDirectError(
                    "This PC isn't trusted by the device yet. Use Apple mode once (or iTunes / Apple Devices), "
                    "tap Trust on the device, then switch back to USB Direct."
                )
            client.start_session(record)
            client.stop_session()
            stream.close()
            log.info("usb direct: trust confirmed (lockdown session started and stopped)")
            # qcast: LD_Disconnect before the capture module starts.
        except (usbmux.MuxError, lockdown.LockdownError) as error:
            text = str(error)
            log.warning("usb direct: lockdown session failed: %s", text)
            if "PasswordProtected" in text or "DeviceLocked" in text:
                raise UsbDirectError("Unlock the device and try again.") from error
            if "InvalidHostID" in text or "Pair" in text:
                raise UsbDirectError(
                    "The device doesn't trust this PC anymore. Use Apple mode once and tap Trust, then try again."
                ) from error
        finally:
            if not keep:
                if mux is not None:
                    mux.stop()
                    time.sleep(0.25)  # let the mux reader finish its last read
                try:
                    usb.util.release_interface(dev, mux_iface.bInterfaceNumber)
                except usb.core.USBError:
                    pass

    def _stop_mux(self, dev) -> None:
        mux, iface = self._mux, self._mux_iface
        self._mux = self._mux_iface = None
        if mux is not None:
            try:
                mux.stop()
            except Exception:  # noqa: BLE001
                pass
            time.sleep(0.25)
        if dev is not None and iface is not None:
            try:
                usb.util.release_interface(dev, iface)
            except usb.core.USBError:
                pass

    def _capture_request(self, dev, enable: bool) -> None:
        """Vendor request 0x52: wIndex 2 turns capture mode on, 0 turns it off.
        The device drops off the bus right after, so an error here is normal."""
        try:
            dev.ctrl_transfer(0x40, 0x52, 0x00, 0x02 if enable else 0x00, b"", timeout=1000)
        except usb.core.USBError as error:
            log.debug("capture request (%s): %s (expected while the device reconnects)", enable, error)
        usb.util.dispose_resources(dev)

    def _wait_reconnect(self, want_capture: bool, patience: float = 15.0):
        """Wait for the device to drop off and come back in the wanted mode."""
        deadline = time.monotonic() + 4.0
        while time.monotonic() < deadline and not self._stop.is_set():
            if find(self.serial) is None:
                break
            time.sleep(0.1)
        deadline = time.monotonic() + patience
        while time.monotonic() < deadline and not self._stop.is_set():
            dev = find(self.serial)
            if dev is not None and (_configs(dev)[1] is not None) == want_capture:
                time.sleep(0.8)  # let Windows finish binding the driver
                return find(self.serial) or dev
            time.sleep(0.25)
        if self._stop.is_set():
            raise UsbDirectError("Stopped.")
        raise UsbDirectError(
            "The device did not come back in screen-capture mode. Unplug it, plug it in again, unlock it and retry."
            if want_capture else
            "The device did not leave screen-capture mode. Unplug it, plug it in again and retry."
        )

    def _deactivate(self) -> None:
        dev = self._dev
        self._dev = None
        if dev is None:
            return
        self._stop_mux(dev)
        try:
            usb.util.release_interface(dev, self._iface.bInterfaceNumber)
        except (usb.core.USBError, AttributeError):
            pass
        try:
            dev.ctrl_transfer(0x40, 0x52, 0x00, 0x00, b"", timeout=1000)
        except usb.core.USBError as error:
            log.debug("disable capture request: %s", error)
        if self._mux_cfg is not None:
            try:
                dev.set_configuration(self._mux_cfg)
            except usb.core.USBError as error:
                log.debug("restore normal configuration: %s", error)
        usb.util.dispose_resources(dev)
        log.info("usb direct: device restored to its normal configuration")

    def _release_capture(self) -> None:
        """Let go of the interfaces but leave the mode switch to _activate."""
        dev = self._dev
        self._dev = None
        if dev is None:
            return
        self._stop_mux(dev)
        try:
            usb.util.release_interface(dev, self._iface.bInterfaceNumber)
        except (usb.core.USBError, AttributeError):
            pass
        usb.util.dispose_resources(dev)
        self._ep_in = self._ep_out = None

    def _write(self, data: bytes, timeout: int = 2000) -> None:
        with self._write_lock:
            self._ep_out.write(data, timeout=timeout)

    def _try_write(self, data: bytes, timeout: int = 100) -> bool:
        try:
            self._write(data, timeout=timeout)
            return True
        except usb.core.USBError as error:
            if _is_timeout(error):
                return False
            raise

    # ------------------------------------------------------------ receive loop

    # Like qcast's qt_recv / qt_protocol threads: one thread only reads USB so a
    # read is always pending; this thread parses and answers. A single thread
    # that also parsed left the device waiting between reads, so frames came in
    # bursts (stutter) and queued up (latency).
    _TIMEOUT = object()

    def _start_reader(self) -> None:
        self._chunks: queue.Queue = queue.Queue()
        self._reading = True
        self._reader = threading.Thread(target=self._read_usb, name="usbdirect-read", daemon=True)
        self._reader.start()

    def _stop_reader(self) -> None:
        self._reading = False
        reader = getattr(self, "_reader", None)
        if reader is not None and reader is not threading.current_thread():
            reader.join(1.0)
        self._reader = None

    def _read_usb(self) -> None:
        ep, chunks = self._ep_in, self._chunks
        while self._reading and not self._stop.is_set():
            try:
                data = ep.read(READ_SIZE, timeout=READ_TIMEOUT_MS)
            except usb.core.USBError as error:
                if _is_timeout(error):
                    chunks.put(self._TIMEOUT)
                    continue
                chunks.put(error)
                return
            except Exception as error:  # noqa: BLE001
                chunks.put(error)
                return
            chunks.put(data.tobytes())

    def _loop(self) -> None:
        self.on_state("waiting", "Unlock the device and keep it unlocked")
        buf = bytearray()
        received = 0
        silent_since = time.monotonic()
        retried = False
        stats_at = time.monotonic()
        stats = {"chunks": 0, "bytes": 0, "frames": 0, "maxq": 0}
        self._start_reader()
        try:
            while not self._stop.is_set():
                try:
                    item = self._chunks.get(timeout=0.5)
                except queue.Empty:
                    item = self._TIMEOUT
                now = time.monotonic()
                if now - stats_at >= 5.0 and received:
                    secs = now - stats_at
                    frames = self.hub.stats["frames"] - stats["frames"]
                    log.info("usb direct: %.1f fps, %.1f Mbps, %d reads/s, reader backlog max %d, keyframes %d, viewers backlog %d",
                             frames / secs, (received - stats["bytes"]) * 8 / secs / 1e6,
                             (stats["chunks"]) / secs, stats["maxq"], self.hub.stats["keyframes"], self.hub.max_backlog())
                    stats = {"chunks": 0, "bytes": received, "frames": self.hub.stats["frames"], "maxq": 0}
                    stats_at = now
                if isinstance(item, BaseException):
                    raise item
                if item is self._TIMEOUT:
                    if received == 0 and not retried and now - silent_since > 4.0:
                        # A device left in capture mode by an interrupted session stays
                        # silent: switch capture mode off and on once.
                        retried = True
                        log.info("usb direct: no data; switching capture mode off and on once")
                        self._stop_reader()
                        self._release_capture()
                        self._activate()
                        self.on_state("waiting", "Unlock the device and keep it unlocked")
                        self._pinged, silent_since = False, time.monotonic()
                        self._start_reader()
                        continue
                    if received == 0 and retried and now - silent_since > 10.0:
                        raise UsbDirectError(
                            "The device opened screen capture but sent nothing. Unlock it, keep it on the "
                            "home screen, then press Start casting again."
                        )
                    if not self._pinged:
                        # qcast: one active PING on the first read timeout, never repeated.
                        ok = self._try_write(wire.ping_active())
                        self._pinged = True
                        log.info("usb direct: active PING %s", "sent" if ok else "not taken")
                    continue
                chunk = item
                stats["chunks"] += 1
                stats["maxq"] = max(stats["maxq"], self._chunks.qsize())
                if received == 0:
                    log.info("usb direct: first %d bytes from the device: %s", len(chunk), chunk[:16].hex())
                received += len(chunk)
                buf += chunk
                while len(buf) >= 8:
                    length = int.from_bytes(buf[0:4], "little")
                    if length < 8 or length > 64 * 1024 * 1024:
                        raise UsbDirectError(f"Corrupt packet length {length} from the device.")
                    if len(buf) < length:
                        break
                    packet = bytes(buf[4:length])
                    del buf[:length]
                    self._handle(packet)
        except usb.core.USBError as error:
            if not self._stop.is_set():
                raise UsbDirectError(f"USB connection lost: {error}") from error
        finally:
            self._stop_reader()

    def _close_stream(self) -> None:
        if self._ep_out is None or self._dev is None:
            return
        try:
            self._write(wire.asyn(wire.HPA0, self._device_audio_clock))
            self._write(wire.asyn(wire.HPD0, wire.EMPTY_CF))
            self._released.wait(1.0)
            self._write(wire.asyn(wire.HPD0, wire.EMPTY_CF))
        except usb.core.USBError:
            pass

    def _handle(self, p: bytes) -> None:
        kind = wire.u32(p)
        if kind == wire.PING:
            if not self._pinged:
                self._write(wire.ping())
                self._pinged = True
            self.on_state("handshake", "Device answered, setting up the stream")
        elif kind == wire.SYNC:
            self._handle_sync(p)
        elif kind == wire.ASYN:
            self._handle_asyn(p)
        else:
            log.debug("usb direct: unknown packet %r", p[:4])

    def _handle_sync(self, p: bytes) -> None:
        sub = wire.u32(p, 12)
        correlation = wire.u64(p, 16)
        body = p[24:]
        if sub == wire.CWPA:
            device_clock = wire.u64(body)
            self._device_audio_clock = device_clock
            self._audio_clock = HostClock(device_clock + 1000)
            w, h = self.display_size
            self._write(wire.hpd1(w, h))
            self._write(wire.hpd1(w, h))
            self._write(wire.reply_clock(correlation, self._audio_clock.id))
            self._write(wire.hpa1(device_clock))
        elif sub == wire.CVRP:
            device_clock = wire.u64(body)
            self._need = wire.need(device_clock)
            self._write(self._need)
            self._write(wire.reply_clock(correlation, device_clock + 0x1000AF))
        elif sub == wire.CLOK:
            clock_ref = wire.u64(p, 4) + 0x10000
            self._clock = HostClock(clock_ref)
            self._write(wire.reply_clock(correlation, clock_ref))
        elif sub == wire.TIME:
            self._write(wire.reply_time(correlation, (self._clock or HostClock(0)).now()))
        elif sub == wire.AFMT:
            self._write(wire.reply_dict(correlation, {"Error": wire.n32(0)}))
        elif sub == wire.SKEW:
            self._write(wire.reply_skew(correlation, self._skew()))
        elif sub == wire.OG:
            self._write(wire.reply_empty(correlation))
        elif sub == wire.STOP:
            self._write(wire.reply_empty(correlation))
        else:
            log.debug("usb direct: unknown sync %r", p[12:16])

    def _handle_asyn(self, p: bytes) -> None:
        sub = wire.u32(p, 12)
        if sub == wire.FEED:
            # Ask for the next frame first: the device waits for NEED before it
            # sends another one, so every millisecond spent parsing here would
            # otherwise come off the frame rate.
            if self._need:
                self._write(self._need)
            try:
                sample = wire.SampleBuffer.parse(p[16:], wire.MEDIA_VIDEO)
            except (ValueError, IndexError, struct.error) as error:
                log.debug("usb direct: bad FEED: %s", error)
                sample = None
            if sample is not None:
                fmt = sample.format
                if fmt is not None and fmt.config:
                    self.info.update(codec=fmt.codec_name, width=fmt.width, height=fmt.height)
                    self.hub.set_codec(fmt.codec_string, fmt.config, fmt.width, fmt.height, fmt.codec_name)
                if sample.data:
                    self.hub.push_video(sample.data)
                    if not self._streaming and self.hub.codec():
                        self._streaming = True
                        log.info("usb direct: first frame, %s %sx%s", self.info["codec"], self.info["width"], self.info["height"])
                        self.on_state("streaming", "")
        elif sub == wire.EAT:
            try:
                sample = wire.SampleBuffer.parse(p[16:], wire.MEDIA_SOUND)
            except (ValueError, IndexError, struct.error):
                return
            fmt = sample.format.audio if sample.format else None
            if fmt is not None:
                self.info["audio"] = f"{int(fmt.sample_rate)} Hz {fmt.channels} ch {fmt.bits}-bit"
            local = self._audio_clock.now() if self._audio_clock else wire.CMTime()
            if self._audio_first is None:
                self._audio_first = (sample.pts, local)
            self._audio_last = (sample.pts, local)
            if fmt is None or (fmt.channels == 2 and fmt.bits == 16):
                self.hub.push_audio(sample.data)
        elif sub == wire.RELS:
            self._released.set()

    def _skew(self) -> float:
        """Device audio clock rate measured against the host clock (Hz)."""
        if not self._audio_first or not self._audio_last:
            return 48000.0
        (dev0, loc0), (dev1, loc1) = self._audio_first, self._audio_last
        local_ns = loc1.value - loc0.value
        ticks = dev1.value - dev0.value
        scale = dev0.scale or 48000
        if local_ns <= 0 or ticks <= 0:
            return float(scale)
        return scale * (local_ns / 1e9) / (ticks / scale)
