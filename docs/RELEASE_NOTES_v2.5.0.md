<!-- Draft for the v2.5.0 release. Not published yet: it waits for the device
     test round and the owner's decisions listed under "Before publishing". -->

**ZenLink 2.5 · Next** brings Android to ZenLink and moves the window to native Rust. iPhone and iPad modes stay the way you know them.

### ✨ What's new

- **🤖 Android, no app on the phone.** Android 5.0+ over USB with the official scrcpy server (v5.0.1) and Google's official `adb` (37.0.1), both included. Picture, sound (Android 11+), mouse and keyboard control, clipboard, Back / Home / Recent Apps.
- **📶 Android over Wi-Fi.** One click from a USB connection, or Android 11+ Wireless debugging pairing with a 6-digit code.
- **🧠 Smart Encoder Detection.** H.264 by default; H.265 only when the phone and Windows both handle it, with automatic fallback. Video profiles (Low Latency, Balanced, High Quality, Streaming, Custom) and settings per device.
- **🪟 Native Rust window.** Media Foundation hardware decoding into Direct3D 11, one frame per refresh, Liquid Glass UI. No WebView2, no Visual C++ runtime.
- **🎬 OBS.** Game Capture of the ZenLink window is smoother and keeps working through rotation.
- **📋 OCR copy.** `Alt+C` copies the text on the screen to the PC, on Apple and Android.
- **📥 Tray & start with Windows.** Close to the notification area (the session keeps running) and start with Windows straight into the tray, both in Settings › Advanced.
- **🧭 First screen.** Choose iPhone / iPad or Android; the Apple Driver / ZenLink Studio choice stays as the Apple step.

### 🛠️ Fixes

- Portable.exe no longer needs the Visual C++ Redistributable.
- Windows N editions: ZenLink explains the missing Media Feature Pack and opens Windows' own settings page instead of failing silently.
- The installer no longer warns about WebView2.
- Settings slide without jitter; icons show on Windows 10.
- A phone plugged in without USB debugging now gets a hint instead of nothing.
- Setup upgrades an installed ZenLink 2.2 in place (one ZenLink in Start and in Apps).

### 📦 Downloads

| File | Size |
| :--- | ---: |
| `ZenLink-v2.5.0-Setup.exe` | 38.6 MB |
| `ZenLink-v2.5.0-Portable.exe` | 51.8 MB |
| `ZenLink-v2.5.0-Portable-Source.zip` | 13.7 MB |

Smaller than the beta even with `adb` inside: unused Python packages, the old WebView2 window and extra PortAudio builds are left out, and Portable.exe stores `adb` compressed.

### ⚠️ Known limits

- Some phones (for example Samsung) need the maker's USB driver; Windows Update usually installs it.
- Android sound needs Android 11+.

### 🙏 Credits

Android mirroring uses the [scrcpy](https://github.com/Genymobile/scrcpy) server by Genymobile and Romain Vimont (Apache-2.0) and Google's unmodified Android platform-tools `adb` (licence notices in `third_party\platform-tools\NOTICE.txt`). Apple modes use pymobiledevice3, quicktime_video_hack, libusb-win32 and UxPlay/playfair. Full list in `docs/THIRD_PARTY.md`.

Android is a trademark of Google LLC. iPhone, iPad and AirPlay are trademarks of Apple Inc. ZenLink is not affiliated with Google or Apple.

<!-- Before publishing (remove this block):
- Device test round passed: Android USB + Wi-Fi, iPhone Apple Driver, Studio cable + AirPlay, audio, OBS, driver switching, OCR.
- Prerelease or stable flag decided by the owner; never touch v2.2.31.
-->
