# ADR-0001: Rust app with a Python sidecar, then 100% Rust

Status: accepted (2026-11). Branch: `rust-next`.

## Context

The target is ZenLink rewritten in Rust:
- no Python runtime;
- native GPU rendering that OBS can capture smoothly;
- Liquid Glass light and dark themes;
- a smaller install;
- every v2.2.3 feature kept (see `AUDIT.md`).

The device protocol layer (CoreDevice / RemoteXPC / tunnel, usbmux / lockdown, AirPlay + FairPlay) is about 4,600 lines of Python. It also depends on pymobiledevice3. Rewriting all of it before shipping would leave users on the OBS-stuttering build for a long time.

## Decision

Ship in stages. The Python backend keeps its HTTP protocol (see AUDIT §2) as the contract between stages.

**Stage 1: Rust shell plus Python sidecar.**
- `zenlink-app`: Slint UI with the same layout as the old app, Liquid Glass light/dark/system, ID/EN.
  - It starts `ZenLink.exe --headless --parent-pid <pid>`, and the sidecar exits with the app.
- `zenlink-video`: Media Foundation decoder (DXVA through `IMFDXGIDeviceManager`).
  - The D3D11 video processor converts NV12 to BGRA and scales.
  - A flip-model swapchain on a child HWND presents frames, with a frame-latency waitable and sync interval 1 on a render thread.
  - Only the newest decoded frame is shown each vblank.
- `zenlink-core`: protocol client.
  - Parses the stream framing and converts AVCC/HVCC to Annex B.
  - Posts input.
  - Reads state.
- Audio stays native in the sidecar (WASAPI via sounddevice).

**Stage 2: port the sidecar crate by crate.** Order: easiest first, largest win first.
1. `zenlink-platform`: DPAPI, firewall, tray, single instance, OCR (WinRT), drivers via the PowerShell scripts.
2. `zenlink-audio`: WASAPI output in Rust.
3. `zenlink-usb`: usbmux + lockdown + ZenLink Studio stream over libusb0 (`rusb` / WinUSB).
4. `zenlink-airplay`: mDNS (`mdns-sd`), RTSP, FairPlay through `native/playfair` FFI.
5. `zenlink-apple`: CoreDevice / RemoteXPC / tunnel. This is the largest piece, so it goes last.

Each step deletes the matching Python module and endpoint proxy. The HTTP contract can then shrink to in-process calls. Stage 2 ends when the installer contains no Python.

## Consequences

- OBS fix and new UI ship early (Stage 1). Size shrinks step by step (Stage 2).
- During Stage 1 the installer still carries Python. It is smaller than v2.2.3 because WebView2 UI assets and the `av` decode path are not needed by the Rust shell.
- No numbers (latency, size, OBS frame drops) are claimed until they are measured on the user's machine.
- `main` keeps the v2.2.3 Python app until the Rust build is tested on a real device.
