@echo off
rem Desktop + Start Menu shortcut with the ZenLink icon.
chcp 65001 >nul
cd /d "%~dp0"
set "TARGET=%~dp0Start ZenLink.bat"
set "STYLE=7"
if exist "%~dp0ZenLink.exe" (
    set "TARGET=%~dp0ZenLink.exe"
    set "STYLE=1"
)
powershell -NoProfile -ExecutionPolicy Bypass -Command ^
  "$ws = New-Object -ComObject WScript.Shell;" ^
  "foreach ($dir in @([Environment]::GetFolderPath('Desktop'), [Environment]::GetFolderPath('Programs'))) {" ^
  "  $s = $ws.CreateShortcut((Join-Path $dir 'ZenLink.lnk'));" ^
  "  $s.TargetPath = '%TARGET%'; $s.WorkingDirectory = '%~dp0';" ^
  "  $s.IconLocation = '%~dp0assets\zenlink.ico'; $s.WindowStyle = %STYLE%;" ^
  "  $s.Description = 'ZenLink - mirror and control your iPhone or iPad'; $s.Save() }"
if errorlevel 1 (
    echo Could not create the shortcuts.
) else (
    echo ZenLink shortcuts created on the Desktop and in the Start Menu.
)
timeout /t 3 >nul
