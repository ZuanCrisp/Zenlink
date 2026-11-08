# Multi-device and file manager: design (roadmap phases 3-6)

Status: design only, 2026-11-09. Nothing here is implemented yet.

## Where 2.5.11 is single-device (audited)

| Layer | Single point | File |
|---|---|---|
| Rust window | `App { view: Option<VideoView>, streaming, ... }`: one video view, one stream state | `apps/desktop/src/main.rs` |
| Android mirror | `static ACTIVE: Mutex<Option<Arc<Shared>>>`, global `AUDIO_TO_PC`/`AUDIO_MUTED`/`AUDIO_VOLUME` | `apps/desktop/src/android.rs` |
| Apple backend | `CastManager._session` (16 uses), one `state` dict, `/api/state` returns one device | `apps/desktop/backend/zenlink/castmanager.py` |
| Apple Driver stream | fixed DisplayService HTTP port `STREAM_PORT = 47422`; userspace tunnel reset on start (`_reset_userspace_tunnel` closes any other tunnel) | `apple/session.py` |
| AirPlay | one receiver on port 7100, one advertised name | `airplay/receiver.py` |
| OBS | one window per app instance ("ZenLink" window capture) | `apps/desktop/src/appwin.rs` |

What is already per-device: adb transport (serial / transport id), Android prefs per device,
Apple device list with ids, the scrcpy server is started per device with its own socket name.

## Target shape

A **session registry** keyed by a stable device id (`android:<serial>`, `apple:<udid>`), each entry
owning its transport, decoder, audio stream and window. One **active** device gets keyboard/mouse
focus and the main window; others render in **preview windows** (own `VideoView` + own top-level
window, so OBS can capture each by title `ZenLink · <device name>`).

```
SessionRegistry (Rust, main thread owns UI handles)
 ├─ AndroidSession(serial)  ── scrcpy sockets (scid per session), decoder, audio sink
 ├─ AppleSession(udid)      ── backend session id → stream port from a pool (47422..47431)
 └─ focus: Option<DeviceId> ── input routing, audio "primary" choice
```

Backend: `CastManager` becomes `CastManager.sessions: dict[udid, Session]`; `/api/state` keeps the
current shape for the focused device (old clients keep working) and adds `sessions: [...]`.
`/api/cast/start` takes an optional `udid`. Each Apple Driver session gets its own stream port
from a small pool; the tunnel reset must only close tunnels of the same udid.

## Risks and rules

- Audio: only one session plays to Windows by default (the focused one); others muted, so audio
  routes never fight. Per-session volume later.
- Decoder: Media Foundation hardware decoders are limited per GPU; open at most N previews
  (measure on the user's GPU; start with 2) and fall back to a lower preview resolution.
- Driver: ZenLink Studio (libusb0) and Apple Driver can mix per device, because the driver is
  per USB device. AirPlay: one receiver can take several senders only if each gets its own
  receiver instance/port and name; start with one AirPlay sender.
- Keep single-device behaviour identical when only one device is connected (regression gate).

## Order

1. Android first (phase 4): `ACTIVE` → map by serial, second `VideoView` in a preview window,
   input routing by focus. Gate: two Android phones mirror at once (device-tested).
2. File manager for Android (phase 5): adb **sync** protocol (`LIST`/`STAT`/`RECV`/`SEND`) in
   `zenlink-android` over the existing adb client, streaming with progress and cancel; a panel in
   the Android workspace (quick access: DCIM, Download, Documents, Movies, Pictures). Works next
   to mirroring (separate adb connection). Gate: fixture tests for the sync framing, then
   device-tested push/pull of a 1 GB file while mirroring.
3. Apple multi-session (phase 6): stream port pool, per-udid tunnels, then iOS files through
   AFC (`com.apple.afc`: media area, DCIM) and house_arrest (apps with File Sharing). iOS never
   exposes the full filesystem; the UI must say so.
