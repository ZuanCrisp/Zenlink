; ZenLink Beta installer (Inno Setup 6). Built by build_portable_release.ps1.
; Separate identity from the stable ZenLink (AppId, name, folders, firewall rule),
; so the beta installs and uninstalls next to it without touching it.
#ifndef AppVersion
  #define AppVersion "2.5.0-beta.1"
#endif
#ifndef AppVersionNum
  ; Numeric part only: VersionInfoVersion must be n.n.n.n.
  #define AppVersionNum "2.5.0"
#endif
#ifndef SourceDir
  #define SourceDir "..\dist\ZenLink"
#endif

[Setup]
AppId={{5AE777B3-F756-4FAC-B6E7-A2125D89D76D}
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
VersionInfoCompany=Zenvy Zenshiro
VersionInfoDescription=ZenLink Beta Setup
VersionInfoVersion={#AppVersionNum}

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
#if FileExists(AddBackslash(CompilerPath) + "Languages\Indonesian.isl")
Name: "indonesian"; MessagesFile: "compiler:Languages\Indonesian.isl"
#endif

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"
Name: "firewall"; Description: "Allow ZenLink Beta on the local network (needed for AirPlay over Wi-Fi; asks for administrator permission)"; GroupDescription: "Network:"

[Files]
Source: "{#SourceDir}\*"; DestDir: "{app}"; Excludes: "Start ZenLink.bat,run.bat,Create-Desktop-Shortcut.bat,Uninstall-ZenLink.bat,portable.txt"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{group}\ZenLink Beta"; Filename: "{app}\ZenLink.exe"
Name: "{group}\Uninstall ZenLink Beta"; Filename: "{uninstallexe}"
Name: "{autodesktop}\ZenLink Beta"; Filename: "{app}\ZenLink.exe"; Tasks: desktopicon

[Run]
; AirPlay (ZenLink Studio over Wi-Fi): the iPhone/iPad must reach ZenLink. Inbound allow, local subnet only.
Filename: "{sys}\cmd.exe"; Parameters: "/c netsh advfirewall firewall delete rule name=""ZenLink Beta (AirPlay)"" & netsh advfirewall firewall add rule name=""ZenLink Beta (AirPlay)"" dir=in action=allow program=""{app}\ZenLinkService.exe"" enable=yes profile=any remoteip=localsubnet"; Flags: runhidden shellexec waituntilterminated; Verb: runas; Tasks: firewall
; Apple Driver mode needs Apple Mobile Device Service (Apple Devices or iTunes).
Filename: "ms-windows-store://pdp/?ProductId=9NP83LWLPZ9K"; Description: "Install Apple Devices from the Microsoft Store (needed for Apple Driver mode)"; Flags: postinstall shellexec skipifsilent unchecked; Check: not AppleServiceInstalled
Filename: "{app}\ZenLink.exe"; Description: "{cm:LaunchProgram,ZenLink Beta}"; Flags: nowait postinstall skipifsilent

[UninstallRun]
; Stop only the processes started from this install folder: the stable ZenLink uses the same exe names.
Filename: "{sys}\WindowsPowerShell\v1.0\powershell.exe"; Parameters: "-NoProfile -Command ""Get-Process ZenLink,ZenLinkService -ErrorAction SilentlyContinue | Where-Object {{ $_.Path -like '{app}\*' } | Stop-Process -Force"""; Flags: runhidden; RunOnceId: "StopZenLinkBeta"
Filename: "{sys}\netsh.exe"; Parameters: "advfirewall firewall delete rule name=""ZenLink Beta (AirPlay)"""; Flags: runhidden shellexec waituntilterminated; Verb: runas; RunOnceId: "DelFirewall"; Check: FirewallRuleExists

[UninstallDelete]
Type: filesandordirs; Name: "{localappdata}\ZenLink-Beta"

[Code]
// Only ask for administrator rights on uninstall when the beta's AirPlay rule exists.
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
        if Pos('|Name=ZenLink Beta (AirPlay)|', Value) > 0 then
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
