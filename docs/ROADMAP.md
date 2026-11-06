# ZenLink v2.5 Beta: staged roadmap (Android + Rust-first)

This is the order of work. **Do one phase at a time.** Do not start a phase before the previous phase's gate passes, unless the user says so.
Track the current phase in `lanjutan.md` and at the top of `docs/BUGS-v2.5.md`.

Every phase ends with:

1. **Green build.** `cargo fmt --check`, `cargo clippy` with no new warnings, `cargo test`, `pytest`, and the app builds.
2. **Evidence.** Measurements and test names, written into the phase's "Result" notes in `lanjutan.md`.
3. **Device check list.** What the user must test on a real device. Never mark it passed yourself.
4. **One or more commits** on `android-v2.5-beta`, plus a short report to the user: Status → Evidence → Changes → Blockers → Next.

**Fallback rule for every migration phase (M-phases).** Keep the old Python path working behind a setting or environment switch, for example `ZL_ENGINE=python|rust`. Only remove it in phase M5, after the user has confirmed the Rust path on real devices.

Measured baseline (v2.2.31, for comparing sizes later):

| Item | Size |
|---|---|
| `dist\ZenLinkService` folder | 169 MB |
| of which FFmpeg (`av.libs`) | 65 MB |
| of which Python, cryptography, pymobiledevice3 and the rest | ~94 MB |
| `ZenLink.exe` (Rust window + video) | 10.4 MB |
| Setup.exe | 60.8 MB |
| Portable.exe | 81.8 MB |

---

## Phase 0: Bootstrap and isolation

Covers: AGENTS.md §2 and §3.

- Clone v2.2.31 into this folder on branch `android-v2.5-beta` and set the version to `2.5.0-beta.1`.
- Isolate every shared resource:
  - backend port 47421;
  - `ZenLink-Beta` app data and prefs folders;
  - a new installer AppId and name;
  - "Beta" suffixes on the window, tray, mutex and firewall rule.
- Create `docs/BUGS-v2.5.md` with the carried-over items B-1… listed in AGENTS.md §7.
- **Gate:**
  - The beta builds and starts next to the running stable v2.2.31.
  - Neither app changes the other's settings, port or install folder.
  - Tests cover the beta port and paths.

## Phase 1: Slim FFmpeg (quick size win, no behaviour change)

FFmpeg is used **only** to decode AAC-ELD audio:

- `zenlink/apple/audio.py` `FfmpegAacEldDecoder` (Apple mirroring sound);
- `zenlink/airplay/receiver.py` `_AudioDecoder` (AirPlay sound).

PyAV's wheel also ships encoders that ZenLink never uses: x265, x264, SVT-AV1, vpx, vmaf, avfilter and others. These DLLs cannot be deleted one by one, because `avcodec` loads them.

- Build a minimal FFmpeg with only the AAC decoder (`--disable-everything --enable-decoder=aac`, plus whatever is strictly needed), as shared DLLs.
  - Pick the binding route after testing. Two options:
    1. PyAV built against it. This is harder on Windows.
    2. A tiny C or Rust shim DLL that exposes `aac_eld_decode`, called from Python through ctypes. Later phases reuse it from Rust.
  - Check that the FFmpeg build licence is LGPL or GPL-compatible, and record the configure line in the third-party notices.
- **Gate:**
  - An AAC-ELD decode test produces bit-identical or equivalent PCM to the old decoder, using a recorded packet fixture (add one).
  - The package size is measured before and after.
  - Device check: Apple mirroring sound and AirPlay sound.

## Phases A1–A6: Android (follow `ANDROID_MASTER_PROMPT.md` §18)

All Android code is Rust, in a new crate such as `rust/crates/zenlink-android`. No new Python.

| Phase | Master prompt stages | Content | Gate (besides green build) |
|---|---|---|---|
| **A1 Foundation** | 1–2 | Audit of scrcpy upstream (pin the version). ADB transport: bundle `adb.exe` + `AdbWinApi*.dll` from platform-tools, or a Rust USB ADB client; decide with evidence. Device discovery, authorization states, `ro.product.*` name resolution, stable device identity. | Parser and name tests from master prompt §4 pass. Device list works with a fake ADB output fixture. |
| **A2 Platform selector UI** | 8 (UI part, moved earlier for testing) | First screen "iPhone / iPad" vs "Android Phone / Tablet", ID/EN toggle, Back, "Remember device type". The Apple popup becomes step two, unchanged. Default theme dark. | Startup-flow acceptance tests from master prompt §2. The Apple path behaves exactly as in v2.2.31. |
| **A3 Mirroring** | 3 | Push and start the scrcpy server, a Rust client for its video socket, H.264 into the existing `zenlink-video` decoder and renderer (Media Foundation + D3D11). | Video from a recorded scrcpy stream fixture plays through `VideoView`. Frame pacing stats work. Device check: USB mirroring. |
| **A4 Controls + audio** | 4–5 | Control socket (touch, keys, scroll, Back/Home/Recents, clipboard, rotation), audio socket into the shared audio engine, A/V sync modes, video-only fallback. | Coordinate mapping tests under rotation and resize. Device check: touch and sound on Android 11+. |
| **A5 Smart encoder + profiles** | 6–7 | Encoder listing and parsing, Auto/H.264/HEVC/AV1 (verified only), bounded fallback, video profiles, per-device profiles. | Encoder-list parser tests and fallback tests. Profiles survive USB↔Wi-Fi. |
| **A6 Wi-Fi, OBS, attribution, validation** | 4 (wireless), 9–12 | TCP/IP and Android 11+ pairing, OBS check with rotation (keep the `set_bounds` wait), size pass, scrcpy Apache-2.0 attribution everywhere, docs. | Device checks listed. Measured sizes recorded. README and notices updated. A beta pre-release is possible **only if the user asks**. |

## Phases M1–M5: move the rest of the Python backend to Rust

Start these after A6, or earlier only if the user asks. Each one replaces a single Python part, behind the `ZL_ENGINE` fallback switch.

| Phase | Replace | With | Risk | Gate |
|---|---|---|---|---|
| **M1 Audio engine** | `audio_out.py` (sounddevice) | WASAPI directly (or `cpal`) in Rust, shared with Android | Low | Port the block-aware target and margins and the 10 ms `MIN_MARGIN` exactly, with the same tests. Device check: Bluetooth headset (MME/BT ~91 ms) has no gaps. |
| **M2 USB Direct (ZenLink Studio cable)** | `zenlink/usbdirect` (pyusb, QuickTime protocol) | `rusb` (libusb0/WinUSB) + port of the protocol, including `SkewMeter`, HPA1/HPA0 and NEED handling | Medium | Unit tests from `tests/test_audit.py` ported. Device check: 60 fps, no stutter while YouTube plays, "Device only" sound. |
| **M3 AirPlay receiver** | `zenlink/airplay` | Rust RTSP, pairing and crypto. Keep `playfair.dll` (FairPlay) through FFI. | Medium–high | `tests/airplay_sim.py` scenarios pass against Rust. Device check: AirPlay video and sound. |
| **M4 Apple pairing / lockdown / iOS 17+ tunnel** | pymobiledevice3 (+ qh3, pydantic, cryptography) | The `idevice` crate (audit maturity first), or a minimal own implementation of only what ZenLink uses | **Highest** | Feature-by-feature parity checklist (pair, trust, developer mode, tunnel, screen stream). Device check on iOS 17 and later. |
| **M5 Remove the Python sidecar** | `ZenLinkService.exe`, PyInstaller | A single Rust exe (plus adb and the scrcpy server, minimal FFmpeg, `playfair.dll`) | — | Only after the user has confirmed M1–M4 on devices. Measure the final sizes against the baseline table. Update the build script and installer. |

## Phases P1–P4: macOS and Linux (after A6; best after M5)

Start these only when the user says so. The portable core and the platform layer rules are in AGENTS.md §3e.

| Phase | Content | Gate |
|---|---|---|
| **P1 macOS: Android** | A macOS platform layer: VideoToolbox decode, Metal (`CAMetalLayer`) presentation inside the Slint window, CoreAudio through `cpal`, vibrancy for glass, macOS `adb` + scrcpy server. Package an `.app` in a `.dmg`. | Builds on a real Mac or a `macos-latest` runner. Android mirroring, control and sound tested by the user on a Mac. Universal (arm64 + x86_64) build, or both, documented. |
| **P2 macOS: Apple Driver mode + ZenLink Studio** | See "Apple modes on macOS and Linux" below. | The user tests on a Mac with an iPhone and an iPad, in both modes, over cable and Wi-Fi. The README lists what differs from Windows. |
| **P3 macOS distribution** | Start unsigned. The arm64 binaries are ad-hoc signed by the linker. The README gets an illustrated first-launch guide: macOS 15+ **System Settings → Privacy & Security → Open Anyway**; older macOS right-click → Open. Developer ID signing + notarization ($99/year Apple Developer account) only when the user decides to; never create certificates or accounts yourself. | A downloaded `.dmg` from a GitHub pre-release opens on a clean Mac by following the README. |
| **P4 Linux: Android** | VA-API (or FFmpeg hwaccel) decode + Vulkan or OpenGL presentation, PipeWire/PulseAudio through `cpal`, a glass fallback (blur only where the compositor supports it). A udev rule for adb devices (`TAG+="uaccess"`). AppImage first, Flatpak or `.deb` later. | Tested by the user on at least one common distro (e.g. Ubuntu LTS) on X11 and Wayland. |
| **P5 Linux: Apple Driver mode + ZenLink Studio** | See "Apple modes on macOS and Linux" below. | The user tests on Linux with an iPhone/iPad in both modes, cable and Wi-Fi. The README lists the distro requirements (usbmuxd version, avahi, firewall ports). |

### Apple modes on macOS and Linux (research notes, 2026-11; verify again before coding)

How each mode works today on Windows:
- **Apple Driver mode** (`zenlink/apple/`) uses pymobiledevice3. The steps are:
  1. Lockdown via usbmuxd (Apple Mobile Device Service).
  2. A CoreDevice tunnel: userspace (in-process TCP stack), CoreDeviceProxy on iOS 17.4+, or RemotePairing over Wi-Fi.
  3. The personalised Developer Disk Image is mounted (it needs network access to Apple's TSS).
  4. The DisplayService screen stream starts. Its frames arrive only on iOS 27+; on iOS 17–26, "Hybrid Control" uses AirPlay for the picture and the CoreDevice HID services for touch, keys and buttons.
  None of this is Windows-specific except where the pair records live (`C:\ProgramData\Apple\Lockdown`).
- **ZenLink Studio, cable** (`zenlink/usbdirect/`) uses Apple's QuickTime screen-capture USB protocol over raw USB:
  - vendor request `0x40/0x52 wIndex 2` exposes the hidden capture configuration (interface subclass `0x2A`, the "Valeria" mode);
  - then CLEAR_HALT on both bulk endpoints;
  - then the handshake (PING, CWPA, AFMT, CVRP, CLOK, TIME, SKEW, NEED, HPA1/HPD1, FEED, EAT!);
  - then trust is proven with its own usbmux + lockdown TLS session using the pair record.
  On Windows this needs the libusb0 filter driver.
- **ZenLink Studio, Wi-Fi** is ZenLink's own AirPlay receiver (`zenlink/airplay/`, `playfair.dll` for FairPlay, python-zeroconf for mDNS).

**Linux.** All three modes are possible. No kernel driver is needed. Requirements and traps found:

1. **USB permission.** Ship a udev rule (`/etc/udev/rules.d/70-zenlink.rules`) for Apple VID `05ac` (and adb devices) with `TAG+="uaccess"`. It gives the logged-in desktop user access with no root, no group change and no re-login. It does not cover headless/SSH sessions. Installing the rule needs root once: the user runs the installer with a polkit or sudo prompt, and ZenLink never does it silently.
2. **usbmuxd conflict (ZenLink Studio cable).** usbmuxd **1.1.1** (still the latest *release*, and in most distros) clamps the configuration to ≤ 4 and switches the phone straight back, so the capture interface disappears. Newer usbmuxd from git master understands the extra modes: `USBMUXD_DEFAULT_DEVICE_MODE=2` is Valeria (QuickTime capture), 3 is CDC-NCM; master's default changed to 3 in 2023. Options, in order of preference:
   - (a) Detect the usbmuxd version and mode, and explain clearly in the app. Offer a guided, optional, user-approved setup of a Valeria-capable usbmuxd, as a systemd drop-in with `USBMUXD_DEFAULT_DEVICE_MODE=2`. It must be reversible. Prior art: Jalrux/iphone-control's `setup.sh --usbmuxd`.
   - (b) While a Studio session runs, own the device without usbmuxd, using ZenLink's own raw usbmux + lockdown code. This only works if usbmuxd is not holding the interface. Never stop or replace the system usbmuxd silently: that breaks other iOS tools.
   Decide with real-device tests. Mode 2 vs 3 may matter for Apple Driver mode's tunnel on iOS 17.0–17.3: test both.
3. **Trust / pair records.** On Linux they are in `/var/lib/lockdown/` (root-only). Read them through the usbmuxd socket (`READ_PAIR_RECORD`) instead of the file, and pair through usbmuxd (`SAVE_PAIR_RECORD`) the normal way. The Studio cable path, which proves trust itself, needs the record: get it through usbmuxd before taking the device.
4. **Apple Driver mode tunnels.**
   - iOS **17.4+**: the userspace in-process tunnel works without root (pymobiledevice3; in Rust, the `idevice` crate's `core_device_proxy` + `tunnel_tcp_stack`).
   - iOS **17.0–17.3.x**: only the privileged kernel tunnel (tunneld, root) works. Mark these versions "not supported on Linux" unless the user accepts a root helper.
   - The DDI mount needs network access to Apple.
5. **Audio over the QuickTime path.** Reports differ: one Linux project hears silence on iOS 26.5.2 without HPA1, while ZenLink on Windows gets 48 kHz PCM with HPA1. Keep ZenLink's exact handshake (HPA1, then HPA0 for "Device only") and verify on Linux with a device.
6. **AirPlay (Wi-Fi) mDNS.** Most desktops run **avahi-daemon** on UDP 5353, and a second mDNS stack can fail with "Address already in use" or confuse discovery. When avahi is running, publish the AirPlay/RAOP services **through avahi's D-Bus API** (e.g. the `zbus` crate). Use ZenLink's own mDNS only when avahi is absent. `systemd-resolved` may also run an mDNS responder: detect it and explain.
7. **Firewall.** ufw or firewalld may block the AirPlay ports (TCP 7000 + session ports, UDP 5353 and the RTP ports). Detect this and show exact instructions. Never change the firewall without the user's approval.
8. **FairPlay.** `playfair` is portable C (UxPlay uses the same lineage). Build it as a static library per platform instead of `playfair.dll`. As on Windows, DRM-protected video (Apple TV app) stays black in AirPlay mirroring.
9. Prior art to study, but do not copy without a licence check:
   - danielpaulus/quicktime_video_hack (MIT; protocol notes; "works but hard to get going" on Linux);
   - Jalrux/iphone-control (Linux USB capture with usbmuxd > 1.1.1);
   - Furruka/iPhoneMirror `linux-port` (GPL-3.0; unfinished; headless USB capture verified on one iPad);
   - FDH2/UxPlay (GPL-3.0; Linux AirPlay receiver).

**macOS.** Both modes are possible, but use the system's own paths where the OS owns the device:

1. **ZenLink Studio, cable.** Do **not** use raw USB on macOS: the OS's usbmuxd and the CoreMediaIO driver own the device. Use the official route QuickTime uses:
   - set CoreMediaIO `kCMIOHardwarePropertyAllowScreenCaptureDevices = 1`;
   - the iPhone/iPad then appears as a muxed `AVCaptureDevice` (video + sound) a moment later;
   - capture it with AVFoundation and feed the frames into the shared renderer.
   No driver, no Developer Mode. Touch control is not part of this path. Offer control through Apple Driver mode's CoreDevice HID path (Hybrid) when the user has Developer Mode on. Verify that sound arrives on the muxed device.
2. **Apple Driver mode.** It is the same as on Linux and Windows, through macOS's own usbmuxd (`/var/run/usbmuxd`). The userspace tunnel needs no sudo on iOS 17.4+. Pair records are owned by macOS: use the usbmuxd socket.
3. **ZenLink Studio, Wi-Fi (AirPlay).** macOS Monterey+ has its own **AirPlay Receiver** listening on ports 7000 and 5000.
   - Run ZenLink's receiver on a free, non-default port. The port is advertised in the mDNS record, so iOS follows it.
   - Publish through macOS's mDNSResponder (dns_sd API), not a second mDNS stack.
   - Use a different receiver name.
   - Tell the user that macOS's own AirPlay Receiver can be turned off in System Settings → General → AirDrop & Handoff if the two get confused.
   - Verify this with a device.
4. **Prior art on Mac.** TikTok LIVE Studio has an official macOS version, with "Cast (iOS)" by cable and over Wi-Fi.
   - The user downloaded its installer into `_research/tiktok-live-studio-mac/tiktok_live_studio_mac_arm-v0.0.1-wid-92pwXyJZHkr.dmg` (5,460,835 bytes, SHA-256 `d5bc25b5…244000b`).
   - The "arm" and "intel" downloads are **byte-identical**, so it is probably a small online installer ("v0.0.1") that downloads the real app.
   - Before building P2, analyse it read-only with the REA tools. Find the real app package (download URL or bundled payload), then how the real app captures: CoreMediaIO/AVFoundation, raw USB, or its own AirPlay receiver, and its ports and mDNS.
   - Never run the installer on the user's PC. Never sign in to anything. Never redistribute TikTok files. `_research/` must be in `.git/info/exclude` and is never committed.
5. **Packaging.**
   - The app is a `ZenLink.app` bundle with `Info.plist`, icon, bundled `adb` and scrcpy server, and the playfair library.
   - It ships as `ZenLink-vX-macOS.dmg`, where the user drags it to Applications. This is the Mac equivalent of Setup.exe; a zipped `.app` is the portable equivalent.
   - Build a universal binary (`lipo` of aarch64 + x86_64), or ship two `.dmg`s.
   - Tooling: `cargo-packager` or a script with `hdiutil`.
   - **Build on Windows, with no Mac.** The user already builds working iOS apps on this PC with `E:\ProjectIOS\nxnative-win`: LLVM clang + `ld64.lld`, an SDK extracted from `E:\ProjectIOS\Xcode_27.1.xip` by `win\sdk\extract_xip.py`, and an ad-hoc Mach-O signer `win\macho_sign.py`. Reuse that approach for macOS:
     1. Extract **MacOSX.sdk** from the same `.xip`. Copy `extract_xip.py` into this repo's tools and change its filter to `Platforms/MacOSX.platform/Developer/SDKs/`. Put the output in `E:\DevTools\sdk\MacOSX.sdk`. The Xcode and Apple SDKs Agreement applies.
     2. Run `rustup target add aarch64-apple-darwin x86_64-apple-darwin`. Add a `.cargo/config.toml` linker setup that uses `clang -target arm64-apple-macos11 -isysroot <MacOSX.sdk> -fuse-ld=lld` (and the x86_64 equivalent). C deps such as playfair are built with the same clang.
     3. For a universal binary, use `llvm-lipo` (check that LLVM has it) or `rcodesign`/another lipo tool. Otherwise ship two builds.
     4. Signing: link with `-adhoc_codesign` and use `macho_sign.py`, or **`rcodesign`** (Rust; runs on Windows). Apple Silicon refuses unsigned binaries, so ad-hoc signing is the minimum. `rcodesign` can also sign with a Developer ID and notarize later, if the user ever gets an account.
     5. Package it: assemble `ZenLink.app` with Python and plistlib. Write Unix permissions (755 for executables) into the zip, as `pack_ipa.py --exec` does. For a `.dmg`, use a Windows-capable tool, or `libdmg-hfsplus` inside the user's WSL (Kali). Verify the result opens on a Mac.
     6. Metal shaders, if any are needed: use Apple's **Metal Developer Tools for Windows** (`E:\ProjectIOS\Metal Developer Tools`).
   - GitHub Actions `macos-latest` is an alternative, but only with the user's approval.
   - **Testing still needs a real Mac.** The user, or someone they ask, runs it. A Windows build is never "verified" until it has run on macOS.
   - `Info.plist` must declare the usage strings macOS asks for: camera/screen-capture devices for CoreMediaIO, local network for AirPlay/Bonjour, and microphone if used.
6. macOS already has iPhone Mirroring (macOS 15+, iPhone only, same Apple ID, not in the EU) and QuickTime capture (no control). ZenLink's value is one app for iPhone, iPad and Android, with control, OBS-friendly output and the ZenLink UI. Do not claim to replace Apple's features.

Size expectation: removing Python and the full FFmpeg should shrink the package a lot.

Size expectation: removing Python and the full FFmpeg should shrink the package a lot. **Do not promise a number.** Measure it in M5 and report it.

---

## How to work: Claude and sub-agents

**Main session (the orchestrator).**
- Owns the current phase, the plan, the commits, `lanjutan.md` and the user reports.
- Works on one phase at a time.

**Sub-agents: use them for the expensive side work and keep the main context small.**
- **Explore / research agent.** Read-only audits, for example: "scrcpy v3.x server protocol: socket order, packet headers, control message layout", "which `idevice` crate features exist", "where Python uses X". It returns a summary, not file dumps.
- **Plan agent.** Draft the implementation plan of a phase before coding big phases (A3, M2, M4).
- **Review.** Before each phase's final commit, run `/code-review` (or a review sub-agent) on the diff. Fix the confirmed findings.
- **Parallel writers.** Only for independent crates or files, for example the encoder-list parser while the UI is in progress.
  - Each writer gets its own git worktree **inside** `E:\Zenlink V.2.5 Beta` (for example `.claude\worktrees\...`).
  - Never let two writers touch the same files.
  - The main session merges.
- Never let a sub-agent push, release, touch drivers or run device tests.

**Model choice (to save cost).** Pass `model` on each sub-agent call.

| Work | Model |
|---|---|
| Main session: owns the phase, writes the core code, decides | **Opus** |
| Explore / read-only research, summarising upstream code or docs, finding usages | **Sonnet** |
| Simple lookups: grep-like searches, listing files, checking one fact | **Haiku** |
| Mechanical writers: parsers with clear specs, tests, localisation strings, docs, attribution | **Sonnet** |
| Plans and reviews for the risky phases (A3 mirroring, A4 audio/A-V sync, M2 USB Direct, M4 Apple tunnel) and any concurrency, timing or protocol bug hunt | **Opus** |

If a Sonnet result looks shallow or wrong, redo that step with Opus. Do not ship it.

**Session hygiene.**
- When a phase is done, update `lanjutan.md`. A new session can then start fresh with "continue from lanjutan.md".
- Long builds run in the background. Do not poll them.

---

## Starter prompts for the user (copy and paste)

- Phase 0: `Baca AGENTS.md dan docs/ROADMAP.md. Kerjakan Phase 0 sampai gate-nya lolos.`
- Next phase: `Lanjut dari lanjutan.md. Kerjakan phase berikutnya di docs/ROADMAP.md sampai gate-nya lolos.`
- Specific phase: `Kerjakan Phase A3 di docs/ROADMAP.md.`
- After a device test: `Hasil tes di HP: <hasil>. Update BUGS-v2.5.md dan perbaiki yang gagal.`
- Bug report: `Bug baru: <cerita + screenshot/log>. Catat di BUGS-v2.5.md, cari penyebabnya, perbaiki, verifikasi.`
