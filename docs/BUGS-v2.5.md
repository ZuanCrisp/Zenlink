# ZenLink v2.5 Beta: bug tracker

Current phase: **Phase 0 (bootstrap and isolation)**, see `docs/ROADMAP.md`.

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

### B-6: Stale WebView2 warning in the installer and Start ZenLink.bat            Status: open
- Reported: carried over (item 6).
- Cause: `packaging/ZenLink.iss` `InitializeSetup` and the WebView2 check in `Start ZenLink.bat` are left from the classic pywebview window. The Rust window does not use WebView2.
- Fix: planned: drop the warning from the default path; keep it only for `ZENLINK_UI=classic`.
- Verification: unverified: needs the installer run on Windows 10 without WebView2.

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
