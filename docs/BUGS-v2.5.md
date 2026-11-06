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
- Fix: n/a.
- Verification: unverified: needs an iPhone on the ZenLink Studio cable path, "Device only" selected; check that the sound plays on the phone and not on the PC.

### B-2: OBS portrait/landscape with Android rotation            Status: open
- Reported: 2026-11-06, carried over (item 2). The iPhone capture survives rotation (user confirmed in v2.2.31). Positioning uses OBS "Fit to screen" (Ctrl+F), as the Settings OBS card says.
- Cause: n/a (Android path not built yet).
- Fix: n/a. Android must render through the same `VideoView` and keep the `set_bounds` 120 ms wait.
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

### B-7: Every prerequisite is bundled or explained by ZenLink itself            Status: open
- Reported: carried over (item 7, general rule).
- Cause: n/a. Tracks B-5 and any future missing-DLL / missing-service case.
- Fix: n/a.
- Verification: checked per release on a clean PC.

---

## New in v2.5

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

### B-10: Licence audit before bundling Android platform-tools `adb`            Status: open (audited, needs the owner's decision)
- Reported: 2026-11-06, phase A1 (master prompt section 13, AGENTS.md §6 item 9).
- Cause: the ADB decision (`docs/android/ADB.md`) bundles Google's `adb.exe`, `AdbWinApi.dll` and `AdbWinUsbApi.dll` next to a GPL-3.0 program. The platform-tools zip comes with Google's SDK licence and a `NOTICE.txt`; whether redistribution as a separate program is allowed was not audited yet.
- Fix: audited 2026-11-07 for platform-tools 37.0.1 (`docs/THIRD_PARTY.md`): SDK licence §3.4 forbids redistribution except where third-party licences require it; §3.5 puts open-source components (adb is AOSP, Apache-2.0 per `NOTICE.txt`) under their own licence alone. The files ship unmodified with `NOTICE.txt` and credits (B-28).
- Verification: the reading is recorded. Open: the owner's decision before a public release (not legal advice).

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


### B-15: No feedback when an Android phone is plugged in without USB debugging            Status: open
- Reported: 2026-11-07 by the user: "udah kusambungin ni ga terjadi apa apa" (TECNO POVA 6 on USB, Start ZenLink.bat, Android screen).
- Cause: proven on the user's PC: ZenLink started the ADB server (log: `android: starting the ADB server with E:\AndroidDev\sdk\platform-tools\adb.exe`), and `adb devices -l` lists no device. Windows shows the phone only as an MTP device (`USB\VID_0E8D&PID_2008`, class WPD); no ADB interface is exposed, so USB debugging is off (or the phone was not replugged after turning it on). The Android screen only says "No Android device yet", which does not tell the user the phone was seen.
- Fix: planned: on Windows, detect a plugged-in phone that has no ADB interface and say so ("phone seen, turn on USB debugging"), behind the platform layer.
- Verification: unverified.

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
  - Still unverified: a PC without any Android SDK, and a real phone (USB and Wi-Fi), with the user's consent.

### B-29: "Close to the tray" and "Start when plugged in" do nothing in the Rust window            Status: open
- Reported: 2026-11-07, found in the release audit (README claim check).
- Cause: `prefs.rs` stores `closeToTray` and `autoStart` and Settings › Advanced shows both toggles, but nothing in `zenlink-app` reads them; only the classic window implemented them (`zenlink/window.py`, `zenlink/tray.py`). Same in v2.2.31 (`E:\ZenLink v2.2.31\rust\crates\zenlink-app\src\prefs.rs` only, read-only check).
- Fix: open. Either implement a Rust tray (Shell_NotifyIcon) and auto-start on plug-in, or hide both toggles. The README no longer claims a tray.
- Verification: code search (`close_to_tray`, `auto_start` have no reader in `src/main.rs`).

