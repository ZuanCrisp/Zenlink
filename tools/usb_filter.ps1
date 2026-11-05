<#
.SYNOPSIS
  ZenLink driver switch for ONE Apple device (run elevated, after consent).

.DESCRIPTION
  ZenLink starts this script through UAC only after the user confirmed the
  change in the app. It never runs on its own. It works on the composite
  parent node USB\VID_05AC&PID_12xx\<serial>.

  Actions:
    bind          USB Direct mode, the way TikTok LIVE Studio does it:
                  remove any libusb0 filter, then install the libusb-win32
                  driver package for this device model as the parent's device
                  driver. Apple Mobile Device Service no longer sees the device
                  until "restore" runs.
    restore       Apple mode: remove libusb0 filters and, when the parent is
                  bound to libusb0, bind Apple's own composite driver back.
    removefilter  Remove a libusb0 UPPER FILTER stacked on Apple's driver.
                  That stack is unsafe (it crashed Windows); ZenLink never
                  creates it.

  Every action:
    1. writes a JSON backup of the parent and child driver state to
       %ProgramData%\ZenLink\drivers\backups\
    2. changes only this device
    3. restarts the device and verifies the expected driver is running
    4. on failure, puts the previous state back (filters, and Apple's driver
       when the bind failed)
  The result is written as JSON to -ResultPath.

  The libusb-win32 package for this device model is taken from the driver
  store (TikTok LIVE Studio's iOS driver, Zadig or an earlier ZenLink
  install); when there is none, one is built from the bundled, validly
  signed libusb-win32 files and signed with a one-time certificate whose
  private key is deleted right away. Nothing is downloaded.

  Driver packages are found by reading the INF files, never by parsing
  pnputil's text, which Windows translates.
#>
param(
    [Parameter(Mandatory)][ValidateSet("bind", "restore", "removefilter")][string]$Action,
    [Parameter(Mandatory)][string]$InstanceId,
    [Parameter(Mandatory)][string]$ResultPath
)
$ErrorActionPreference = "Stop"

# A click in the console turns on QuickEdit selection, which pauses the
# script until Enter is pressed. Turn QuickEdit off for this window.
try {
    Add-Type -Namespace ZenLink -Name Console -MemberDefinition @"
[DllImport("kernel32.dll")] public static extern System.IntPtr GetStdHandle(int h);
[DllImport("kernel32.dll")] public static extern bool GetConsoleMode(System.IntPtr h, out uint m);
[DllImport("kernel32.dll")] public static extern bool SetConsoleMode(System.IntPtr h, uint m);
"@
    $hIn = [ZenLink.Console]::GetStdHandle(-10)
    $mode = 0
    if ([ZenLink.Console]::GetConsoleMode($hIn, [ref]$mode)) {
        [void][ZenLink.Console]::SetConsoleMode($hIn, (($mode -band (-bnot 0x40)) -bor 0x80))
    }
} catch {}
$Host.UI.RawUI.WindowTitle = "ZenLink - USB driver ($Action)"
$script:stepCount = 0
Write-Host ""
Write-Host "  ZenLink - changing the USB driver ($Action)" -ForegroundColor Magenta
Write-Host "  Keep the cable plugged in until this window closes." -ForegroundColor Gray
Write-Host ""
$result = [ordered]@{ action = $Action; instance = $InstanceId; ok = $false; message = ""; backup = $null; rolledBack = $false; reboot = $false; steps = @() }
function Step([string]$text) {
    $script:result.steps += $text
    $script:stepCount++
    $pct = [Math]::Min(95, 8 + $script:stepCount * 14)
    Write-Progress -Activity "ZenLink USB driver ($Action)" -Status $text -PercentComplete $pct
    Write-Host ("  [{0,3}%] {1}" -f $pct, $text) -ForegroundColor Cyan
}
function Finish {
    $dir = Split-Path -Parent $ResultPath
    if ($dir -and -not (Test-Path $dir)) { New-Item -ItemType Directory -Force -Path $dir | Out-Null }
    ($script:result | ConvertTo-Json -Depth 6) | Set-Content -Path $ResultPath -Encoding UTF8
}

Add-Type -TypeDefinition @"
using System; using System.Runtime.InteropServices;
public static class PlNewDev {
  [DllImport("newdev.dll", CharSet = CharSet.Unicode, SetLastError = true)]
  public static extern bool UpdateDriverForPlugAndPlayDevices(IntPtr hwnd, string hardwareId, string infPath, uint flags, out bool reboot);
}
"@

try {
    $principal = [Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
    if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) { throw "Administrator rights are required." }
    if ($InstanceId -notmatch '^USB\\VID_05AC&PID_(12[0-9A-F]{2})\\[^&\\]+$') { throw "Not an Apple composite device: $InstanceId" }
    $devPid = $Matches[1].ToUpper()

    $enumRoot = "HKLM:\SYSTEM\CurrentControlSet\Enum"
    $parentKey = Join-Path $enumRoot $InstanceId
    if (-not (Test-Path $parentKey)) { throw "Device not found. Plug it in and try again." }
    $hwid = ($InstanceId -split '\\')[0..1] -join '\'   # USB\VID_05AC&PID_12AB

    function Get-State {
        $p = Get-ItemProperty $parentKey
        $kids = @()
        Get-PnpDevice -PresentOnly | Where-Object { $_.InstanceId -like "USB\VID_05AC&PID_$devPid&MI_*" } | ForEach-Object {
            $k = Get-ItemProperty (Join-Path $enumRoot $_.InstanceId)
            $kids += [ordered]@{ instance = $_.InstanceId; service = $k.Service; upperFilters = @($k.UpperFilters | Where-Object { $_ }) }
        }
        [ordered]@{
            parent   = [ordered]@{ instance = $InstanceId; service = $p.Service; driver = $p.Driver; upperFilters = @($p.UpperFilters | Where-Object { $_ }); lowerFilters = @($p.LowerFilters | Where-Object { $_ }) }
            children = $kids
        }
    }
    function Set-Upper([string]$key, [string[]]$values) {
        $values = @($values | Where-Object { $_ })
        if ($values.Count) { Set-ItemProperty $key -Name UpperFilters -Type MultiString -Value $values }
        else { Remove-ItemProperty $key -Name UpperFilters -ErrorAction SilentlyContinue }
    }
    function Remove-LibusbFilters($state) {
        if ($state.parent.upperFilters -contains "libusb0") {
            Set-Upper $parentKey @($state.parent.upperFilters | Where-Object { $_ -ne "libusb0" })
            Step "removed libusb0 filter from the parent"
        }
        foreach ($c in $state.children) {
            if ($c.upperFilters -contains "libusb0") {
                $ck = Join-Path $enumRoot $c.instance
                if (Test-Path $ck) { Set-Upper $ck @($c.upperFilters | Where-Object { $_ -ne "libusb0" }) }
                Step "removed libusb0 filter from $($c.instance)"
            }
        }
    }
    # Driver packages are found by reading the published INF files themselves.
    # pnputil's text ("Published Name:", "Provider Name:") is translated on
    # non-English Windows, so it is never parsed.
    # INF text without comments (";" to end of line), so commented-out
    # model lines never count.
    function Read-InfText([string]$path) {
        $b = [IO.File]::ReadAllBytes($path)
        $t = if ($b.Length -ge 2 -and $b[0] -eq 0xFE -and $b[1] -eq 0xFF) { [Text.Encoding]::BigEndianUnicode.GetString($b) }
             elseif ($b.Length -ge 2 -and (($b[0] -eq 0xFF -and $b[1] -eq 0xFE) -or $b[1] -eq 0)) { [Text.Encoding]::Unicode.GetString($b) }
             else { [Text.Encoding]::UTF8.GetString($b) }
        (($t -split "\r?\n") | ForEach-Object { $_ -replace ';.*$', '' }) -join "`n"
    }
    # Newest published package first; only packages for this exact model's
    # composite parent (USB\VID_05AC&PID_xxxx, not one of its &MI_ interfaces).
    function Find-OemInf([scriptblock]$match) {
        $infs = Get-ChildItem (Join-Path $env:WINDIR "INF") -Filter "oem*.inf" -File -ErrorAction SilentlyContinue |
            Sort-Object { [int]($_.BaseName -replace '\D', '') } -Descending
        foreach ($f in $infs) {
            $t = Read-InfText $f.FullName
            if ($t -match "VID_05AC&PID_$devPid(?![0-9A-F&])" -and (& $match $t)) { return $f.Name }
        }
        return $null
    }
    function Find-LibusbInf { Find-OemInf { param($t) $t -match 'AddService\s*=\s*"?libusb0\b' } }
    # Apple's composite-parent package (appleusb.inf from Apple Devices / iTunes).
    function Find-AppleInf { Find-OemInf { param($t) $t -match 'AppleUSB_CCGPDriverInstall' } }
    function Bind([string]$inf) {
        $path = Join-Path $env:WINDIR "INF\$inf"
        $once = {
            $reboot = $false
            $ok = [PlNewDev]::UpdateDriverForPlugAndPlayDevices([IntPtr]::Zero, $hwid, $path, 1, [ref]$reboot)
            if ($reboot) { $script:result.reboot = $true }
            if ($ok) { 0 } else { [Runtime.InteropServices.Marshal]::GetLastWin32Error() }
        }
        $code = & $once
        if ($code -eq 259) { $code = Invoke-WithAppleServicePaused $once }  # device held open, see bind
        return $code
    }
    # Runs $work with Apple Mobile Device Service stopped, then starts every
    # service it stopped again, whatever happened.
    function Invoke-WithAppleServicePaused([scriptblock]$work) {
        $paused = @(Get-Service -ErrorAction SilentlyContinue | Where-Object { $_.DisplayName -like "Apple Mobile Device*" -and $_.Status -eq "Running" })
        foreach ($svc in $paused) {
            Step "pausing $($svc.DisplayName) so Windows can switch the driver"
            Stop-Service -Name $svc.Name -Force -ErrorAction SilentlyContinue
        }
        try {
            Start-Sleep -Milliseconds 1500
            return (& $work)
        } finally {
            foreach ($svc in $paused) {
                Start-Service -Name $svc.Name -ErrorAction SilentlyContinue
                Step "started $($svc.DisplayName) again"
            }
        }
    }
    # Removes one certificate from a LocalMachine store; in "My" its private key
    # is deleted too. -DeleteKey is a dynamic parameter of the certificate
    # provider: it only exists when the cert path is given directly (piping a
    # certificate into Remove-Item -DeleteKey fails to bind on Windows
    # PowerShell 5.1, which broke the bind in v2.2).
    function Remove-CertWithKey([string]$store, [string]$thumb) {
        $path = "Cert:\LocalMachine\$store\$thumb"
        if (-not (Test-Path -LiteralPath $path)) { return }
        if ($store -eq "My") {
            try { Remove-Item -Path $path -DeleteKey -Force -ErrorAction Stop; return } catch {}
            try {
                $c = Get-Item -LiteralPath $path
                $k = [Security.Cryptography.X509Certificates.RSACertificateExtensions]::GetRSAPrivateKey($c)
                if ($k -is [Security.Cryptography.RSACng]) { $k.Key.Delete() }
                elseif ($k -is [Security.Cryptography.RSACryptoServiceProvider]) { $k.PersistKeyInCsp = $false; $k.Clear() }
            } catch {}
        }
        Remove-Item -LiteralPath $path -Force -ErrorAction SilentlyContinue
    }
    function Remove-ZenLinkCert([string]$thumb) {
        foreach ($store in "Root", "TrustedPublisher", "My") { Remove-CertWithKey $store $thumb }
    }
    # Repairs PCs where the v2.2 bug stopped a bind half-way: the signing key
    # was left in LocalMachine\My and the certificate trusted although no
    # driver package was installed with it. ZenLink never keeps signing keys,
    # and a ZenLink certificate stays trusted only while a ZenLink package in
    # the driver store is signed with it.
    function Repair-ZenLinkCerts {
        Get-ChildItem "Cert:\LocalMachine\My" -ErrorAction SilentlyContinue | Where-Object { $_.Subject -like "CN=ZenLink USB Direct*" } | ForEach-Object {
            Remove-CertWithKey "My" $_.Thumbprint
            Step "removed a leftover ZenLink signing key ($($_.Thumbprint.Substring(0, 8)))"
        }
        $used = @{}
        Get-ChildItem "$env:WINDIR\System32\DriverStore\FileRepository" -Directory -Filter "zenlink_usbdirect_*" -ErrorAction SilentlyContinue | ForEach-Object {
            Get-ChildItem $_.FullName -Filter "*.cat" -File -ErrorAction SilentlyContinue | ForEach-Object {
                $s = Get-AuthenticodeSignature -FilePath $_.FullName -ErrorAction SilentlyContinue
                if ($s.SignerCertificate) { $used[$s.SignerCertificate.Thumbprint] = $true }
            }
        }
        foreach ($store in "Root", "TrustedPublisher") {
            Get-ChildItem "Cert:\LocalMachine\$store" -ErrorAction SilentlyContinue | Where-Object { $_.Subject -like "CN=ZenLink USB Direct*" -and -not $used[$_.Thumbprint] } | ForEach-Object {
                Remove-CertWithKey $store $_.Thumbprint
                Step "removed an unused ZenLink certificate from $store ($($_.Thumbprint.Substring(0, 8)))"
            }
        }
    }

    # Build and sign a libusb-win32 driver package for this device model, the
    # way libwdi (TikTok LIVE Studio, Zadig) does: an INF for this PID, the
    # libusb-win32 binaries already on this PC, a catalog signed with a
    # self-signed certificate whose private key is deleted right after
    # signing, and only its public part trusted (Root + TrustedPublisher).
    function New-LibusbPackage {
        # 1. ZenLink's bundled libusb-win32 1.2.6.0 (drivers\libusb-win32),
        # 2. a libusb-win32 package already in the driver store, 3. System32.
        $bundled = Join-Path (Split-Path -Parent $PSScriptRoot) "drivers\libusb-win32"
        $repo = Get-ChildItem "$env:WINDIR\System32\DriverStore\FileRepository" -Directory -ErrorAction SilentlyContinue |
            Where-Object { Test-Path (Join-Path $_.FullName "amd64\libusb0.sys") } | Select-Object -First 1
        if (Test-Path (Join-Path $bundled "amd64\libusb0.sys")) {
            $sys = Join-Path $bundled "amd64\libusb0.sys"
            $dll = Join-Path $bundled "amd64\libusb0.dll"
            $dll32 = Join-Path $bundled "x86\libusb0_x86.dll"
        } elseif ($repo) {
            $sys = Join-Path $repo.FullName "amd64\libusb0.sys"
            $dll = Join-Path $repo.FullName "amd64\libusb0.dll"
            $dll32 = Join-Path $repo.FullName "x86\libusb0_x86.dll"
        } else {
            $sys = "$env:WINDIR\System32\drivers\libusb0.sys"
            $dll = "$env:WINDIR\System32\libusb0.dll"
            $dll32 = "$env:WINDIR\SysWOW64\libusb0.dll"
        }
        foreach ($f in $sys, $dll, $dll32) {
            if (-not (Test-Path $f)) { throw "libusb-win32 is not installed on this PC ($f missing). Install TikTok LIVE Studio's iOS driver or libusb-win32 first." }
        }
        foreach ($f in $sys, $dll, $dll32) {
            $sig = Get-AuthenticodeSignature $f
            if ($sig.Status -ne "Valid") { throw "$f is not validly signed ($($sig.Status)); refusing to package it." }
        }
        Step "libusb-win32 binaries from $(Split-Path -Parent (Split-Path -Parent $sys))"

        $family = if ($devPid -eq "12AB") { "iPad" } elseif ($devPid -eq "12A8") { "iPhone" } else { "Apple device" }
        $pkg = Join-Path $env:ProgramData "ZenLink\drivers\package-$devPid"
        if (Test-Path $pkg) { Remove-Item $pkg -Recurse -Force }
        New-Item -ItemType Directory -Force -Path (Join-Path $pkg "amd64"), (Join-Path $pkg "x86") | Out-Null
        Copy-Item $sys (Join-Path $pkg "amd64\libusb0.sys")
        Copy-Item $dll (Join-Path $pkg "amd64\libusb0.dll")
        Copy-Item $dll32 (Join-Path $pkg "x86\libusb0_x86.dll")
        $name = "zenlink_usbdirect_$($devPid.ToLower())"
        $date = (Get-Item $sys).VersionInfo
        $ver = "{0}.{1}.{2}.{3}" -f $date.FileMajorPart, $date.FileMinorPart, $date.FileBuildPart, $date.FilePrivatePart
        $inf = @"
; $name.inf - libusb-win32 device driver for $family (VID_05AC&PID_$devPid)
; Generated by ZenLink USB Direct. libusb-win32 is (c) libusb-win32 contributors, GNU LGPL.
[Strings]
DeviceName = "$family (ZenLink USB Direct)"
VendorName = "Apple, Inc."
SourceName = "ZenLink USB Direct driver"
DeviceID   = "VID_05AC&PID_$devPid"
DeviceGUID = "{$([guid]::NewGuid().ToString().ToUpper())}"

[Version]
Signature   = "`$Windows NT`$"
Class       = "libusb-win32 devices"
ClassGuid   = {EB781AAF-9C70-4523-A5DF-642A87ECA567}
Provider    = "libusb-win32"
CatalogFile = $name.cat
DriverVer   = $(Get-Date -Format "MM/dd/yyyy"), $ver

[ClassInstall32]
Addreg = libusb_class_install_add_reg

[libusb_class_install_add_reg]
HKR,,,0,"libusb-win32 devices"
HKR,,Icon,,-20

[Manufacturer]
%VendorName% = Devices, NTAMD64

[SourceDisksNames]
1 = %SourceName%

[SourceDisksFiles.amd64]
libusb0.sys     = 1,amd64
libusb0.dll     = 1,amd64
libusb0_x86.dll = 1,x86

[DestinationDirs]
libusb_files_sys       = 10,system32\drivers
libusb_files_dll       = 10,system32
libusb_files_dll_wow64 = 10,syswow64

[libusb_files_sys]
libusb0.sys

[libusb_files_dll]
libusb0.dll

[libusb_files_dll_wow64]
libusb0.dll, libusb0_x86.dll

[LIBUSB_WIN32_DEV.NTAMD64]
CopyFiles = libusb_files_sys, libusb_files_dll, libusb_files_dll_wow64
FeatureScore = 0x6F

[LIBUSB_WIN32_DEV.NTAMD64.HW]
DelReg = libusb_del_reg_hw
AddReg = libusb_add_reg_hw

[LIBUSB_WIN32_DEV.NTAMD64.Services]
AddService = libusb0, 0x00000002, libusb_add_service

[libusb_del_reg_hw]
HKR,,LowerFilters
HKR,,UpperFilters

[libusb_add_reg_hw]
HKR,,SurpriseRemovalOK,0x00010001,1
HKR,,DeviceInterfaceGUIDs,0x00010000,%DeviceGUID%

[libusb_add_service]
DisplayName   = "libusb-win32 - Kernel Driver $ver"
ServiceType   = 1
StartType     = 3
ErrorControl  = 0
ServiceBinary = %12%\libusb0.sys

[Devices.NTAMD64]
%DeviceName% = LIBUSB_WIN32_DEV.NTAMD64, USB\%DeviceID%
"@
        $infPath = Join-Path $pkg "$name.inf"
        Set-Content -Path $infPath -Value $inf -Encoding ASCII
        Step "wrote $name.inf for PID $devPid"

        $catPath = Join-Path $pkg "$name.cat"
        New-FileCatalog -Path $pkg -CatalogFilePath $catPath -CatalogVersion 2.0 | Out-Null

        $cert = New-SelfSignedCertificate -Type CodeSigningCert -Subject "CN=ZenLink USB Direct ($family $devPid)" `
            -CertStoreLocation "Cert:\LocalMachine\My" -KeyAlgorithm RSA -KeyLength 2048 -HashAlgorithm SHA256 `
            -KeyExportPolicy NonExportable -NotAfter (Get-Date).AddYears(20)
        try {
            # Trust the public certificate BEFORE signing. Set-AuthenticodeSignature
            # checks the chain as soon as it has signed, and with an untrusted
            # self-signed root it reports Status "UnknownError" ("A certificate
            # chain processed, but terminated in a root certificate which is not
            # trusted by the trust provider") although the catalog was signed.
            # v2.2.2 and older treated that as a failed signature.
            $cer = Join-Path $pkg "$name.cer"
            Export-Certificate -Cert $cert -FilePath $cer | Out-Null
            Import-Certificate -FilePath $cer -CertStoreLocation "Cert:\LocalMachine\Root" | Out-Null
            Import-Certificate -FilePath $cer -CertStoreLocation "Cert:\LocalMachine\TrustedPublisher" | Out-Null
            $signed = Set-AuthenticodeSignature -FilePath $catPath -Certificate $cert -HashAlgorithm SHA256
            if (-not $signed.SignerCertificate -or $signed.SignerCertificate.Thumbprint -ne $cert.Thumbprint) {
                throw "Signing the driver catalog failed: $($signed.StatusMessage)"
            }
            # Verify from disk, the way Windows will check it when installing.
            $check = Get-AuthenticodeSignature -FilePath $catPath
            if ($check.Status -ne "Valid") {
                throw "The signed driver catalog does not verify ($($check.Status)): $($check.StatusMessage)"
            }
        } catch {
            # Nothing of a failed attempt stays trusted or on disk.
            Remove-ZenLinkCert $cert.Thumbprint
            Remove-Item $pkg -Recurse -Force -ErrorAction SilentlyContinue
            throw
        } finally {
            # The private key is never kept: nothing else can be signed with it.
            Remove-CertWithKey "My" $cert.Thumbprint
        }
        if (Test-Path -LiteralPath "Cert:\LocalMachine\My\$($cert.Thumbprint)") {
            Remove-ZenLinkCert $cert.Thumbprint
            throw "Windows did not delete the temporary signing key; nothing was installed."
        }
        Step "signed the package (certificate $($cert.Thumbprint.Substring(0, 8)), private key deleted)"
        $script:result.certificate = $cert.Thumbprint
        $script:result.package = $pkg
        return [pscustomobject]@{ inf = $infPath; thumbprint = $cert.Thumbprint }
    }

    function Wait-Service([string]$service) {
        Step "waiting for the device to restart with $service"
        $deadline = (Get-Date).AddSeconds(25)
        do {
            Start-Sleep -Milliseconds 800
            $dev = Get-PnpDevice -InstanceId $InstanceId -ErrorAction SilentlyContinue
            $svc = (Get-ItemProperty $parentKey -ErrorAction SilentlyContinue).Service
            if ($dev -and $dev.Status -eq "OK" -and $svc -eq $service) { return $true }
        } while ((Get-Date) -lt $deadline)
        return $false
    }

    # 1. Backup
    $before = Get-State
    $backupDir = Join-Path $env:ProgramData "ZenLink\drivers\backups"
    New-Item -ItemType Directory -Force -Path $backupDir | Out-Null
    $backup = Join-Path $backupDir ("{0}-{1}-{2}.json" -f $Action, ($InstanceId -split '\\')[-1], (Get-Date -Format "yyyyMMdd-HHmmss"))
    ($before | ConvertTo-Json -Depth 6) | Set-Content -Path $backup -Encoding UTF8
    $result.backup = $backup
    Step "backup saved"
    Repair-ZenLinkCerts

    switch ($Action) {
        "bind" {
            $inf = Find-LibusbInf
            $infPath = $null
            $addedCert = $null
            if ($inf) {
                $infPath = Join-Path $env:WINDIR "INF\$inf"
                Step "using driver package $inf"
            } else {
                # No package for this model: build one the way libwdi/Zadig do.
                $built = New-LibusbPackage
                $infPath = $built.inf
                $addedCert = $built.thumbprint
                $inf = Split-Path -Leaf $infPath
            }
            Remove-LibusbFilters $before
            $install = {
                $reboot = $false
                $ok = [PlNewDev]::UpdateDriverForPlugAndPlayDevices([IntPtr]::Zero, $hwid, $infPath, 1, [ref]$reboot)
                if ($reboot) { $script:result.reboot = $true }
                if ($ok) { 0 } else { [Runtime.InteropServices.Marshal]::GetLastWin32Error() }
            }
            $code = & $install
            if ($code -eq 259) {
                # 259: Windows could not remove the device because a program still
                # has it open (usually Apple Mobile Device Service). Pause Apple's
                # service for this one change, retry, and always start it again.
                $code = Invoke-WithAppleServicePaused $install
            }
            if ($code -ne 0 -and $addedCert) {
                Remove-ZenLinkCert $addedCert
                Remove-Item (Split-Path -Parent $infPath) -Recurse -Force -ErrorAction SilentlyContinue
                Step "removed the certificate and package made for this attempt"
            }
            if ($code -eq 259) {
                throw "Windows could not switch the driver because another program has the device open (error 259). Close Apple Devices, iTunes and Phone Link, unplug the cable, plug it in again and retry. The device keeps Apple's driver."
            }
            if ($code -ne 0) {
                # Filters stay removed: a libusb0 filter is never put back.
                throw "Windows could not install the libusb-win32 driver (error $code). The device keeps Apple's driver."
            }
            Step "installed $inf as the device driver"
            if ($result.reboot) {
                # Windows finishes the switch at the next restart; rolling back
                # now would undo a change that is only waiting for it.
                $result.message = "The driver is installed, but Windows needs a restart to finish. Restart Windows, then plug the device in again."
                $result.after = Get-State
                $result.ok = $true
                return
            }
            if (-not (Wait-Service "libusb0")) {
                $apple = Find-AppleInf
                $back = $false
                if ($apple -and (Bind $apple) -eq 0) { $back = Wait-Service "usbccgp" }
                $result.rolledBack = $back
                if ($back) { throw "The device did not start with the libusb-win32 driver. Apple's driver was put back." }
                throw "The device did not start with the libusb-win32 driver, and Apple's driver could not be put back automatically. Unplug the cable, plug it in again and press Restore Apple driver."
            }
            $result.message = "USB Direct driver installed. Unplug the cable and plug it in again."
        }
        "restore" {
            Remove-LibusbFilters $before
            if ($before.parent.service -ne "usbccgp") {
                $apple = Find-AppleInf
                if (-not $apple) { throw "Apple's USB driver package is not in the driver store. Install Apple Devices (Microsoft Store) and try again." }
                $code = Bind $apple
                if ($code -eq 259) { throw "Windows could not switch the driver because another program has the device open (error 259). Close TikTok LIVE Studio and other capture apps, unplug the cable, plug it in again and retry." }
                if ($code -ne 0) { throw "Windows could not bind Apple's driver back (error $code)." }
                Step "bound Apple driver $apple"
                if ($result.reboot) {
                    $result.message = "Apple's driver is installed, but Windows needs a restart to finish. Restart Windows, then plug the device in again."
                    $result.after = Get-State
                    $result.ok = $true
                    return
                }
            } else {
                pnputil /restart-device "$InstanceId" | Out-Null
            }
            if (-not (Wait-Service "usbccgp")) { throw "The device did not come back on Apple's driver yet. Unplug the cable and plug it in again." }
            $result.message = "Apple driver restored. Unplug the cable and plug it in again."
        }
        "removefilter" {
            Remove-LibusbFilters $before
            pnputil /restart-device "$InstanceId" | Out-Null
            if (-not (Wait-Service $before.parent.service)) {
                # Never put the unsafe filter back; a replug finishes the restart.
                throw "The filter is removed, but the device has not restarted yet. Unplug the cable and plug it in again."
            }
            $result.message = "libusb0 filter removed. Unplug the cable and plug it in again."
        }
    }
    $result.after = Get-State
    $result.ok = $true
} catch {
    $result.message = $_.Exception.Message
} finally {
    Finish
    Write-Progress -Activity "ZenLink USB driver ($Action)" -Completed
    Write-Host ""
    if ($result.ok) { Write-Host "  Done: $($result.message)" -ForegroundColor Green }
    else { Write-Host "  Not changed: $($result.message)" -ForegroundColor Yellow }
    Write-Host "  This window closes in 4 seconds." -ForegroundColor Gray
    Start-Sleep -Seconds 4
}
