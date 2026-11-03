@echo off
rem ZenLink in console (diagnostic) mode: the log stays visible in this window.
chcp 65001 >nul
cd /d "%~dp0"
title ZenLink (console)
if exist "%~dp0ZenLink.exe" (
    "%~dp0ZenLink.exe" %*
    goto :end
)
if not exist "%~dp0.venv\Scripts\python.exe" (
    echo First run Start ZenLink.bat once to set up the Python environment.
    goto :end
)
"%~dp0.venv\Scripts\python.exe" -X dev -m zenlink %*
:end
echo.
echo ZenLink exited. Log: zenlink.log
pause
