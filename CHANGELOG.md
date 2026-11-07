# Changelog

Full notes for each version are on the [Releases](https://github.com/zenvyzenshiro/zenlink/releases) page.

## 2.5.1

### Added
- **Device picker:** the Apple screen lists every connected iPhone / iPad (USB or Wi-Fi, iOS version, status) and the ones used before.
- **Android apps launcher:** a searchable grid of the phone's apps; open one on the phone or (Android 10+) in its own window.
- **Drop files on the window:** Android installs an `.apk` or saves other files to Download; iPhone / iPad (Apple Driver mode) sends files to an app with File Sharing.
- **In-app updates:** Settings › Updates (Stable or Beta channel), SHA-256 checked, settings kept.
- **Logs in one place:** `%LOCALAPPDATA%\ZenLink-Next\logs`, opened from Settings › About.
- **Microsoft Store edition:** MSIX package (`packaging/build_msix.py`); the Store version gets updates from the Store.

### Fixed
- ZenLink Studio over Wi-Fi no longer looks stuck on "Connecting…".
- "Backend not found" says what is missing and what to do.
- The Android app list no longer fails with "os error 10060" on slower phones.
- The installer's single firewall prompt also covers the bundled `adb`.

## 2.5.0

### Added
- **Android, no app on the phone:** Android 5.0+ over USB or Wi-Fi (QR or pairing code) with the official scrcpy server (v5.0.1) and Google's `adb`. Picture, sound (Android 11+), mouse and keyboard, clipboard.
- **Smart Encoder Detection:** H.264 by default, H.265 only when verified, with fallback; video profiles and per-device settings.
- **Native Rust window:** Media Foundation hardware decoding into Direct3D 11, Liquid Glass UI (Slint). No WebView2, no Visual C++ runtime.
- OCR copy on Apple and Android, tray and start with Windows, auto unlock (Apple Driver mode), first screen to choose iPhone / iPad or Android.

### Fixed
- Portable.exe no longer needs the Visual C++ Redistributable; Windows N editions get a clear Media Feature Pack message; the installer no longer warns about WebView2.
- OBS Game Capture is smoother and survives rotation; Android sound no longer crackles.

## 2.2.2

### Added
- **Hybrid Control for iOS 17–26 (Experimental).** In Apple Driver mode, an iOS 17–26 device mirrors its picture and sound to ZenLink over AirPlay, while touch, keyboard, gestures and buttons go over Apple's own driver through CoreDevice HID. Both run at once without touching the USB driver. Not yet confirmed on a real iOS 17–26 device.
- **Control level, from what actually opened:** Native Control, Hybrid Control (Experimental), Mirroring Only or Unsupported. Shown on the device chip, in the idle screen and under Settings → Connection & Drivers → Control level.
- **AirPlay guide for Hybrid:** while ZenLink waits for the device to mirror, the step-by-step guide is shown in the app.
- **Capability check on the device:** before claiming control, ZenLink checks that the device offers the display and HID services (mounting the developer image when needed). When they're missing, the picture keeps running and the reason is shown.
- iOS compatibility matrix in the README.

### Changed
- Apple Driver mode now starts on iOS 17 and later; iOS 16 and older get a clear message pointing to ZenLink Studio.
- iOS 27 and later keep the native Apple mode unchanged.

### Fixed
- **Apple Driver mode in the exe failed at "Secure connection"** with "No package metadata was found for pyimg4". The build left out a package's metadata that the developer-image code reads, and also `wintun.dll`, which the tunnel code loads. Both are included now, and every build checks itself (`ZenLink.exe --selftest`) before it can be released.

### Not changed
- ZenLink Studio (cable and AirPlay), the USB driver switch, restore, automatic rollback and the error 259 handling.

## 2.2.1
- Fixed switching to ZenLink Studio failing with "A parameter cannot be found that matches parameter name 'DeleteKey'".
- Driver switching works on Windows in any language, survives Apple Mobile Device Service holding the device (error 259), says when a restart is needed, and undoes a failed switch cleanly.
- Apple Driver mode on iOS 26 and older showed a clear message instead of a black screen.

## 2.2
- Apple mode sound no longer crackles; App Switcher, Notifications and Control Center buttons toggle; double-click no longer goes full screen in Apple mode.
- Language choice in the first menu, stats beside the device name, single Portable.exe, opening ZenLink again shows its window.
