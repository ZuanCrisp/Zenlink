@echo off
rem Removes ZenLink's data and shortcuts from this PC (portable / source copy).
rem The installed version (Setup.exe) is removed from Windows Settings > Apps.
chcp 65001 >nul
setlocal
cd /d "%~dp0"
title Uninstall ZenLink
echo ZenLink - clean uninstall
echo.
echo This will:
echo   1. close ZenLink
echo   2. delete its settings, saved passcode, logs and WebView2 cache
echo   3. delete the ZenLink shortcuts
echo   4. optionally delete this folder
echo.
echo If you used USB Direct, open ZenLink first and press
echo "Restore Apple driver" in Settings ^> Connection ^& Drivers.
echo.
choice /c YN /m "Continue"
if errorlevel 2 exit /b 0

taskkill /im ZenLink.exe /f >nul 2>&1
powershell -NoProfile -Command "Get-CimInstance Win32_Process -Filter \"Name like 'python%%.exe'\" | Where-Object { $_.CommandLine -match '-m zenlink' } | ForEach-Object { Stop-Process -Id $_.ProcessId -Force }" >nul 2>&1
timeout /t 1 >nul

for %%F in (settings.json secrets.json devices.json zenlink.log) do if exist "%~dp0%%F" del /f /q "%~dp0%%F"
if exist "%~dp0.webview" rmdir /s /q "%~dp0.webview"
if exist "%LOCALAPPDATA%\ZenLink" rmdir /s /q "%LOCALAPPDATA%\ZenLink"
powershell -NoProfile -Command "foreach ($d in @([Environment]::GetFolderPath('Desktop'), [Environment]::GetFolderPath('Programs'))) { Remove-Item -LiteralPath (Join-Path $d 'ZenLink.lnk') -ErrorAction SilentlyContinue }"
echo Data and shortcuts removed.
echo.
choice /c YN /m "Also delete the ZenLink folder %~dp0"
if errorlevel 2 goto :done
start "" /min cmd /c "timeout /t 2 >nul & rmdir /s /q "%~dp0""
:done
echo Done.
timeout /t 3 >nul
