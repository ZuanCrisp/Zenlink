@echo off
setlocal EnableDelayedExpansion
chcp 65001 >nul
cd /d "%~dp0"
title ZenLink

rem --- Pinned adb: Google platform-tools. The ZenLink window downloads it on request
rem (Android page, "Install adb") with the same revision and SHA-256 as
rem apps\desktop\src\android.rs; ZenshiroDev\tooling\windows\tools\get-adb.ps1 does the
rem same by hand. Change these only together with ZenshiroDev\docs\THIRD_PARTY.md.
set "ZL_ADB_REV=37.0.1"
set "ZL_ADB_SHA256=45f4d63113e895ebde0c90f194099a4676b6ac653bd28d54314a9e022bbc1a99"

rem Installed or packaged copy: ZenLink.exe next to this file, no source code.
if exist "%~dp0ZenLink.exe" if not exist "%~dp0Cargo.toml" (
    start "" "%~dp0ZenLink.exe"
    exit /b 0
)


rem ZenLink (one Rust program, native GPU video).
rem Android: ZenLink finds adb in the Android SDK (ANDROID_HOME,
rem ANDROID_SDK_ROOT, %LOCALAPPDATA%\Android\Sdk) or on PATH. Machine-only
rem settings such as ANDROID_HOME can go in "Start ZenLink.local.bat" next
rem to this file or in its parent folder (not part of the repository). It can
rem also move dev outputs out of the checkout: CARGO_TARGET_DIR (Rust build),
rem ZENLINK_DATA_DIR (settings and logs), ZENLINK_OUT_DIR (release build).
if exist "%~dp0Start ZenLink.local.bat" (
    call "%~dp0Start ZenLink.local.bat"
) else if exist "%~dp0..\Start ZenLink.local.bat" call "%~dp0..\Start ZenLink.local.bat"
rem ZenLink.exe finds this checkout (native DLLs, driver tools, data\) through
rem ZENLINK_SOURCE, also when CARGO_TARGET_DIR builds it outside the checkout.
set "ZENLINK_SOURCE=%~dp0."
set "NEXT=%~dp0ZenLink.exe"
if exist "%NEXT%" goto :start_next
rem Source checkout: build the window from the current code first, so a
rem manual test always runs the latest changes. Unchanged code builds in seconds.
set "NEXT=%~dp0target\release\ZenLink.exe"
if defined CARGO_TARGET_DIR set "NEXT=%CARGO_TARGET_DIR%\release\ZenLink.exe"
call :build_next || exit /b 1
if not exist "%NEXT%" (
    echo ZenLink.exe isn't built yet and Rust isn't installed. Install Rust from https://rustup.rs and run this again.
    pause
    exit /b 1
)

:start_next
rem First start on this PC: check Apple Devices and the AirPlay firewall rule once.
set "STAMP=%LOCALAPPDATA%\ZenLink-Next\.setup-checked"
if not exist "%STAMP%" (
    call :check_env
    if not exist "%LOCALAPPDATA%\ZenLink-Next" mkdir "%LOCALAPPDATA%\ZenLink-Next"
    echo checked> "%STAMP%"
)
start "" "%NEXT%"
exit /b 0

:build_next
if not exist "%~dp0Cargo.toml" exit /b 0
where cargo >nul 2>&1 || if exist "%USERPROFILE%\.cargo\bin\cargo.exe" set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
where cargo >nul 2>&1 || (
    echo Rust is not installed, so the window cannot be rebuilt from the code.
    if exist "%NEXT%" echo Starting the last build instead.
    exit /b 0
)
echo Building the ZenLink window from the current code...
rem Elapsed time and an estimate in the window title (ZenshiroDev\tooling\dev\build-window.ps1).
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0ZenshiroDev\tooling\dev\build-window.ps1" -Workspace "%~dp0."
set "RC=!errorlevel!"
if not "!RC!"=="0" (
    echo.
    echo The build failed. If ZenLink is still open, close it and run Start ZenLink.bat again.
    pause
    exit /b 1
)
exit /b 0

:check_env
rem Apple Mobile Device Service (Apple Driver mode): comes with Apple Devices or iTunes.
sc query "Apple Mobile Device Service" >nul 2>&1
if errorlevel 1 (
    echo  - Apple Devices isn't installed. Apple Driver mode needs it; ZenLink Studio doesn't.
    choice /c YN /n /m "   Install Apple Devices from the Microsoft Store now? [Y/N] "
    if !errorlevel! equ 1 winget install -e --id 9NP83LWLPZ9K --source msstore --accept-package-agreements --accept-source-agreements
) else (
    echo  - Apple Devices: OK
)
rem Windows Firewall: lets the iPhone/iPad reach ZenLink for AirPlay (ZenLink Studio over Wi-Fi).
netsh advfirewall firewall show rule name="ZenLink (AirPlay)" verbose 2>nul | find /i "%NEXT%" >nul
if errorlevel 1 (
    choice /c YN /n /m " - Allow ZenLink on the local network for AirPlay (asks for administrator permission)? [Y/N] "
    if !errorlevel! equ 1 powershell -NoProfile -Command "Start-Process netsh -Verb RunAs -WindowStyle Hidden -Wait -ArgumentList 'advfirewall firewall add rule name=\"ZenLink (AirPlay)\" dir=in action=allow program=\"%NEXT%\" enable=yes profile=any remoteip=localsubnet'"
) else (
    echo  - Firewall for AirPlay: OK
)
echo  - USB drivers: installed from inside ZenLink when you pick ZenLink Studio with the cable.
exit /b 0
