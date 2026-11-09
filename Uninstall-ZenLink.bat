@echo off
rem Removes ZenLink's data and shortcuts from this PC (portable / source copy).
rem The installed version (Setup.exe) is removed from Windows Settings > Apps.
chcp 65001 >nul
setlocal
rem The app folder: this script sits in it (repository root or packaged copy).
set "ROOT=%~dp0"
pushd "%ROOT%"
title Uninstall ZenLink
echo ZenLink - clean uninstall
echo.
echo This will:
echo   1. close ZenLink (only the copy started from this folder)
echo   2. delete its settings, saved passcode, logs and WebView2 cache
echo   3. delete the ZenLink shortcuts
echo   4. optionally delete this folder
echo.
echo If you used USB Direct, open ZenLink first and press
echo "Restore Apple driver" in Settings ^> Connection ^& Drivers.
echo.
choice /c YN /m "Continue"
if errorlevel 2 exit /b 0

rem Stop only what runs from this folder: the stable ZenLink uses the same exe names.
rem Child processes (adb, the driver helper) are stopped too.
powershell -NoProfile -Command "$d = '%ROOT%'; $all = Get-CimInstance Win32_Process; $own = @($all | Where-Object { $_.ExecutablePath -and $_.ExecutablePath.StartsWith($d, 'OrdinalIgnoreCase') }); $ids = $own.ProcessId; $kids = @($all | Where-Object { $ids -contains $_.ParentProcessId }); ($kids + $own) | ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }" >nul 2>&1
timeout /t 1 >nul

for %%F in (settings.json secrets.json devices.json zenlink.log) do if exist "%ROOT%%%F" del /f /q "%ROOT%%%F"
if exist "%ROOT%.webview" rmdir /s /q "%ROOT%.webview"
if exist "%ROOT%data" rmdir /s /q "%ROOT%data"
if exist "%LOCALAPPDATA%\ZenLink-Next" rmdir /s /q "%LOCALAPPDATA%\ZenLink-Next"
powershell -NoProfile -Command "foreach ($d in @([Environment]::GetFolderPath('Desktop'), [Environment]::GetFolderPath('Programs'))) { Remove-Item -LiteralPath (Join-Path $d 'ZenLink.lnk') -ErrorAction SilentlyContinue }"
echo Data and shortcuts removed.
echo.
choice /c YN /m "Also delete the ZenLink folder %ROOT%"
if errorlevel 2 goto :done
popd
start "" /min /d "%TEMP%" cmd /c "timeout /t 2 >nul & rmdir /s /q "%ROOT%""
:done
echo Done.
timeout /t 3 >nul
