# ADB transport decision (phase A1)

Decision: **bundle the official Google `adb` server** (`adb.exe`, `AdbWinApi.dll`,
`AdbWinUsbApi.dll` from platform-tools, unmodified) and talk to it from Rust over
the documented ADB host protocol on `127.0.0.1:5037`
(`rust/crates/zenlink-android/src/adb.rs`). A Rust USB ADB client is **not** used.

## Evidence

| Point | Bundled `adb` server + Rust host-protocol client | Rust USB ADB client (e.g. `rusb`) |
| :--- | :--- | :--- |
| USB driver | Uses whatever the phone already has: Google USB driver, the OEM drivers (Samsung, Xiaomi, MediaTek, ...) through `AdbWinApi.dll`. | Needs WinUSB on the ADB interface. OEM drivers such as Samsung's are not WinUSB, so many phones would need a driver change (AGENTS.md §4: never without consent). |
| Other tools | Shares one server with Android Studio, scrcpy, phone suites. | Two programs cannot claim the same USB interface: a running `adb` server (very common) blocks it, or we block it. |
| Wi-Fi | TCP/IP, Android 11+ pairing, TLS and mDNS are built in. | Must reimplement pairing (SPAKE2), TLS and mDNS. |
| Authorization | Uses the user's existing `~/.android/adbkey`; phones already trusted stay trusted. | Needs its own RSA key handling; phones ask again. |
| Size | `adb.exe` 8,273,560 B + `AdbWinApi.dll` 108,184 B + `AdbWinUsbApi.dll` 73,368 B = **8.06 MiB** (local platform-tools 37.0.1, measured 2026-11-06). | Smaller on disk (no `adb.exe`), but more code to own. |
| Antivirus | Google-signed binaries (signature checked: Valid). | Our own code; no extra binary. |
| Imports | `adb.exe` imports only Windows system DLLs + `AdbWinApi.dll` (checked); no VC++ runtime needed. | n/a |

The size cost is real (8 MiB). It buys every OEM driver, coexistence with other
adb tools and Wi-Fi pairing for free. A Rust USB client stays an option for later
if a measured need appears; the device list and naming code do not depend on the
transport.

## Server rules

- `check_server` first. If a server with the bundled host version (41) runs,
  share it, even if another tool started it.
- If a server with a **different** version runs, do not start our `adb`: an adb
  client of another version kills the server ("adb server version (x) doesn't
  match this client (y); killing..."). Tell the user which tool is in the way and
  ask (master prompt section 16).
- Start our server only when nothing listens on the port, without a console window
  (`CREATE_NO_WINDOW`).
- `ANDROID_ADB_SERVER_PORT` is honoured like `adb` does.

## Versions

- Latest platform-tools on 2026-11-06: **37.0.2**
  (`platform-tools_r37.0.2-win.zip`, 8,239,125 bytes, SHA-1
  `d7ee482b3874f1e0d6d5dca135f6bd58f5716db0`, from `repository2-3.xml`).
- scrcpy 5.0 ships platform-tools 37.0.1.
- Bundling happens in phase A3, with the then-latest version, checksum recorded in
  `docs/THIRD_PARTY.md`, and the licence audit (B-10).

## Not verified yet

- Behaviour with a real phone (authorization prompt, USB and Wi-Fi). Needs a device
  and the user's consent (AGENTS.md §4).
