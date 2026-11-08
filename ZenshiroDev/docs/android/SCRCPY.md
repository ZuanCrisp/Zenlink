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
- Then, on the first socket, the device meta: the device name, 64 bytes, UTF-8,
  NUL-padded (`DEVICE_NAME_FIELD_LENGTH` in `DesktopConnection.java`).

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

See the comment in `ZenshiroDev/crates/zenlink-android/src/scrcpy.rs`.

## ZenLink implementation (phase A3)

- `ZenshiroDev/crates/zenlink-android/src/mirror.rs`: `Mirror::start` pushes the server
  (`sync:`), creates `reverse:forward:localabstract:scrcpy_<scid>;tcp:<port>` (or a
  forward with `tunnel_forward=true` when reverse fails), runs
  `shell:CLASSPATH=... app_process / com.genymobile.scrcpy.Server 5.0.1 scid=<scid> log_level=info video_codec=h264 audio=false control=false max_size=1920 video_bit_rate=8000000 max_fps=60`,
  accepts the video socket (10 s limit), removes the tunnel, reads the device meta and
  the codec id. The scid is 31 bits, formatted `%08x` (the server parses it as hex).
- Closing the shell socket ends the server on the device (`Stopper`, `Session` drop).
- `apps/desktop/src/android.rs` converts packets for the shared decoder:
  config (Annex B SPS/PPS) to an avcC record, frames to 4-byte NAL lengths; session
  packets are skipped because a rotation is followed by a new config.
- Test without a device: `ZL_ANDROID_FIXTURE=apps\desktop\tests\fixtures\android\synthetic-h264-360x640.scrcpy`
  with `ZL_ADB=none` plays a recorded stream on the Android screen, paced by its PTS.

## Sound (phase A4)

- Option `audio=true audio_codec=raw`: PCM s16le, 48 kHz, stereo (`AudioConfig.java`),
  about 1.5 Mbps. No decoder is needed on the PC; Opus/AAC/FLAC can come later for
  Wi-Fi bandwidth. Default server values: `audio_source=output`, `audio_dup=false`.
- The audio socket is the second socket (after video, before control). It starts
  with the codec id; `0` means the device disabled the stream (for example Android
  10 or older, or capture refused) and `1` a configuration error
  (`Streamer.writeDisableStream`). ZenLink then mirrors video only.
- Audio packets use the same 12-byte header as video (no session packets).

## Control (phase A4)

- Server options: `control=true clipboard_autosync=false`. The control socket is the
  third socket (video, audio, control); without audio it is the second.
- Messages (`ControlMessageReader.java`, v5.0.1): type byte, then big-endian fields.
  Touch = action u8, pointer id i64, position (x i32, y i32, width u16, height u16),
  pressure u16 fixed point (1.0 = 0xffff), action button i32, buttons i32. Scroll =
  position, h and v as i16 fixed point of `value / 16` (range -16..16), buttons i32.
  Text = u32 length + UTF-8, at most 300 bytes. Set clipboard = sequence i64, paste u8,
  u32 length + UTF-8.
- The position's width/height must equal the current video size (from the session
  packet), or the server ignores the event (for example just after a rotation).
- ZenLink uses pointer id -2 (generic finger: a touchscreen event) for the mouse.
- The device writes clipboard, ack and UHID messages on the same socket; ZenLink
  drains them.

## Smart Encoder Detection (phase A5)

- `list_encoders=true` makes the v5.0.1 server print its encoders through `Ln.i` and exit without mirroring (`Server.java`, `LogUtils.buildEncoderListMessage`). Line format: `--video-codec=<codec> --video-encoder=<name>`, padded, then on API 29+ `(hw|sw|hybrid)`, optional `[vendor]` and `(alias for <canonical>)`. Parser: `zenlink-android/src/encoders.rs`.
- Codec names: `h264`, `h265`, `av1`, `vp8`, `vp9` (`VideoCodec.java`). ZenLink offers only codecs it can play: H.264 always, H.265 when Windows has a decoder and the phone an encoder; AV1/VP8/VP9 never (no decoder path yet).
- `video_encoder=<name>` picks a specific encoder; the name is checked (`[A-Za-z0-9._-]`, at most 128 bytes) before it goes into the shell command.
- Fallback plan (at most 4 attempts): the chosen encoder, up to two non-software encoders of the codec, the server default for the codec, then H.264 with the server default. The server's own `downsize_on_error` (default true, `SurfaceEncoder.MAX_SIZE_FALLBACK`) covers lowering the resolution. The user's choice is never overwritten.
- Live changes: the v5.0.1 control protocol has no bitrate/codec message, so picture changes restart the session.
- Per-device settings: `android-devices.json` next to the prefs, keyed by `identity::DeviceId::key` (the hardware serial, the same over USB and Wi-Fi). Stores names, profile, picture values and the last working codec/encoder. No ADB keys or IP addresses.

