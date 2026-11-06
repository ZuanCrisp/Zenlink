; ZenLink installer (Inno Setup 6). Built by build_portable_release.ps1.
#ifndef AppVersion
  #define AppVersion "2.2.3"
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
Name: "firewall"; Description: "Allow ZenLink on the local network (needed for AirPlay over Wi-Fi; asks for administrator permission)"; GroupDescription: "Network:"

[Files]
Source: "{#SourceDir}\*"; DestDir: "{app}"; Excludes: "Start ZenLink.bat,run.bat,Create-Desktop-Shortcut.bat,Uninstall-ZenLink.bat,portable.txt"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{group}\ZenLink"; Filename: "{app}\ZenLink.exe"
Name: "{group}\ZenLink Next (Preview)"; Filename: "{app}\ZenLinkNext.exe"; Check: FileExists(ExpandConstant('{app}\ZenLinkNext.exe'))
Name: "{group}\Uninstall ZenLink"; Filename: "{uninstallexe}"
Name: "{autodesktop}\ZenLink"; Filename: "{app}\ZenLink.exe"; Tasks: desktopicon

[Run]
; AirPlay (ZenLink Studio over Wi-Fi): the iPhone/iPad must reach ZenLink. Inbound allow, local subnet only.
Filename: "{sys}\cmd.exe"; Parameters: "/c netsh advfirewall firewall delete rule name=""ZenLink (AirPlay)"" & netsh advfirewall firewall add rule name=""ZenLink (AirPlay)"" dir=in action=allow program=""{app}\ZenLink.exe"" enable=yes profile=any remoteip=localsubnet"; Flags: runhidden shellexec waituntilterminated; Verb: runas; Tasks: firewall
; Apple Driver mode needs Apple Mobile Device Service (Apple Devices or iTunes).
Filename: "ms-windows-store://pdp/?ProductId=9NP83LWLPZ9K"; Description: "Install Apple Devices from the Microsoft Store (needed for Apple Driver mode)"; Flags: postinstall shellexec skipifsilent unchecked; Check: not AppleServiceInstalled
Filename: "{app}\ZenLink.exe"; Description: "{cm:LaunchProgram,ZenLink}"; Flags: nowait postinstall skipifsilent

[UninstallRun]
Filename: "{sys}\taskkill.exe"; Parameters: "/im ZenLinkNext.exe /f"; Flags: runhidden; RunOnceId: "KillZenLinkNext"
Filename: "{sys}\taskkill.exe"; Parameters: "/im ZenLink.exe /f"; Flags: runhidden; RunOnceId: "KillZenLink"
Filename: "{sys}\netsh.exe"; Parameters: "advfirewall firewall delete rule name=""ZenLink (AirPlay)"""; Flags: runhidden shellexec waituntilterminated; Verb: runas; RunOnceId: "DelFirewall"; Check: FirewallRuleExists

[UninstallDelete]
Type: filesandordirs; Name: "{localappdata}\ZenLink"

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
