<#
.SYNOPSIS
  Builds the ZenLink release files into .\release\:

    ZenLink-v<ver>-Win64-Standalone.zip   ZenLink.exe + everything it needs (no Python)
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
    "--exclude-module", "IPython"
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
foreach ($f in "LICENSE", "README.md", "Start ZenLink.bat", "run.bat", "Create-Desktop-Shortcut.bat", "Uninstall-ZenLink.bat") {
    Copy-Item $f $app
}

Write-Host "[3/4] Packing the standalone zip..." -ForegroundColor Magenta
$portable = Join-Path $build "portable\ZenLink"
New-Item -ItemType Directory -Force (Split-Path $portable) | Out-Null
Copy-Item -Recurse $app $portable
"ZenLink keeps its settings in this folder (portable mode)." | Set-Content (Join-Path $portable "portable.txt")
Compress-Archive -Path $portable -DestinationPath (Join-Path $release "ZenLink-v$Version-Win64-Standalone.zip") -CompressionLevel Optimal

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
