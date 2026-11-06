@echo off
rem Desktop + Start Menu shortcut with the ZenLink icon.
chcp 65001 >nul
cd /d "%~dp0"
set "TARGET=%~dp0Start ZenLink.bat"
set "STYLE=7"
rem ZenLink.exe needs its backend: ZenLinkService.exe or the .venv.
if exist "%~dp0ZenLink.exe" if exist "%~dp0ZenLinkService.exe" set "TARGET=%~dp0ZenLink.exe"
if exist "%~dp0ZenLink.exe" if exist "%~dp0.venv\Scripts\python.exe" set "TARGET=%~dp0ZenLink.exe"
if /i "%TARGET%"=="%~dp0ZenLink.exe" set "STYLE=1"
powershell -NoProfile -ExecutionPolicy Bypass -Command ^
  "$ws = New-Object -ComObject WScript.Shell;" ^
  "foreach ($dir in @([Environment]::GetFolderPath('Desktop'), [Environment]::GetFolderPath('Programs'))) {" ^
  "  $s = $ws.CreateShortcut((Join-Path $dir 'ZenLink Next.lnk'));" ^
  "  $s.TargetPath = '%TARGET%'; $s.WorkingDirectory = '%~dp0';" ^
  "  $s.IconLocation = '%~dp0assets\zenlink.ico'; $s.WindowStyle = %STYLE%;" ^
  "  $s.Description = 'ZenLink Next - mirror and control your iPhone, iPad or Android'; $s.Save() }"
if errorlevel 1 (
    echo Could not create the shortcuts.
) else (
    echo ZenLink Next shortcuts created on the Desktop and in the Start Menu.
)
timeout /t 3 >nul
