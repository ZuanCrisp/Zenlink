# Third-party components bundled with ZenLink Next

Design references for the Rust window are listed in `docs/rust/THIRD_PARTY.md`.

## FFmpeg (AAC decoder only), in `zenlink/zlaac.dll`

- What: the AAC / AAC-ELD decoder (`libavcodec`), `libswresample` and `libavutil`,
  statically linked into `zlaac.dll` (source: `native/zlaac/`). Used for the
  iPhone/iPad sound in Apple Driver mode and AirPlay. Replaces PyAV, whose wheel
  bundled the full FFmpeg (~65 MB).
- Version: FFmpeg **9.0.2** (the same FFmpeg version as PyAV 19.0.2, which it replaces).
- Source: https://ffmpeg.org/releases/ffmpeg-9.0.2.tar.xz
  - SHA-256 `8c3850283eb25fa026482078a04051e0be17347b09ef81a0849bec15a96e002e`
  - OpenPGP signature `ffmpeg-9.0.2.tar.xz.asc` verified: "Good signature from FFmpeg
    release signing key", key `FCF9 86EA 15E6 E293 A564 4F10 B432 2F04 D676 58D8`.
- Licence: **LGPL-2.1-or-later** (configure reports "License: LGPL version 2.1 or
  later"; no `--enable-gpl`, no `--enable-nonfree`). LGPL-2.1+ is compatible with
  ZenLink's GPL-3.0-or-later. The corresponding FFmpeg source is the unmodified
  tarball above; the build scripts are in `native/zlaac/`.
- Configure line (`native/zlaac/build_ffmpeg.sh`, MSYS2 UCRT64 gcc):

  ```
  configure --target-os=mingw32 --arch=x86_64 --cc=gcc --enable-static --disable-shared
    --disable-everything --disable-programs --disable-doc --disable-network
    --disable-autodetect --disable-avdevice --disable-avformat --disable-avfilter
    --disable-swscale --disable-pthreads --disable-w32threads --disable-os2threads
    --disable-x86asm --disable-debug --enable-avcodec --enable-avutil
    --enable-swresample --enable-decoder=aac --enable-small
  ```

## Test fixtures only (not bundled)

- `tests/fixtures/eld_*.zlau` are AAC-ELD streams of a synthetic signal, encoded on
  the developer's PC with the Fraunhofer FDK AAC encoder 2.0.3
  (https://github.com/mstorsjo/fdk-aac, tag v2.0.3, tarball SHA-256
  `e25671cd96b10bad896aa42ab91a695a9e573395262baed4e4a2ff178d6a3a78`) through
  `native/zlaac/tools/make_eld_fixture.cpp`. No FDK code or binary is part of
  ZenLink or its repository.
