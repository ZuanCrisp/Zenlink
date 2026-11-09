<#
.SYNOPSIS
  Builds the ZenLink release files into out\release\ (work folders: out\build, out\dist):

    ZenLink-v<ver>-Portable.exe           one ZenLink.exe, nothing to install (no Python)
    ZenLink-v<ver>-Setup.exe              installer (Inno Setup 6, if installed)
    ZenLink-v<ver>-Manual.zip             manual install: ZenLink.exe + source + Start ZenLink.bat (sets up Python and adb on first run)

  ZenLink.exe is the Rust window (apps\desktop, ZenshiroDev\crates\). The Python backend
  (apps\desktop\backend) is built as
  ZenLinkService.exe; ZenLink.exe starts it headless. Needs cargo (rustup).

.EXAMPLE
  powershell -ExecutionPolicy Bypass -File .\build_portable_release.ps1
#>
param(
    [string]$Version = "",
    # Output folder. Default: $env:ZENLINK_OUT_DIR, else out\ in the repository root.
    [string]$Out = ""
)
$ErrorActionPreference = "Stop"
# This script lives in the repository root; all paths below are relative to it.
$root = $PSScriptRoot
Set-Location $root

if (-not $Version) {
    $Version = (Select-String -Path apps\desktop\backend\pyproject.toml -Pattern '^version\s*=\s*"([^"]+)"').Matches[0].Groups[1].Value
}
$py = Join-Path $root ".venv\Scripts\python.exe"
if (-not (Test-Path $py)) { throw "Run Start ZenLink.bat once first (it creates .venv)." }

# Android: no package carries adb. ZenLink downloads Google's official
# platform-tools zip on request (apps\desktop\src\android.rs ADB_ZIP_*), and
# Start ZenLink.bat does the same for source mode (ZL_ADB_REV/ZL_ADB_SHA256).
# Both pins must name the same zip.
$pinRev = (Select-String -Path "Start ZenLink.bat" -Pattern '^set "ZL_ADB_REV=(.+)"$').Matches[0].Groups[1].Value
$pinSha = (Select-String -Path "Start ZenLink.bat" -Pattern '^set "ZL_ADB_SHA256=(.+)"$').Matches[0].Groups[1].Value
$rs = Get-Content -Raw apps\desktop\src\android.rs
if ($rs -notmatch [regex]::Escape("platform-tools_r$pinRev-win.zip") -or $rs -notmatch [regex]::Escape($pinSha)) {
    throw "adb pin in apps\desktop\src\android.rs differs from Start ZenLink.bat (r$pinRev)."
}

# Every build output goes to out\ (ignored by git), or to -Out / ZENLINK_OUT_DIR.
$out = if ($Out) { $Out } elseif ($env:ZENLINK_OUT_DIR) { $env:ZENLINK_OUT_DIR } else { Join-Path $root "out" }
$out = [IO.Path]::GetFullPath($out)
$release = Join-Path $out "release"
$build = Join-Path $out "build"
$dist = Join-Path $out "dist"
Remove-Item -Recurse -Force $release, $build, $dist -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $release, $build | Out-Null

Write-Host "[1/4] Installing build tools..." -ForegroundColor Magenta
& $py -m pip install --disable-pip-version-check -q -r apps\desktop\backend\requirements.txt pyinstaller
if ($LASTEXITCODE) { throw "pip failed" }

Write-Host "[2/4] Building ZenLinkService.exe (PyInstaller) and ZenLink.exe (Rust)..." -ForegroundColor Magenta
$sep = ";"
$pyiArgs = @(
    "-m", "PyInstaller", "ZenshiroDev\tooling\windows\packaging\launch.py",
    # The backend package lives in apps\desktop\backend (monorepo).
    "--paths", "apps\desktop\backend",
    "--distpath", $dist, "--workpath", (Join-Path $build "pyinstaller"), "--specpath", $build,
    "--name", "ZenLinkService", "--onedir", "--windowed", "--noconfirm", "--clean",
    # Never pack executables (no UPX: antivirus false positives).
    "--noupx",
    "--icon", (Join-Path $root "ZenshiroDev\resources\assets\zenlink.ico"),
    "--contents-directory", ".",
    "--version-file", (Join-Path $build "version_info.txt"),
    # Bundle layout (unchanged): assets\, tools\, drivers\ next to the exe.
    "--add-data", "$root\ZenshiroDev\resources\assets${sep}assets",
    "--add-data", "$root\ZenshiroDev\tooling\windows\tools${sep}tools",
    "--add-data", "$root\ZenshiroDev\resources\windows\drivers${sep}drivers",
    # Not --collect-all: that also copies every .py source next to the
    # bytecode already in the PYZ (529 files, 6.3 MB).
    "--collect-submodules", "pymobiledevice3", "--collect-data", "pymobiledevice3", "--collect-binaries", "pymobiledevice3",
    # Package metadata (dist-info) of pymobiledevice3 and everything it
    # depends on: some read their own version at import (pyimg4 does), and
    # without it the developer image / tunnel import fails in the exe.
    "--recursive-copy-metadata", "pymobiledevice3",
    "--collect-submodules", "pmd_pytcp", "--collect-data", "pmd_pytcp", "--collect-binaries", "pmd_pytcp",
    "--collect-submodules", "pmd_net_addr", "--collect-data", "pmd_net_addr", "--collect-binaries", "pmd_net_addr",
    "--collect-submodules", "pytun_pmd3", "--collect-data", "pytun_pmd3", "--collect-binaries", "pytun_pmd3",  # wintun.dll, loaded when the tunnel module is imported
    # Only the x64 PortAudio (ZenshiroDev\tooling\windows\packaging\pyi-hooks\hook-sounddevice.py).
    "--additional-hooks-dir", "$root\ZenshiroDev\tooling\windows\packaging\pyi-hooks",
    "--hidden-import", "_sounddevice_data",
    "--collect-submodules", "zeroconf", "--collect-data", "zeroconf", "--collect-binaries", "zeroconf",
    "--add-binary", "$root\apps\desktop\backend\zenlink\airplay\playfair.dll${sep}zenlink\airplay",
    # Sound: minimal FFmpeg AAC decoder (ZenshiroDev\resources\windows\native\zlaac). PyAV (full FFmpeg,
    # ~65 MB) is only pulled in by pymobiledevice3's unused VNC transcoder.
    "--add-binary", "$root\apps\desktop\backend\zenlink\zlaac.dll${sep}zenlink",
    "--exclude-module", "av",
    "--hidden-import", "Crypto.Cipher.AES",
    "--collect-submodules", "zenlink",
    "--exclude-module", "tkinter",
    "--exclude-module", "matplotlib",
    "--exclude-module", "IPython",
    # The classic WebView2 window (pywebview, pythonnet, pystray, Pillow,
    # ui\) is not packaged: ZenLink.exe is the window and the backend runs
    # --headless (ZenshiroDev/docs/SIZE_AUDIT.md, set A). The source zip keeps it
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
    # ZenshiroDev/docs/SIZE_AUDIT.md, B1/B4/B5): pydantic comes with pymobiledevice3's
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
    "--exclude-module", "lib2to3",
    # pyimg4 imports these but the developer image mount never calls them;
    # zenlink\_lzss.py stands in. Their unsigned .pyd files were blocked by
    # Smart App Control (GitHub issue #2).
    "--exclude-module", "lzss",
    "--exclude-module", "lzfse"
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
"@ | Set-Content -Encoding UTF8 (Join-Path $build "version_info.txt")
& $py @pyiArgs
if ($LASTEXITCODE) { throw "PyInstaller failed" }

$app = Join-Path $dist "ZenLinkService"

# Code signing (GitHub issue #2: Smart App Control blocks unsigned native
# modules without reputation). Off until a trusted certificate exists. Set
# ZENLINK_SIGN_ARGS to the signtool options after "sign", e.g. for Azure
# Trusted Signing: /fd SHA256 /tr http://timestamp.acs.microsoft.com /td SHA256
#   /dlib <path>\Azure.CodeSigning.Dlib.dll /dmdf <path>\metadata.json
# Files that already carry a valid signature (python313.dll, wintun.dll) are kept.
function Invoke-Sign([string[]]$files) {
    if (-not $env:ZENLINK_SIGN_ARGS) { return }
    $todo = @($files | Where-Object { (Get-AuthenticodeSignature $_).Status -ne "Valid" })
    if (-not $todo) { return }
    $signtool = (Get-Command signtool.exe -ErrorAction SilentlyContinue).Source
    if (-not $signtool) { throw "ZENLINK_SIGN_ARGS is set but signtool.exe is not on PATH" }
    # Batches keep the command line short.
    for ($i = 0; $i -lt $todo.Count; $i += 50) {
        $batch = $todo[$i..([Math]::Min($i + 49, $todo.Count - 1))]
        & $signtool sign ($env:ZENLINK_SIGN_ARGS -split " ") $batch
        if ($LASTEXITCODE) { throw "signtool failed" }
    }
    Write-Host "    signed $($todo.Count) file(s)" -ForegroundColor Green
}
if (-not $env:ZENLINK_SIGN_ARGS) { Write-Warning "ZENLINK_SIGN_ARGS not set: release files are NOT code-signed (Smart App Control may block them)." }
Invoke-Sign @(Get-ChildItem $app -Recurse -Include *.exe, *.dll, *.pyd | ForEach-Object FullName)

# Import every device-path module inside the built exe (ZenshiroDev\tooling\windows\packaging\launch.py
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
foreach ($gone in "lzss*.pyd", "lzfse*.pyd", "pydantic_core","sqlite3.dll", "_sqlite3.pyd", "libportaudio32bit.dll", "libportaudioarm64.dll", "libportaudio64bit-asio.dll", "webview", "pythonnet", "PIL", "chardet", "Python.Runtime.dll") {
    if (Get-ChildItem $app -Recurse -Filter $gone) { throw "$gone found in $app (see ZenshiroDev/docs/SIZE_AUDIT.md)" }
}
if (-not (Test-Path (Join-Path $app "_sounddevice_data\portaudio-binaries\libportaudio64bit.dll"))) { throw "PortAudio x64 missing in $app" }
foreach ($f in "LICENSE", "README.md", "Start ZenLink.bat") {
    Copy-Item $f $app
}
# Uninstall-ZenLink.bat is in the root; run.bat and Create-Desktop-Shortcut.bat
# live in ZenshiroDev\tooling\windows\scripts. A packaged copy keeps all three
# next to ZenLink.exe (each script finds its root either way).
Copy-Item "Uninstall-ZenLink.bat" $app
foreach ($f in "run.bat", "Create-Desktop-Shortcut.bat") {
    Copy-Item (Join-Path "ZenshiroDev\tooling\windows\scripts" $f) $app
}
# Android: the scrcpy server is built into ZenLink.exe, its licence ships
# beside it. adb is downloaded by ZenLink on request (not packaged).
$scrcpyDest = Join-Path $app "third_party\scrcpy"
New-Item -ItemType Directory -Force $scrcpyDest | Out-Null
Copy-Item "ZenshiroDev\resources\third_party\scrcpy\LICENSE" $scrcpyDest
# Credits and licences of everything bundled (scrcpy, adb, FFmpeg, ...).
Copy-Item "ZenshiroDev\docs\THIRD_PARTY.md" $app
& cargo build --release --manifest-path Cargo.toml -p zenlink-app
if ($LASTEXITCODE) { throw "cargo failed" }
# CARGO_TARGET_DIR lets the release build while a dev ZenLink.exe from
# target\ is open (Windows locks a running exe).
$rustTarget = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $root "target" }
$rustExe = Join-Path $rustTarget "release\ZenLink.exe"
Copy-Item $rustExe $app
# The Rust Apple engine decodes the device's sound with the same zlaac.dll (src/aac.rs).
Copy-Item "$root\apps\desktop\backend\zenlink\zlaac.dll" $app
Invoke-Sign @(Join-Path $app "ZenLink.exe")

Write-Host "[3/4] Building the portable single-file ZenLink.exe..." -ForegroundColor Magenta
# Same app as one .exe: it unpacks itself to a temporary folder on start and
# keeps its settings in %LOCALAPPDATA%\ZenLink-Next.
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
# shortcut: only the outer one-file exe is signed; the .pyd/.dll it unpacks to
# %TEMP%\_MEI* stay unsigned. Ship the signed folder build inside the Portable
# exe instead once a certificate exists (see ZenshiroDev/docs/SMART_APP_CONTROL.md).
Invoke-Sign @($service)
# The Rust ZenLink.exe with that backend packed inside (unpacked to
# %LOCALAPPDATA%\ZenLink-Next\service on start).
$env:ZENLINK_BACKEND_EXE = $service
& cargo build --release --manifest-path Cargo.toml -p zenlink-app --features embedded-backend --target-dir (Join-Path $build "rust-portable")
Remove-Item Env:ZENLINK_BACKEND_EXE
if ($LASTEXITCODE) { throw "cargo (portable) failed" }
Copy-Item (Join-Path $build "rust-portable\release\ZenLink.exe") (Join-Path $release "ZenLink-v$Version-Portable.exe")
Invoke-Sign @(Join-Path $release "ZenLink-v$Version-Portable.exe")

Write-Host "[4/4] Building the installer and the source zip..." -ForegroundColor Magenta
$iscc = @(
    "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe",
    "$env:ProgramFiles\Inno Setup 6\ISCC.exe",
    "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe"
) | Where-Object { Test-Path $_ } | Select-Object -First 1
if ($iscc) {
    # Top-level entries of the install folder: the next Setup removes exactly
    # these before copying (clean update, ZenshiroDev/tooling/windows/packaging/ZenLink.iss).
    $skip = @("Start ZenLink.bat", "run.bat", "Create-Desktop-Shortcut.bat", "Uninstall-ZenLink.bat", "portable.txt", "install-manifest.txt")
    $names = @(Get-ChildItem -Force $app | Where-Object { $skip -notcontains $_.Name } | ForEach-Object { $_.Name }) + "install-manifest.txt"
    [IO.File]::WriteAllLines((Join-Path $app "install-manifest.txt"), [string[]]$names)
    & $iscc "/DAppVersion=$Version" "/DAppVersionNum=$VersionNum" "/DSourceDir=$app" "/O$release" "ZenshiroDev\tooling\windows\packaging\ZenLink.iss"
    if ($LASTEXITCODE) { throw "Inno Setup failed" }
    Invoke-Sign @(Get-ChildItem $release -Filter "*-Setup.exe" | ForEach-Object FullName)
} else {
    Write-Warning "Inno Setup 6 not found: skipping Setup.exe (winget install JRSoftware.InnoSetup)."
}
$srcZip = Join-Path $release "ZenLink-v$Version-Manual.zip"
git archive --format=zip --prefix="ZenLink-v$Version/" -o $srcZip HEAD
# Ready-built ZenLink.exe next to Start ZenLink.bat (no Rust toolchain needed).
# adb is not packed: Start ZenLink.bat downloads Google's official copy
# (ZenshiroDev\tooling\windows\tools\get-adb.ps1). The website, agent notes and the scrcpy server
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
