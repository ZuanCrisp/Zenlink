<#
.SYNOPSIS
  Builds the ZenLink release files into .\release\:

    ZenLink-v<ver>-Portable.exe           one ZenLink.exe, nothing to install (no Python)
    ZenLink-v<ver>-Setup.exe              installer (Inno Setup 6, if installed)
    ZenLink-v<ver>-Portable-Source.zip    source + Start ZenLink.bat (sets up Python on first run)

.EXAMPLE
  powershell -ExecutionPolicy Bypass -File .\build_portable_release.ps1
#>
param([string]$Version = "")
$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot

if (-not $Version) {
    $Version = (Select-String -Path pyproject.toml -Pattern '^version\s*=\s*"([^"]+)"').Matches[0].Groups[1].Value
}
$py = Join-Path $PSScriptRoot ".venv\Scripts\python.exe"
if (-not (Test-Path $py)) { throw "Run Start ZenLink.bat once first (it creates .venv)." }

$release = Join-Path $PSScriptRoot "release"
$build = Join-Path $PSScriptRoot "build"
$dist = Join-Path $PSScriptRoot "dist"
Remove-Item -Recurse -Force $release, $build, $dist -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $release | Out-Null

Write-Host "[1/4] Installing build tools..." -ForegroundColor Magenta
& $py -m pip install --disable-pip-version-check -q -r requirements.txt pyinstaller
if ($LASTEXITCODE) { throw "pip failed" }

Write-Host "[2/4] Building ZenLink.exe (PyInstaller)..." -ForegroundColor Magenta
$sep = ";"
$pyiArgs = @(
    "-m", "PyInstaller", "packaging\launch.py",
    "--name", "ZenLink", "--onedir", "--windowed", "--noconfirm", "--clean",
    "--icon", "assets\zenlink.ico",
    "--contents-directory", ".",
    "--version-file", "packaging\version_info.txt",
    "--add-data", "ui${sep}ui",
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
    "--collect-all", "webview",
    "--collect-all", "_sounddevice_data",
    "--collect-all", "zeroconf",
    "--add-binary", "zenlink\airplay\playfair.dll${sep}zenlink\airplay",
    "--hidden-import", "Crypto.Cipher.AES",
    "--collect-submodules", "zenlink",
    "--hidden-import", "clr",
    "--exclude-module", "tkinter",
    "--exclude-module", "matplotlib",
    "--exclude-module", "IPython",
    # Smaller build: Pillow is only used for the tray icon (PNG/ICO).
    "--exclude-module", "PIL._avif",
    "--exclude-module", "PIL.AvifImagePlugin",
    "--exclude-module", "PIL._webp",
    "--exclude-module", "PIL.WebPImagePlugin",
    "--exclude-module", "PIL._imagingft",
    "--exclude-module", "PIL.ImageFont",
    "--exclude-module", "PIL.ImageQt",
    "--exclude-module", "PIL.ImageTk",
    "--exclude-module", "pydoc",
    "--exclude-module", "lib2to3"
)
# Version resource (Author: Zenvy Zenshiro)
$parts = ($Version.Split(".") + @("0", "0", "0", "0"))[0..3] -join ", "
@"
VSVersionInfo(
  ffi=FixedFileInfo(filevers=($parts), prodvers=($parts), mask=0x3f, flags=0x0, OS=0x40004,
                    fileType=0x1, subtype=0x0, date=(0, 0)),
  kids=[
    StringFileInfo([StringTable('040904B0', [
      StringStruct('CompanyName', 'Zenvy Zenshiro'),
      StringStruct('FileDescription', 'ZenLink - iPhone and iPad mirroring for Windows'),
      StringStruct('FileVersion', '$Version'),
      StringStruct('InternalName', 'ZenLink'),
      StringStruct('LegalCopyright', 'Copyright (C) 2026 Zenvy Zenshiro. GPL-3.0-or-later.'),
      StringStruct('OriginalFilename', 'ZenLink.exe'),
      StringStruct('ProductName', 'ZenLink'),
      StringStruct('ProductVersion', '$Version')])]),
    VarFileInfo([VarStruct('Translation', [1033, 1200])])
  ]
)
"@ | Set-Content -Encoding UTF8 packaging\version_info.txt
& $py @pyiArgs
if ($LASTEXITCODE) { throw "PyInstaller failed" }

$app = Join-Path $dist "ZenLink"

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
Test-FrozenExe (Join-Path $app "ZenLink.exe")
foreach ($f in "LICENSE", "README.md", "Start ZenLink.bat", "run.bat", "Create-Desktop-Shortcut.bat", "Uninstall-ZenLink.bat") {
    Copy-Item $f $app
}

Write-Host "[3/4] Building the portable single-file ZenLink.exe..." -ForegroundColor Magenta
# Same app as one .exe: it unpacks itself to a temporary folder on start and
# keeps its settings in %LOCALAPPDATA%\ZenLink.
$oneArgs = @()
for ($i = 0; $i -lt $pyiArgs.Count; $i++) {
    if ($pyiArgs[$i] -eq "--onedir") { $oneArgs += "--onefile"; continue }
    if ($pyiArgs[$i] -eq "--contents-directory") { $i++; continue }
    $oneArgs += $pyiArgs[$i]
}
$oneArgs += @("--distpath", (Join-Path $build "onefile-dist"), "--workpath", (Join-Path $build "onefile-work"))
& $py @oneArgs
if ($LASTEXITCODE) { throw "PyInstaller (one file) failed" }
Test-FrozenExe (Join-Path $build "onefile-dist\ZenLink.exe")
Copy-Item (Join-Path $build "onefile-dist\ZenLink.exe") (Join-Path $release "ZenLink-v$Version-Portable.exe")

Write-Host "[4/4] Building the installer and the source zip..." -ForegroundColor Magenta
$iscc = @(
    "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe",
    "$env:ProgramFiles\Inno Setup 6\ISCC.exe",
    "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe"
) | Where-Object { Test-Path $_ } | Select-Object -First 1
if ($iscc) {
    & $iscc "/DAppVersion=$Version" "/DSourceDir=$app" "/O$release" "packaging\ZenLink.iss"
    if ($LASTEXITCODE) { throw "Inno Setup failed" }
} else {
    Write-Warning "Inno Setup 6 not found: skipping Setup.exe (winget install JRSoftware.InnoSetup)."
}
git archive --format=zip --prefix="ZenLink-v$Version/" -o (Join-Path $release "ZenLink-v$Version-Portable-Source.zip") HEAD

Get-ChildItem $release | ForEach-Object {
    "{0,-45} {1,8:N1} MB" -f $_.Name, ($_.Length / 1MB)
}
Write-Host "Done: $release" -ForegroundColor Green
