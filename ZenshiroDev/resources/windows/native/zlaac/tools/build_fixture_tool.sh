#!/usr/bin/env bash
# Dev-only: builds make_eld_fixture.exe against the FDK AAC encoder source.
#   PATH=/ucrt64/bin:/usr/bin bash native/zlaac/tools/build_fixture_tool.sh <fdk-aac-src> <out-dir>
set -euo pipefail
FDK=$1
OUT=$2
HERE=$(cd "$(dirname "$0")" && pwd)
mkdir -p "$OUT/obj"
INC=()
for d in libAACenc libFDK libSYS libMpegTPEnc libSBRenc libSACenc libPCMutils; do
  INC+=("-I$FDK/$d/include")
done
objs=()
for d in libAACenc libFDK libSYS libMpegTPEnc libSBRenc libSACenc libPCMutils; do
  for c in "$FDK/$d"/src/*.cpp; do
    o="$OUT/obj/${d}_$(basename "${c%.cpp}").o"
    [ -f "$o" ] || g++ -O2 -w -c "${INC[@]}" "$c" -o "$o"
    objs+=("$o")
  done
done
g++ -O2 "${INC[@]}" "$HERE/make_eld_fixture.cpp" "${objs[@]}" -static -o "$OUT/make_eld_fixture.exe"
ls -l "$OUT/make_eld_fixture.exe"
