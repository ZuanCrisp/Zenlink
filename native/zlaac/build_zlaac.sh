#!/usr/bin/env bash
# Links zlaac.dll against the static minimal FFmpeg from build_ffmpeg.sh.
#   PATH=/ucrt64/bin:/usr/bin bash native/zlaac/build_zlaac.sh <ffmpeg-prefix> <out-dir> [version]
# The DLL only imports Windows system DLLs (no libgcc / winpthread DLLs).
set -euo pipefail
FF=$1
OUT=$2
VER=${3:-2.5.0}
HERE=$(cd "$(dirname "$0")" && pwd)
NUM=$(echo "$VER" | sed 's/-.*//' | awk -F. '{printf "%d,%d,%d,0", $1, $2, $3}')
mkdir -p "$OUT"
printf '#define ZL_VER_NUM %s\n#define ZL_VER_STR "%s"\n' "$NUM" "$VER" > "$OUT/zlaac_ver.h"
windres --use-temp-file -I"$OUT" -i "$HERE/zlaac.rc" -o "$OUT/zlaac_rc.o"
gcc -O2 -shared -s -o "$OUT/zlaac.dll" "$HERE/zlaac.c" "$OUT/zlaac_rc.o" \
  -I"$FF/include" -L"$FF/lib" -lswresample -lavcodec -lavutil -lbcrypt \
  -static -static-libgcc -Wl,--gc-sections -Wl,--no-insert-timestamp
rm -f "$OUT/zlaac_rc.o" "$OUT/zlaac_ver.h"
ls -l "$OUT/zlaac.dll"
