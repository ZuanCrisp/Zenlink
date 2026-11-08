@echo off
rem ZenLink in console (diagnostic) mode: the log stays visible in this window.
chcp 65001 >nul
rem The app folder: this script sits there in a packaged copy, or in
rem ZenshiroDev\tooling\windows\scripts in a source checkout.
set "ROOT=%~dp0"
if not exist "%ROOT%ZenLink.exe" if not exist "%ROOT%Start ZenLink.bat" set "ROOT=%~dp0..\..\..\..\"
pushd "%ROOT%"
set "ROOT=%CD%\"
title ZenLink (console)
set "PYTHONPATH=%ROOT%apps\desktop\backend;%PYTHONPATH%"
if exist "%ROOT%ZenLinkService.exe" (
    "%ROOT%ZenLinkService.exe" %*
    goto :end
)
if not exist "%ROOT%.venv\Scripts\python.exe" (
    echo First run Start ZenLink.bat once to set up the Python environment.
    goto :end
)
"%ROOT%.venv\Scripts\python.exe" -X dev -m zenlink %*
:end
echo.
echo ZenLink exited. Log: zenlink.log
pause
