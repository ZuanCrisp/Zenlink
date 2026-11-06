# scrcpy server protocol notes (pinned v5.0.1)

Source: scrcpy `doc/develop.md` at tag v5.0.1 (it documents the protocol used by
4.x and 5.x; the 5.0 changes are client-side). These notes are a summary for the
Rust client (phases A2-A4); upstream is the reference.

## Start

1. `adb push scrcpy-server-v5.0.1 /data/local/tmp/scrcpy-server.jar`
2. `adb reverse localabstract:scrcpy_<SCID> tcp:27183` (fallback: `adb forward
   tcp:27183 localabstract:scrcpy_<SCID>`). `SCID` is a 31-bit random number.
3. `adb shell CLASSPATH=/data/local/tmp/scrcpy-server.jar app_process / com.genymobile.scrcpy.Server 5.0.1 scid=<SCID> key=value ...`
   The first argument must equal the client version exactly.

## Sockets

- Up to three, opened in this order: video, audio, control. A disabled one is skipped.
- Forward tunnel only: the device sends one dummy byte on the first socket.
- Then, on the first socket, the device meta (currently the device name). Its exact
  layout is in upstream `DesktopConnection.java` (`sendDeviceMeta`); read it there
  when implementing (phase A2).

## Video and audio streams

- First the codec id, `u32` big-endian. Video: `h264` 0x68323634, `h265`
  0x68323635, `av1` 0x00617631, `vp8` 0x00767038, `vp9` 0x00767039. Audio: `opus`
  0x6F707573, `aac` 0x00616163, `flac` 0x666C6163, `raw` 0x00726177.
- Video only: a 12-byte session packet per capture session (a rotation starts a new
  one). Byte 0 has the MSB set (session packet flag); the lowest bit of byte 3 is the
  "client resized" flag (flex displays); bytes 4-7 width, bytes 8-11 height.
- Then media packets with a 12-byte header: bit 63 = 0 (media packet flag),
  bit 62 config packet, bit 61 key frame, bits 0-60 PTS (`u61`); then the packet
  size `u32`; then the raw `MediaCodec` payload.
- Session and media packets are told apart by the first bit (MSB).

## Control

- The control message format is defined by the upstream client and server source
  and their unit tests; port them with tests in phase A5.

## Version bumps

See the comment in `rust/crates/zenlink-android/src/scrcpy.rs`.
