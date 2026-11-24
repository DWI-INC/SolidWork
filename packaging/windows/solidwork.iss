; Inno Setup script for the SolidWork installer on Windows.
; Built by the release workflow, once per architecture:
;   iscc /DVersion=1.0.0 /DArch=x64 /DSourceDir=target\x86_64-pc-windows-msvc\release /DOutputDir=dist packaging\windows\solidwork.iss
; Arch is x86, x64 or arm64.

#ifndef Version
  #define Version "0.0.0"
#endif
#ifndef Arch
  #define Arch "x64"
#endif
#ifndef SourceDir
  #define SourceDir "..\..\target\release"
#endif
#ifndef OutputDir
  #define OutputDir "..\..\dist"
#endif

[Setup]
AppId={{064A009D-D793-594F-AC04-CD565E9AA2A4}
AppName=SolidWork
AppVersion={#Version}
AppVerName=SolidWork {#Version}
AppPublisher=SolidWork
AppPublisherURL=https://github.com/DWI-INC/SolidWork
AppSupportURL=https://github.com/DWI-INC/SolidWork/issues
AppUpdatesURL=https://github.com/DWI-INC/SolidWork/releases
DefaultDirName={autopf}\SolidWork
DefaultGroupName=SolidWork
DisableProgramGroupPage=yes
LicenseFile=..\..\LICENSE
OutputDir={#OutputDir}
OutputBaseFilename=SolidWork-{#Version}-windows-{#Arch}-setup
SetupIconFile=solidwork.ico
UninstallDisplayIcon={app}\solidwork.exe
UninstallDisplayName=SolidWork {#Version}
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
; installs for all users with admin rights, or just for the current user without them
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ChangesAssociations=yes
CloseApplications=yes
#if Arch == "x64"
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
#elif Arch == "arm64"
ArchitecturesAllowed=arm64
ArchitecturesInstallIn64BitMode=arm64
#endif

[Languages]
Name: "german"; MessagesFile: "compiler:Languages\German.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"
Name: "assoc"; Description: "SolidWork-Dateien (.pfprt, .pfasm, .pfdrw) mit SolidWork öffnen"; GroupDescription: "Dateizuordnungen:"
Name: "assocstep"; Description: "STEP-Dateien (.stp, .step) mit SolidWork öffnen"; GroupDescription: "Dateizuordnungen:"; Flags: unchecked

[Files]
Source: "{#SourceDir}\solidwork.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\solidwork-mcp.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\README.md"; DestDir: "{app}"; Flags: ignoreversion isreadme
Source: "solidwork.ico"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\SolidWork"; Filename: "{app}\solidwork.exe"; IconFilename: "{app}\solidwork.ico"
Name: "{group}\{cm:UninstallProgram,SolidWork}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\SolidWork"; Filename: "{app}\solidwork.exe"; IconFilename: "{app}\solidwork.ico"; Tasks: desktopicon

[Registry]
; one ProgID per file type, opened with solidwork.exe
Root: HKA; Subkey: "Software\Classes\SolidWork.Part"; ValueType: string; ValueData: "SolidWork-Teil"; Flags: uninsdeletekey; Tasks: assoc
Root: HKA; Subkey: "Software\Classes\SolidWork.Part\DefaultIcon"; ValueType: string; ValueData: "{app}\solidwork.ico"; Tasks: assoc
Root: HKA; Subkey: "Software\Classes\SolidWork.Part\shell\open\command"; ValueType: string; ValueData: """{app}\solidwork.exe"" ""%1"""; Tasks: assoc
Root: HKA; Subkey: "Software\Classes\.pfprt\OpenWithProgids"; ValueType: string; ValueName: "SolidWork.Part"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc
Root: HKA; Subkey: "Software\Classes\SolidWork.Assembly"; ValueType: string; ValueData: "SolidWork-Baugruppe"; Flags: uninsdeletekey; Tasks: assoc
Root: HKA; Subkey: "Software\Classes\SolidWork.Assembly\DefaultIcon"; ValueType: string; ValueData: "{app}\solidwork.ico"; Tasks: assoc
Root: HKA; Subkey: "Software\Classes\SolidWork.Assembly\shell\open\command"; ValueType: string; ValueData: """{app}\solidwork.exe"" ""%1"""; Tasks: assoc
Root: HKA; Subkey: "Software\Classes\.pfasm\OpenWithProgids"; ValueType: string; ValueName: "SolidWork.Assembly"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc
Root: HKA; Subkey: "Software\Classes\SolidWork.Drawing"; ValueType: string; ValueData: "SolidWork-Zeichnung"; Flags: uninsdeletekey; Tasks: assoc
Root: HKA; Subkey: "Software\Classes\SolidWork.Drawing\DefaultIcon"; ValueType: string; ValueData: "{app}\solidwork.ico"; Tasks: assoc
Root: HKA; Subkey: "Software\Classes\SolidWork.Drawing\shell\open\command"; ValueType: string; ValueData: """{app}\solidwork.exe"" ""%1"""; Tasks: assoc
Root: HKA; Subkey: "Software\Classes\.pfdrw\OpenWithProgids"; ValueType: string; ValueName: "SolidWork.Drawing"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc
Root: HKA; Subkey: "Software\Classes\SolidWork.Step"; ValueType: string; ValueData: "STEP-Modell"; Flags: uninsdeletekey; Tasks: assocstep
Root: HKA; Subkey: "Software\Classes\SolidWork.Step\DefaultIcon"; ValueType: string; ValueData: "{app}\solidwork.ico"; Tasks: assocstep
Root: HKA; Subkey: "Software\Classes\SolidWork.Step\shell\open\command"; ValueType: string; ValueData: """{app}\solidwork.exe"" ""%1"""; Tasks: assocstep
Root: HKA; Subkey: "Software\Classes\.stp\OpenWithProgids"; ValueType: string; ValueName: "SolidWork.Step"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assocstep
Root: HKA; Subkey: "Software\Classes\.step\OpenWithProgids"; ValueType: string; ValueName: "SolidWork.Step"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assocstep
; SolidWork's own file types open with SolidWork by default
Root: HKA; Subkey: "Software\Classes\.pfprt"; ValueType: string; ValueData: "SolidWork.Part"; Flags: uninsdeletevalue; Tasks: assoc
Root: HKA; Subkey: "Software\Classes\.pfasm"; ValueType: string; ValueData: "SolidWork.Assembly"; Flags: uninsdeletevalue; Tasks: assoc
Root: HKA; Subkey: "Software\Classes\.pfdrw"; ValueType: string; ValueData: "SolidWork.Drawing"; Flags: uninsdeletevalue; Tasks: assoc

[Run]
Filename: "{app}\solidwork.exe"; Description: "{cm:LaunchProgram,SolidWork}"; Flags: nowait postinstall skipifsilent
