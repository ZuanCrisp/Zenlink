# AGENTS.md — ZenLink v2.5.1 Beta (Android + iOS, Rust-first)

Read this whole file before you do anything. It is the operating manual for this folder.
The full Android feature specification is in `docs/ANDROID_MASTER_PROMPT.md`. It is part of these instructions: read it completely too.

Owner: **Zenvy Zenshiro** (https://github.com/zenvyzenshiro). Repository: https://github.com/zenvyzenshiro/zenlink

---

## 0. Start-up checklist (do these in order, every new session)

1. Run `pwd` / `Get-Location`. Confirm that you are in `E:\Zenlink v.2.5.1 Beta`. If you are not, stop and `cd` there.
2. Read `lanjutan.md` in this folder if it exists. It is your checkpoint from earlier sessions. Continue from its "Next" list.
3. Read `docs/ROADMAP.md`. It sets the phase order, the gates and how to use sub-agents. Work **one phase at a time**. Then read `docs/ANDROID_MASTER_PROMPT.md`, and `docs/BUGS-v2.5.md` once it exists.
4. If the `caveman` skill is installed, invoke it and use it for chat replies. Persisted files (code, comments, commits, docs) stay in plain full English.
5. If the bootstrap in §2 is not done yet (no `.git` here), do it first.
6. Run `git status` and `git log --oneline -5`. Never discard changes you did not make.

Reply to the user in **concise Indonesian**. Write code, comments, commits and docs in **English**.

---

## 1. ISOLATED DEVELOPMENT DIRECTORY — MANDATORY

Develop ZenLink v2.5.1 Beta **only** in:

`E:\Zenlink v.2.5.1 Beta` (branch `v2.5.1-beta`, cloned on 2026-11-07 from `E:\Zenlink v.2.5 Beta` at the v2.5.0 release commit 5900f11)

Released versions that must never be changed from here:

| Folder | What it is | Rule |
| :--- | :--- | :--- |
| `E:\Zenlink v.2.5 Beta` | Source of the released v2.5.0 (branch `android-v2.5-beta`, tag `v2.5.0`). | **Read and copy only.** Never modify, build, clean, delete or `git` write there. Hotfixes for v2.5.0 only when the user asks. |
| `E:\ZenLink v2.2.31` | Current stable, released as v2.2.31. The folder was renamed from `E:\ZenLink v2.2.3`, so treat any instruction about "v2.2.3" as meaning this folder. | **Read and copy only.** Never modify, build, clean, delete or `git` write there. |
| `E:\ZenLink` | Old v2.2 | Never touch. |

Strict requirements:

- Create the development directory if it does not exist.
- Copy the project from `E:\ZenLink v2.2.31` into it with the bootstrap in §2.
- Do all Rust reconstruction, Android integration, testing and builds only inside `E:\Zenlink v.2.5.1 Beta`.
- Never modify, overwrite, delete or clean files inside `E:\Zenlink v.2.5 Beta`, `E:\ZenLink v2.2.31` or `E:\ZenLink`.
- Keep build outputs, caches, configurations, logs and release artifacts separate. See §3.
- Do not change the stable installation, its settings or its USB driver configuration during development.
- Before every destructive command (`Remove-Item`, `rm -rf`, `git clean`, `git reset --hard`, `cargo clean`) and before every build, print the working directory. Check that the absolute target path starts with `E:\Zenlink v.2.5.1 Beta`.
- The released v2.5.0 and the stable v2.2.31 must always stay fully working rollback versions.

The goal is complete isolation: v2.5.1 Beta development must never damage or replace v2.5.0 or v2.2.31.

---

## 2. Bootstrap (one time; done for v2.5.1 on 2026-11-07, kept as history)

Use a git clone, not a file copy. It brings full history and a separate `.git`, and it leaves out ignored junk such as `.venv`, `build`, `release`, `rust/target*`, logs and personal settings.

```powershell
Set-Location "E:\Zenlink V.2.5 Beta"
# docs\ and AGENTS.md already exist here, so clone into a temp folder and move the contents in.
git clone "E:\ZenLink v2.2.31" "E:\Zenlink V.2.5 Beta\_clone"
Get-ChildItem -Force "E:\Zenlink V.2.5 Beta\_clone" | Move-Item -Destination "E:\Zenlink V.2.5 Beta" -Force
Remove-Item "E:\Zenlink V.2.5 Beta\_clone"   # empty now
git remote set-url origin https://github.com/zenvyzenshiro/zenlink.git
git switch -c android-v2.5-beta              # base = v2.2.31 (commit 894c329)
git config user.name "Zenvy Zenshiro"
git config user.email "85529012+zenvyzenshiro@users.noreply.github.com"
```

The `Move-Item -Force` step may hit a clash between this folder's `AGENTS.md`/`CLAUDE.md`/`docs\` and the cloned ones. If so, keep **this folder's** `AGENTS.md`, `CLAUDE.md`, `docs\ANDROID_MASTER_PROMPT.md`, `docs\ROADMAP.md` and `.claude\commands\` (`/mulai`, `/lanjut`, `/bug`). Commit them to the branch.

Then:

- Add `lanjutan.md` and `_research/` to `.git/info/exclude`. They must **never** be committed or pushed. `_research/` holds third-party files kept only for read-only analysis, such as the TikTok LIVE Studio Mac installer.
- **Mac target priority: Apple Silicon (arm64) first.** It is the only Mac architecture that is built, tested and supported at first. Keep the code architecture-neutral, so an x86_64 (Intel) or universal build is just an extra build target later. Ship Intel only as "untested" until someone tests it on an Intel Mac.
- Do **not** copy these from the stable folder: `settings.json`, `devices.json`, `airplay.json`, `secrets.json`, `*.log`, `.venv`. They are personal or runtime data.
- Create a fresh Python venv here. Use `Start ZenLink.bat`, or `py -3.13 -m venv .venv` then `.venv\Scripts\pip install -r requirements.txt`.
- Set the version to `2.5.0-beta.1` (or the agreed beta number) in `pyproject.toml`, `rust/Cargo.toml` and `packaging/ZenLink.iss`.
- First commit on the branch: `Start ZenLink v2.5 Beta from v2.2.31`.

GitHub note: the user's local DNS sometimes resolves `github.com` to a dead address. If `git push` or `fetch` fails with "Failed to connect to github.com:443", retry with:
`git -c http.curloptResolve=github.com:443:140.82.121.4 push ...`
Do not change git config permanently. `gh` and `api.github.com` usually work normally.

---

## 3. Isolation of runtime resources (stable and beta must not collide)

**v2.5.1 beta (done 2026-11-07, `tests/test_next_isolation.py`).** It runs next to an installed v2.5.0 and a v2.2.31 Portable.exe:

| Resource | v2.2.31 | v2.5.0 (released) | v2.5.1 beta |
| :--- | :--- | :--- | :--- |
| Backend / local stream / AirPlay ports | 47321 / 47322 / 7000 | 47421 / 47422 / 7100 | **47521 / 47522 / 7200** |
| Data and prefs folder (`%APPDATA%`, `%LOCALAPPDATA%`) | `ZenLink` | `ZenLink-Next` | **`ZenLink-Beta`** |
| Log in `%TEMP%` | `zenlink-next.log` | `zenlink-next-v25.log` | **`zenlink-v251-beta.log`** |
| AirPlay name | ZenLink | ZenLinkStudio | **ZenLinkStudioBeta** |
| Window title, tray class, autostart value | ZenLink | ZenLink, `ZenLinkTrayV25`, `ZenLink` | **ZenLink Beta, `ZenLinkTrayV251Beta`, `ZenLink Beta`** |
| Installer | AppId `{6B0E8C1A-…}`, `{autopf}\ZenLink` | same AppId (upgrades v2.2.x) | **AppId `{9C4F2B71-3E8A-4D16-B5C0-7A2E91F4D3B8}`, `AppName=ZenLink Beta`, `{autopf}\ZenLink Beta`** |

**Before the final v2.5.1 release** (only when the user asks): restore the v2.5.0 installer identity (AppId `{6B0E8C1A-5D3F-4E0A-9C2B-2E7A1F5D9A10}`, `AppName=ZenLink`, `{autopf}\ZenLink`, autostart value `ZenLink`, AirPlay name `ZenLinkStudio`, window title `ZenLink`), so Setup upgrades v2.5.0 in place. Decide with the user whether the data folder moves back to `ZenLink-Next` (keeps v2.5.0 settings) or migrates.

The original v2.5 rules, still valid for every resource:

| Resource | Stable uses | Beta must use |
| :--- | :--- | :--- |
| Backend HTTP port | `127.0.0.1:47321` (`zenlink/server.py`, `zenlink-core` `PORT`) | Different default, e.g. **47421**, in both Python and Rust. `ZENLINK_PORT` still overrides. |
| Rust UI prefs | `%APPDATA%\ZenLink\next-prefs.json` (`prefs.rs`) | `%APPDATA%\ZenLink-Beta\...` (`ZL_PREFS` still overrides) |
| Installed/portable data | `%LOCALAPPDATA%\ZenLink` (`paths.py`, `main.rs`, `sidecar.rs` unpacks the service to `...\ZenLink\service`) | `%LOCALAPPDATA%\ZenLink-Beta` |
| Installer identity | `ZenLink.iss` `AppId {6B0E8C1A-5D3F-4E0A-9C2B-2E7A1F5D9A10}`, `AppName=ZenLink`, `{autopf}\ZenLink` | Beta builds: a new AppId. **v2.5.0 release (owner, 2026-11-07): same AppId, `AppName=ZenLink`, `{autopf}\ZenLink`**, so Setup upgrades v2.2.x in place (`packaging/ZenLink.iss`). Data folder and ports stay separate. |
| Window / tray names, single-instance mutex, firewall rule name | "ZenLink" | Add a "Beta" suffix so the two do not fight or replace each other |
| Cargo build dir | `rust\target` | `rust\target` **inside the beta folder** (never point `CARGO_TARGET_DIR` at the stable folder) |
| ADB server | port 5037 is shared by all adb clients on the PC | Do not kill a foreign adb server blindly. Detect version conflicts and tell the user (see master prompt §16). |
| USB drivers (Apple / libusb0) | system-wide | Never install, remove or switch drivers without asking the user first |

Add tests where practical: default port, prefs path and data dir all contain "Beta"/47521.

---

## 3b. Top priority: small, light, fast

The user's main priority is a **small download, low resource use and optimal performance**. Weigh every decision against it.

- **Measure, do not guess.** Record these before and after every phase that changes dependencies or packaging:
  - Setup.exe, Portable.exe and installed folder size;
  - idle and mirroring RAM and CPU;
  - startup time.
  Keep the numbers in `lanjutan.md`, and in the release notes when releasing.
- **Biggest wins, in order:**
  1. Slim FFmpeg (65 MB of the 169 MB service folder; ROADMAP Phase 1).
  2. Remove the Python runtime and its libraries (~94 MB; M1–M5).
  3. Keep Android lean: no full SDK; only `adb` + `AdbWinApi*.dll` + the scrcpy server (~100 KB), or a Rust USB ADB client if it proves reliable.
- **Rust release profile.** It is already size-tuned: `opt-level="s"`, `lto=true`, `codegen-units=1`, `panic="abort"`, `strip=true`. Keep these settings. Only change `opt-level` if a measured hot path (decode, render or audio) needs `3`, and then apply it per package.
- **Static CRT (B-5).** It adds only a small amount to the exe. Measure it. It removes a runtime dependency, so it is worth it.
- **No UPX or other exe packers.** They cause antivirus false positives and slower starts.
- **New dependencies.** Before adding a crate or library, check its size and its transitive dependencies (`cargo tree`, `cargo bloat` if available). Prefer the `windows` crate and std over heavy frameworks. Disable unused default features.
- **Runtime.**
  - No busy loops: wait on events or vblank, as the renderer does.
  - Use bounded queues and allocate nothing per frame in the hot path.
  - Do no work while hidden or idle, beyond what is needed.
  - Never do slow scans during a session (see §5 item 6).

## 3c. Antivirus-friendly builds (ZenLink must never look like malware)

- No exe packers or obfuscators (UPX, Themida and the like).
- No hidden downloads of executables at runtime. Anything ZenLink runs is bundled, or comes from an official source the user clicks, such as the Microsoft Store link.
- Keep full version resources on every exe and DLL: CompanyName "Zenvy Zenshiro", ProductName, FileDescription, version. The PyInstaller backend already has them. Add the same to the Rust exe (`winres`/`embed-resource`) and to any new helper.
- Keep the existing behaviour of asking before driver, firewall or admin actions. Never use silent elevation tricks, never inject into other processes, never add autostart without the user's setting.
- Code signing: there is no certificate now. Do not buy, create or install certificates yourself. If false positives appear, tell the user. The options are signing, and submitting the file to Microsoft's false-positive portal (only with the user's approval, because it uploads the file).
- Before a release, the user may want a VirusTotal check. Uploading a build to a third party needs the user's explicit approval each time.
- The bundled `adb.exe` must be the unmodified official Google platform-tools binary, signed by Google. Never patch it.

## 3d. scrcpy: always the latest official release

- Use the **latest official release** from https://github.com/Genymobile/scrcpy/releases at the time you work. Do not use a fork or an old copy. The master prompt names v5.0.1; check whether a newer release exists, and use the newest.
- Download `scrcpy-server-vX.Y.Z` (and, if bundled, the Windows platform-tools `adb`) only from the official release assets. Verify the published checksum (SHA-256), and record the version, URL and checksum in `docs/THIRD_PARTY.md` (or the existing notices file).
- The scrcpy server and the client protocol must match **exactly**. The server is started with its version string, and a mismatch fails. Pin the version in one constant in the Rust code (e.g. `SCRCPY_VERSION`), and have a test that the bundled server file name matches it.
- Updating scrcpy later is a deliberate step:
  1. Read the upstream changelog and protocol changes.
  2. Bump the constant and the server file.
  3. Run the protocol tests.
  4. Record the new version in the bug tracker and the release notes.

## 3d-2. Build tools: look first, install to E: only if missing

The user has approved this: when a build needs a tool, **first search the user's C: and E: drives** for an existing install. Only install it if it is missing, and then install it **on E:**.

1. **Check these known locations first** (found 2026-11-06):
   - Rust: `C:\Users\ZenvyZenshiro\.cargo\bin` (cargo 1.95, target `x86_64-pc-windows-msvc` only).
   - Python 3.13: `py -3.13`.
   - Git 2.55.
   - LLVM 23.1.4: `C:\Program Files\LLVM\bin` (`clang.exe`, `ld64.lld.exe`, `lld.exe`, `llvm-ar.exe`; not on PATH).
   - MSVC build tools / Windows SDK: `E:\Tools` (VC, MSBuild, SDK).
   - Inno Setup 6: `%LOCALAPPDATA%\Programs\Inno Setup 6\ISCC.exe`.
   - Android SDK, JDK 21, Gradle: `E:\AndroidDev` (`sdk\platform-tools` has `adb`).
   - Apple: Xcode `E:\ProjectIOS\Xcode_27.1.xip`; iPhoneOS SDK `E:\ProjectIOS\nxnative-win\win\sdk\out\iPhoneOS.sdk`; Metal Developer Tools `E:\ProjectIOS\Metal Developer Tools`; the Windows-to-Apple build scripts are in `E:\ProjectIOS\nxnative-win\win` (`extract_xip.py`, `macho_sign.py`, `build_app.py`, `toolchain.json`, `doctor.py`).
   - Vulkan SDK: `C:\VulkanSDK\1.4.363.0`.
   - WSL: Kali Linux.
   Also search with `where`, `Get-Command`, the registry uninstall keys, and `E:\*`/`C:\Program Files*` before deciding a tool is missing.
2. **If it is missing**, install it under **`E:\DevTools\<tool>`**, never C:.
   - Rustup targets and cargo tools may stay in the existing `.cargo`. They are small, but report it.
   - Download only from the official source: the project's GitHub releases or the vendor site. Verify the checksum or signature when one is published.
   - Record every install in `docs/TOOLCHAIN.md`: tool, version, source URL, checksum, path. Note it in `lanjutan.md` too.
   - Prefer portable zips over installers that change the system. Ask the user before any installer that needs admin rights, changes PATH system-wide, or installs a service or driver.
3. **Disk space is tight.** On 2026-11-06, C: had about 9.6 GB free and E: about 41 GB. Check free space before large downloads (an Xcode xip is ~2 GB; SDK extraction ~2 GB). Never fill C:. Never delete user files to make room. Tell the user instead.
4. Never modify or move the user's existing tool folders, such as `E:\ProjectIOS`. Copy what you need into this repo or `E:\DevTools`.

## 3e. Cross-platform readiness (macOS first, then Linux, later)

Many users ask for a Mac version. The v2.5 work is Windows-first, but **new code must not block macOS and Linux ports**. Platform ports are later phases (see `docs/ROADMAP.md`, P1–P4). Until then, follow these rules:

- **Platform layer.** Keep platform-specific code behind small traits or modules, with a Windows implementation now:
  - video decode (Media Foundation → later VideoToolbox / VA-API);
  - video presentation (D3D11 swap chain + child HWND → later Metal `CAMetalLayer` / Vulkan or GL);
  - audio output;
  - USB driver management (Windows only);
  - window effects (DWM blur → macOS vibrancy → Linux fallback);
  - paths and app data;
  - process/sidecar control;
  - tray.
- **Portable core.** The Android stack (ADB, scrcpy protocol, encoder detection, profiles, device naming), the protocol code, settings and UI logic must be plain cross-platform Rust. Use `#[cfg(windows)]` only inside the platform layer, never scattered through the core.
- **Audio.** Use `cpal` (it uses WASAPI on Windows) or a thin trait with a WASAPI backend, so macOS (CoreAudio) and Linux (PulseAudio/PipeWire) follow for free. Keep the v2.2.31 buffer and margin logic in the portable part. Latency rule: before switching to it, measure output latency and gaps with `cpal` against the current path, on the same devices (wired and Bluetooth). If `cpal` is measurably worse on Windows, keep a direct WASAPI backend behind the same trait. Users must never get more audio delay because of the port.
- **UI.** Slint works on all three. Glass effects must have a graceful fallback (a solid translucent theme) where blur is unavailable. Keyboard shortcuts must map Alt↔Option and Ctrl↔Cmd where it matters.
- **Paths and binaries.** Use `dirs`-style APIs, not hard-coded `%APPDATA%`, in new code. The bundled `adb` and the scrcpy server get a per-platform location.
- **CI.** When the user agrees to it, add a GitHub Actions job that at least runs `cargo check` and the portable crates' tests on `macos-latest` and `ubuntu-latest`, so portability does not rot. Release builds for Mac and Linux come only in the platform phases.
- Never claim Mac or Linux support before it has been built and tested on real hardware.
- **Apple Driver mode and ZenLink Studio on macOS and Linux are planned too**, not only Android (ROADMAP P2 and P5, "Apple modes on macOS and Linux"). So keep these parts portable:
  - the iOS protocol code: the QuickTime USB handshake, usbmux, lockdown, the CoreDevice tunnel and HID, AirPlay/RTSP and pairing;
  - the transport behind a trait: Windows libusb0 raw USB; Linux raw USB with udev `uaccess`; macOS AVFoundation capture instead of raw USB;
  - pair-record access behind a trait: Windows `ProgramData\Apple\Lockdown`; the usbmuxd socket on Linux and macOS;
  - mDNS behind a trait: own stack on Windows; avahi D-Bus on Linux when avahi is present; dns_sd/mDNSResponder on macOS;
  - FairPlay `playfair` built from source per platform.
- Platform system changes, such as Linux udev rules, a usbmuxd override, firewall rules, or macOS settings, follow the same consent rule as Windows drivers. Explain them, ask, make them reversible, and never apply them silently.

## 4. Hard safety rules (inherited from v2.2.x work, still apply)

- **No hardware tests without consent.** Never start USB or driver operations, mirroring or driver changes on the user's real devices unless the user says so. The user tests on devices and reports back.
- **Never stack libusb0 on Apple's driver.** No filter drivers on top of Apple's.
- Do not change Windows firewall, certificates, UAC, Developer Mode or other system security settings on your own.
- Do not disable Android security restrictions (master prompt §11, §19).
- Never claim a number you did not measure. Never mark a test passed when no device was available. Write "unverified".
- Commit only the files you need. Never commit `lanjutan.md`, logs, `settings.json`, `devices.json`, secrets or build outputs.
- **Releases:** never publish, overwrite or delete a GitHub release or tag without explicit user approval in chat. A beta release must be a **pre-release** (`gh release create v2.5.1-beta.N --prerelease`) and must never be marked "latest". Do not touch the v2.2.31 or v2.5.0 releases.
- Do not push to `main` without approval. Push only to the `v2.5.1-beta` branch.
- Commit identity: `Zenvy Zenshiro <85529012+zenvyzenshiro@users.noreply.github.com>`. End every commit message with:
  `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`
- Release notes never start with a "## ZenLink vX" heading. Assets per release: `Setup.exe`, `Portable.exe`, `Portable-Source.zip`.

---

## 5. Project knowledge (what you are building on)

**Architecture (v2.2.31):**

- **Window:** `rust/crates/zenlink-app` (Rust + Slint).
  - The UI is in `ui/app.slint`, `widgets.slint` and `theme.slint`.
  - The window uses the femtovg OpenGL renderer with DWM blur ("Liquid Glass").
  - Strings are bilingual through `I18n.t("id", "en")`.
- **Video:** `rust/crates/zenlink-video`.
  - Media Foundation hardware decode (H.264/HEVC) into a D3D11 texture ring (`d3d.rs` `Ring`: a FIFO with 6 slots, at most 3 queued).
  - A child HWND `ZenLinkVideo` with a FLIP_DISCARD swapchain, one frame per vblank (`view.rs`).
  - Reuse this for Android. Do not build a second renderer.
- **Backend:** `rust/crates/zenlink-core` talks to a **Python sidecar** (`zenlink/`, `ZenLinkService.exe`) over HTTP on 127.0.0.1.
  - The sidecar handles Apple pairing (pymobiledevice3), AirPlay, USB Direct (QuickTime protocol in `zenlink/usbdirect`) and audio (`audio_out.py`, sounddevice).
  - Rust-first goal: put new Android code in **Rust** (e.g. a new `zenlink-android` crate). Do not add new Python for Android.
- **Packaging:** `build_portable_release.ps1`.
  - It builds PyInstaller onedir + onefile, the Rust exe, Inno Setup (`%LOCALAPPDATA%\Programs\Inno Setup 6\ISCC.exe`) and the source zip.
  - It honours `CARGO_TARGET_DIR`.

**Behaviours you must not regress:**

1. **OBS Game Capture.** OBS's graphics hook hooks both D3D11 (the video) and OpenGL (the Slint window). On every video size change, `VideoView::set_bounds` waits up to 120 ms on the UI thread until the render thread has run ResizeBuffers and presented. Without that, the OpenGL frame wins the re-hook and OBS goes blank (`gl_copy_backbuffer: failed to blit: 1286`). Android must go through the same `VideoView`.
2. **Frame pacing.** One queued frame per refresh. No "newest wins" mailbox (it caused periodic judder).
3. **Window fit.** `fit_prefs_key` means only fit, rotation, crop and fit_window refit the window. A user resize is remembered (`user_scale`). Clicking other settings must not shrink the window.
4. **Audio.** `NativeAudio` target and margins scale with the output block size, so Bluetooth headsets on MME at ~91 ms work. `MIN_MARGIN` is 10 ms.
5. **USB Direct clock.** `SkewMeter` measures the device audio clock over at least 6 s. The iPhone's video players sync to the SKEW we report.
6. **Backend.** No driver scans while a session is active, because they caused GIL stalls. The driver INF scan is cached.
7. **Theme default: dark (final, decided by the user).** The default is **Liquid Glass Dark** (`theme.slint dark: true`). This **overrides** the master prompt wherever it says "Liquid Glass Light as the default": read those lines as "Liquid Glass Dark by default, Light as the alternative". New screens (platform selector, Android workspace) also open dark on a fresh install. Keep Light and System working.

8. **UI animations and smoothness must stay.** Keep everything from v2.2.31:
   - the Liquid Glass look (DWM blur, glass cards, tints);
   - the settings slide-in/out with the window following the animated chrome (`hold_stage`);
   - hover and press states, smooth transitions and theme changes.
   New screens (platform selector, Android setup and device list, Android settings) must use the same widgets (`Glass`, `Card`, `Seg`, `Toggle`, `GlassButton`, …) and the same animation style and timing. They must not be static or jumpy. Animations must stay smooth (no dropped UI frames) and must never stall the video: the video stays on its own D3D swap chain and render thread. Do not trade away animations to save size or CPU. Make them cheaper instead (e.g. redraw only what changes).

**v2.2.31 is the final baseline.** Everything released in v2.2.31 is approved and final, including the UI, the dark default, the settings cards, the OBS behaviour and the fixes above. Build Android on top of it. Do not redesign or "improve away" any existing v2.2.31 behaviour unless the user asks.

**Test tools:**

- `rust/tools/fake_backend.py <video.mp4> --port N [--jitter MS]` serves a recorded stream to the window without a device.
  - The video argument is required. If it fails to start, the app may start the real backend on that port, so check first.
- App environment variables:
  - `ZENLINK_PORT`
  - `ZL_PREFS=<file>`: always use a temporary prefs file in tests so you never overwrite the user's settings.
  - `ZL_OPEN_SETTINGS=1`
  - `ZL_STATS_LOG=1`: writes frame stats to `%TEMP%\zenlink-next.log`.
- Python tests: `.venv\Scripts\python -m pytest tests -q`.
- Rust: `cargo fmt --check`, `cargo clippy --release`, `cargo test --release` (the `ring_tests` in `zenlink-video`).
- Do not steal focus or type into the user's foreground apps during UI tests. If the ZenLink window is behind other windows, do not send keystrokes. Use env vars, the HTTP API or `ZL_PREFS` instead.

---

## 6. The work: Android integration

Implement everything in `docs/ANDROID_MASTER_PROMPT.md`, in the phase order of `docs/ROADMAP.md`. That order is: Phase 0 isolation, Phase 1 slim FFmpeg, A1–A6 Android, then M1–M5 moving the remaining Python backend to Rust, each behind a `ZL_ENGINE` fallback. Where it conflicts with this file, this file wins. One example: the default theme is dark, not light (§5 item 7).

Its key mandatory items:

1. A new **first screen, platform selection**: "iPhone / iPad" vs "Android Phone / Tablet".
   - It has a bilingual Indonesia/English toggle, with Indonesian as the default on a fresh install.
   - The existing Apple Driver / ZenLink Studio popup stays unchanged as the Apple **second** step.
   - Back / "Change device type" is available from both paths. "Remember device type" is a separate setting from Apple's "Remember my choice".
2. **Android over ADB** using the official **scrcpy server**, with a **native Rust client**.
   - Rendering goes through the existing `zenlink-video` pipeline. Never embed a scrcpy window.
   - Pin and document the exact scrcpy server version and protocol.
3. USB and Wi-Fi (TCP/IP and Android 11+ pairing) discovery, authorization states, reconnect and multi-device support.
4. **Real device names** from `ro.product.*` properties, with truthful fallbacks.
   - Never hardcode example names such as "Samsung Galaxy A57" or "TECNO POVA 6".
   - Device identity must be stable, never based on the IP address.
5. **Smart Encoder Detection.** H.264 is the default, HEVC and AV1 are offered only when verified, with a bounded fallback.
6. Video profiles (Low Latency / Balanced / High Quality / Streaming-OBS / Custom) and per-device profiles.
7. Audio through the existing engine, A/V sync modes, and a video-only fallback.
8. Android controls: touch, keys, clipboard, rotation, Back/Home/Recents.
9. Attribution: scrcpy (Apache-2.0) credited in the README, About, third-party notices and installer. Audit ADB/platform-tools licences against GPL-3.0.
10. Docs: the README is repositioned as "iPhone, iPad & Android Mirroring for Windows". Confirmed results are kept separate from theoretical ones.

**Android settings: expose the freedom scrcpy gives, unlike iOS.** Check every option against the pinned scrcpy version's docs (`doc/` in the repo) before you expose it. Hide any option that the device or version does not support.

- **Resolution:** a max-size preset list plus Custom (e.g. 720 / 1080 / 1440 / native), and crop.
- **FPS:** 30 / 60 / 90 / 120 / 144 / Custom.
  - Only offer a value when the device's display refresh rate and encoder can reach it.
  - Show the measured FPS next to the chosen one.
- **Bitrate:** a slider with presets. Codec and encoder come from Smart Encoder Detection.
- **Virtual display** (`--new-display`, where supported). It mirrors a separate display with its own resolution and DPI, e.g. a desktop-like 1920×1080 while the phone screen stays off. Mark it as an advanced option.
- **Orientation:** lock or rotate the capture, and show the angle.
- **Audio:**
  - source: device playback, or microphone where supported;
  - codec: Opus default, AAC or FLAC where supported;
  - bitrate;
  - "no audio".
- **Camera mirroring** (`--video-source=camera`, Android 12+): camera ID, size, fps. Show it only if supported.
- **Device behaviour:**
  - stay awake;
  - turn the screen off while mirroring;
  - power off when closing;
  - UHID keyboard, mouse and gamepad (where supported).

Store these settings per device (master prompt §7), and use the "Custom" profile for manual values.

Keep the build green at every milestone. Do not stop at a plan or a UI mockup.

---

## 7. Bug handling — "update every bug and verify everything"

Keep `docs/BUGS-v2.5.md` in the repo as the bug tracker. One entry per bug:

```
### B-<n>: <short title>            Status: open | fixed (unverified) | fixed (verified) | won't fix
- Reported: <date, by whom, how to reproduce>
- Cause: <what was proven, with evidence: log line, measurement, code path>
- Fix: <commit hash, files>
- Verification: <test name / measurement / "user confirmed on <device>"> — or "unverified: needs <device>"
```

Rules:

- Find the **cause** before you fix anything: logs, measurements, code reading. Never guess-and-patch.
- Every fix gets an automated test when practical (Python `tests/`, Rust `#[cfg(test)]`). If it is not practical, record a reproducible manual check.
- After each fix, run **all** tests (pytest + cargo test + fmt + clippy) and build the app. Fix anything that breaks.
- A bug is "verified" only with evidence: a passing test, a measurement, or the user's confirmation. Otherwise mark it "fixed (unverified)" and list what is needed.
- When the user reports a new bug, add it to the tracker first, then work on it.

**Open items carried over from v2.2.31. Add them as B-1… on day one:**

1. **"Hanya perangkat / Device only" sound (ZenLink Studio, cable).** The session sends HPA0 after the first frame. It is not confirmed on an iPhone whether the sound returns to the phone.
2. **OBS portrait/landscape.** The capture now survives rotation (user confirmed). Positioning is fixed with OBS "Fit to screen" (Ctrl+F), as explained in the Settings OBS card. Re-check this with Android rotation.
3. **Stutter fixes** (renderer FIFO, audio margin, SkewMeter). The user confirmed these with an iPhone. Keep them regression-tested.
4. **Window resize kept after clicking settings.** Confirmed in a fake-stream test. User confirmation on a device is still pending.
5. **Portable.exe / Source-zip `ZenLink.exe` need the Visual C++ runtime.** The Rust exe imports `VCRUNTIME140.dll`. The installer works, because PyInstaller's onedir copies `VCRUNTIME140.dll` and `VCRUNTIME140_1.dll` next to it. Portable.exe and the source zip's `ZenLink.exe` do not ship it.
   - Effect: on a clean PC without the VC++ Redistributable, Windows' loader shows "VCRUNTIME140.dll was not found" before any ZenLink code runs. ZenLink cannot show its own message or offer an install at that point.
   - Fix: build the Rust exe with a static CRT. Add `rust/.cargo/config.toml` containing `[target.x86_64-pc-windows-msvc] rustflags = ["-C", "target-feature=+crt-static"]`. This also covers the `embedded-backend` portable build and the `CARGO_TARGET_DIR` builds.
   - Verify: check the exe's imports with `dumpbin /imports` or a `pefile` script, so that no `VCRUNTIME140*.dll` and no `api-ms-win-crt-*` appear. Also check that the PyInstaller backend still starts (its bootloader bundles its own runtime). Test the portable build in Windows Sandbox, a clean VM, or a PC without VC++ redist.
   - Record the exe size before and after.
6. **Stale WebView2 warning in the installer.** `packaging/ZenLink.iss` (around line 102) and the `check_env` step in `Start ZenLink.bat` still warn about, or install, Microsoft Edge WebView2. That runtime was only used by the old classic window (`python -m zenlink`, pywebview). The Rust window does not use it.
   - Fix: remove the warning from the default path. Keep it only for the `ZENLINK_UI=classic` fallback, or remove it together with the classic UI later.
   - Verify: the installer runs on Windows 10 without WebView2 and shows no warning.
7. **General rule for prerequisites.** Anything ZenLink needs must be either bundled, or detected by ZenLink itself with a clear bilingual message and a one-click official source. Examples: the Apple Devices Store link, which already exists, and the ZenLink Studio driver button. Never depend on a DLL whose absence makes Windows fail before ZenLink can explain anything.

---

## 8. Checkpoints — `lanjutan.md` (never committed)

Keep `lanjutan.md` in this folder as your private checkpoint, in Indonesian. Update it:

- after every commit,
- after every user test report,
- before any long build,
- before the session's context gets large.

It holds:

- the current state and the last commit;
- what was measured and the numbers;
- what is unverified;
- the exact next steps;
- traps (for example the DNS push workaround, and the ports and paths used).

A fresh session must be able to continue from it alone. It is listed in `.git/info/exclude`. Check `git status` never shows it.

---

## 9. Definition of done for each milestone

- `cargo fmt --check`, `cargo clippy` (no new warnings) and `cargo test` pass.
- `pytest` passes.
- The app builds, and the beta runs **side by side** with the stable v2.2.31 without touching its port, settings or install.
- `docs/BUGS-v2.5.md` is updated, and every claim in it has evidence or is marked unverified.
- The commit is on `v2.5.1-beta`. `lanjutan.md` is updated.
- The report to the user (Indonesian, short) uses: Status → Evidence → Changes → Blockers → Next.

A release happens only when the user asks. It is a beta **pre-release**, built in the beta folder, with 3 assets, and `latest` is never moved.

---

## 10. v2.5.1 scope (agreed with the user on 2026-11-07)

Ideas taken from comparing ZenLink with TwinDesk (GPL-3.0, Electron/Kotlin). Take ideas only, write the code in Rust; no code copied. Each item goes through the normal bug/feature rules, and device results stay "unverified" until the user tests.

1. **Android apps in their own window.** scrcpy `--new-display` + `--start-app` (Android 11+ for app windows). A list of installed apps, each opened as its own ZenLink window. Check the pinned scrcpy version's docs first.
2. **Drag and drop files onto the window.**
   - Android: an `.apk` installs (`adb install`), other files go to `/sdcard/Download/` (`adb push`). Show progress and the result.
   - iPhone/iPad (Apple Driver mode only: needs lockdown/usbmux): files go to an app that has File Sharing (house_arrest/AFC), for example VLC or Documents. Photos into the camera roll are not possible this way. ZenLink Studio mode has no data channel (libusb0 replaces Apple's driver).
3. **Phone camera as a webcam.** Android only: scrcpy `--video-source=camera` (Android 12+), shown in ZenLink and, on Windows 11, exposed as a virtual camera (`MFCreateVirtualCamera`; Windows 10 falls back to OBS capture). iPhone needs an app on the phone, so it is out of scope.
4. **Phone as a microphone.** Android only: scrcpy `--audio-source=mic` (Android 11+), played to a PC output or a virtual audio device. iPhone: not possible without an app on the phone.
5. **Update check and one-click update** (asked 2026-11-07; agree the design with the user before building). Check GitHub Releases (`api.github.com/repos/zenvyzenshiro/zenlink/releases`) at start and from a "Check for updates" button, and show a notice with the version and notes. Download and install only after the user clicks (section 3c: no hidden downloads): the Setup.exe asset of the official release, its SHA-256 checked against the release asset digest, then run. The same AppId upgrades in place, and settings in `%APPDATA%`/`%LOCALAPPDATA%` stay. Portable: download the new Portable.exe next to the old one and swap after exit. A beta build looks at pre-releases, a release build only at releases. A setting turns the automatic check off.

Not taken: connection/latency badge (user said no), KVM/shared mouse, calls and SMS, Mesh, ReZume (need a companion app or do not fit "small, light, fast").
