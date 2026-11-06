# Size audit of the beta package (2026-11-07)

Analysis only: nothing listed here is removed yet. Every number is measured
on the beta build in `dist\ZenLinkService` (built 2026-11-06 22:35) unless it
says "estimate".

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

Also noted: `build_portable_release.ps1` does not pass `--noupx`. UPX is not
installed on this PC, so nothing is packed now, but a UPX on PATH would pack
the backend (AGENTS.md §3b and §3c forbid it).

## Totals

| | Raw (installed) | Setup (LZMA) | Portable (zlib) |
| :--- | ---: | ---: | ---: |
| Conservative: B1–B7 + C1 | 15.2 MB | 7.7 MB | 8.8 MB |
| Optimistic: + A + C2 + D1 | 23.9 MB | 11.3 MB | 18.4 MB |
