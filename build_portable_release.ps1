<#
.SYNOPSIS
  Builds the ZenLink release files into .\release\:

    ZenLink-v<ver>-Portable.exe           one ZenLink.exe, nothing to install (no Python)
    ZenLink-v<ver>-Setup.exe              installer (Inno Setup 6, if installed)
    ZenLink-v<ver>-Portable-Source.zip    source + Start ZenLink.bat (sets up Python and adb on first run)

  ZenLink.exe is the Rust window (rust\). The Python backend is built as
  ZenLinkService.exe; ZenLink.exe starts it headless. Needs cargo (rustup).

.EXAMPLE
  powershell -ExecutionPolicy Bypass -File .\build_portable_release.ps1
#>
param(
    [string]$Version = "",
    # Folder with Google's platform-tools (adb.exe, AdbWinApi.dll,
    # AdbWinUsbApi.dll, NOTICE.txt). Default: ANDROID_HOME / ANDROID_SDK_ROOT /
    # Android Studio's SDK folder.
    [string]$PlatformTools = ""
)
$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot

if (-not $Version) {
    $Version = (Select-String -Path pyproject.toml -Pattern '^version\s*=\s*"([^"]+)"').Matches[0].Groups[1].Value
}
$py = Join-Path $PSScriptRoot ".venv\Scripts\python.exe"
if (-not (Test-Path $py)) { throw "Run Start ZenLink.bat once first (it creates .venv)." }

# Android (B-28): every package carries Google's official adb, unmodified.
# The build stops when it is missing or not validly signed by Google.
$adbFiles = "adb.exe", "AdbWinApi.dll", "AdbWinUsbApi.dll", "NOTICE.txt"
if (-not $PlatformTools) {
    $PlatformTools = @($env:ANDROID_HOME, $env:ANDROID_SDK_ROOT, (Join-Path $env:LOCALAPPDATA "Android\Sdk")) |
        Where-Object { $_ } | ForEach-Object { Join-Path $_ "platform-tools" } |
        Where-Object { Test-Path (Join-Path $_ "adb.exe") } | Select-Object -First 1
}
if (-not $PlatformTools) { throw "Google platform-tools not found: pass -PlatformTools <folder> or set ANDROID_HOME." }
foreach ($f in $adbFiles) {
    $p = Join-Path $PlatformTools $f
    if (-not (Test-Path $p)) { throw "Missing $p" }
    if ($f -ne "NOTICE.txt") {
        $sig = Get-AuthenticodeSignature $p
        if ($sig.Status -ne "Valid" -or $sig.SignerCertificate.Subject -notmatch "CN=Google LLC") {
            throw "$p is not validly signed by Google ($($sig.Status))"
        }
    }
}
$adbRev = (Select-String -Path (Join-Path $PlatformTools "source.properties") -Pattern '^Pkg.Revision=(.+)$').Matches[0].Groups[1].Value
Write-Host "    platform-tools $adbRev from $PlatformTools" -ForegroundColor Green
# The source package downloads the revision pinned in Start ZenLink.bat; every
# package must carry the same adb.
$pinRev = (Select-String -Path "Start ZenLink.bat" -Pattern '^set "ZL_ADB_REV=(.+)"$').Matches[0].Groups[1].Value
if ($adbRev -ne $pinRev) { throw "platform-tools $adbRev differs from ZL_ADB_REV=$pinRev in Start ZenLink.bat." }
foreach ($f in $adbFiles) {
    Write-Host ("    {0,-18} {1}" -f $f, (Get-FileHash (Join-Path $PlatformTools $f) -Algorithm SHA256).Hash)
}

$release = Join-Path $PSScriptRoot "release"
$build = Join-Path $PSScriptRoot "build"
$dist = Join-Path $PSScriptRoot "dist"
Remove-Item -Recurse -Force $release, $build, $dist -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $release | Out-Null

Write-Host "[1/4] Installing build tools..." -ForegroundColor Magenta
& $py -m pip install --disable-pip-version-check -q -r requirements.txt pyinstaller
if ($LASTEXITCODE) { throw "pip failed" }

Write-Host "[2/4] Building ZenLinkService.exe (PyInstaller) and ZenLink.exe (Rust)..." -ForegroundColor Magenta
$sep = ";"
$pyiArgs = @(
    "-m", "PyInstaller", "packaging\launch.py",
    "--name", "ZenLinkService", "--onedir", "--windowed", "--noconfirm", "--clean",
    # Never pack executables (AGENTS.md: no UPX, antivirus false positives).
    "--noupx",
    "--icon", "assets\zenlink.ico",
    "--contents-directory", ".",
    "--version-file", "packaging\version_info.txt",
    "--add-data", "assets${sep}assets",
    "--add-data", "tools${sep}tools",
    "--add-data", "drivers${sep}drivers",
    "--collect-all", "pymobiledevice3",
    # Package metadata (dist-info) of pymobiledevice3 and everything it
    # depends on: some read their own version at import (pyimg4 does), and
    # without it the developer image / tunnel import fails in the exe.
    "--recursive-copy-metadata", "pymobiledevice3",
    "--collect-all", "pmd_pytcp",
    "--collect-all", "pmd_net_addr",
    "--collect-all", "pytun_pmd3",  # wintun.dll, loaded when the tunnel module is imported
    # Only the x64 PortAudio (packaging\pyi-hooks\hook-sounddevice.py).
    "--additional-hooks-dir", "packaging\pyi-hooks",
    "--hidden-import", "_sounddevice_data",
    "--collect-all", "zeroconf",
    "--add-binary", "zenlink\airplay\playfair.dll${sep}zenlink\airplay",
    # Sound: minimal FFmpeg AAC decoder (native\zlaac). PyAV (full FFmpeg,
    # ~65 MB) is only pulled in by pymobiledevice3's unused VNC transcoder.
    "--add-binary", "zenlink\zlaac.dll${sep}zenlink",
    "--exclude-module", "av",
    "--hidden-import", "Crypto.Cipher.AES",
    "--collect-submodules", "zenlink",
    "--exclude-module", "tkinter",
    "--exclude-module", "matplotlib",
    "--exclude-module", "IPython",
    # The classic WebView2 window (pywebview, pythonnet, pystray, Pillow,
    # ui\) is not packaged: ZenLink.exe is the window and the backend runs
    # --headless (docs/SIZE_AUDIT.md, set A). The source zip keeps it
    # (ZENLINK_UI=classic with Start ZenLink.bat).
    "--exclude-module", "webview",
    "--exclude-module", "clr",
    "--exclude-module", "clr_loader",
    "--exclude-module", "pythonnet",
    "--exclude-module", "bottle",
    "--exclude-module", "proxy_tools",
    "--exclude-module", "pystray",
    "--exclude-module", "PIL",
    # requests falls back to charset_normalizer (set C2).
    "--exclude-module", "chardet",
    "--exclude-module", "pydoc",
    # Never imported by the headless backend (trace + lazy-import scan in
    # docs/SIZE_AUDIT.md, B1/B4/B5): pydantic comes with pymobiledevice3's
    # remote server, setuptools/PyInstaller/pefile are build tools, sqlite3
    # has no user. The frozen self-test checks every device-path module.
    "--exclude-module", "pydantic",
    "--exclude-module", "pydantic_core",
    "--exclude-module", "setuptools",
    "--exclude-module", "pkg_resources",
    "--exclude-module", "_distutils_hack",
    "--exclude-module", "PyInstaller",
    "--exclude-module", "pefile",
    "--exclude-module", "sqlite3",
    "--exclude-module", "_sqlite3",
    "--exclude-module", "lib2to3"
)
# Version resource (Author: Zenvy Zenshiro)
# Numeric part only ("2.5.0-beta.1" -> "2.5.0"): Windows version fields are n.n.n.n.
$VersionNum = ($Version -split "-")[0]
$parts = ($VersionNum.Split(".") + @("0", "0", "0", "0"))[0..3] -join ", "
@"
VSVersionInfo(
  ffi=FixedFileInfo(filevers=($parts), prodvers=($parts), mask=0x3f, flags=0x0, OS=0x40004,
                    fileType=0x1, subtype=0x0, date=(0, 0)),
  kids=[
    StringFileInfo([StringTable('040904B0', [
      StringStruct('CompanyName', 'Zenvy Zenshiro'),
      StringStruct('FileDescription', 'ZenLink service'),
      StringStruct('FileVersion', '$Version'),
      StringStruct('InternalName', 'ZenLinkService'),
      StringStruct('LegalCopyright', 'Copyright (C) 2026 Zenvy Zenshiro. GPL-3.0-or-later.'),
      StringStruct('OriginalFilename', 'ZenLinkService.exe'),
      StringStruct('ProductName', 'ZenLink'),
      StringStruct('ProductVersion', '$Version')])]),
    VarFileInfo([VarStruct('Translation', [1033, 1200])])
  ]
)
"@ | Set-Content -Encoding UTF8 packaging\version_info.txt
& $py @pyiArgs
if ($LASTEXITCODE) { throw "PyInstaller failed" }

$app = Join-Path $dist "ZenLinkService"

# Import every device-path module inside the built exe (packaging\launch.py
# --selftest). A missing module or package metadata fails the build here.
function Test-FrozenExe([string]$exe) {
    $report = Join-Path $build ("selftest-" + [IO.Path]::GetFileNameWithoutExtension($exe) + "-" + (Get-Random) + ".json")
    $p = Start-Process -FilePath $exe -ArgumentList "--selftest", "`"$report`"" -PassThru -Wait
    if (-not (Test-Path $report)) { throw "Self-test of $exe wrote no report (exit $($p.ExitCode))" }
    $r = Get-Content $report -Raw | ConvertFrom-Json
    if (-not $r.ok) {
        $bad = $r.modules.PSObject.Properties | Where-Object { $_.Value -ne "ok" } | ForEach-Object { "$($_.Name): $($_.Value)" }
        throw "Self-test of $exe failed:`n$($bad -join "`n")"
    }
    Write-Host "    self-test OK: $exe ($(@($r.modules.PSObject.Properties).Count) modules)" -ForegroundColor Green
}
Test-FrozenExe (Join-Path $app "ZenLinkService.exe")
# The full FFmpeg from PyAV must not come back (Phase 1 size win).
if (Get-ChildItem $app -Recurse -Filter "avcodec-*.dll") { throw "PyAV/FFmpeg DLLs found in $app" }
# Size audit: removed parts must stay out, the one PortAudio must be in.
foreach ($gone in "pydantic_core", "sqlite3.dll", "_sqlite3.pyd", "libportaudio32bit.dll", "libportaudioarm64.dll", "libportaudio64bit-asio.dll", "webview", "pythonnet", "PIL", "chardet", "Python.Runtime.dll") {
    if (Get-ChildItem $app -Recurse -Filter $gone) { throw "$gone found in $app (see docs/SIZE_AUDIT.md)" }
}
if (-not (Test-Path (Join-Path $app "_sounddevice_data\portaudio-binaries\libportaudio64bit.dll"))) { throw "PortAudio x64 missing in $app" }
foreach ($f in "LICENSE", "README.md", "Start ZenLink.bat", "run.bat", "Create-Desktop-Shortcut.bat", "Uninstall-ZenLink.bat") {
    Copy-Item $f $app
}
# Android: adb next to ZenLink.exe (found by find_adb); the scrcpy server is
# built into ZenLink.exe, its licence ships beside it.
$adbDest = Join-Path $app "third_party\platform-tools"
New-Item -ItemType Directory -Force $adbDest | Out-Null
foreach ($f in $adbFiles) { Copy-Item (Join-Path $PlatformTools $f) $adbDest }
$scrcpyDest = Join-Path $app "third_party\scrcpy"
New-Item -ItemType Directory -Force $scrcpyDest | Out-Null
Copy-Item "third_party\scrcpy\LICENSE" $scrcpyDest
# Credits and licences of everything bundled (scrcpy, adb, FFmpeg, ...).
Copy-Item "docs\THIRD_PARTY.md" $app
& cargo build --release --manifest-path rust\Cargo.toml -p zenlink-app
if ($LASTEXITCODE) { throw "cargo failed" }
# CARGO_TARGET_DIR lets the release build while a dev ZenLink.exe from
# rust\target is open (Windows locks a running exe).
$rustTarget = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $PSScriptRoot "rust\target" }
$rustExe = Join-Path $rustTarget "release\ZenLink.exe"
Copy-Item $rustExe $app

Write-Host "[3/4] Building the portable single-file ZenLink.exe..." -ForegroundColor Magenta
# Same app as one .exe: it unpacks itself to a temporary folder on start and
# keeps its settings in %LOCALAPPDATA%\ZenLink-Beta.
$oneArgs = @()
for ($i = 0; $i -lt $pyiArgs.Count; $i++) {
    if ($pyiArgs[$i] -eq "--onedir") { $oneArgs += "--onefile"; continue }
    if ($pyiArgs[$i] -eq "--contents-directory") { $i++; continue }
    $oneArgs += $pyiArgs[$i]
}
$oneArgs += @("--distpath", (Join-Path $build "onefile-dist"), "--workpath", (Join-Path $build "onefile-work"))
& $py @oneArgs
if ($LASTEXITCODE) { throw "PyInstaller (one file) failed" }
$service = Join-Path $build "onefile-dist\ZenLinkService.exe"
Test-FrozenExe $service
# The Rust ZenLink.exe with that backend packed inside (unpacked to
# %LOCALAPPDATA%\ZenLink-Beta\service on start).
$env:ZENLINK_BACKEND_EXE = $service
$env:ZENLINK_ADB_DIR = (Resolve-Path $PlatformTools).Path
& cargo build --release --manifest-path rust\Cargo.toml -p zenlink-app --features embedded-backend --target-dir (Join-Path $build "rust-portable")
Remove-Item Env:ZENLINK_BACKEND_EXE, Env:ZENLINK_ADB_DIR
if ($LASTEXITCODE) { throw "cargo (portable) failed" }
Copy-Item (Join-Path $build "rust-portable\release\ZenLink.exe") (Join-Path $release "ZenLink-v$Version-Portable.exe")

Write-Host "[4/4] Building the installer and the source zip..." -ForegroundColor Magenta
$iscc = @(
    "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe",
    "$env:ProgramFiles\Inno Setup 6\ISCC.exe",
    "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe"
) | Where-Object { Test-Path $_ } | Select-Object -First 1
if ($iscc) {
    # Top-level entries of the install folder: the next Setup removes exactly
    # these before copying (clean update, packaging/ZenLink.iss).
    $skip = @("Start ZenLink.bat", "run.bat", "Create-Desktop-Shortcut.bat", "Uninstall-ZenLink.bat", "portable.txt", "install-manifest.txt")
    $names = @(Get-ChildItem -Force $app | Where-Object { $skip -notcontains $_.Name } | ForEach-Object { $_.Name }) + "install-manifest.txt"
    [IO.File]::WriteAllLines((Join-Path $app "install-manifest.txt"), [string[]]$names)
    & $iscc "/DAppVersion=$Version" "/DAppVersionNum=$VersionNum" "/DSourceDir=$app" "/O$release" "packaging\ZenLink.iss"
    if ($LASTEXITCODE) { throw "Inno Setup failed" }
} else {
    Write-Warning "Inno Setup 6 not found: skipping Setup.exe (winget install JRSoftware.InnoSetup)."
}
$srcZip = Join-Path $release "ZenLink-v$Version-Portable-Source.zip"
git archive --format=zip --prefix="ZenLink-v$Version/" -o $srcZip HEAD
# Ready-built ZenLink.exe next to Start ZenLink.bat (no Rust toolchain needed).
# adb is not packed: Start ZenLink.bat downloads Google's official copy
# (tools\get-adb.ps1). The website, agent notes and the scrcpy server
# (built into ZenLink.exe) are export-ignore in .gitattributes.
Add-Type -AssemblyName System.IO.Compression, System.IO.Compression.FileSystem
$zip = [IO.Compression.ZipFile]::Open($srcZip, "Update")
try {
    [void][IO.Compression.ZipFileExtensions]::CreateEntryFromFile($zip, $rustExe, "ZenLink-v$Version/ZenLink.exe")
} finally { $zip.Dispose() }

Get-ChildItem $release | ForEach-Object {
    "{0,-45} {1,8:N1} MB" -f $_.Name, ($_.Length / 1MB)
}
Write-Host "Done: $release" -ForegroundColor Green
