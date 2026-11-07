# ZenLink v2.5 Beta: bug tracker

Current phase: **A5 (smart encoder + profiles) done in code; B-26 shortcuts in the Rust window done; next A6**, see `docs/ROADMAP.md`.

Rules: `AGENTS.md` §7. A bug is "verified" only with evidence (a passing test, a
measurement, or the user's confirmation on a named device). Otherwise it is
"fixed (unverified)" with what is still needed.

---

## Carried over from v2.2.31

### B-1: "Hanya perangkat / Device only" sound (ZenLink Studio, cable)            Status: open
- Reported: 2026-11-06, carried over from v2.2.31 (AGENTS.md §7 item 1).
- Cause: n/a. The session sends HPA0 after the first frame (`zenlink/usbdirect`).
- Fix: n/a: the code path is unchanged from v2.2.31; only a device check is left. Re-read on 2026-11-07: "Hanya perangkat" in the Rust window posts `/api/audio {"enabled": false}` (`main.rs` audio body); `castmanager.set_audio` calls `session.set_audio_to_pc(False)` and stops the PC sound; a session started that way still finishes the known-good handshake with HPA1, then sends HPA0 on the first frame (`session.py`). No defect found in code.
- Verification: unverified: needs an iPhone on the ZenLink Studio cable path, "Device only" selected; check that the sound plays on the phone and not on the PC.

### B-2: OBS portrait/landscape with Android rotation            Status: fixed (unverified)
- Reported: 2026-11-06, carried over (item 2). The iPhone capture survives rotation (user confirmed in v2.2.31). Positioning uses OBS "Fit to screen" (Ctrl+F), as the Settings OBS card says.
- Cause: n/a (Android path was not built yet).
- Fix: done in A3/A4: Android renders through the same `VideoView` (no second renderer), so a rotation goes through `set_bounds` with the 120 ms ResizeBuffers wait like the iPhone path; the OBS card's Ctrl+F advice applies.
- Verification: unverified: needs OBS Game Capture + an Android device rotating (phase A6).

### B-3: Stutter fixes stay regression-tested (renderer FIFO, audio margin, SkewMeter)            Status: fixed (verified)
- Reported: carried over (item 3).
- Cause: proven in v2.2.x (periodic judder from a "newest wins" mailbox; audio margin too small for large output blocks; device clock skew).
- Fix: v2.2.31 (`zenlink-video` `d3d.rs` Ring FIFO; `NativeAudio` block-aware margins, `MIN_MARGIN` 10 ms; `SkewMeter` ≥ 6 s).
- Verification: user confirmed on an iPhone (v2.2.31). Regression guards: `cargo test --release` (`ring_tests`), `pytest` audio and SkewMeter tests. Keep them green in every phase.

### B-4: Window resize kept after clicking settings            Status: fixed (unverified)
- Reported: carried over (item 4).
- Cause: settings changes refit the window; fixed in v2.2.31 with `fit_prefs_key` + `user_scale`.
- Fix: v2.2.31 (322822b).
- Verification: confirmed in a fake-stream test (`rust/tools/fake_backend.py`). unverified: needs user confirmation on a real device.

### B-5: Portable.exe / Source-zip `ZenLink.exe` need the Visual C++ runtime            Status: fixed (unverified)
- Reported: carried over (item 5).
- Cause: the Rust exe imports `VCRUNTIME140.dll`. The installer works only because PyInstaller's onedir ships it next to the exe; Portable.exe and the source zip do not. On a PC without the VC++ Redistributable, Windows fails before ZenLink can explain anything.
- Fix: static CRT via `rust/.cargo/config.toml` (`-C target-feature=+crt-static` for `x86_64-pc-windows-msvc`); it also covers the `embedded-backend` and `CARGO_TARGET_DIR` builds.
- Verification: import check with `pefile` on `rust/target/release/ZenLink.exe`: no `VCRUNTIME140*.dll` and no `api-ms-win-crt-*` imports (2026-11-07). Exe 11,746,304 → 11,916,800 B (includes the OCR snapshot and B-27 code of the same build). Unverified: a run on a clean PC / Windows Sandbox (needs the user's approval).

### B-6: Stale WebView2 warning in the installer and Start ZenLink.bat            Status: fixed (unverified)
- Reported: carried over (item 6).
- Cause: `packaging/ZenLink.iss` `InitializeSetup` and the WebView2 check in `Start ZenLink.bat` are left from the classic pywebview window. The Rust window does not use WebView2.
- Fix: installer check removed; `Start ZenLink.bat` checks WebView2 only before the classic window (`ZENLINK_UI=classic`, label `:check_webview2`).
- Verification: `tests/test_prereqs.py` (installer has no WebView2 check; the default setup path does not install it). Unverified: the installer run on Windows 10 without WebView2.

### B-7: Every prerequisite is bundled or explained by ZenLink itself            Status: fixed (unverified)
- Reported: carried over (item 7, general rule).
- Cause: a missing DLL or service that Windows reports before ZenLink can explain it.
- Fix (v2.5 audit, 2026-11-07): Visual C++ runtime: static CRT (B-5). WebView2: no longer used or asked for (B-6, classic window left out of the packages). Media Foundation on N editions: delay-loaded, ZenLink explains it (B-27). Android `adb` and the scrcpy server: bundled (B-28). Apple Mobile Device Service: the installer and the app offer the Microsoft Store page. ZenLink Studio driver: the app's own button, with consent. Nothing else is required.
- Verification: unverified: a clean PC / Windows Sandbox run (needs the user's approval).

### B-8: Baseline Rust code is not rustfmt-formatted            Status: fixed (verified)
- Reported: 2026-11-06 by Claude during Phase 0: `cargo fmt --check` fails on the untouched v2.2.31 code (`build.rs`, `main.rs`, `view.rs`, … lines up to 621 characters; no `rustfmt.toml`).
- Cause: v2.2.31 was never run through rustfmt. The gate in AGENTS.md §9 requires `cargo fmt --check`.
- Fix: one formatting-only commit (`cargo fmt`), no code change.
- Verification: `cargo fmt --check` passes; `cargo test --release` and `cargo clippy --release` pass after it.

### B-9: ZenLink Next (beta) and stable must not share ports, data, installer identity or process kills            Status: fixed (unverified)
- Reported: 2026-11-06, Phase 0 (AGENTS.md §3).
- Cause: v2.2.31 hard-codes port 47321 (backend), 47322 (local Apple stream), 7000 (AirPlay), `%APPDATA%\ZenLink`, `%LOCALAPPDATA%\ZenLink`, the installer AppId, the "ZenLink (AirPlay)" firewall rule, `%TEMP%\zenlink-next.log`, and its uninstallers kill `ZenLink.exe`/`ZenLinkService.exe` by name (which would close the stable app).
- Fix: Phase 0 isolation commit; renamed to "ZenLink Next" on 2026-11-06 at the user's request. It uses 47421 / 47422 / 7100, `ZenLink-Next` data folders, `zenlink-next-v25.log`, AppId `{5AE777B3-F756-4FAC-B6E7-A2125D89D76D}`, `AppName=ZenLink Next`, `{autopf}\ZenLink Next`, firewall rule "ZenLink Next (AirPlay)", AirPlay name "ZenLinkStudioNext", window/tray/AppUserModelID "ZenLink Next". Uninstallers stop only processes running from the beta's own folder.
- Verification: `tests/test_next_isolation.py` and `zenlink-core` `isolation_tests` pass. Side-by-side run: see `lanjutan.md`. unverified: AirPlay on port 7100 with a real iPhone/iPad; installer install/uninstall next to an installed stable v2.2.31.

### B-10: Licence audit before bundling Android platform-tools `adb`            Status: fixed (decided)
- Reported: 2026-11-06, phase A1 (master prompt section 13, AGENTS.md §6 item 9).
- Cause: the ADB decision (`docs/android/ADB.md`) bundles Google's `adb.exe`, `AdbWinApi.dll` and `AdbWinUsbApi.dll` next to a GPL-3.0 program.
- Fix: audited 2026-11-07 for platform-tools 37.0.1 (`docs/THIRD_PARTY.md`): SDK licence §3.4 forbids redistribution except where third-party licences require it; §3.5 puts open-source components (adb is AOSP, Apache-2.0 per `NOTICE.txt`) under their own licence alone. Decision (2026-11-07, the owner delegated it: "semua tentukan aja paling bagus"): bundle the files unmodified, signature-checked, with `NOTICE.txt`, credits in README, About and THIRD_PARTY.md; adb runs as a separate program (no linking). Not legal advice.
- Verification: build script refuses files that are not validly signed by Google LLC; `NOTICE.txt` ships in every package.

### B-11: Android device names, identity and list (phase A1)            Status: fixed (unverified)
- Reported: 2026-11-06, phase A1 gate (master prompt section 4).
- Cause: n/a (new code).
- Fix: `rust/crates/zenlink-android` (adb host-protocol client, `devices -l` parser, getprop parser, naming, identity, discovery); notes in `docs/android/`.
- Verification: 43 Rust tests pass (`cargo test --release -p zenlink-android`), including a fake in-process ADB server; `tests/test_scrcpy_pin.py` passes. Unverified on hardware: needs real Android phones over USB and Wi-Fi (with the user's consent) to confirm names and identity.

### B-12: Icons show as empty boxes on Windows 10 (v2.2.31)            Status: fixed (unverified)
- Reported: 2026-11-06 by the user, with a photo of v2.2.31 on a Windows 10 PC (confirmed by the user): every icon is drawn as an empty rectangle; text renders fine.
- Cause: all icons use the font "Segoe Fluent Icons", which ships with Windows 11 only. Windows 10 has "Segoe MDL2 Assets".
- Fix: `Theme.icons` is set at startup (`icon_font()` in `main.rs`): "Segoe Fluent Icons" when `%WINDIR%\Fonts\SegoeIcons.ttf` exists, else "Segoe MDL2 Assets". `FlowStep` now uses `Theme.icons` too. All 37 code points the UI uses exist in segmdl2.ttf 1.86 (checked with WPF `GlyphTypeface.CharacterToGlyphMap`). No font is bundled (Microsoft licence). Test hook `ZL_ICON_FONT=mdl2`.
- Verification: `icon_tests` (Rust), `tests/test_prereqs.py`; screenshot with `ZL_ICON_FONT=mdl2` on Windows 11 shows every icon. Unverified: the user's Windows 10 PC (its segmdl2.ttf version may differ).

### B-13: Platform selection before the Apple chooser (phase A2)            Status: fixed (unverified)
- Reported: 2026-11-06, phase A2 gate (master prompt section 2, startup flow acceptance tests).
- Cause: n/a (new flow). v2.2.31 opened the Apple Driver / ZenLink Studio chooser directly.
- Fix: `rust/crates/zenlink-app/src/flow.rs` (navigation rules), `prefs.rs` (`platform`, `rememberPlatform`, separate from the backend's `rememberConnectionMode`), `ui/app.slint` (`PlatformSelector`, `PlatformCard`, `AndroidHome`, Back in the Apple chooser, Settings card "Device type"), `main.rs` (wiring; Android poll reads an already running ADB server only, never starts or stops one; video and Apple Start only in the Apple workspace; going back stops a running Apple session).
- Verification: 10 app tests pass (`flow::tests` 7: fresh install, old config, iPhone path, Android path, Back from both paths, remembered platform, backend needs choice; `prefs::tests` 3: Indonesian + dark defaults, old file keeps language/theme, platform round trip). Screenshots with a stub backend and temporary prefs (no device, ADB port redirected): fresh install Indonesian, English, light theme with a saved selection, remembered Android, remembered Apple (chooser with Back). Unverified: clicking through Continue/Back in the real window, keyboard focus and screen-reader labels, and the Apple path with a real iPhone (needs the user).

### B-14: Android video mirroring over ADB with the scrcpy server (phase A3)            Status: fixed (verified)
- Reported: 2026-11-07, phase A3 gate (ROADMAP: a recorded scrcpy stream plays through `VideoView`, frame pacing stats work, USB mirroring on a device).
- Cause: n/a (new feature).
- Fix:
  - `zenlink-core`: `source.rs` (`VideoSource` / `RecordStream`: the decoder no longer depends on the backend's HTTP stream; `BackendSource` keeps the Apple path unchanged), `nal.rs` (`split_annexb`, `annexb_to_lengths`, `avcc_from_annexb`).
  - `zenlink-video`: `decoder::run` takes any `VideoSource`; `VideoView::start_source`. Same decoder, ring, renderer and OBS-safe `set_bounds` for both platforms.
  - `zenlink-android`: `adb.rs` (`push` over `sync:`, `reverse`, `forward` and their removal), `mirror.rs` (scrcpy v5.0.1 stream: 64-byte device meta, codec id, session/config/media packets; `Mirror::start`: push the server, reverse tunnel with forward fallback, `app_process` start, `Stopper`).
  - `zenlink-app`: `android.rs` (`DeviceSource` with 3 tries then an error, `FixtureSource` for `ZL_ANDROID_FIXTURE`, scrcpy server lookup with size check, `adb` lookup: `ZL_ADB`, bundled `platform-tools`, `ANDROID_HOME`, `ANDROID_SDK_ROOT`, `%LOCALAPPDATA%\Android\Sdk`, PATH); `main.rs` (Start per ready device and in the top bar, Stop, Restart, window fit for the Android picture; the poll starts an ADB server once when none runs and never kills one); `ui/app.slint` (Start buttons, connecting/error text).
  - Synthetic fixture `tests/fixtures/android/synthetic-h264-360x640.scrcpy` (106 KB, 120 frames) from `rust/tools/make_android_fixture.py`.
  - Video only in this phase: `audio=false control=false`, H.264, max 1920 px, 8 Mbps, 60 fps cap.
- Verification:
  - Tests: `zenlink-android` 52 (sync push chunks and FAIL, reverse/forward requests, packet round trip and wire layout against scrcpy's flags, device meta, server command, and an end-to-end `Mirror::start` against a fake adb + device: push 100 KB, reverse, shell, stream, stop closes the shell), `zenlink-core` 10, `zenlink-app` 18 (packet to record, fixture plays and loops paced by PTS, committed fixture is valid, missing fixture, other codecs refused), `zenlink-video` 5, pytest 81.
  - Measured on the developer's PC (Windows 11, stub backend, `ZL_ADB=none`, no device): the fixture played through `VideoView` with hardware decode, decoded 29.6-30.7 fps for a 30 fps stream, 0 frames dropped, presented once per display refresh (~170 Hz), the window fitted itself to the 360x640 portrait picture, and the recording looped (stream end, reopen) without errors.
  - User confirmed on 2026-11-07: USB mirroring works on a TECNO POVA 6 (`TECNO LI7`, Android 15). Log: `scrcpy: [server] INFO: Device: [TECNO] TECNO TECNO LI7 (Android 15)`, `android: mirroring "TECNO LI7" codec=h264`, `decoder H264 hardware=true`.
  - Still unverified: Wi-Fi, the forward-tunnel fallback on a device, rotation (new session + config), and very large keyframes.


### B-15: No feedback when an Android phone is plugged in without USB debugging            Status: fixed (unverified)
- Reported: 2026-11-07 by the user: "udah kusambungin ni ga terjadi apa apa" (TECNO POVA 6 on USB, Start ZenLink.bat, Android screen).
- Cause: proven on the user's PC: ZenLink started the ADB server, and `adb devices -l` lists no device. Windows shows the phone only as an MTP device (`USB\VID_0E8D&PID_2008`, class WPD); no ADB interface is exposed, so USB debugging is off (or the phone was not replugged after turning it on). The Android screen only said "No Android device yet".
- Fix: `zenlink-app/src/usbphones.rs` (Windows platform layer, SetupAPI, read-only): when ADB lists nothing and a present USB portable device (class WPD) is not Apple's (VID 05AC), the Android screen says "Windows sees a phone on USB, but ADB does not yet. Turn on USB debugging, then unplug and plug the cable in again."
- Verification: test `picks_usb_portable_devices_but_not_apple`. Unverified on a phone: plug in with USB debugging off.

### B-16: Android sound (phase A4, part 1)            Status: fixed (unverified)
- Reported: 2026-11-07, phase A4 gate ("audio socket into the shared audio engine, video-only fallback"; device check: sound on Android 11+). The user asked for sound before controls.
- Cause: n/a (new feature). The v2.2.31 sound engine (`NativeAudio`) is Python; Android code must be Rust (AGENTS.md section 6).
- Fix:
  - New crate `zenlink-audio`: `buffer.rs` is a line-by-line port of the v2.2.31 `NativeAudio` jitter buffer (start after one packet + margin, +5 ms per underrun capped at 50 ms, -1 ms after 30 s calm, 10 ms minimum, drop beyond twice the target, output block size counts); `wasapi.rs` is a WASAPI shared-mode, event-driven output on its own COM thread (48 kHz stereo s16 with AUTOCONVERTPCM, reopens when the device changes). No new third-party crate (cpal not needed; AGENTS.md section 3e latency rule).
  - `zenlink-android` `mirror.rs`: optional audio socket (`audio=true audio_codec=raw`: PCM s16le 48 kHz stereo, no decoder on the PC); codec id 0 (disabled) or 1 (error) means video only; the tunnel stays until all sockets are connected; `Stopper` closes every socket.
  - `zenlink-app`: `android.rs` `play_audio` thread; sound only on Android 11+ (`audio_supported`) and when Settings › Audio is "On this PC" ("Device only" keeps the sound on the phone; since B-18 a change restarts a running session at once); volume and mute follow the window at once; the stats line shows the sound latency.
  - Low-latency mode only (sound as it arrives, picture as it decodes), as on the Apple cable path. A/V sync modes are still to do.
- Verification:
  - Tests: `zenlink-audio` 6 buffer tests (start rule, underrun growth and calm shrink, cap and floor, drop beyond twice the target, Bluetooth-size blocks, jitter floor); `zenlink-android` 55 (raw audio socket end to end, device without audio falls back to video only, server command); all workspace tests, fmt, clippy (no new warnings).
  - Measured on the developer's PC, default output, silence (muted): WASAPI output latency 23.4 ms (v2.2.31 measured ~22-23 ms), 0 underruns in 2 s; socket to output with 10 ms packets in real time: 53 ms total (queue + output).
  - Unverified: sound from a real phone (TECNO POVA 6, Android 15) on USB, Bluetooth headsets, Android 10 (video-only fallback) on a device.

### B-17: OBS Game Capture is less smooth than the ZenLink window            Status: fixed (unverified)
- Reported: 2026-11-07 by the user, forwarding a ZenLink Studio (Apple) user's report: the mirror in the app is smooth, but OBS Game Capture shows frame drops ("tetep ga 1:1 sama yang di hasil mirror ... tetep ada frame drop kalau di obs, cuma ga se sering yang v sblmnya"). Applies to every mode (Apple Driver, ZenLink Studio, Android): all of them render through `zenlink-video` `VideoView`.
- Cause (measured, developer PC: AMD RX 6700 XT, 2560x1440 at 170 Hz; Android fixtures at 15/30/60 fps through the real renderer, `ZL_STATS_LOG=1` pacing probe):
  - OBS's graphics hook copies the back buffer only inside `Present`, at most once per `frame_interval` (obs-studio `graphics-hook.h` `frame_ready`/`capture_ready`; `game-capture.c`: interval = OBS frame time, halved unless "Limit capture framerate" is on). OBS then renders the latest copy on its own clock.
  - The renderer presented once per refresh and took a new frame only at the next refresh. A decoded frame therefore reached `Present` 0 to 1 refresh late (decode-to-present p50 3.5 ms, max 11.7 ms at 170 Hz; up to 16.7 ms on a 60 Hz laptop). 60 fps content was presented at 2/3/4-refresh gaps (p50 17.64 ms, max 23.65 ms) instead of 16.7 ms. On screen the eye does not see it, but the hook samples that jittered timing and OBS samples it a second time at 60 fps, so frames repeat and drop in the recording.
  - Also measured, not the cause: 15-20 presents per second wait one extra refresh on the swap chain's waitable object (work <= 0.3 ms, Present <= 0.5 ms). It does not change with 2 or 3 buffers, frame latency 1 or 2, the animated background on or off, or content at 15, 30 or 60 fps, so it is outside the renderer (DWM composition).
- Fix (`zenlink-video` `view.rs`, `d3d.rs`): after the swap chain has room (frame-latency waitable, unchanged: still at most one new frame per refresh, FIFO order, no "newest wins"), the render thread waits for the next decoded frame (`Ring::arrived` condvar, signalled by `Ring::push`) and presents it at once. Idle, it presents at least every 50 ms; a resize request wakes it at once (OBS re-hook handshake, AGENTS section 5 item 1). `ZL_PRESENT=vblank` restores the old behaviour for A/B tests. The probe logs `pacing ...` and an `obs-model ...` line once a second with `ZL_STATS_LOG=1`.
- Verification (measured, same PC, 20 s per run):
  - Decode-to-present: p50 3.5 ms -> 0.3 ms; max 11.7 ms -> 0.5 ms (60 fps).
  - New-frame present gaps at 60 fps: p50 17.64 / max 23.65 ms -> p50 16.70 / max 17.44 ms. Frames dropped by catch-up: 6 -> 0. Presents per second: ~150 -> content rate (60).
  - OBS model at 60 fps output (the hook rules above, averaged over 32 clock phases): ZenLink-added repeats/skips per second 14.6/3.1 -> 1.3/0.2 (capture 16.9/5.3 -> 4.3/0.6; the floor for frames captured exactly at decode time is 2.3-3.0/0.4-2.2, the beat between two 60 Hz clocks).
  - Resize handshake: 7.9-104 ms -> 1.5-3.8 ms, no 120 ms timeouts.
  - Tests: `push_wakes_a_waiting_renderer_and_records_the_frame` plus the existing ring tests.
  - Unverified: real OBS Game Capture on the user's or tester's PC (Apple and Android), a 60 Hz laptop, a laptop with two GPUs (OBS and ZenLink on different GPUs adds a cross-adapter copy; ask for the GPU names if it still stutters), the Apple path end to end (same renderer code, not run with the fake backend this time).

### B-18: "Device only" sound did nothing during an Android session            Status: fixed (unverified)
- Reported: 2026-11-07 by the user ("mode audio hanya perangkat belum"): switching Settings › Audio to "Device only" while mirroring kept the sound on the PC.
- Cause: the audio choice is a scrcpy server start option (`audio=true|false`); `DeviceSource` read it only when a session started (code path `DeviceSource::new`), and nothing restarted the session.
- Fix: `main.rs` `android_key()` (audio target + picture options); `on_prefs_changed` restarts a running Android session when the key changes ("Applying the Android settings…"). With "Device only" the server starts with `audio=false`, so Android keeps playing on the phone.
- Verification: build and fixture smoke test (window starts, picture plays). Unverified on a phone: sound moves back to the phone after the switch.

### B-19: Android picture settings before Start (resolution, FPS, bitrate, H.264/H.265)            Status: fixed (unverified)
- Reported: 2026-11-07 by the user (AGENTS.md section 6, Android settings).
- Fix:
  - `AndroidVideoCard` (app.slint) on the Android start screen and in Settings when the Android screen is open: resolution 720p/1080p/1440p/native (`max_size`), FPS cap 30/60/90/120/144 (`max_fps`), bitrate 4/8/16/24/40 Mbps (`video_bit_rate`), codec H.264 / H.265. Saved in prefs (`androidMaxSize`, `androidFps`, `androidBitrate`, `androidCodec`); a change restarts a running session.
  - H.265 is offered only when Windows has an HEVC decoder (`zenlink_video::decoder_available`, Media Foundation lookup); hvcC from the scrcpy config (`nal::hvcc_from_annexb`). Bounded fallback: a failed H.265 start (no encoder on the phone, bad stream) retries with H.264 (`DeviceSource::fail`).
- Not done yet: per-device settings (master prompt section 7), hiding FPS values above the phone's refresh rate (the server caps to what the encoder delivers; the measured FPS is in the stats line), custom values, encoder choice by name.
- Verification: tests `hvcc_from_scrcpy_config_round_trips`, `h265_config_becomes_an_hvcc_record`, `failed_h265_falls_back_to_h264`, prefs round trip. Unverified on a phone: each option, H.265 on a phone with an HEVC encoder.

### B-20: Android control from the PC (touch, scroll, buttons, keyboard, paste)            Status: fixed (unverified)
- Reported: 2026-11-07 by the user ("tombol belum bisa", "update buat control di pc nya ntuk android").
- Cause: the server was started with `control=false`; the bottom bar sent Apple actions only.
- Fix:
  - `zenlink-android` `control.rs`: scrcpy v5.0.1 control messages (inject keycode, text, touch, scroll, back-or-screen-on, notification/settings panels, set clipboard, display power, rotate), byte layouts checked against `ControlMessageReader.java` v5.0.1.
  - `mirror.rs`: `control=true clipboard_autosync=false`, third socket (video, audio, control); device messages on the control socket are drained so it never fills.
  - Window: left button = touch (generic finger, frame pixels from the last session packet, the same rotation mapping as the Apple path), wheel = scroll, right click = BACK, middle click = HOME (as scrcpy). Android bottom bar: Back, Home, Recents, Quick settings, Notifications, volume down/up, Power. PC keyboard (Settings › PC keyboard): text as text, editing and arrow keys as key codes, Ctrl+letter with Ctrl, Ctrl+V pastes the PC clipboard. Touch toggle gates mouse input.
- Not done yet: device clipboard to PC, UHID keyboard/mouse/gamepad, multi-touch gestures, key repeat counts.
- Verification: tests `control.rs` (7 layout tests), `keys_map_like_scrcpy`, `control_goes_to_the_socket_in_frame_pixels`; all tests, fmt, clippy (no new warnings). Unverified on a phone.

### B-21: Beta window crashed at start after the HEVC check was added            Status: fixed (verified)
- Reported: 2026-11-07, found by the developer's own smoke test before release (never shipped).
- Cause: `hevc_ok()` ran Media Foundation (multithreaded COM) on the UI thread before winit created the window; winit then panicked: `OleInitialize failed! Result was: RPC_E_CHANGED_MODE` (backtrace from `CreateWindowExW`).
- Fix: the HEVC lookup runs on its own short-lived thread (`main.rs` `hevc_ok`).
- Verification: the release exe starts and plays the Android fixture (`decoded=30.6 presented=157.2` in the stats log, no errors).

### B-22: Settings slide jitters / blinks when opening or closing            Status: fixed (unverified)
- Reported: 2026-11-07 by the user ("kalau tutup buka setting, animasinya kek rada ngeblink ato ngebug").
- Cause (measured with `ZL_SETTINGS_DEMO=1 ZL_STATS_LOG=1`, which opens and closes Settings every 1.5 s and logs every tick): the panel width came from a Slint animation (`sp`), while the window width was set from a 16 ms timer that read that animation. By the time Slint drew, the animation had moved on, so the panel was wider or narrower than the window and the picture area jumped. Stage width, which should stay constant: closing 421 -> 612 -> 484 -> 455 -> 433 -> 454 px; opening 454 -> 284 -> 392 -> 418 -> 454 px. The same code is in v2.2.31.
- Fix: `sp` is set from Rust (`main.rs` `slide`, `slide_ease` = the same `cubic-bezier(0.05, 0.7, 0.1, 1)`, 320 ms open / 240 ms close) on the same tick as the window width.
- Verification: same measurement after the fix: opening keeps the stage at 453.5-454.5 px (rounding); closing keeps it at 421 px until the window reaches its 470 px minimum, then it widens smoothly to 454 px (the minimum window width, unchanged). Test `slide_curve_matches_the_css_bezier`. Unverified: the user's eyes on their screen.

### B-23: Mouse clicks did not touch the Android phone            Status: fixed (unverified)
- Reported: 2026-11-07 by the user: the keyboard works, clicks on the picture do nothing.
- Cause: touch needs the video size (scrcpy ignores a touch made for another size). The size comes in the session packet, which the server sends before the first config packet. `first_config()` read and dropped that packet, so the size stayed 0 and `Shared::pointer` sent nothing. Keys need no size, so they worked.
- Fix: `first_config` stores the session size (`android.rs`).
- Verification: test `session_size_before_the_config_is_kept_for_touch` (fails before the fix). Unverified on the phone.

### B-24: Android Quick settings / Notifications buttons did not close the panel            Status: fixed (unverified)
- Reported: 2026-11-07 by the user: pressing the button again does not close the panel.
- Cause: the buttons only sent "expand"; there was no state to send "collapse".
- Fix: the bar remembers the open panel; the same button again sends COLLAPSE_PANELS, the other one switches; a touch, Back, Home or Recents forgets it (the phone closes the panel itself).
- Verification: test `panel_buttons_toggle`. Unverified on the phone.

### B-25: Default window size more roomy            Status: fixed (verified)
- Reported: 2026-11-07 by the user (screenshot of the window with Settings open, wanting that size at start).
- Fix: `app.slint` preferred size 620 x 900 (was 480 x 820); on first show the window is kept inside the monitor's work area (95%) for small or high-DPI screens (`clamp_to_work_area`). With Settings open at start (the default) the width is the idle stage plus the panel, as before.
- Verification: measured at 100% scaling on a 2560x1440 screen: the window opens at 824 x 900 (was 824 x 820) on the Android screen and the platform screen.

### B-26: Settings and README list shortcuts the Rust window does not have            Status: fixed (unverified)
- Reported: 2026-11-07, by the user (screenshot of the README "Keyboard Shortcuts" table: "belum lengkap di setting").
- Cause: the shortcuts (Space, Alt+H/C/R/V/D/U/L, Ctrl+K, Alt+Left, G/M) live in the classic web UI (`ui/app.js`, `ui/keys.js`). The Rust window handles only F11/Esc and, for Android, typed keys. The Settings card "Control & shortcuts" and the README still describe the classic set.
- Fix: `src/shortcuts.rs` (routing + US-layout HID table, 5 tests) and `main.rs` `run_shortcut`. Both: Space, M, Ctrl+, , Alt+H, Alt+Left, Alt+L. Apple Driver mode: the PC keyboard as a hardware keyboard (`/cast/key`), Alt+U/D/V, Ctrl+K, Ctrl+C/X/V/A/Z (`/api/keyboard/action`). Android: Alt+N/S/R, Alt+Up/Down. A click on the picture gives the keys to the window on both platforms. Settings now show a "Keyboard shortcuts" card listing exactly this set per platform. Alt+C / Alt+R (OCR) ported too: the renderer reads the shown picture back from the swap chain (`VideoView::snapshot`), ZenLink sends it as BMP to the backend's Windows OCR (`/api/ocr`) and puts the text on the PC clipboard.
- Verification: unverified: needs an iPhone in Apple Driver mode and an Android phone. Routing is unit-tested.

### B-27: Windows "N" editions have no Media Foundation decoder            Status: fixed (unverified)
- Reported: 2026-11-07, from the clean-PC review (user asked whether ZenLink runs on other PCs without extra installs).
- Cause: N/KN editions ship without the Media Feature Pack, so the H.264 decoder the picture needs is missing. Not seen on a device yet; from Microsoft's edition notes.
- Fix: `ZenLink.exe` delay-loads `mfplat.dll` (`build.rs`: `/DELAYLOAD:mfplat.dll` + `delayimp.lib`), so Windows can start it without Media Foundation. `zenlink_video::media_foundation_present()` guards every Media Foundation call; at start a bilingual sheet explains the missing Media Feature Pack and opens Windows' own Settings › Optional features (`ms-settings:optionalfeatures`). ZenLink downloads nothing.
- Evidence: `pefile` shows `mfplat.dll` only in the delay-import table.
- Verification: unverified: needs an N edition (VM or Windows Sandbox, with the user's approval).

### B-28: Release packages carry neither adb nor the scrcpy server            Status: fixed (unverified)
- Reported: 2026-11-07, found in the A6 audit (the user asked why the Android screen says "ADB not found" when adb should be bundled).
- Cause: `docs/android/ADB.md` (A1) decided to bundle Google's `adb`, but `build_portable_release.ps1` and `packaging/ZenLink.iss` never copied it, nor `third_party/scrcpy/scrcpy-server-v5.0.1`. `find_adb` / `load_server` look next to the exe and in parent folders, so the source checkout worked, and adb came from `ANDROID_HOME=E:\AndroidDev\sdk` set in the untracked `Start ZenLink.local.bat`. Running `rust\target\release\ZenLink.exe` without that variable shows "ADB not found". An installed or portable build on another PC would have had no Android at all.
- Fix: the scrcpy server is built into `ZenLink.exe` (`include_bytes!`, 733,930 B; test `builtin_server_matches_the_pinned_release`). The build script takes Google's platform-tools (`-PlatformTools`, or ANDROID_HOME / ANDROID_SDK_ROOT / Android Studio's SDK), checks that `adb.exe`, `AdbWinApi.dll` and `AdbWinUsbApi.dll` are validly signed by Google LLC, prints the revision and SHA-256s, and puts them with `NOTICE.txt` in `third_party\platform-tools` (installer and source zip). Portable.exe packs them (`ZENLINK_ADB_DIR`) and unpacks them to `<data folder>\platform-tools` on first use. Local platform-tools: 37.0.1, all three signatures Valid (Google LLC), 2026-11-07.
- Verification (2026-11-07, full `build_portable_release.ps1` run, no device, temporary prefs, stub backend, private adb port 47599 so the user's adb on 5037 was not touched, `ANDROID_HOME`/`ANDROID_SDK_ROOT`/`ZL_ADB` unset):
  - Installed layout (`dist\ZenLinkService\ZenLink.exe`): the app started `dist\ZenLinkService\third_party\platform-tools\adb.exe` as the adb server; the Android screen shows "No Android device yet" (no "ADB not found").
  - Portable.exe: `%LOCALAPPDATA%\ZenLink-Next\platform-tools` did not exist before; the app unpacked the four files there and started that `adb.exe`. All four SHA-256s equal the originals; the Authenticode signature of the unpacked `adb.exe` is Valid (Google LLC).
  - Repeated on the v2.5.0 packages (round-2 cleanup): same result for both.
  - Still unverified: a PC without any Android SDK, and a real phone (USB and Wi-Fi), with the user's consent.

### B-29: "Close to the tray" and "Start when plugged in" do nothing in the Rust window            Status: fixed (verified)
- Reported: 2026-11-07, found in the release audit (README claim check).
- Cause: `prefs.rs` stored `closeToTray` and `autoStart` and Settings › Advanced showed both toggles, but nothing in `zenlink-app` read them; only the classic window implemented a tray. Same in v2.2.31.
- Fix (owner's request 2026-11-07: tray and "start with Windows"): `zenlink-app/src/tray.rs`.
  - Tray icon (Shell_NotifyIcon) while "Close to the tray" is on; left click shows or hides, menu Open / Quit (Indonesian or English); re-added after Explorer restarts.
  - The close button and Alt+F4 hide the window to the tray when the option is on (the session keeps running), else quit.
  - "Start when plugged in" (never implemented) is replaced by "Start with Windows": an `HKCU\...\Run` value `ZenLink` = `"<exe>" --background`, written only when the user turns it on, removed when off and by the uninstaller. `--background` starts in the tray (or minimised when the tray option is off).
  - One ZenLink per user: a second start shows the running window (also when it is hidden in the tray). Tests with `ZL_PREFS` are exempt.
- Verification: test `run_command_quotes_the_path_and_starts_in_the_background`; smoke test (temp prefs, no device): `--background` with the tray option leaves the window hidden, and the helper's show request (the same one a second start sends) makes it visible. 2026-11-07, installed v2.5.0 with the owner's settings (backed up and restored): turning on "Start with Windows" wrote `HKCU\...\Run\ZenLink = "%LOCALAPPDATA%\Programs\ZenLink\ZenLink.exe" --background`; launching exactly that command (what Windows runs at sign-in) kept the window hidden and `Shell_NotifyIconGetRect` found the tray icon. Still to see by eye: the tray menu, and a real sign-in after a restart.

### B-30: The app was still called "ZenLink Next"            Status: fixed (verified)
- Reported: 2026-11-07 by the owner: "nama jadi ZenLink".
- Fix: window, installer, shortcuts, AirPlay name (`ZenLinkStudio`), firewall rule and scripts say "ZenLink"; version 2.5.0. The installer now uses v2.2.x's AppId and folder, so Setup upgrades an installed v2.2.x in place and removes its old `_internal` first. v2.5 keeps its own data folder (`ZenLink-Next`) and ports (47421, AirPlay 7100), so a v2.2.31 Portable.exe still runs next to it; rollback = the v2.2.31 Setup from its release. The firewall helper now deletes only its own program's rule, so an installed and a portable copy do not remove each other's rule.
- Verification: `tests/test_next_isolation.py` (AppId, name, `_internal` cleanup, no "Next" in the installer). 2026-11-07 (owner approved): v2.2.31 Setup installed silently per user, then v2.5.0 Setup over it: one uninstall entry `ZenLink 2.5.0` at the same folder, exit code 0, all v2.2.31-only folders gone (the first run left 3 small portaudio/`__init__.py` files; added to `[InstallDelete]`, re-run clean: installed files = build output + uninstaller). Not tested: a clean PC / Windows Sandbox (Sandbox is not enabled on this PC) and Windows N.

### B-31: Slint warning "'maximized' shadows the builtin property"            Status: fixed (verified)
- Reported: 2026-11-07, build output (also in v2.2.31).
- Fix: `AppWindow.maximized` renamed `is-maximized` (`app.slint`, `main.rs`).
- Verification: `cargo build` / `cargo clippy` print no warnings.

### B-32: Toasts and tooltips hidden behind the picture            Status: fixed (verified)
- Reported: 2026-11-07 by the owner (screenshot: the "Ingat: Alt+R…" toast cut by the video; a Quick Settings tooltip clipped).
- Cause: the picture is a child window (D3D11 swap chain, needed for OBS and frame pacing). A child window always covers what the parent window (Slint) draws in its area.
- Fix: `VideoView::set_holes` cuts the toast and tooltip rectangles out of the picture's window region (`SetWindowRgn`, rounded rect minus holes); `main.rs` passes the rectangles each frame tick, only changed regions are applied. OBS still captures the full swap chain (the region only affects what Windows shows).
- Verification: screenshot with the recorded Android fixture and a test toast (`ZL_TOAST_TEST`): the toast shows over the playing picture.

### B-33: The window opened too small            Status: fixed (verified)
- Reported: 2026-11-07 by the owner.
- Fix: the first fit uses up to 85% of the work-area height (was smaller), `clamp_to_work_area` keeps at least 85%; a size the user picked is still remembered.
- Verification: test start on the owner's PC opened at 824×900 (screenshot).

### B-34: Shortcuts: Space removed, sound needs Alt+M            Status: fixed (verified)
- Reported: 2026-11-07 by the owner: Space for start/stop must go; sound on/off must be a combination, not plain M.
- Fix: `shortcuts.rs` never maps a plain key (all keys go to the phone or iPad); `Alt+M` toggles the PC sound on both platforms and shows a toast; Space start/stop removed; the in-app list, README and swipe tips updated (swipe from the left edge = back).
- Verification: test `plain_keys_are_never_shortcuts_and_alt_m_mutes`.

### B-35: Android Wi-Fi pairing needed typing an address and a code            Status: fixed (unverified)
- Reported: 2026-11-07 by the owner: "bisa sambungkan dengan QR, biar simple".
- Fix: "Pair with QR" in the Wi-Fi card shows `WIFI:T:ADB;S:zenlink-<random>;P:<random>;;` (the Android Studio format). ZenLink polls the ADB server's mDNS list (`host:mdns:services`) for `_adb-tls-pairing` with that name, pairs with the password, then connects to the phone's `_adb-tls-connect` service at the same IP. Gives up after 2 minutes; Cancel stops it. The code pairing stays.
- Verification: tests `mdns_list_and_qr_pairing_lookup`, `qr_text_and_tokens`; the QR shown in the app (`ZL_QR_TEST`) decodes with zxing-cpp to the expected text. Unverified: an Android 11+ phone scanning it on the same Wi-Fi.

### B-36: Settings silently reset when the file has a UTF-8 byte order mark            Status: fixed (verified)
- Reported: 2026-11-07, found in the autostart test: a settings file written by Windows PowerShell (`Set-Content -Encoding utf8`, also older Notepad) loaded as defaults.
- Cause: `serde_json` rejects a leading BOM; `prefs::load` then fell back to the defaults.
- Fix: `prefs::parse` strips a BOM before parsing.
- Verification: test `a_file_with_a_byte_order_mark_still_loads`.

### B-37: Android sound crackles on the PC            Status: fixed (unverified)
- Reported: 2026-11-07 by the owner ("audio ga jernih, pecah-pecah"), TECNO POVA 6 over USB.
- Cause: the owner's log (`zenlink-next-v25.log`) shows, every 5 s, 1-3 gaps and 54-267 ms of sound dropped (`sound: 2 gaps, 117 ms dropped in 5 s; margin 20.0 ms ...`). Android used the Apple cable buffer settings: a 10 ms margin and chunk drops when the queue passed twice the target. Sound over ADB arrives in bursts next to the video, and the phone's clock drifts from the PC's, so the queue ran dry (gap) and then overflowed (cut) in turn.
- Fix: `zenlink-audio` smooth mode for Android only: 40 ms kept queued (60 ms over Wi-Fi; scrcpy's own default is 50 ms), drift corrected by playing up to 0.5% faster or slower (linear interpolation, about 8 cents), a hard cut only 150 ms above the target. Apple playback is unchanged.
- Verification: tests `smooth_mode_absorbs_drift_and_bursts_without_cuts` (120 s simulated, ±0.3% drift, 3-packet bursts: no drops, at most the first start wait) and `plain_mode_cracks_under_the_same_load` (the old mode reproduces the gaps/drops). Unverified: listening on the phone.

### B-38: Android header showed the model code ("TECNO LI7") instead of the device name            Status: fixed (unverified)
- Cause: when the picture starts, the scrcpy header's name (`ro.product.model`) replaced the list's marketing name.
- Fix: `android.rs` keeps the list's name; the scrcpy name is only a fallback.
- Verification: code path; unverified on the device.

### B-39: A phone on USB and Wi-Fi at once: no way to pick the link            Status: fixed (unverified)
- Fix: the row says "USB + Wi-Fi" and shows two Start buttons (USB, Wi-Fi); the choice is kept for restarts (`android_via`).
- Verification: builds; unverified on a device.

### B-40: Wi-Fi connect showed no progress; instructions were hard to follow            Status: fixed (verified)
- Fix: a moving bar and the current step (1/3 reading the address, 2/3 turning on ADB over Wi-Fi, 3/3 connecting; pairing; connecting) under the device list; the Wi-Fi card is now "Option 1 · Scan a QR code" (numbered steps) and "Option 2 · Pairing code or IP address" (collapsed, numbered steps).
- Verification: screenshot of the card (`ZL_QR_TEST`).

### B-41: Opening the (i) explanations made cards overlap; the pink box clashed            Status: fixed (verified)
- Cause: wrapped text nested inside another element or layout is measured too short by Slint 1.18, so the card was shorter than its content.
- Fix: the explanation is a direct, neutral text in the card (with a separator); steps are plain texts.
- Verification: screenshot with every explanation open (`ZL_INFO_OPEN=1`): no overlap.

### B-42: Default window still too small/narrow            Status: fixed (verified)
- Fix: first sizing uses 85% of the work-area height and up to 55% of its width; the idle stage next to Settings is 32% of the screen (440-640 px); the settings slide no longer resizes with the old height in the first ticks.
- Verification: on a 2560x1392 work area the window opens at 1408x1183 (Android) / 1024x1183 (Apple with Settings), was 824x900.

### B-43: iOS Control Center button sometimes did the wrong thing            Status: fixed (unverified)
- Cause: ZenLink remembered "Control Center is open" forever; if the user closed it on the device, the next press sent the close swipe (up from the bottom), which left the app.
- Fix: the remembered overlay is forgotten after a click on the picture or 20 s. Alt+Q (Control Center) and Alt+N (Notification Center) now work on iPhone/iPad too.
- Verification: test `platform_specific_shortcuts`; unverified on an iPhone.

### B-44: Auto unlock with a saved passcode (feature)            Status: fixed (unverified)
- Request: 2026-11-07 by the owner (Apple mode only, not ZenLink Studio).
- Fix: Settings › "Auto unlock": save a passcode or password (Windows DPAPI, existing `/api/passcode`), toggle "Unlock when mirroring starts" (sends `wake` 1.5 s after the picture starts, Apple Driver/Hybrid only). Alphanumeric passwords and custom-length codes now type with Shift and Return.
- Verification: tests `PasscodeTests` (3); unverified on an iPhone.

### B-45: No Cable/Wi-Fi, mode or Start on the Apple "Plug in" screen            Status: fixed (unverified)
- Reported: 2026-11-07 by the owner (screenshot): the empty Apple screen only said "Plug in an iPhone or iPad"; Cable/Wi-Fi and Change mode were only in Settings.
- Fix: the Apple placeholder shows the mode (Apple Driver / ZenLink Studio · Change), a Cable (USB) / Wi-Fi switch (same `set-link` as Settings) and a Start button; the hint text follows the chosen link.
- Verification: builds; seen with the fake backend (no device) in both modes. Unverified with an iPhone.

### B-46: Settings showed options the chosen Apple mode does not have            Status: fixed (unverified)
- Reported: 2026-11-07 by the owner: hide what a mode does not support.
- Cause: ZenLink Studio (USB Direct / AirPlay) has no touch, keyboard, buttons or device shortcuts: `server.py` `hub_route` answers 409 for touch/key/button and `/api/keyboard/action` only runs for the `apple`/`hybrid` backends. Wi-Fi setup is Apple Driver only; the Studio driver install is Studio only.
- Fix: in ZenLink Studio the Control card, the device shortcuts (Back, Lock, Notification/Control Center, U, D, V, Ctrl+K, Ctrl+C...), Auto unlock, Set up Wi-Fi, Restore Apple driver and the bottom dock are hidden; in Apple Driver mode the Install ZenLink Studio driver button is hidden. The libusb0 filter warning stays in both. Device audio (On this PC / Device only) stays in both: it works in both.
- Verification: builds; seen with the fake backend: ZenLink Studio has no Control card or dock, Apple Driver has both. Unverified with an iPhone.

### B-47: Window opened at the top left, not in the middle of the screen            Status: fixed (verified)
- Reported: 2026-11-07 by the owner (screenshot on a 2560x1440 screen).
- Cause: the first sizing (`clamp_to_work_area`) set the size but kept Windows' default position (26, 26); the fit to the picture then changed the width again.
- Fix: the first tick centres the window in the monitor's work area and keeps it centred while its size settles (up to 3 s, stops if the user moves it, not during the Settings slide).
- Verification: test `first_window_opens_in_the_middle_of_the_work_area`; measured with the fake backend: window centre (1280, 696.5) on a 2560x1392 work area, stable at 1, 2, 3 and 8 s.

### B-48: Android apps in their own window (v2.5.1 feature)            Status: fixed (partly verified)
- Reported: 2026-11-07, owner: open a phone app (e.g. WhatsApp) as its own PC window, like TwinDesk. Idea only; code written from the scrcpy v5.0.1 sources.
- Cause: new feature. scrcpy 5.0.1 has `list_apps`, `new_display` (Android 10+, `Server.java`), `flex_display` with `RESIZE_DISPLAY` (type 21: u16 width, u16 height) and `START_APP` (type 16: 1-byte length + package; `+` force-stops first), read from `Options.java`, `ControlMessageReader.java`, `Controller.java` and `LogUtils.buildAppListMessage`.
- Fix: `zenlink-android/src/apps.rs` (app list parser, package-name check), `control.rs` (StartApp, ResizeDisplay), `mirror.rs` (`NewDisplay`, `new_display=WxH/dpi flex_display=true`). Android screen: an "Apps" button per Android 10+ device, a searchable picker. A pick starts `ZenLink.exe --android-app serial=… package=…` (`appwin.rs`, `ui/appwin.slint`): its own window and taskbar entry, the same VideoView, a virtual display sized to the window (multiples of 8, 160 dpi × display scale) that follows window resizes after 250 ms, Back and Restart app, keyboard and Ctrl+V like the main window, sound off (the phone keeps it). No backend, tray or single-instance check in that process; log `%TEMP%\zenlink-v251-beta-apps.log`.
- Verification: tests `apps::tests` (6), `control::tests::start_app_and_resize_layout`, `mirror::tests::new_display_in_the_server_command`, `appwin::tests` (5); manual: an app window started without a phone opens and shows "HP tidak tersambung lagi…" with Try again (screenshot). Measured on a TECNO POVA 6 (LI7, Android 15, USB, 2026-11-07, owner's consent): the real app list parsed 389 of 389 entries (`apps::tests::real_list` with `ZL_APPS_LIST`, incl. a long-name entry); `--android-app` opened Chrome on virtual display 231 (424x776/160, hardware H.264 `c2.mtk.avc.encoder`) in its own window; resizing the window to 900x620 re-laid Chrome out in landscape (flex display); closing the window ended the session and left no scrcpy display or `app_process` on the phone. Unverified: the Apps button and picker in the main window (needs clicks), touch and keyboard in the app window, several app windows at once, Wi-Fi.

### B-49: Update check and in-app update (v2.5.1 feature)            Status: fixed (partly verified)
- Reported: 2026-11-07, owner: notify about a new version and update in the app, like Claude Code or Codex; the new version replaces the old one, the user's settings stay, nothing old piles up; switch between Beta and Stable.
- Cause: new feature. Design in AGENTS.md section 10 item 5; owner chose a fixed install path with a clean install (not one folder per version) so shortcuts, taskbar pins, the firewall rule, autostart and OBS window capture keep the same exe path.
- Fix: `zenlink-core/src/update.rs` (semver order with pre-releases, channel Stable/Beta, newest release from the GitHub releases JSON, drafts skipped, assets accepted only from `https://github.com/zenvyzenshiro/zenlink/releases/download/` with a `sha256:` digest, short notes). `zenlink-app/src/updater.rs`: HTTPS GET through WinHTTP (TLS 1.2/1.3, no HTTPS-to-HTTP redirect) and SHA-256 through BCrypt (no new crates), download to `.part`, size and SHA-256 check, nothing left on failure; install kind: Installed (`unins000.exe` next to the exe) runs Setup `/SILENT /SP- /SUPPRESSMSGBOXES /NORESTART /MERGETASKS=!firewall /ZLRELAUNCH=1` and quits; Portable downloads the new exe next to the old one and starts it with `--replace-old=<old>`, which waits for the old process and deletes the old exe; a source checkout opens the release page. Switching channel installs the other edition next to this one (its Setup runs normally, nothing quits). Settings › Updates card: version, "Check at start" (prefs `updateCheck`, default on), channel (prefs `updateChannel`, "" = the build's own), status, progress, notes, Update and Release notes buttons; a start check 8 s after start shows a toast when a newer version exists (skipped with `ZL_PREFS` unless `ZL_UPDATE_FEED` is set). `packaging/ZenLink.iss`: `[Code]` removes exactly the entries listed in the previous install's `install-manifest.txt` before copying (never `unins*`, never names with paths), and a `[Run]` entry relaunches ZenLink after a silent update; `build_portable_release.ps1` writes the manifest. Settings in `%APPDATA%`/`%LOCALAPPDATA%` are never touched by an update.
- Verification: tests `update::tests` (6), `updater::tests` (4: SHA-256 known values, https-only URL split, replace only an exe next to this one, a failed download leaves nothing). Measured 2026-11-07: `updater::tests::live_download` (ignored, real network) fetched the GitHub feed and downloaded the v2.5.0 Portable.exe (54,850,560 bytes in 2.0 s, through GitHub's redirect), size and SHA-256 matched the published digest, a wrong digest was refused and left no file. The app against the real feed logged `update: up to date (Beta)`; against a local test feed it showed "Versi baru tersedia: v2.5.1-beta.2" with notes and the Update button (screenshot). ISCC compiles the changed `.iss`. ZenLink.exe release build: 13,931,008 bytes. Unverified: a real silent Setup upgrade (manifest cleanup and relaunch) and a real portable replace, which need a published newer release; channel switch install.
