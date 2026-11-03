; ZenLink installer (Inno Setup 6). Built by build_portable_release.ps1.
#ifndef AppVersion
  #define AppVersion "2.0.0"
#endif
#ifndef SourceDir
  #define SourceDir "..\dist\ZenLink"
#endif

[Setup]
AppId={{6B0E8C1A-5D3F-4E0A-9C2B-2E7A1F5D9A10}
AppName=ZenLink
AppVersion={#AppVersion}
AppVerName=ZenLink {#AppVersion}
AppPublisher=Zenvy Zenshiro
AppPublisherURL=https://github.com/zenvyzenshiro/zenlink
AppSupportURL=https://github.com/zenvyzenshiro/zenlink/issues
AppCopyright=Copyright (C) 2026 Zenvy Zenshiro
DefaultDirName={autopf}\ZenLink
DefaultGroupName=ZenLink
DisableProgramGroupPage=yes
LicenseFile=..\LICENSE
SetupIconFile=..\assets\zenlink.ico
UninstallDisplayIcon={app}\ZenLink.exe
OutputBaseFilename=ZenLink-v{#AppVersion}-Setup
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
MinVersion=10.0.17763
CloseApplications=yes
VersionInfoCompany=Zenvy Zenshiro
VersionInfoDescription=ZenLink Setup
VersionInfoVersion={#AppVersion}

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
#if FileExists(AddBackslash(CompilerPath) + "Languages\Indonesian.isl")
Name: "indonesian"; MessagesFile: "compiler:Languages\Indonesian.isl"
#endif

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"

[Files]
Source: "{#SourceDir}\*"; DestDir: "{app}"; Excludes: "Start ZenLink.bat,run.bat,Create-Desktop-Shortcut.bat,Uninstall-ZenLink.bat,portable.txt"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{group}\ZenLink"; Filename: "{app}\ZenLink.exe"
Name: "{group}\Uninstall ZenLink"; Filename: "{uninstallexe}"
Name: "{autodesktop}\ZenLink"; Filename: "{app}\ZenLink.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\ZenLink.exe"; Description: "{cm:LaunchProgram,ZenLink}"; Flags: nowait postinstall skipifsilent

[UninstallRun]
Filename: "{sys}\taskkill.exe"; Parameters: "/im ZenLink.exe /f"; Flags: runhidden; RunOnceId: "KillZenLink"

[UninstallDelete]
Type: filesandordirs; Name: "{localappdata}\ZenLink"

[Code]
function InitializeSetup(): Boolean;
var
  Version: String;
begin
  Result := True;
  // ZenLink's window uses Microsoft Edge WebView2 (built into Windows 11).
  if not RegQueryStringValue(HKLM, 'SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}', 'pv', Version) and
     not RegQueryStringValue(HKCU, 'Software\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}', 'pv', Version) then
    MsgBox('ZenLink needs the Microsoft Edge WebView2 Runtime (included in Windows 11). ' +
           'If ZenLink does not open, install it from https://developer.microsoft.com/microsoft-edge/webview2/', mbInformation, MB_OK);
end;
