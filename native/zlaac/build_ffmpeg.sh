#!/usr/bin/env bash
# Builds the minimal FFmpeg that ZenLink needs (AAC/AAC-ELD decoder + swresample),
# as static libraries, with MSYS2 UCRT64 gcc. Run from an MSYS2 bash:
#   PATH=/ucrt64/bin:/usr/bin bash native/zlaac/build_ffmpeg.sh <ffmpeg-src> <build-dir> <prefix>
# The configure line is recorded in docs/THIRD_PARTY.md. LGPL-2.1+ (no --enable-gpl).
set -euo pipefail
SRC=$1
BUILD=$2
PREFIX=$3
mkdir -p "$BUILD"
cd "$BUILD"
"$SRC/configure" \
  --prefix="$PREFIX" \
  --target-os=mingw32 --arch=x86_64 --cc=gcc \
  --enable-static --disable-shared \
  --disable-everything \
  --disable-programs --disable-doc --disable-network --disable-autodetect \
  --disable-avdevice --disable-avformat --disable-avfilter --disable-swscale \
  --disable-pthreads --disable-w32threads --disable-os2threads \
  --disable-x86asm --disable-debug \
  --enable-avcodec --enable-avutil --enable-swresample \
  --enable-decoder=aac \
  --enable-small
make -j"$(nproc)"
make install
