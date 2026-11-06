@echo off
setlocal EnableDelayedExpansion
chcp 65001 >nul
cd /d "%~dp0"
title ZenLink Next

rem Standalone build: ZenLink.exe and its backend ZenLinkService.exe sit next to this file.
if exist "%~dp0ZenLinkService.exe" if exist "%~dp0ZenLink.exe" (
    start "" "%~dp0ZenLink.exe"
    exit /b 0
)

set "VENV=%~dp0.venv"
set "PY=%VENV%\Scripts\python.exe"
set "PYW=%VENV%\Scripts\pythonw.exe"
set "STAMP=%VENV%\.requirements"

rem Fast path: environment exists and requirements.txt hasn't changed.
if exist "%PY%" if exist "%STAMP%" (
    fc /b "%~dp0requirements.txt" "%STAMP%" >nul 2>&1 && goto :launch
)

echo ZenLink Next - first-time setup
echo.

if not exist "%PY%" (
    set "SYS_PY="
    for %%V in (3.13 3.12 3.11) do (
        if not defined SYS_PY (
            py -%%V -c "import sys" >nul 2>&1 && set "SYS_PY=py -%%V"
        )
    )
    if not defined SYS_PY (
        python -c "import sys; raise SystemExit(0 if (3,11) <= sys.version_info[:2] <= (3,13) else 1)" >nul 2>&1 && set "SYS_PY=python"
    )
    if not defined SYS_PY (
        echo Python 3.13 isn't installed. Installing it for this user with winget...
        winget install -e --id Python.Python.3.13 --scope user --silent --accept-package-agreements --accept-source-agreements
        for %%P in ("%LOCALAPPDATA%\Programs\Python\Python313\python.exe") do if exist "%%~P" set "SYS_PY="%%~P""
        if not defined SYS_PY (
            py -3.13 -c "import sys" >nul 2>&1 && set "SYS_PY=py -3.13"
        )
    )
    if not defined SYS_PY (
        echo Python 3.11, 3.12 or 3.13 is required and couldn't be installed automatically.
        echo Install it from https://www.python.org/downloads/ then open Start ZenLink.bat again.
        echo.
        pause
        exit /b 1
    )
    echo [1/3] Creating the Python environment...
    !SYS_PY! -m venv "%VENV%" || goto :failed
)

echo [2/3] Installing components (about a minute the first time)...
"%PY%" -m pip install --disable-pip-version-check -q --upgrade pip >nul 2>&1
"%PY%" -m pip install --disable-pip-version-check -q -r "%~dp0requirements.txt" || goto :failed
copy /y "%~dp0requirements.txt" "%STAMP%" >nul

echo [3/3] Checking Windows components...
call :check_env
echo Setup complete. Run Create-Desktop-Shortcut.bat for a desktop shortcut.

:launch
rem ZenLink window (Rust, native GPU video). It starts the Python backend
rem from .venv by itself. Set ZENLINK_UI=classic for the old window.
rem Android: ZenLink Next finds adb in the Android SDK (ANDROID_HOME,
rem ANDROID_SDK_ROOT, %LOCALAPPDATA%\Android\Sdk) or on PATH. Machine-only
rem settings such as ANDROID_HOME can go in "Start ZenLink.local.bat" next
rem to this file (not part of the repository).
if exist "%~dp0Start ZenLink.local.bat" call "%~dp0Start ZenLink.local.bat"
if /i "%ZENLINK_UI%"=="classic" goto :classic
set "NEXT=%~dp0ZenLink.exe"
if exist "%NEXT%" goto :start_next
rem Source checkout: build the window from the current code first, so a
rem manual test always runs the latest changes. Unchanged code builds in seconds.
set "NEXT=%~dp0rust\target\release\ZenLink.exe"
call :build_next || exit /b 1
if not exist "%NEXT%" goto :classic

:start_next
start "" "%NEXT%"
exit /b 0

:classic
call :check_webview2
if exist "%PYW%" (
    start "" "%PYW%" -m zenlink
) else (
    start "" "%PY%" -m zenlink
)
exit /b 0

:build_next
if not exist "%~dp0rust\Cargo.toml" exit /b 0
where cargo >nul 2>&1 || if exist "%USERPROFILE%\.cargo\bin\cargo.exe" set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
where cargo >nul 2>&1 || (
    echo Rust is not installed, so the window cannot be rebuilt from the code.
    if exist "%NEXT%" echo Starting the last build instead.
    exit /b 0
)
echo Building the ZenLink Next window from the current code. The first build takes a few minutes...
pushd "%~dp0rust"
cargo build --release -p zenlink-app
set "RC=!errorlevel!"
popd
if not "!RC!"=="0" (
    echo.
    echo The build failed. If ZenLink Next is still open, close it and run Start ZenLink.bat again.
    pause
    exit /b 1
)
exit /b 0

:check_webview2
rem Microsoft Edge WebView2: only the classic window (ZENLINK_UI=classic)
rem uses it. The Rust window does not need it (B-6).
reg query "HKLM\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}" /v pv >nul 2>&1 || ^
reg query "HKCU\Software\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}" /v pv >nul 2>&1 || (
    echo  - Installing Microsoft Edge WebView2 for the classic window...
    winget install -e --id Microsoft.EdgeWebView2Runtime --silent --accept-package-agreements --accept-source-agreements
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
"%PY%" -c "from zenlink import firewall; raise SystemExit(0 if firewall.has_rule() else 1)" >nul 2>&1
if errorlevel 1 (
    choice /c YN /n /m " - Allow ZenLink Next on the local network for AirPlay (asks for administrator permission)? [Y/N] "
    if !errorlevel! equ 1 "%PY%" -c "from zenlink import firewall; firewall.request_rule()"
) else (
    echo  - Firewall for AirPlay: OK
)
echo  - USB drivers: installed from inside ZenLink when you pick ZenLink Studio with the cable.
exit /b 0

:failed
echo.
echo Setup failed. Check the internet connection and run Start ZenLink.bat again.
pause
exit /b 1
