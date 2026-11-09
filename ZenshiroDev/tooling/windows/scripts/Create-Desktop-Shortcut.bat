@echo off
rem Desktop + Start Menu shortcut with the ZenLink icon.
chcp 65001 >nul
rem The app folder: this script sits there in a packaged copy, or in
rem ZenshiroDev\tooling\windows\scripts in a source checkout.
set "ROOT=%~dp0"
if not exist "%ROOT%ZenLink.exe" if not exist "%ROOT%Start ZenLink.bat" set "ROOT=%~dp0..\..\..\..\"
pushd "%ROOT%"
set "ROOT=%CD%\"
set "TARGET=%ROOT%Start ZenLink.bat"
set "STYLE=7"
rem The icon: ZenLink.exe's own in a packaged copy, ZenshiroDev\resources in a source checkout.
set "ICON=%ROOT%ZenshiroDev\resources\assets\zenlink.ico"
rem A packaged copy (no source code) starts ZenLink.exe directly.
if exist "%ROOT%ZenLink.exe" if not exist "%ROOT%Cargo.toml" (
    set "TARGET=%ROOT%ZenLink.exe"
    set "ICON=%ROOT%ZenLink.exe,0"
)
if /i "%TARGET%"=="%ROOT%ZenLink.exe" set "STYLE=1"
powershell -NoProfile -ExecutionPolicy Bypass -Command ^
  "$ws = New-Object -ComObject WScript.Shell;" ^
  "foreach ($dir in @([Environment]::GetFolderPath('Desktop'), [Environment]::GetFolderPath('Programs'))) {" ^
  "  $s = $ws.CreateShortcut((Join-Path $dir 'ZenLink.lnk'));" ^
  "  $s.TargetPath = '%TARGET%'; $s.WorkingDirectory = '%ROOT%';" ^
  "  $s.IconLocation = '%ICON%'; $s.WindowStyle = %STYLE%;" ^
  "  $s.Description = 'ZenLink - mirror and control your iPhone, iPad or Android'; $s.Save() }"
if errorlevel 1 (
    echo Could not create the shortcuts.
) else (
    echo ZenLink shortcuts created on the Desktop and in the Start Menu.
)
timeout /t 3 >nul
