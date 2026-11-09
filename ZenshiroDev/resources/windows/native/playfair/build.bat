@echo off
rem Builds playfair.dll in this folder (FairPlay key decryption for AirPlay) with MSVC.
rem Needs Visual Studio or Build Tools with the C++ workload.
setlocal
set "VSWHERE=%ProgramFiles(x86)%\Microsoft Visual Studio\Installer\vswhere.exe"
for /f "usebackq delims=" %%I in (`"%VSWHERE%" -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath`) do set "VS=%%I"
if not defined VS ( echo Visual Studio C++ build tools not found. & exit /b 1 )
call "%VS%\VC\Auxiliary\Build\vcvars64.bat" >nul
cd /d "%~dp0"
cl /nologo /O2 /LD /MT export.c playfair.c omg_hax.c hand_garble.c modified_md5.c sap_hash.c /Fe:"%~dp0playfair.dll" /link /NOLOGO
del *.obj "%~dp0playfair.exp" "%~dp0playfair.lib" 2>nul
