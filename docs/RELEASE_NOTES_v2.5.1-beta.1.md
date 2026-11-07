<!-- Release notes for the v2.5.1-beta.1 pre-release. -->

**ZenLink 2.5.1 Beta 1** is a pre-release for testing. It installs next to ZenLink 2.5.0 (its own folder, settings, port and logs), so 2.5.0 stays as it is.

### ✨ What's new

- **📱 Pick your iPhone / iPad.** The Apple screen lists every connected device (USB or Wi-Fi, iOS version, status) and the ones you used before. Click one to choose what Start mirrors; remove old ones with ✕.
- **🤖 Android apps launcher.** While a phone is mirrored, the dock has **Apps**: a searchable grid of the phone's apps. Open an app on the phone's screen, or (Android 10+) in its own window.
- **📂 Drop files on the window.** Android: an `.apk` is installed, any other file goes to the phone's Download folder, with progress. iPhone / iPad (Apple Driver mode): pick an app with File Sharing (VLC, Documents …) and the file lands in its folder, visible in Files › On My iPhone.
- **⬆️ Updates inside ZenLink.** Settings › Updates checks GitHub (Stable or Beta channel), downloads with a SHA-256 check and updates in place, keeping your settings.
- **🧾 Logs in one place.** Window, backend, crash and setup logs are in `%LOCALAPPDATA%\ZenLink-Beta\logs`. Settings › About › **Open log folder** (or Start menu › ZenLink Beta logs). Please attach them to bug reports.

### 🛠️ Fixes

- ZenLink Studio over Wi-Fi no longer looks stuck on "Connecting…": it says it is waiting for the iPhone / iPad and shows the Screen Mirroring steps.
- "Backend not found" now says what is missing and what to do.
- The Android app list no longer fails with "os error 10060" on slower phones.

### ⚠️ Known limits (beta)

- Sending files to an iPhone / iPad and Apple over Wi-Fi are not yet confirmed on a device.
- Windows may show SmartScreen: the files are not code-signed yet.

**Assets:** `Setup.exe` (installer), `Portable.exe` (no install), `Portable-Source.zip` (source package; run `Start ZenLink.bat`).
