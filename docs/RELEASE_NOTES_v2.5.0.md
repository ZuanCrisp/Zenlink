<!-- Release notes for v2.5.0 (assets rebuilt 2026-11-07 with the auto unlock,
     Android sound and USB/Wi-Fi choice fixes). -->

**ZenLink 2.5 · Next** brings Android to ZenLink and moves the window to native Rust. iPhone and iPad modes stay the way you know them.

### ✨ What's new

- **🤖 Android, no app on the phone.** Android 5.0+ over USB with the official scrcpy server (v5.0.1) and Google's official `adb` (37.0.1), both included. Picture, sound (Android 11+), mouse and keyboard control, clipboard, Back / Home / Recent Apps.
- **📶 Android over Wi-Fi.** One click from a USB connection, or Android 11+ Wireless debugging: scan the QR code ZenLink shows (no typing), or enter a 6-digit pairing code.
- **🧠 Smart Encoder Detection.** H.264 by default; H.265 only when the phone and Windows both handle it, with automatic fallback. Video profiles (Low Latency, Balanced, High Quality, Streaming, Custom) and settings per device.
- **🪟 Native Rust window.** Media Foundation hardware decoding into Direct3D 11, one frame per refresh, Liquid Glass UI. No WebView2, no Visual C++ runtime.
- **🎬 OBS.** Game Capture of the ZenLink window is smoother and keeps working through rotation.
- **📋 OCR copy.** `Alt+C` copies the text on the screen to the PC, on Apple and Android.
- **📥 Tray & start with Windows.** Close to the notification area (the session keeps running) and start with Windows straight into the tray, both in Settings › Advanced.
- **🔓 Auto unlock (Apple Driver mode).** Save the iPhone/iPad passcode or password in Settings; ZenLink unlocks the device when mirroring starts. Stored encrypted with Windows DPAPI on this PC only. `Alt+U` unlocks at any time.
- **🔌 USB or Wi-Fi, your choice.** An Android phone connected both ways shows "USB + Wi-Fi", and Start lets you pick the connection.
- **🧭 First screen.** Choose iPhone / iPad or Android; the Apple Driver / ZenLink Studio choice stays as the Apple step.

### 🛠️ Fixes

- Portable.exe no longer needs the Visual C++ Redistributable.
- Windows N editions: ZenLink explains the missing Media Feature Pack and opens Windows' own settings page instead of failing silently.
- The installer no longer warns about WebView2.
- Settings slide without jitter; icons show on Windows 10.
- A phone plugged in without USB debugging now gets a hint instead of nothing.
- Setup upgrades an installed ZenLink 2.2 in place (one ZenLink in Start and in Apps).
- Notifications and tooltips are no longer hidden behind the picture.
- The window opens larger (up to 85% of the screen height); a size you pick is remembered.
- Android sound no longer crackles: the buffer follows the phone's clock smoothly and keeps more margin on Wi-Fi.
- Android Wi-Fi setup shows each step (1/3, 2/3, 3/3) with a progress bar, and the QR / pairing-code instructions are numbered and clearer.
- Settings info panels no longer overlap, and their colours match the theme.
- The window opens wider on a fresh start.
- Shortcuts: `Alt+N` / `Alt+Q` open notifications and Control Center (or Quick Settings on Android); a second press closes them. `Alt+M` turns the sound on the PC on or off. Plain keys always go to the phone or iPad (Space no longer starts or stops mirroring).

### 📦 Downloads

| File | Size |
| :--- | ---: |
| `ZenLink-v2.5.0-Setup.exe` | 38.6 MB |
| `ZenLink-v2.5.0-Portable.exe` | 52.2 MB |
| `ZenLink-v2.5.0-Portable-Source.zip` | 8.9 MB |

Smaller than the beta even with `adb` inside: unused Python packages, the old WebView2 window and extra PortAudio builds are left out, and Portable.exe stores `adb` compressed. The source package downloads Google's `adb` (37.0.1, checksum and signature checked) and the Python packages at the release build's exact versions the first time `Start ZenLink.bat` runs.

### ⚠️ Known limits

- Some phones (for example Samsung) need the maker's USB driver; Windows Update usually installs it.
- Android sound needs Android 11+.
- Android Wi-Fi QR pairing and the ZenLink Studio "Device only" sound were checked without a phone so far. Please report anything that does not work in [Issues](https://github.com/zenvyzenshiro/zenlink/issues).

### 🙏 Credits

Android mirroring uses the [scrcpy](https://github.com/Genymobile/scrcpy) server by Genymobile and Romain Vimont (Apache-2.0) and Google's unmodified Android platform-tools `adb` (licence notices in `third_party\platform-tools\NOTICE.txt`). Apple modes use pymobiledevice3, quicktime_video_hack, libusb-win32 and UxPlay/playfair. Full list in `docs/THIRD_PARTY.md`.

Android is a trademark of Google LLC. iPhone, iPad and AirPlay are trademarks of Apple Inc. ZenLink is not affiliated with Google or Apple.

<!-- Before publishing (remove this block):
- Device test round passed: Android USB + Wi-Fi, iPhone Apple Driver, Studio cable + AirPlay, audio, OBS, driver switching, OCR.
- Prerelease or stable flag decided by the owner; never touch v2.2.31.
-->
