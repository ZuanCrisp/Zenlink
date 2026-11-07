# Downloads Google's official Android platform-tools (adb) for the
# Manual.zip package, so the source zip does not have to carry them.
#
# Only these files are kept, unmodified, in third_party\platform-tools:
# adb.exe, AdbWinApi.dll, AdbWinUsbApi.dll, NOTICE.txt.
# The zip is pinned to one revision and checked against its SHA-256 before
# anything is extracted. The installer and Portable.exe still bundle adb.
#
# Exit codes: 0 = adb ready (or already there), 1 = download/check failed.
# The pinned revision and SHA-256 live in Start ZenLink.bat (ZL_ADB_REV,
# ZL_ADB_SHA256), which passes them here; the defaults below must match.
param(
    [string]$Dest = (Join-Path (Split-Path -Parent $PSScriptRoot) "third_party\platform-tools"),
    [string]$Revision = "37.0.1",
    [string]$Sha256 = "45f4d63113e895ebde0c90f194099a4676b6ac653bd28d54314a9e022bbc1a99"
)
$ErrorActionPreference = "Stop"
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

# When updating: download platform-tools_r<rev>-win.zip, check that adb.exe
# is signed by Google, and put the new revision and SHA-256 in Start ZenLink.bat
# and here. build_portable_release.ps1 stops if the bundled adb differs.
$Url = "https://dl.google.com/android/repository/platform-tools_r$Revision-win.zip"
$Sha256 = $Sha256.ToLowerInvariant()
$Files = "adb.exe", "AdbWinApi.dll", "AdbWinUsbApi.dll", "NOTICE.txt"

if (($Files | Where-Object { -not (Test-Path (Join-Path $Dest $_)) }).Count -eq 0) {
    Write-Host "  - adb: OK"
    exit 0
}

$tmp = Join-Path ([IO.Path]::GetTempPath()) ("zenlink-adb-" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force $tmp | Out-Null
try {
    $zip = Join-Path $tmp "platform-tools.zip"
    Write-Host "  - Downloading adb (Google platform-tools r$Revision, about 8 MB)..."
    Write-Host "    $Url"
    $wc = New-Object Net.WebClient
    $wc.DownloadFile($Url, $zip)
    $got = (Get-FileHash -Algorithm SHA256 $zip).Hash.ToLowerInvariant()
    if ($got -ne $Sha256) {
        Write-Host "    SHA-256 does not match ($got). Nothing was installed." -ForegroundColor Red
        exit 1
    }
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $z = [IO.Compression.ZipFile]::OpenRead($zip)
    try {
        New-Item -ItemType Directory -Force $Dest | Out-Null
        foreach ($f in $Files) {
            $e = $z.GetEntry("platform-tools/$f")
            if (-not $e) { throw "platform-tools/$f is missing from the zip" }
            [IO.Compression.ZipFileExtensions]::ExtractToFile($e, (Join-Path $Dest $f), $true)
        }
    } finally { $z.Dispose() }
    $sig = Get-AuthenticodeSignature (Join-Path $Dest "adb.exe")
    if ($sig.Status -ne "Valid" -or $sig.SignerCertificate.Subject -notmatch "Google") {
        Write-Host "    adb.exe is not signed by Google ($($sig.Status)). Removing it." -ForegroundColor Red
        foreach ($f in $Files) { Remove-Item -Force -ErrorAction SilentlyContinue (Join-Path $Dest $f) }
        exit 1
    }
    Write-Host "  - adb: installed in third_party\platform-tools"
    exit 0
} catch {
    Write-Host "    Could not get adb: $($_.Exception.Message)" -ForegroundColor Red
    exit 1
} finally {
    Remove-Item -Recurse -Force -ErrorAction SilentlyContinue $tmp
}
