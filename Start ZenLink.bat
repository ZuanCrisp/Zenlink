@echo off
setlocal EnableDelayedExpansion
chcp 65001 >nul
cd /d "%~dp0"
title ZenLink

rem Standalone build: ZenLink.exe sits next to this file.
if exist "%~dp0ZenLink.exe" (
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

echo ZenLink - first-time setup
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
        echo Python 3.11, 3.12 or 3.13 is required.
        echo Install it from https://www.python.org/downloads/ or run:
        echo     winget install -e --id Python.Python.3.13
        echo then open Start ZenLink.bat again.
        echo.
        pause
        exit /b 1
    )
    echo [1/2] Creating the Python environment...
    !SYS_PY! -m venv "%VENV%" || goto :failed
)

echo [2/2] Installing components (about a minute the first time)...
"%PY%" -m pip install --disable-pip-version-check -q --upgrade pip >nul 2>&1
"%PY%" -m pip install --disable-pip-version-check -q -r "%~dp0requirements.txt" || goto :failed
copy /y "%~dp0requirements.txt" "%STAMP%" >nul
echo Setup complete. Run Create-Desktop-Shortcut.bat for a desktop shortcut.

:launch
if exist "%PYW%" (
    start "" "%PYW%" -m zenlink
) else (
    start "" "%PY%" -m zenlink
)
exit /b 0

:failed
echo.
echo Setup failed. Check the internet connection and run Start ZenLink.bat again.
pause
exit /b 1
