# ZENLINK — COMPLETE ANDROID INTEGRATION USING SCRCPY + RUST

## MISSION

Act as a Principal Rust Engineer, Android Platform Engineer, Windows Graphics Engineer, Low-Latency Streaming Specialist, and Open-Source Compliance Engineer.

**Project:** ZenLink  
**Repository:** https://github.com/zenvyzenshiro/zenlink  
**Primary Android reference:** https://github.com/Genymobile/scrcpy  
**Reference version:** scrcpy v5.0.1, subject to checking the latest compatible upstream revision.

Your objective is to extend ZenLink into a unified **iPhone, iPad, Android Phone, and Android Tablet mirroring application for Windows**.

Implement real Android mirroring using the proven scrcpy architecture while building native Rust integration suitable for ZenLink.

**Do not build a separate Android application with a separate user interface. Android must be integrated into the existing ZenLink application.**

This is an actual implementation task, not a planning-only task.

Preserve all existing iOS functionality and ongoing Rust reconstruction work.

---

# 0. MANDATORY CAVEMAN WORKFLOW

Before beginning:

1. Discover the `/caveman` skill or command available in the current Claude Code environment.
2. Read its original instructions.
3. Invoke `/caveman` and apply its workflow throughout this implementation.
4. Inspect `CLAUDE.md`, `AGENTS.md`, installed project skills, and existing development conventions.
5. Do not replace an installed Caveman implementation with an invented definition.

If `/caveman` is unavailable, use this concise reporting format as a fallback:

- Status
- Evidence
- Changes
- Blockers
- Next

Keep execution reports direct, concise, and evidence-based.

Use English for implementation reports and technical documentation.

Do not confuse Caveman's concise communication style with simplified engineering. The implementation itself must remain comprehensive.

Use existing codebase-memory and repository exploration skills if available.

---

# 1. REPOSITORY AUDIT

Inspect the actual current working tree before making modifications.

The ZenLink project is undergoing a Rust-first reconstruction.

Determine:

- Which modules are already implemented in Rust.
- Which modules still depend on Python.
- Which UI framework is currently active.
- How Slint is integrated.
- Which GPU backend is used.
- How the video pipeline operates.
- How audio is handled.
- How settings and devices are persisted.
- How Windows installers and portable builds are generated.
- Which Apple-specific modules must remain isolated.

Do not assume the GitHub default branch contains all local changes.

Do not overwrite ongoing Rust migration work.

Create a dedicated Android integration branch or isolated worktree.

Preserve unrelated changes.

---

# 2. UNIFIED DEVICE SELECTION

Redesign the initial connection experience to support two device families.

## Apple Devices

Display:

**iPhone / iPad**

Description:

"Mirror and control compatible Apple devices using USB or Wi-Fi."

Preserve existing Apple Mode and ZenLink Studio functionality.

## Android Devices

Display:

**Android Phone / Tablet**

Description:

"High-performance Android mirroring and control over USB or Wi-Fi."

Support common Android manufacturers, including:

- Samsung
- Xiaomi
- Redmi
- POCO
- OPPO
- OnePlus
- Vivo
- Realme
- Google Pixel
- Motorola
- ASUS
- Lenovo tablets
- Other ADB-compatible Android devices

Do not introduce manufacturer-specific restrictions without technical justification.

Do not require Samsung DeX.

## Interface requirements

Preserve ZenLink's existing premium design.

- Liquid Glass Light as the default theme.
- Liquid Glass Dark as the alternative.
- Native Rust/Slint UI.
- Smooth transitions.
- Responsive device cards.
- Automatic device discovery.
- Clear connection status.
- English and Indonesian localization.
- Device-specific settings persistence.

The Android and Apple workspaces must share a consistent visual design.

Do not allow UI glass animations to interfere with video presentation.


## MANDATORY STARTUP FLOW — PLATFORM SELECTION BEFORE EXISTING POPUP

**This requirement has priority over any startup flow that directly opens Apple Driver / ZenLink Studio selection.** The existing Apple connection-mode popup must be retained but moved to the second step of the Apple workflow.

Implement the following first-launch and normal-launch navigation:

```text
Launch ZenLink
  -> Device Platform Selection (new first screen)
     -> iPhone / iPad
        -> Existing Apple Driver / ZenLink Studio selection popup (unchanged features)
           -> Existing USB or Wi-Fi and Remember My Choice controls
           -> Existing Apple device discovery, trust, pairing, and mirroring
     -> Android Phone / Tablet
        -> Android USB / Wi-Fi connection and pairing flow
           -> Android device discovery and model recognition
           -> Smart Encoder Detection and profile selection
           -> Android mirroring workspace
```

### Initial Welcome / Platform Selection

- Display a polished welcome screen asking **"Which device would you like to connect?"** or the accurate Indonesian equivalent **"Perangkat apa yang ingin kamu sambungkan?"**
- Keep **"Bahasa / Language"** and the **Indonesia / English** switcher visibly available at this first step, positioned consistently with the existing selection popup.
- Default to Indonesian on a fresh installation, unless an existing saved language choice is present.
- Language selection must update the current screen, the following Apple popup, the Android setup wizard, status messages, errors, and settings immediately without restarting.
- Present two large, clearly clickable Liquid Glass cards: **"iPhone / iPad"** and **"Android Phone / Tablet"**. These are platform choices, not USB/driver modes.
- Provide concise feature descriptions, correct icons, keyboard focus and accessibility labels, hover/pressed/selected states, and a responsive layout.
- Use Liquid Glass Light by default and the same Liquid Glass Dark design when dark mode is selected; preserve existing theme settings.
- Keep the existing Apple popup's contents, Apple Driver / ZenLink Studio descriptions, language switch, cable/Wi-Fi selection, "Remember My Choice", warnings, and Continue action intact. Do not collapse the Apple popup into the first screen.
- Selecting **iPhone / iPad** must navigate to the existing Apple Driver / ZenLink Studio popup, **not** directly launch mirroring or force a driver switch.
- Selecting **Android Phone / Tablet** must navigate to the Android setup/connection workflow, **not** the Apple driver popup.
- Include a clear **Back / Change Device Type** navigation path from both workflows to the platform selector without exiting ZenLink.
- Allow the platform to be changed later in Settings or by a navigation control, while safely disconnecting active sessions and preserving the user's separate Apple and Android preferences.
- Show discovered-device context (e.g., device count or connection badge) when available, but never require a connected device just to choose a platform.
- Automatic discovery may **suggest/highlight** the platform, but must not silently make a new platform selection or install/switch drivers.
- Restore or bypass platform selection only if the user explicitly chose to remember the platform; keep this preference separate from the existing Apple-mode "Remember My Choice" setting.
- Migrate old configurations without resetting saved language, theme, or Apple driver-mode preferences. Users with no explicit platform preference must see the new platform selection first.
- Handle the case where Apple and Android devices are connected simultaneously without silently choosing the wrong platform.

### UI copy and localization checks

Suggested English strings: "Welcome to ZenLink", "Which device would you like to connect?", "iPhone / iPad", "Android Phone / Tablet", "Continue", "Back", "Change device type", "Remember device type".

Suggested Indonesian strings: "Selamat datang di ZenLink", "Perangkat apa yang ingin kamu sambungkan?", "iPhone / iPad", "Ponsel / Tablet Android", "Lanjut", "Kembali", "Ganti jenis perangkat", "Ingat jenis perangkat".

Use existing localization architecture. These are suggested strings and must fit the actual application context.

### Startup flow acceptance tests

- New installation, Indonesian default: platform selection appears **before** the Apple Driver / ZenLink Studio popup.
- Switching to English immediately updates every visible label and preserves the selection when navigating Back and Continue.
- iPhone/iPad path retains every existing Apple mode, USB/Wi-Fi, and remember-choice behavior.
- Android path opens Android discovery/pairing without changing any Apple driver.
- Back navigation works from both paths and does not corrupt session state.
- Restart without explicit remembered platform presents the platform selector; restart with opted-in remembered platform follows that explicit choice with a clear way to change it.
- No connected devices, one device, multiple Android devices, and mixed Apple+Android devices are handled correctly.
- Light/Dark theme, keyboard focus, screen-reader labels, and responsive scaling are verified.

---

# 3. ANDROID MIRRORING ENGINE

## Primary foundation: scrcpy

Official source:

https://github.com/Genymobile/scrcpy

Use scrcpy's proven Android-side capture architecture as the primary foundation.

Investigate the current upstream implementation, protocol, server lifecycle, and dependency requirements.

Do not simply embed a second scrcpy window inside ZenLink.

**Preferred architecture:**

Android device → ADB transport → scrcpy Android server → native Rust client → hardware decoder → GPU presentation → ZenLink workspace.

The goal is direct integration with ZenLink's native video infrastructure.

## Engineering strategy

Evaluate:

1. Reusing the official scrcpy Android server.
2. Implementing a compatible Rust client.
3. Using an existing Rust client implementation as a reference.
4. Reusing ZenLink's current video and audio engines.
5. Retaining an optional upstream scrcpy-based diagnostic fallback where useful.

Reference implementations:

- https://github.com/Genymobile/scrcpy
- https://github.com/bdrtr/scrcpy-panel
- https://github.com/androguard/droidmirror

Audit the technical maturity and licenses of these references before using their code.

The official scrcpy implementation is the compatibility baseline.

Do not rewrite the Android server without compelling evidence.

Document the exact supported scrcpy server version and protocol revision.

Do not assume the protocol remains stable across releases.

---

# 4. USB AND WIRELESS CONNECTION

## USB Mode

Implement:

- ADB device discovery.
- USB debugging status.
- Device authorization status.
- Unauthorized-device handling.
- Offline-device recovery.
- Automatic reconnect.
- Multiple-device selection.
- Manufacturer/model detection.
- Serial-based device selection.
- Clear driver diagnostics.


## ANDROID BRAND, MODEL, AND MARKETING-NAME DISCOVERY — MANDATORY

The device selector must identify connected Android phones/tablets by a human-readable **manufacturer + marketed device name** when trustworthy device metadata supplies it, not merely by serial number, an internal board name, or a cryptic model identifier.

Examples of intended presentation (illustrative only, never hardcode these names without evidence from the connected device):

- **Samsung Galaxy A57** rather than an unexplained `SM-...` identifier.
- **TECNO POVA 6** rather than an internal `TECNO-...`/board identifier.
- **Xiaomi / Redmi / POCO**, **OPPO**, **Vivo**, **Realme**, **Infinix**, **Google Pixel**, and compatible Android tablets with their actual names where provided.

### Retrieval and resolution policy

1. Use ADB's currently connected/authorized device enumeration as the discovery starting point (`adb devices -l` or an equivalent Rust implementation).
2. Only after authorization, query suitable read-only system properties such as `ro.product.manufacturer`, `ro.product.brand`, `ro.product.marketname`, `ro.product.model`, `ro.product.name`, `ro.product.device`, `ro.build.version.release`, and `ro.build.version.sdk`, using actual property availability on the device.
3. Also consider manufacturer-specific marketing-name properties only when verified and safe; do not assume they exist across Android versions.
4. Normalize case, whitespace, vendor prefixes, duplicate branding, malformed values, and empty placeholders. Preserve correct branded spelling and avoid labels such as "Samsung Samsung Galaxy...".
5. Prefer a reliably reported marketing name when present. When absent, fall back to the platform-provided model name; if that is only a model code, show a truthful brand+model code instead of inventing a commercial product name.
6. Never infer a marketing model from an unverified or ambiguous code. An optional maintained model-name database is acceptable only if its source, version, mapping confidence, and fallback behavior are documented and audited.
7. Keep user-defined device nicknames distinct from detected marketing/model metadata. Show the nickname first where configured, with the detected model accessible beneath or in details.
8. Use a stable, non-IP identity strategy for per-device profiles; wireless IP addresses, transport IDs, and user-facing names are not reliable unique identifiers. Handle duplicate/blank serials and USB/Wi-Fi reconnection carefully.
9. For unauthorized or offline devices, do not claim marketing-name detection succeeded. Show a safe placeholder, the reported transport identifier where appropriate, and clear authorization instructions.
10. Do not upload Android serials, names, or device inventories to a remote service for identification without explicit user consent.

### Device list details

Display the best available device name, manufacturer/brand, Android version, USB/Wi-Fi connection type, authorization status, and mirroring readiness. On selection, retain and display the exact codec/encoder capabilities detected on **that** Android device.

The platform selection card should remain **Android Phone / Tablet**; real discovered names such as Samsung Galaxy A57 or TECNO POVA 6 belong in the next Android-specific device list, not as static hardcoded platforms.

Add parser and device-name tests for Samsung-style model codes, TECNO marketing names, Xiaomi/Redmi/POCO labels, missing `ro.product.marketname`, duplicate brand prefixes, Android tablets, untrusted/unauthorized devices, multiple identical models, USB-to-Wi-Fi reconnect, and malformed Unicode/whitespace.

The user should not manually execute ADB commands during ordinary operation.

## First-time setup

Provide guided instructions:

1. Open Android Settings.
2. Enable Developer Options.
3. Enable USB Debugging.
4. Connect a USB cable.
5. Accept the RSA authorization request.
6. Return to ZenLink.
7. Start mirroring.

Provide manufacturer-specific guidance only when needed.

For Xiaomi-family devices, explain additional security debugging requirements if input injection fails.

Avoid changing global Android security settings automatically.

## Wireless Mode

Support both:

- ADB over TCP/IP.
- Android 11+ Wireless Debugging pairing.

Provide:

- Automatic network-device discovery where supported.
- Pairing code entry.
- Device IP and port configuration.
- Existing paired-device detection.
- Wireless reconnect.
- Connection quality reporting.
- Proper timeout handling.
- USB-to-Wi-Fi switching.
- Wi-Fi-to-USB switching.

Never expose the ADB debugging service to untrusted networks without explicit user action.

USB should be the recommended default for low-latency gaming and OBS.

Wireless should remain a first-class supported connection mode.

---

# 5. SMART ENCODER DETECTION

This feature is mandatory.

Implement a **Smart Encoder Detection System** for Android.

When a device connects:

1. Identify the device and Android version.
2. Query available video encoders.
3. Determine supported codecs.
4. Identify encoder implementations.
5. Inspect relevant resolutions and capabilities.
6. Determine which codecs are usable with ZenLink's decoder.
7. Select an appropriate starting configuration.
8. Save the selected configuration per device.

Use upstream scrcpy encoder-listing behavior as the reference.

Support these codec families when available:

- H.264 / AVC
- H.265 / HEVC
- AV1
- VP8 / VP9 where supported by the implemented pipeline

Do not expose unusable options as fully supported.

Do not assume the newest codec is the fastest.

## Default behavior

Use H.264 as the initial low-latency default.

Offer HEVC as a high-quality alternative when both encoder and decoder support are verified.

Offer AV1 only when the complete encode/decode path is supported.

## Encoder selection

Allow the user to choose:

- Automatic
- A specific codec
- A specific hardware encoder
- Software encoder, when available and appropriate

Never silently claim hardware acceleration if the chosen implementation is software-based.

## Encoder fallback

If encoder initialization fails:

1. Record the error.
2. Attempt another compatible hardware encoder.
3. Consider another compatible codec.
4. Reduce resolution when appropriate.
5. Inform the user of the fallback.
6. Preserve the user's original preferred settings for future attempts.

Do not create an endless retry loop.

---

# 6. ADVANCED VIDEO SETTINGS

Create a complete Android video settings panel.

## Basic profiles

Provide:

**Low Latency**
- H.264 preferred.
- Minimal video buffering.
- Hardware decoding.
- FPS selected according to device capability.
- Optimized for gaming.

**Balanced — Default**
- Automatic compatible encoder.
- Stable frame pacing.
- Sensible bitrate.
- Stable audio/video synchronization.

**High Quality**
- Prefer efficient hardware codecs.
- Higher bitrate where useful.
- Higher resolution.
- Prioritize image fidelity.

**Streaming / OBS**
- Stable capture frame pacing.
- Predictable timestamps.
- Audio/video synchronization.
- OBS-compatible presentation.

**Custom**
- Full manual control.

These are profiles, not fixed guarantees.

## Advanced controls

Expose:

- Codec.
- Specific video encoder.
- Target bitrate.
- Maximum FPS.
- Maximum resolution.
- Device-native resolution when supported.
- Video buffering.
- Hardware decoding preference.
- Audio buffering.
- Rotation behavior.
- Display selection where supported.
- Screen-off mirroring.
- Video recording.
- Performance statistics.

Start with sensible ranges, but discover actual device limitations instead of assuming that every device supports every setting.

Support 30, 60, 90, and 120 FPS options where appropriate.

Do not promise 120 FPS solely because the display has a 120 Hz refresh rate.

Preserve correct aspect ratio and orientation.

## Dynamic settings

Investigate which parameters can be changed during an active session.

Implement live changes only where supported by the selected server/protocol.

Otherwise, perform a controlled session restart while preserving the user's connection state.

Do not falsely advertise live bitrate adjustment if the selected scrcpy implementation requires restarting the encoder.

---

# 7. PER-DEVICE PROFILES

Implement persistent device profiles.

Each Android phone or tablet must retain its own configuration.

Store:

- Device identifier.
- User-defined device name.
- Manufacturer.
- Model.
- Android version.
- Preferred connection method.
- Preferred codec.
- Preferred encoder.
- Bitrate.
- FPS limit.
- Resolution limit.
- Video buffer.
- Audio buffer.
- Preferred synchronization mode.
- Audio device.
- Recording settings.
- Last successful configuration.

- Detected marketing name, raw manufacturer/brand/model properties, name-resolution source/confidence, and optional user nickname (with sensible privacy handling).

Examples:

A Samsung phone can use HEVC at high quality while a Xiaomi device uses H.264 for lower latency.

Settings for one device must not overwrite another device's configuration.

Handle duplicate model names and changing wireless addresses correctly.

Avoid storing unnecessary personal information or secret ADB credentials.

Provide:

- Restore defaults.
- Reset current device profile.
- Automatic optimization.
- Export/import profiles where practical.

---

# 8. LOW-LATENCY VIDEO PIPELINE

The Android integration must reuse ZenLink's Rust-native rendering infrastructure wherever technically feasible.

Target:

Android MediaCodec → USB/Wi-Fi transport → Rust receiver → hardware decoder → D3D11 GPU surface → ZenLink viewport.

Investigate:

- Media Foundation decoding.
- D3D11 hardware acceleration.
- GPU texture sharing.
- Zero-copy opportunities.
- Bounded frame queues.
- Timestamp-based presentation.
- Hardware synchronization.
- Multi-monitor frame pacing.
- Low-overhead UI composition.

Avoid unnecessary CPU memory copies.

Avoid additional encoding stages during preview.

Prefer dropping stale video frames over accumulating latency in the interactive preview.

Use controlled, measurable buffering.

Keep video rendering separate from Liquid Glass effects.

## Performance diagnostics

Measure:

- Received FPS.
- Decoded FPS.
- Presented FPS.
- Dropped frames.
- Duplicated frames.
- Queue depth.
- Decode latency.
- Render latency.
- Network jitter.
- End-to-end latency when measurable.

Never claim zero latency without a precise and valid measurement.

---

# 9. AUDIO AND VIDEO SYNCHRONIZATION

Audio/video synchronization is mandatory.

Investigate scrcpy's audio transport and timestamp semantics.

Implement audio output using the existing ZenLink native WASAPI engine where appropriate.

Support device audio forwarding on Android versions where upstream scrcpy supports it.

Provide graceful video-only mode when audio capture is unavailable or restricted.

## Synchronization modes

**Low Latency**
- Small buffers.
- Fast presentation.
- Minimal intentional delay.
- May tolerate additional jitter.

**Balanced — Default**
- Adaptive synchronization.
- Limited buffering.
- Stable audio/video alignment.

**Streaming / OBS**
- Timestamp-aware presentation.
- Stable synchronization.
- Drift correction.
- Predictable output behavior.

## Requirements

- Track audio/video timestamps correctly.
- Avoid unbounded latency accumulation.
- Recover from audio underruns.
- Support output-device changes.
- Handle packet jitter.
- Prevent A/V drift over long sessions.
- Expose diagnostic offset information.
- Maintain synchronization when recording.

Do not assume audio and video automatically remain synchronized just because they originate from the same Android device.

Do not claim perfect synchronization without real-device measurements.

Respect Android content-capture restrictions and protected-media policies.

---

# 10. NATIVE OBS INTEGRATION

Android mirroring must work smoothly with OBS.

Avoid repeating the previously reported ZenLink issue where mirroring appears smooth locally but stutters inside OBS Window Capture.

Investigate:

- D3D11 surface presentation.
- Windows Graphics Capture.
- DirectComposition.
- Desktop Duplication.
- OBS frame capture.
- Mixed refresh rates.
- Dual monitors.
- Hybrid GPU laptops.
- Frame pacing mismatch.

Test ordinary Window Capture and Display Capture compatibility.

Consider a dedicated OBS output only if it provides a measurable benefit and can be implemented safely.

Do not force an additional video encode/decode cycle merely to support OBS.

Keep the native Android preview responsive during streaming.

Audio must remain synchronized with the captured video.

---

# 11. ANDROID CONTROL SUPPORT

Implement the scrcpy control functionality supported by the selected protocol.

Core features:

- Touch interaction.
- Mouse input.
- Keyboard input.
- Scroll.
- Back.
- Home.
- Recent apps.
- Screen rotation.
- Clipboard synchronization.
- Text input.
- Device wake.
- Device screen-off functionality.
- Full-screen mirroring.
- Device orientation changes.

Evaluate UHID keyboard/mouse support.

Preserve the original interaction semantics where possible.

Support high-DPI coordinate mapping.

Ensure touch coordinates stay correct when rotating or resizing the mirror viewport.

Do not silently bypass Android input restrictions.

---

# 12. MULTIPLE DEVICES

Support multiple detected Android phones/tablets in the device selector.

At minimum:

- Discover all devices.
- Select the intended device.
- Prevent accidental connection to the wrong device.
- Maintain separate profiles.
- Display connection and authorization status.

Design the architecture so multiple simultaneous sessions can be supported without a rewrite.

Only advertise simultaneous mirroring after implementing and testing it.

Avoid sharing mutable session state between different devices.

---

# 13. LIGHTWEIGHT MODULAR PACKAGING

ZenLink must remain lightweight.

Do not bundle an unnecessary full Android SDK.

Use the smallest practical set of Android dependencies:

- ADB platform-tools components where required.
- Compatible scrcpy server.
- Required native libraries.
- ZenLink Rust Android module.

Do not require the full standalone scrcpy GUI application in production if the native Rust client already implements its functionality.

Prefer reusing the existing video renderer and audio engine instead of bundling duplicate media stacks.

Investigate a modular packaging structure:

**ZenLink Core**
- Native Rust application.
- Slint UI.
- Video renderer.
- Audio engine.
- Shared settings.
- Diagnostics.

**Apple Module**
- Existing Apple connectivity.
- Apple Mode.
- ZenLink Studio.
- AirPlay support.

**Android Module**
- ADB transport.
- scrcpy server integration.
- Android control protocol.
- Encoder detection.
- Android-specific settings.

Consider optional Android module delivery if it meaningfully reduces the initial installation size.

Any optional download must originate from verified, trustworthy sources and use integrity verification.

Offline portable builds must have a clearly documented complete-package option.

## Size optimization

Measure:

- Core executable size.
- Android module size.
- ADB dependency size.
- scrcpy server size.
- Setup installer size.
- Portable archive size.
- Installed application footprint.

Do not invent a final size target or claim that Rust automatically produces smaller applications.

Avoid Electron and unnecessary Python/Node.js runtimes.

---

# 14. OPEN-SOURCE ATTRIBUTION — MANDATORY

Update ZenLink's existing README section:

**🌟 Author, Attribution & Open-Source Lineage**

Preserve the existing creator attribution and all previously credited projects.

Add scrcpy to the original open-source sources list.

Use this attribution entry once the Android implementation actually incorporates scrcpy:

**scrcpy** by **Genymobile and scrcpy contributors** (Apache License 2.0) — the Android mirroring and control foundation used by ZenLink Android, including Android-side screen/audio capture and the scrcpy communication protocol. ZenLink integrates this technology into its own native Rust interface and rendering pipeline.

Source:
https://github.com/Genymobile/scrcpy

License:
https://github.com/Genymobile/scrcpy/blob/master/LICENSE

Do not remove, replace, or misrepresent the existing original authors.

Preserve:

**Creator & Lead Developer: Zenvy Zenshiro**

GitHub:
https://github.com/zenvyzenshiro

Update the creator's feature description to include the Android integration once it is implemented.

## Additional credit locations

Also update:

1. README — License & Credits.
2. Application Settings → About → Open-Source Licenses.
3. Third-party notices or equivalent license inventory.
4. Distribution license files.
5. Installer/package notices where required.

Preserve the Apache-2.0 license text and applicable attribution/notice obligations for redistributed code or binaries.

Audit licenses of ADB/platform-tools and all bundled dependencies independently.

Check compatibility with ZenLink's existing GPL-3.0 licensing.

Ensure that source modifications and redistributed binaries comply with applicable licenses.

Do not claim affiliation, endorsement, or official partnership with Genymobile.

Do not credit scrcpy as the creator of ZenLink.

Do not falsely claim that a referenced library is included if it was only studied.

---

# 15. UPDATE USER-FACING DOCUMENTATION

ZenLink currently documents itself primarily as an iPhone/iPad mirroring application.

Update documentation to describe the expanded platform support.

New positioning:

**ZenLink — iPhone, iPad & Android Mirroring for Windows**

Use accurate feature descriptions.

Document:

- Supported operating systems.
- Android version requirements.
- Developer Options.
- USB Debugging.
- Wireless Debugging.
- Android USB driver troubleshooting.
- Audio support limitations.
- Encoder selection.
- Codec support.
- Bitrate/FPS configuration.
- Device profile persistence.
- OBS integration.
- Recording.
- Permission handling.
- Known limitations.

Keep existing Apple-specific documentation intact.

Do not erase historical benchmarks.

Clearly separate confirmed hardware tests from theoretical capabilities.

---

# 16. ANDROID UX AND ERROR HANDLING

The user experience must remain straightforward for nontechnical users.

Examples:

**USB Debugging Disabled**

Explain how to enable Developer Options and USB Debugging.

**Device Unauthorized**

Explain that the RSA authorization must be accepted on the Android device.

**Device Offline**

Provide safe reconnection guidance.

**Encoder Unsupported**

Automatically select a compatible option where possible.

**Audio Unavailable**

Continue video mirroring and explain why audio is unavailable.

**Wireless Connection Failed**

Show the relevant pairing/network error without exposing sensitive information.

**ADB Server Conflict**

Detect incompatible or conflicting ADB instances and resolve safely without disrupting unrelated applications unnecessarily.

Do not display raw stack traces as the only explanation.

Implement structured diagnostic logs while protecting sensitive device data.

---

# 17. TESTING AND BENCHMARKING

Create automated tests for:

- Android device discovery.
- Device authorization state handling.

- Android marketing-name/property parsing and fallback precedence.
- Device nickname persistence and identity across USB/Wi-Fi transport changes.
- First-screen platform selection, remembered-platform opt-in, language toggle, and Back navigation.
- Compatibility regression: Apple Driver / ZenLink Studio popup still appears as Apple flow step two.
- Encoder list parsing.
- Codec selection.
- Encoder fallback.
- Bitrate configuration.
- Per-device settings.

- Verified device-name/marketing-name resolution and stable per-device identity (shared with the device picker).
- Transport recovery.
- Session restart.
- Video timestamp handling.
- Audio/video synchronization.
- Device selection.
- Resource cleanup.

Test hardware-dependent functionality on actual devices when available.

Target testing configurations:

- Samsung phone.
- Xiaomi/Redmi/POCO phone.
- At least one other Android manufacturer.
- Android tablet.
- Android 11+ audio-capable device.
- USB connection.
- Wi-Fi connection.
- H.264.
- H.265.
- AV1 where hardware supports it.
- OBS recording and streaming.

Benchmark:

- Startup time.
- Device detection time.
- Mirroring startup latency.
- CPU usage.
- RAM usage.
- GPU usage.
- Decoded FPS.
- Presented FPS.
- Frame-time stability.
- A/V synchronization error.
- End-to-end latency.
- Package size.

Where possible, compare ZenLink Android directly against official scrcpy using equivalent settings and hardware.

Do not claim ZenLink outperforms scrcpy unless measurements demonstrate it.

Do not mark tests as passed when no physical Android device was available.

---

# 18. IMPLEMENTATION EXECUTION ORDER

Proceed through the implementation in this order:

**Stage 1 — Audit**
Inspect the existing ZenLink Rust architecture and upstream scrcpy.

**Stage 2 — Foundation**
Create the Android device/transport abstraction and ADB integration.

**Stage 3 — Mirroring**
Integrate a compatible scrcpy server with the Rust client and native video renderer.

**Stage 4 — Controls**
Implement input, gestures, clipboard, orientation, and session management.

**Stage 5 — Audio**
Implement Android audio forwarding and timestamp-aware synchronization.

**Stage 6 — Smart Encoder**
Implement capability discovery, automatic codec selection, fallback, and manual controls.

**Stage 7 — Device Profiles**
Implement persistent per-device configuration and presets.

**Stage 8 — UI**
Integrate Android into the **new first-step platform selector**, retain the **existing Apple Driver / ZenLink Studio popup as the Apple second step**, add the persistent Indonesia/English language toggle, and implement Android device discovery with accurate manufacturer/marketing model naming. Preserve the Liquid Glass interface.

**Stage 9 — OBS**
Validate capture compatibility and optimize frame pacing.

**Stage 10 — Optimization**
Reduce memory allocations, redundant dependencies, and distribution size.

**Stage 11 — Attribution**
Update README, About, License & Credits, and third-party notices.

**Stage 12 — Validation**
Build, test, benchmark, and document actual results.

Stages may overlap when necessary, but maintain a functional build at each major milestone.

---

# 19. STRICT IMPLEMENTATION RULES

- Use the existing Rust reconstruction as the foundation.
- Preserve the existing Apple Mode and Studio Mode.
- Do not replace working Apple functionality with Android code.
- Do not require a separate scrcpy application window.
- Do not add an unnecessary Electron runtime.
- Do not fabricate support for unsupported codecs.
- Do not force AV1 on incompatible devices.
- Do not assume every device supports 120 FPS.
- Do not increase buffering merely to make FPS statistics look stable.
- Do not disable Android security restrictions.
- Do not silently install unsafe USB drivers.
- Do not erase existing open-source acknowledgments.
- Do not remove existing ZenLink features.

- Do not replace or bypass the existing Apple Driver / ZenLink Studio popup; it must follow iPhone/iPad selection.
- Do not hardcode Samsung Galaxy A57, TECNO POVA 6, or any other illustrative device names as detected facts.
- Do not treat unauthenticated device properties as available, or mistake a changing Wi-Fi address for stable device identity.
- Do not silently override the user's language choice or Apple-mode remember setting while introducing the new platform preference.
- Do not publish a new release without authorization.
- Do not claim zero latency.
- Do not pretend a UI mockup is a working implementation.
- Do not introduce unnecessary heavyweight dependencies.
- Do not stop after producing only documentation or a plan.

Run appropriate Rust formatting, linting, compilation, and test commands.

Repair implementation errors discovered during the build.

When hardware testing is unavailable, complete all testable implementation work and explicitly document what still requires physical devices.

---

# 20. REQUIRED DELIVERABLES

Provide:

**A. Architecture**

The Android transport, scrcpy integration, Rust client, video renderer, and audio pipeline.

**B. Implemented Features**

Working Android capabilities, including USB, Wi-Fi, encoder detection, device profiles, and input controls.

**C. Interface**

Implemented first-step iPhone/iPad vs Android Phone/Tablet selection, immediate Indonesian/English switching, unchanged second-step Apple mode popup, accurate Android brand/marketing-name discovery with fallbacks, and the Liquid Glass Android workspace.

**D. Performance**

Measured latency, CPU, memory, frame pacing, and package sizes.

**E. Attribution**

Updated README and licenses including Genymobile/scrcpy.

**F. Testing**

Actual passed, failed, skipped, and unverified tests.

**G. Compatibility**

Device and Android-version support backed by evidence.

**H. Remaining Work**

Explicit blockers and hardware-dependent validation requirements.

**I. Build Output**

Successful Windows build artifacts and their locations, if produced.

**J. Caveman Report**

Finish with a concise:

Status → Evidence → Changes → Blockers → Next

---

# FINAL INSTRUCTION

Execute the implementation now.

Use `/caveman` according to its installed project definition.

Audit the codebase, integrate Android support, implement native Rust mirroring using the scrcpy foundation, build Smart Encoder Detection, preserve audio/video synchronization, add per-device profiles, integrate the Liquid Glass interface, and update all required open-source attribution.

Do not stop at an architectural proposal.

The final goal is a unified, lightweight, Rust-first ZenLink for Windows that supports iPhone, iPad, Android phones, and Android tablets through a consistent premium interface.

Preserve the original ZenLink identity and make Android a properly integrated first-class platform.

**Mandatory UX completion condition:** On startup, present a bilingual Device Platform Selection step **before** the Apple Driver / ZenLink Studio popup, unless the user explicitly opted to remember a platform. Preserve all original Apple popup controls. After selecting Android, show actual detected brand and marketing name (when verifiable) for connected Android phones/tablets, with truthful fallbacks and per-device profiles. Do not mark this task complete based only on a new UI mockup.
