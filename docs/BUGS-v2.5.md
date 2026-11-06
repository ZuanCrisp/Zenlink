# ZenLink v2.5 Beta: bug tracker

Current phase: **A3 (Android video mirroring) done; next A4**, see `docs/ROADMAP.md`.

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

### B-5: Portable.exe / Source-zip `ZenLink.exe` need the Visual C++ runtime            Status: open
- Reported: carried over (item 5).
- Cause: the Rust exe imports `VCRUNTIME140.dll`. The installer works only because PyInstaller's onedir ships it next to the exe; Portable.exe and the source zip do not. On a PC without the VC++ Redistributable, Windows fails before ZenLink can explain anything.
- Fix: planned: static CRT via `rust/.cargo/config.toml` (`-C target-feature=+crt-static` for `x86_64-pc-windows-msvc`). Measure the exe size before and after.
- Verification: unverified: needs an import check (`dumpbin /imports` or `pefile`: no `VCRUNTIME140*.dll`, no `api-ms-win-crt-*`) and a run on a clean PC / Windows Sandbox.

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

### B-10: Licence audit before bundling Android platform-tools `adb`            Status: open
- Reported: 2026-11-06, phase A1 (master prompt section 13, AGENTS.md §6 item 9).
- Cause: the ADB decision (`docs/android/ADB.md`) bundles Google's `adb.exe`, `AdbWinApi.dll` and `AdbWinUsbApi.dll` next to a GPL-3.0 program. The platform-tools zip comes with Google's SDK licence and a `NOTICE.txt`; whether redistribution as a separate program is allowed was not audited yet.
- Fix: n/a until phase A3 (read the SDK licence and `NOTICE.txt` of the version that is bundled, record the result in `docs/THIRD_PARTY.md`).
- Verification: unverified: needs the audit; nothing is bundled yet.

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

### B-14: Android video mirroring over ADB with the scrcpy server (phase A3)            Status: fixed (unverified)
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
  - Unverified: USB mirroring on a real Android device (needs the user), Wi-Fi, the forward-tunnel fallback on a device, rotation (new session + config), and very large keyframes.

