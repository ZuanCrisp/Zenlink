; ZenLink v2.5.1 BETA installer (Inno Setup 6). Built by build_portable_release.ps1.
; Beta identity: its own AppId, "ZenLink Beta" name, {autopf}\ZenLink Beta folder and
; %LOCALAPPDATA%\ZenLink-Beta data, so it installs next to v2.5.0 and never
; replaces it. For the final v2.5.1 release, restore the v2.2.x/v2.5.0 AppId,
; AppName=ZenLink and {autopf}\ZenLink so Setup upgrades in place (AGENTS.md section 3).
#ifndef AppVersion
  #define AppVersion "2.5.1-beta.1"
#endif
#ifndef AppVersionNum
  ; Numeric part only: VersionInfoVersion must be n.n.n.n.
  #define AppVersionNum "2.5.1"
#endif
#ifndef SourceDir
  #define SourceDir "..\dist\ZenLink"
#endif

[Setup]
AppId={{9C4F2B71-3E8A-4D16-B5C0-7A2E91F4D3B8}
AppName=ZenLink Beta
AppVersion={#AppVersion}
AppVerName=ZenLink Beta {#AppVersion}
AppPublisher=Zenvy Zenshiro
AppPublisherURL=https://github.com/zenvyzenshiro/zenlink
AppSupportURL=https://github.com/zenvyzenshiro/zenlink/issues
AppCopyright=Copyright (C) 2026 Zenvy Zenshiro
DefaultDirName={autopf}\ZenLink Beta
DefaultGroupName=ZenLink Beta
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
; Setup writes its own log; it is copied to the app's log folder at the end.
SetupLogging=yes
VersionInfoCompany=Zenvy Zenshiro
VersionInfoDescription=ZenLink Setup
VersionInfoVersion={#AppVersionNum}

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
#if FileExists(AddBackslash(CompilerPath) + "Languages\Indonesian.isl")
Name: "indonesian"; MessagesFile: "compiler:Languages\Indonesian.isl"
#endif

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"
Name: "firewall"; Description: "Allow ZenLink on the local network (needed for AirPlay over Wi-Fi; asks for administrator permission)"; GroupDescription: "Network:"

[InstallDelete]
; An upgrade from v2.2.x (its libraries sit directly in {app}): remove what
; v2.5 no longer ships, measured by comparing both build folders.
Type: filesandordirs; Name: "{app}\av"
Type: filesandordirs; Name: "{app}\av.libs"
Type: filesandordirs; Name: "{app}\PIL"
Type: filesandordirs; Name: "{app}\webview"
Type: filesandordirs; Name: "{app}\pythonnet"
Type: filesandordirs; Name: "{app}\clr_loader"
Type: filesandordirs; Name: "{app}\chardet"
Type: filesandordirs; Name: "{app}\pydantic_core"
Type: filesandordirs; Name: "{app}\setuptools"
Type: filesandordirs; Name: "{app}\ui"
Type: filesandordirs; Name: "{app}\pywebview-*.dist-info"
Type: filesandordirs; Name: "{app}\typer-*.dist-info"
Type: filesandordirs; Name: "{app}\sounddevice-*.dist-info"
Type: files; Name: "{app}\sqlite3.dll"
Type: files; Name: "{app}\_sqlite3.pyd"
Type: files; Name: "{app}\_sounddevice_data\portaudio-binaries\*"
Type: filesandordirs; Name: "{app}\_sounddevice_data\portaudio-binaries\.github"
Type: files; Name: "{app}\_sounddevice_data\__init__.py"

[Files]
Source: "{#SourceDir}\*"; DestDir: "{app}"; Excludes: "Start ZenLink.bat,run.bat,Create-Desktop-Shortcut.bat,Uninstall-ZenLink.bat,portable.txt"; Flags: ignoreversion recursesubdirs createallsubdirs

[Dirs]
Name: "{localappdata}\ZenLink-Beta\logs"

[Icons]
Name: "{group}\ZenLink Beta"; Filename: "{app}\ZenLink.exe"
Name: "{group}\Uninstall ZenLink Beta"; Filename: "{uninstallexe}"
; The log folder (window, backend, crash and setup logs) for bug reports.
Name: "{group}\ZenLink Beta logs"; Filename: "{localappdata}\ZenLink-Beta\logs"
Name: "{autodesktop}\ZenLink Beta"; Filename: "{app}\ZenLink.exe"; Tasks: desktopicon

[Run]
; AirPlay (ZenLink Studio over Wi-Fi): the iPhone/iPad must reach ZenLink. Inbound allow, local subnet only.
Filename: "{sys}\cmd.exe"; Parameters: "/c netsh advfirewall firewall delete rule name=""ZenLink (AirPlay)"" program=""{app}\ZenLinkService.exe"" & netsh advfirewall firewall add rule name=""ZenLink (AirPlay)"" dir=in action=allow program=""{app}\ZenLinkService.exe"" enable=yes profile=any remoteip=localsubnet"; Flags: runhidden shellexec waituntilterminated; Verb: runas; Tasks: firewall
; Apple Driver mode needs Apple Mobile Device Service (Apple Devices or iTunes).
Filename: "ms-windows-store://pdp/?ProductId=9NP83LWLPZ9K"; Description: "Install Apple Devices from the Microsoft Store (needed for Apple Driver mode)"; Flags: postinstall shellexec skipifsilent unchecked; Check: not AppleServiceInstalled
Filename: "{app}\ZenLink.exe"; Description: "{cm:LaunchProgram,ZenLink}"; Flags: nowait postinstall skipifsilent
; In-app update (v2.5.1): ZenLink runs this Setup with /SILENT /ZLRELAUNCH=1 and
; quits; start the new version when the silent install is done.
Filename: "{app}\ZenLink.exe"; Flags: nowait; Check: RelaunchAfterUpdate

[UninstallRun]
; Stop only the processes started from this install folder: a portable ZenLink uses the same exe names.
Filename: "{sys}\WindowsPowerShell\v1.0\powershell.exe"; Parameters: "-NoProfile -Command ""Get-Process ZenLink,ZenLinkService -ErrorAction SilentlyContinue | Where-Object {{ $_.Path -like '{app}\*' } | Stop-Process -Force"""; Flags: runhidden; RunOnceId: "StopZenLink"
Filename: "{sys}\netsh.exe"; Parameters: "advfirewall firewall delete rule name=""ZenLink (AirPlay)"" program=""{app}\ZenLinkService.exe"""; Flags: runhidden shellexec waituntilterminated; Verb: runas; RunOnceId: "DelFirewall"; Check: FirewallRuleExists

[UninstallDelete]
Type: filesandordirs; Name: "{localappdata}\ZenLink-Beta"

[Registry]
; "Start with Windows" (Settings) writes this value itself; remove it on uninstall.
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: none; ValueName: "ZenLink Beta"; Flags: dontcreatekey uninsdeletevalue

[Code]
// Only ask for administrator rights on uninstall when the AirPlay rule exists.
function FirewallRuleExists(): Boolean;
var
  Names: TArrayOfString;
  I: Integer;
  Value: String;
begin
  Result := False;
  if RegGetValueNames(HKLM, 'SYSTEM\CurrentControlSet\Services\SharedAccess\Parameters\FirewallPolicy\FirewallRules', Names) then
    for I := 0 to GetArrayLength(Names) - 1 do
      if RegQueryStringValue(HKLM, 'SYSTEM\CurrentControlSet\Services\SharedAccess\Parameters\FirewallPolicy\FirewallRules', Names[I], Value) then
        if Pos('|Name=ZenLink (AirPlay)|', Value) > 0 then
        begin
          Result := True;
          Exit;
        end;
end;

function AppleServiceInstalled(): Boolean;
var
  ImagePath: String;
begin
  Result := RegQueryStringValue(HKLM, 'SYSTEM\CurrentControlSet\Services\Apple Mobile Device Service', 'ImagePath', ImagePath);
end;

function RelaunchAfterUpdate(): Boolean;
begin
  Result := WizardSilent() and (ExpandConstant('{param:ZLRELAUNCH|0}') = '1');
end;

// Clean update (v2.5.1): install-manifest.txt lists the top-level files and
// folders the previous ZenLink installed in {app}. Remove exactly those before
// the new files are copied, so nothing a newer version no longer ships stays
// behind. Nothing else in {app} is touched (a user may have picked a folder
// with other files), and settings live in %APPDATA% / %LOCALAPPDATA%.
procedure RemovePreviousFiles();
var
  Lines: TArrayOfString;
  I: Integer;
  Name, Path: String;
begin
  if not LoadStringsFromFile(ExpandConstant('{app}\install-manifest.txt'), Lines) then
    Exit;
  for I := 0 to GetArrayLength(Lines) - 1 do
  begin
    Name := Trim(Lines[I]);
    if (Name = '') or (Name = '.') or (Pos('..', Name) > 0) or (Pos('', Name) > 0) or (Pos('/', Name) > 0) or (Pos(':', Name) > 0) then
      Continue;
    if CompareText(Copy(Name, 1, 5), 'unins') = 0 then
      Continue;
    Path := ExpandConstant('{app}') + Name;
    if DirExists(Path) then
      DelTree(Path, True, True, True)
    else if FileExists(Path) then
      DeleteFile(Path);
  end;
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssInstall then
    RemovePreviousFiles();
  if CurStep = ssDone then
    // Keep the setup log next to the app's own logs (best effort).
    FileCopy(ExpandConstant('{log}'), ExpandConstant('{localappdata}\ZenLink-Beta\logs\setup.log'), False);
end;

function InitializeSetup(): Boolean;
begin
  // No prerequisite check: the Rust window does not use Microsoft Edge
  // WebView2 (only the old classic window did). See B-6.
  Result := True;
end;
