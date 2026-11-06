"""ZenLink's PyInstaller hook for sounddevice (takes priority over the
contrib hook through --additional-hooks-dir).

The contrib hook bundles every PortAudio build in _sounddevice_data
(32-bit, ARM64, ASIO, macOS). ZenLink's backend is 64-bit Python on
Windows, where sounddevice loads libportaudio64bit.dll; the ASIO build is
only used with SD_ENABLE_ASIO, which ZenLink never sets. So only that DLL
and the README (licence notes) are collected. docs/SIZE_AUDIT.md, B7.
"""

import pathlib

from PyInstaller.utils.hooks import get_module_file_attribute

module_dir = pathlib.Path(get_module_file_attribute("sounddevice")).parent
data_dir = module_dir / "_sounddevice_data" / "portaudio-binaries"
destdir = str(data_dir.relative_to(module_dir))
lib = data_dir / "libportaudio64bit.dll"
if not lib.is_file():
    raise SystemExit(f"ZenLink hook: {lib} is missing")
binaries = [(str(lib), destdir)]
datas = [(str(data_dir / "README.md"), destdir)] if (data_dir / "README.md").is_file() else []
