# Build toolchain (ZenLink Next)

Tools already on the PC are used first (AGENTS.md §3d-2). Anything new goes to `E:\DevTools`.

| Tool | Version | Where | Source / checksum | Used for |
|---|---|---|---|---|
| Rust (cargo, rustc) | 1.95 | `C:\Users\ZenvyZenshiro\.cargo\bin` | existing | the window (`rust/`) |
| Python | 3.13.5 (`py -3.13`) | existing | existing | backend, tests, PyInstaller |
| Inno Setup | 6 | `%LOCALAPPDATA%\Programs\Inno Setup 6\ISCC.exe` | existing | `Setup.exe` |
| MSYS2 UCRT64 gcc, make, windres | gcc (UCRT64), GNU Make 4.4.1 | `C:\msys64` | existing | `native/zlaac` (FFmpeg + zlaac.dll) |
| FFmpeg source | 9.0.2 | `E:\DevTools\src\ffmpeg-9.0.2` | ffmpeg.org, SHA-256 `8c3850283eb25fa026482078a04051e0be17347b09ef81a0849bec15a96e002e`, signature verified | zlaac.dll |
| FDK AAC source | 2.0.3 | `E:\DevTools\src\fdk-aac-2.0.3` | github.com/mstorsjo/fdk-aac, SHA-256 `e25671cd96b10bad896aa42ab91a695a9e573395262baed4e4a2ff178d6a3a78` | test fixtures only (never bundled) |

Build outputs live in `E:\DevTools\build\` (not in the repository):
`ffmpeg-aac\out` (static FFmpeg), `fdk-tool\make_eld_fixture.exe`.

## Rebuild zlaac.dll

From Git Bash in the repository root:

```
MSYSTEM=UCRT64 /c/msys64/usr/bin/env.exe PATH=/ucrt64/bin:/usr/bin TMP='E:\DevTools\build\tmp' TEMP='E:\DevTools\build\tmp' \
  /c/msys64/usr/bin/bash.exe native/zlaac/build_ffmpeg.sh /e/DevTools/src/ffmpeg-9.0.2 /e/DevTools/build/ffmpeg-aac /e/DevTools/build/ffmpeg-aac/out
MSYSTEM=UCRT64 /c/msys64/usr/bin/env.exe PATH=/ucrt64/bin:/usr/bin TMP='E:\DevTools\build\tmp' TEMP='E:\DevTools\build\tmp' \
  /c/msys64/usr/bin/bash.exe native/zlaac/build_zlaac.sh /e/DevTools/build/ffmpeg-aac/out zenlink 2.5.0-beta.1
```

`TMP`/`TEMP` must point at a writable folder: without them gcc tries `C:\WINDOWS`.
Then run `tests/test_aac.py`: the pinned output hashes must still match.
