# Size audit of the beta package (2026-11-07)

Every number here is measured on beta builds made in this folder, unless it
says "estimate". The audit was first run on the build in
`dist\ZenLinkService` from 2026-11-06 22:35; the "Applied" and "Result"
sections are from the final build of 2026-11-07.

## Method

1. **Bundle inventory.** Sizes of every file and folder in the PyInstaller
   onedir, and of every top-level package inside its PYZ archive
   (`build\ZenLinkService\PYZ-00.pyz`, compressed bytes, read with
   `PyInstaller.archive.readers.ZlibArchiveReader`).
2. **What the product loads.** The installed and portable builds start the
   backend as `ZenLinkService.exe --headless` (`zenlink-core/src/sidecar.rs`).
   A trace imports every `zenlink` module except the classic window
   (`zenlink.window`, `zenlink.tray`, `zenlink.__main__`), plus the launcher's
   self-test list (`packaging/launch.py`). Then it records every top-level
   package in `sys.modules`.
3. **Lazy imports.** For every loaded module, an AST scan finds imports inside
   functions and `importlib` calls whose package is not loaded yet. Those are
   on-demand dependencies and are kept. Found: `sounddevice` (audio), `zeroconf`
   (AirPlay, Wi-Fi pairing), `sslpsk_pmd3` (not installed; optional in
   pymobiledevice3), `traitlets`/`IPython` (pymobiledevice3's interactive
   shell only), and optional accelerators (`brotli`, `aiodns`, ...) that are not
   installed.
4. **Compressed cost.** LZMA preset 9e (close to Inno Setup) and zlib 9
   (PyInstaller onefile in Portable.exe) of each candidate set
   (`compress_sets.py`). PYZ parts are already compressed and are counted as is.

Limits: the trace proves what is imported at start and by the self-test
modules. A function in pymobiledevice3 that the device paths call could still
import a module that the scan reads but the trace does not load. Every
removal therefore needs the frozen self-test extended to the removed names,
plus a device test.

## Who uses what

| Area | Python packages / native files | Platform |
| :--- | :--- | :--- |
| Android (A1–A6) | none: Rust `zenlink-android`, Google `adb`, the scrcpy server. The backend is used only for Windows OCR (`/api/ocr`, PowerShell). | Android |
| Apple Driver mode | `pymobiledevice3`, `cryptography`, `qh3` (QUIC tunnel), `pmd_pytcp`, `pmd_net_*`, `pytun_pmd3` (wintun), `pyimg4`, `developer_disk_image`, `ipsw_parser`, `requests`, `lzfse`/`lzss` | Apple |
| ZenLink Studio (USB Direct) | `pyusb` + `libusb0.dll`, `zlaac.dll` | Apple |
| AirPlay | `zeroconf`, `ifaddr`, `pycryptodome` (`Crypto`), `playfair.dll`, `zlaac.dll` | Apple |
| Sound out | `sounddevice` + PortAudio (`libportaudio64bit.dll`) | Apple (Android audio is in Rust) |
| Backend server | `aiohttp`, `yarl`, `multidict`, `psutil` | both |
| Classic window (legacy, `ZENLINK_UI=classic`) | `webview` (pywebview), `pythonnet`, `clr_loader`, `bottle`, `pystray`, `PIL`, `ui\` | none in the Rust window |

Not moved to Rust yet: the whole Apple side (M1–M5 in `docs/ROADMAP.md`).

## Findings

| Set | Raw MB | LZMA MB (Setup) | zlib MB (Portable) | Evidence | Risk |
| :--- | ---: | ---: | ---: | :--- | :--- |
| B1 `pydantic` + `pydantic_core` | 6.64 | 2.42 | 2.97 | Not loaded; only `yarl` mentions it, for pydantic's own schema hook | Low |
| B2 fastapi, starlette, uvicorn, anyio, asgi_webdav (+ helpers) | 1.03 | 1.03 | 1.03 | Not loaded; pymobiledevice3's remote server / WebDAV CLI | Low–medium (Apple) |
| B3 typer, click, markdown_it, shellingham | 0.55 | 0.55 | 0.55 | Not loaded; pymobiledevice3 CLI | Low–medium (Apple) |
| B4 setuptools, PyInstaller, pefile, altgraph | 1.28 | 1.27 | 1.27 | Not loaded; build-time packages | Low |
| B5 `sqlite3.dll` + `_sqlite3.pyd` | 1.72 | 0.75 | 0.92 | Not loaded | Low |
| B6 pykdebugparser, dataclass_wizard, construct, opentelemetry, pcapng, bpylist2, pyiosbackup, ... | 0.78 | 0.78 | 0.78 | Not loaded; other pymobiledevice3 services | Low–medium (Apple) |
| B7 PortAudio for 32-bit, ARM64, macOS and ASIO | 1.73 | 0.46 | 0.82 | `sounddevice` loads `libportaudio64bit.dll` on x64 Python; ASIO only with `SD_ENABLE_ASIO`, which ZenLink never sets | Very low |
| C1 `RECORD` files in `*.dist-info` | 1.44 | 0.46 | 0.49 | `importlib.metadata.version()` reads `METADATA`, not `RECORD` | Low |
| C2 `chardet` (requests uses `charset_normalizer` without it) | 2.21 | 0.91 | 1.19 | Loaded by `requests` for text-encoding guesses | Low–medium |
| A classic window (pywebview, pythonnet, PIL, pystray, bottle, `ui\`) | 6.54 | 2.62 | 3.03 | Never loaded with `--headless`; reached only by running `ZenLinkService.exe` alone or `ZENLINK_UI=classic` | Low technically, but it drops the classic fallback in packaged builds (the source zip keeps it) |
| D1 Portable.exe packs adb uncompressed | — | — | saves 5.3 | `include_bytes!` stores 9.61 MB raw; zlib gives 4.30 MB | Low (needs a small decompressor) |

Kept on purpose (loaded, needed): `cryptography` 9.9 MB, `qh3` 5.5 MB,
`python313.dll`, `libcrypto-3.dll`, `pymobiledevice3`, `Crypto`, `zeroconf`.
Loaded only because pymobiledevice3 imports them at module level, for its
CLI: `pygments`, `prompt_toolkit`, `rich`, `questionary`, `wcwidth`
(about 4.5 MB). Removing them needs pymobiledevice3 patched, so they wait
for M1–M5, which removes Python entirely.

## Applied in v2.5 (2026-11-07)

Only the low-risk sets, each guarded:

- **B1, B4, B5**: PyInstaller `--exclude-module` for `pydantic`,
  `pydantic_core`, `setuptools`, `pkg_resources`, `_distutils_hack`,
  `PyInstaller`, `pefile`, `sqlite3`, `_sqlite3`.
- **B7**: project hook `packaging/pyi-hooks/hook-sounddevice.py` collects only
  `libportaudio64bit.dll`.
- **D1**: Portable.exe stores adb deflate-compressed (`build.rs`) and
  `android.rs` unpacks it byte-for-byte.
- **`--noupx`** is passed, so a UPX on PATH can never pack the backend.
- Guard: the build fails if any removed file comes back or
  `libportaudio64bit.dll` is missing. The frozen self-test (29 modules,
  including `sounddevice`, `zenlink.audio_out`, `zeroconf`, `usb.core`,
  `zenlink.ocr`) passes in the onedir and the onefile build.

Round 2 (2026-11-07, owner: "clean kan semua"):

- **A**: the classic WebView2 window is not packaged (`webview`, `pythonnet`,
  `clr_loader`, `bottle`, `proxy_tools`, `pystray`, `PIL`, `ui\`). Started
  alone, `ZenLinkService.exe` now opens `ZenLink.exe`; the server serves `/ui/`
  only when the folder exists. The source zip keeps the classic window.
- **C2**: `chardet` left out; `requests` uses `charset_normalizer` (both in
  the frozen self-test, now 32 modules).
- The build guard also fails on `webview`, `pythonnet`, `PIL`, `chardet`,
  `Python.Runtime.dll`.

Not applied: B2, B3, B6 (pymobiledevice3 may import them on device paths
that the start-up trace does not reach, e.g. `construct`), C1 (0.46 MB). They
wait for M1–M5.

## Result after round 2 (measured, 2026-11-07, v2.5.0)

| File | Round 1 | Round 2 | Change |
| :--- | ---: | ---: | ---: |
| Setup.exe | 43,897,801 B | 40,425,424 B (38.6 MiB) | −7.9% |
| Portable.exe | 58,616,320 B | 54,302,720 B (51.8 MiB) | −7.4% |
| Installed folder | 103,240,623 B | 94,431,410 B | −8.5% |
| Source zip | 14,340,730 B | 14,345,090 B | |

From the first beta build with adb (48,815,889 / 70,203,392 B): Setup −17.2%,
Portable −22.6%.

## Result of round 1 (measured, 2026-11-07)

| File | Before cleanup (with adb) | After | Change |
| :--- | ---: | ---: | ---: |
| Setup.exe | 48,815,889 B | 43,897,801 B (41.9 MiB) | −10.1% |
| Portable.exe | 70,203,392 B | 58,616,320 B (55.9 MiB) | −16.5% |
| Installed folder | 114,925,748 B | 103,240,623 B | −10.2% |
| Source zip | — | 14,340,730 B | |

For reference, the earlier beta build without adb or the scrcpy server
(B-28) was Setup 44,753,339 B and Portable 58,327,040 B. The final packages
now carry Android's adb and scrcpy server and are about the same size.

Device behaviour after the cleanup is unverified until the Apple and
Android device test round.
