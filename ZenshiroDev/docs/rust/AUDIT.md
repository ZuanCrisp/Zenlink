# ZenLink audit and feature inventory (v2.2.3 baseline)

Baseline: `main` at v2.2.3 (Python 3 + aiohttp backend, pywebview / WebView2 UI).
Purpose: list everything the Rust rewrite must keep, and where each piece lives today.

## 1. Architecture today

| Layer | Where | Size (lines) | Notes |
| --- | --- | --- | --- |
| Window, tray, single instance | `apps/desktop/backend/zenlink/__main__.py`, `window.py`, `tray.py`, `restart.py` | ~480 | pywebview (WebView2), pystray |
| Local HTTP + WebSocket API | `apps/desktop/backend/zenlink/server.py` | 383 | aiohttp on `127.0.0.1:47321` |
| Cast orchestration | `apps/desktop/backend/zenlink/castmanager.py`, `connection.py`, `capability.py` | ~735 | picks Apple / USB Direct / AirPlay |
| Apple mode (CoreDevice, RemoteXPC, screen + HID) | `apps/desktop/backend/zenlink/apple/*` | ~1,800 | uses pymobiledevice3 |
| ZenLink Studio (USB Direct, libusb0) | `apps/desktop/backend/zenlink/usbdirect/*` | ~2,000 | usbmux, lockdown, QuickTime-style stream, H.264 |
| AirPlay receiver | `apps/desktop/backend/zenlink/airplay/*`, `ZenshiroDev/resources/windows/native/playfair` (C) | ~820 + C | mDNS, FairPlay, mirroring stream |
| Drivers | `apps/desktop/backend/zenlink/drivers/*`, `ZenshiroDev/tooling/windows/tools/usb_filter.ps1` | ~340 + PS | libusb0 install / rollback, catalog signing |
| Native audio | `apps/desktop/backend/zenlink/audio_out.py` | 244 | sounddevice (WASAPI) |
| OCR | `apps/desktop/backend/zenlink/ocr.py`, `ZenshiroDev/tooling/windows/tools/ocr.ps1` | 85 + PS | Windows.Media.Ocr |
| Secrets | `apps/desktop/backend/zenlink/secret.py` | 63 | DPAPI passcode |
| UI | `apps/desktop/backend/ui/*.js`, `index.html`, `styles.css` | ~3,300 | WebCodecs decode, MediaStreamTrackGenerator, `<video>` present |

Python runtime and dependencies: aiohttp, pywebview, pymobiledevice3, av (FFmpeg), zeroconf,
pycryptodome, cryptography, pyusb, sounddevice, pystray, Pillow. Most of the installer size
comes from these packages, not from ZenLink code.

## 2. Backend protocol (what any client must speak)

- `GET /cast/codec` returns `{codec, description (base64 avcC/hvcC), keyframesOnRequest}`.
- `GET /cast/stream.bin` is a stream of records `[u32 BE len][u8 type][payload]`.
  - `0` = keyframe.
  - `2` = keyframe that needs a decoder rebuild.
  - `3` = new codec config (JSON, same shape as `/cast/codec`).
  - Anything else = delta frame.
  - Payload is length-prefixed (AVCC / HVCC) NAL units.
- `POST /cast/pli` asks for a keyframe. Only Apple mode supports it.
- Input is posted as JSON:
  - `POST /cast/touch` with `{type: contact|release, x, y}`. `x` and `y` are 0..65535 in the device's touch space.
  - `POST /cast/button` with `{name, state}`.
  - `POST /cast/key` posts keyboard input.
- `GET /cast/audio.bin` carries `[u32 BE len][PCM s16 48 kHz stereo]`. It is used only when the backend has no native audio.
- `GET /ws` pushes state events. `/api/*` handles the rest: state, device name, cast start/stop, connection, drivers, passcode, OCR, keyboard actions, audio, client log, app show/restart, firewall, Wi-Fi prepare, support, Zadig.

## 3. Feature inventory (must survive the rewrite)

**Connection**
- Apple Driver mode (USB + Wi-Fi after setup).
- ZenLink Studio (USB Direct, libusb0, high bitrate).
- AirPlay Screen Mirroring.
- Connection chooser with "remember my choice".

**Picture**
- HEVC and H.264.
- Quality presets: best / normal / performance.
- Fit: contain / cover / fill, plus fit window to device.
- Smoothing.
- Rotation: auto, plus Alt+H.
- Un-cropped full-resolution decode.
- Stats in the header.

**Input**
- Touch with the mouse.
- Keyboard: HID usages, Ctrl shortcuts.
- Gestures: Home swipe, App Switcher, back with the mouse Back button or Alt+Left, trackpad scroll.
- Control panel with Home, lock, volume, App Switcher, Notification Center and Control Center.
- Alt+U wake and unlock with the passcode (DPAPI).
- Alt+L lock.
- Alt+V paste from the PC.
- Alt+C copy text from the screen (OCR).
- Alt+D stealth dim.
- Alt+R.

**Audio**
- Audio on this PC or on the device only.
- Volume, lip sync, buffer size (low / normal / high).
- Native WASAPI output.

**App**
- Tray and close to tray.
- Always on top.
- Auto start casting.
- Animated dots background.
- ID/EN UI, with Indonesian as the default.
- Themes: Liquid Glass light (default), dark and system, plus a glass intensity setting.
- Single instance.
- Restart.
- Support bundle.
- Firewall check and allow.
- Driver status, install and rollback, Zadig.

## 4. Problems the rewrite must fix

1. **OBS stutter.** The picture is smooth in ZenLink, but OBS Window Capture and Display Capture stutter on a 2-monitor setup.
   - Cause (hypothesis): WebView2 promotes the `<video>` to a DirectComposition overlay (MPO), and frame writes are not aligned to vblank.
   - v2.2.3 mitigation: rAF pacing plus `--disable-direct-composition-video-overlays`.
   - Rust fix: D3D11 flip-model swapchain on a normal HWND, presented with sync interval 1 from a dedicated render thread. One present per vblank, newest frame wins.
   - Must be measured (OBS stats: skipped / lagged frames) before anyone claims it is fixed.
2. **Driver catalog signing.** The error was "root certificate which is not trusted by the trust provider". `Set-AuthenticodeSignature` validates the chain against a self-signed root that was not yet trusted. Fixed in v2.2.3: trust first, then sign, then verify.
3. **Size and startup.** The Python runtime plus WebView2 adds weight. The Rust shell removes WebView2. Porting the sidecar removes Python.

## 5. Risks

- `pymobiledevice3` covers CoreDevice / RemoteXPC / tunnel (QUIC + TLS-PSK) / DVT. Porting it is the largest single task.
- FairPlay (playfair C) needs a Rust FFI wrapper or a port.
- HEVC hardware decode on Windows needs the "HEVC Video Extensions" MFT. H.264 is built in.
