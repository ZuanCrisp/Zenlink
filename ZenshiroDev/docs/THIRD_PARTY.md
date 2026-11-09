# Third-party components bundled with ZenLink

Design references for the Rust window are listed in `ZenshiroDev/docs/rust/THIRD_PARTY.md`.

## FFmpeg (AAC decoder only), in `ZenshiroDev/resources/windows/native/zlaac/zlaac.dll`

- What: the AAC / AAC-ELD decoder (`libavcodec`), `libswresample` and `libavutil`,
  statically linked into `zlaac.dll` (source: `ZenshiroDev/resources/windows/native/zlaac/`). Used for the
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
  tarball above; the build scripts are in `ZenshiroDev/resources/windows/native/zlaac/`.
- Configure line (`ZenshiroDev/resources/windows/native/zlaac/build_ffmpeg.sh`, MSYS2 UCRT64 gcc):

  ```
  configure --target-os=mingw32 --arch=x86_64 --cc=gcc --enable-static --disable-shared
    --disable-everything --disable-programs --disable-doc --disable-network
    --disable-autodetect --disable-avdevice --disable-avformat --disable-avfilter
    --disable-swscale --disable-pthreads --disable-w32threads --disable-os2threads
    --disable-x86asm --disable-debug --enable-avcodec --enable-avutil
    --enable-swresample --enable-decoder=aac --enable-small
  ```

## Test fixtures only (not bundled)

- `ZenshiroDev/tooling/dev/fixtures/eld_*.zlau` are AAC-ELD streams of a synthetic signal, encoded on
  the developer's PC with the Fraunhofer FDK AAC encoder 2.0.3
  (https://github.com/mstorsjo/fdk-aac, tag v2.0.3, tarball SHA-256
  `e25671cd96b10bad896aa42ab91a695a9e573395262baed4e4a2ff178d6a3a78`) through
  `ZenshiroDev/resources/windows/native/zlaac/tools/make_eld_fixture.cpp`. No FDK code or binary is part of
  ZenLink or its repository.
- `apps/desktop/tests/fixtures/android/synthetic-h264-360x640.scrcpy` is a generated picture
  (colour bars, a moving block, a frame counter) encoded to H.264 with libx264
  through PyAV in the developer venv (`ZenshiroDev/tooling/dev/make_android_fixture.py`) and
  wrapped in the scrcpy v5.0.1 video socket format. It holds no third-party
  content; no x264 code or binary is part of ZenLink.

## scrcpy server, in `ZenshiroDev/resources/third_party/scrcpy/`

- What: the official scrcpy server (`scrcpy-server-v5.0.1`), pushed to the
  Android device at `/data/local/tmp/scrcpy-server.jar` and run there with
  `app_process`. ZenLink has its own Rust client (`ZenshiroDev/crates/zenlink-android`);
  no scrcpy client code or window is used.
- Version: scrcpy **5.0.1**, pinned in `zenlink_android::scrcpy::SCRCPY_VERSION`.
  The server and the client protocol must match exactly.
- Source: https://github.com/Genymobile/scrcpy/releases/tag/v5.0.1, asset
  `scrcpy-server-v5.0.1` (733,930 bytes).
  - SHA-256 `764eb6f79811d5211fe9df341120882ba9994c7a61b897d7bf3fb662e53bc536`,
    as listed in the release's `SHA256SUMS.txt`.
  - `SHA256SUMS.txt.asc` verified: "Good signature from Romain Vimont", primary key
    `4569 58E8 5A18 5DD5 C2D1 E4E8 0C82 2B29 8461 FA03`.
  - Both files and the licence are kept next to the server.
- Licence: **Apache-2.0** (`ZenshiroDev/resources/third_party/scrcpy/LICENSE`), Copyright (C) 2018 Genymobile,
  Copyright (C) 2018-2026 Romain Vimont. Apache-2.0 is compatible with GPL-3.0.
  Credited in the README, in Settings › About & credits, and in the installed
  `THIRD_PARTY.md` with `third_party\scrcpy\LICENSE`.
- Checked by the Rust test
  `scrcpy::tests::bundled_server_is_present_with_the_pinned_size`.

## Android platform-tools `adb` (downloaded on request)

- What: `adb.exe`, `AdbWinApi.dll`, `AdbWinUsbApi.dll` and `NOTICE.txt` from
  Google's Android SDK Platform-Tools, **unmodified**. ZenLink starts this adb
  only when no ADB server runs; it never stops another program's server.
  Decision and evidence: `ZenshiroDev/docs/android/ADB.md`.
- Version: platform-tools **37.0.1** (`source.properties` `Pkg.Revision=37.0.1`).
  - Source: Google's official platform-tools for Windows
    (https://dl.google.com/android/repository/platform-tools-latest-windows.zip,
    listed in https://dl.google.com/android/repository/repository2-3.xml),
    installed through the Android SDK manager in `E:\AndroidDev\sdk`.
  - `adb.exe` SHA-256 `B4A6B455702684652CCCF7B46258B29E653538904359A58FD4931CF3EF286B3F`;
    the build prints all four SHA-256s.
  - Authenticode: `adb.exe`, `AdbWinApi.dll` and `AdbWinUsbApi.dll` are validly
    signed by **Google LLC**. `build_portable_release.ps1` stops if not.
- Where (since 2.5.11): no ZenLink package carries adb any more. When Android is
  chosen and no adb is found, ZenLink offers "Install adb": it downloads Google's
  `platform-tools_r37.0.1-win.zip` from dl.google.com (8,044,989 bytes, SHA-256
  `45f4d63113e895ebde0c90f194099a4676b6ac653bd28d54314a9e022bbc1a99`, both
  checked before anything is extracted) and extracts the same four files,
  unmodified, to `%LOCALAPPDATA%\ZenLink-Next\platform-tools`
  (`apps/desktop/src/android.rs` `install_adb`). Source mode uses the same
  in-app button; `ZenshiroDev\tooling\windows\tools\get-adb.ps1` is the manual
  equivalent (also checks that `adb.exe` is Google-signed). Installs of 2.5.1 and older kept adb in
  `third_party\platform-tools\` next to ZenLink.exe; ZenLink still uses it there.
- Licence (audit for B-10, 2026-11-07; a reading of the texts, not legal advice):
  - The platform-tools package comes with the Android SDK License Agreement
    (`package.xml`, `android-sdk-license`). §3.4 forbids redistributing the
    SDK "except to the extent required by applicable third party licenses".
  - §3.5: "Use, reproduction and distribution of components of the SDK
    licensed under an open source software license are governed solely by
    the terms of that open source software license".
  - `NOTICE.txt` lists adb's components (Android Open Source Project, mainly
    Apache-2.0, plus the notices of its libraries). adb is AOSP code
    (`packages/modules/adb`).
  - So the redistribution rests on §3.5 and Apache-2.0: ship the files
    unmodified with `NOTICE.txt` and credit them. Other projects do the same,
    for example scrcpy's Windows release ships these three files.
  - Apache-2.0 programs can be shipped next to a GPL-3.0 program; adb runs as
    a separate process.
  - The owner decides whether this reading is enough before a public release
    (B-10).
