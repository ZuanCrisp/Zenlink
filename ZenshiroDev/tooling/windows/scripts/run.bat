@echo off
rem ZenLink in diagnostic mode: starts ZenLink.exe and opens its log folder.
chcp 65001 >nul
rem The app folder: this script sits there in a packaged copy, or in
rem ZenshiroDev\tooling\windows\scripts in a source checkout.
set "ROOT=%~dp0"
if not exist "%ROOT%ZenLink.exe" if not exist "%ROOT%Start ZenLink.bat" set "ROOT=%~dp0..\..\..\..\"
pushd "%ROOT%"
set "ROOT=%CD%\"
if exist "%ROOT%ZenLink.exe" (
    start "" "%ROOT%ZenLink.exe" %*
) else (
    call "%ROOT%Start ZenLink.bat"
)
start "" explorer.exe "%LOCALAPPDATA%\ZenLink-Next\logs"
popd
