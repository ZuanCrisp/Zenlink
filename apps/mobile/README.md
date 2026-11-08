# ZenLink Mobile (Android, work in progress)

One APK for Android phones, tablets and Android TV / Google TV. A phone can send its
screen; any device can be the receiver. Everything stays on the local network: no
account, no cloud.

Status: foundation. Discovery (mDNS `_zenlink._tcp`), pairing with a 6-digit code and
the encrypted link work; screen capture and playback come next (roadmap phase 8).

## Build (Windows, command line)

Needs JDK 17 or 21 (not newer: the Android Gradle Plugin 8.7 does not run on JDK 25)
and the Android SDK (platform 35, build-tools 35).

```powershell
$env:JAVA_HOME = "<JDK 21 folder>"
$env:ANDROID_HOME = "<Android SDK folder>"
cd apps\mobile
.\gradlew.bat assembleDebug testDebugUnitTest
```

The APK is `app\build\outputs\apk\debug\app-debug.apk`. Install it with
`adb install -r app\build\outputs\apk\debug\app-debug.apk`.

If Gradle stops with "Unable to establish loopback connection", set
`JAVA_TOOL_OPTIONS=-Djdk.net.unixdomain.tmpdir=<a short writable folder>`.

## How it works

| Part | File |
|---|---|
| Home / Send / Receive screens (Jetpack Compose, D-pad friendly) | `ui/` |
| Discovery: NsdManager, `_zenlink._tcp`, TXT `v` (link version) and `tv` | `net/Discovery.kt` |
| TCP connect / listen with timeouts | `net/Link.kt` |
| Encrypted link v1 (ECDH P-256, HKDF-SHA256 with the pairing code, AES-256-GCM per direction) | `net/SecureLink.kt` |
| Record types (video/audio config and frames, touch, ping, bye) | `net/Wire.kt` |

Security note: the 6-digit code protects against anyone passively watching the
network. An active attacker present during pairing could test codes offline; a PAKE
(SPAKE2) is the planned upgrade. This is written in `SecureLink.kt` too.

Permissions: network and Wi-Fi state, multicast (discovery), foreground service for
screen capture (phase 8), notifications. No location, no storage.
