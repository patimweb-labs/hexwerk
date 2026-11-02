; Inno Setup fuer Hexwerk.
; Gebaut vom Release-Workflow:  iscc /DAppVersion=0.4.0 /DSourceDir=<dir> installer.iss

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
#ifndef SourceDir
  #define SourceDir "..\..\crates\desktop\target\release"
#endif

[Setup]
AppId={{9E4A7C21-6D3B-4F58-8A14-2C7E5B9D3F60}
AppName=Hexwerk
AppVersion={#AppVersion}
AppVerName=Hexwerk {#AppVersion}
AppPublisher=Patrick Weppelmann
AppPublisherURL=https://github.com/Smotherer007/hexwerk
AppSupportURL=https://github.com/Smotherer007/hexwerk/issues
AppCopyright=Copyright (C) 2026 Patrick Weppelmann
; {autopf} ist unter 64-Bit-Systemen "Program Files", unter 32-Bit "Program Files (x86)".
DefaultDirName={autopf}\Hexwerk
DefaultGroupName=Hexwerk
DisableProgramGroupPage=yes
LicenseFile=..\..\LICENSE
OutputDir={#SourceDir}\..\..\..\dist
OutputBaseFilename=hexwerk-{#AppVersion}-windows-x86_64-setup
SetupIconFile=..\..\assets\windows\hexwerk.ico
UninstallDisplayIcon={app}\hexwerk-desktop.exe
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
; Nur auf 64-Bit-Systemen installieren: das Programm ist x86_64.
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible

[Languages]
Name: "de"; MessagesFile: "compiler:Languages\German.isl"
Name: "en"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#SourceDir}\hexwerk-desktop.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\hexwerk.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\README.md"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\Hexwerk"; Filename: "{app}\hexwerk-desktop.exe"
Name: "{autodesktop}\Hexwerk"; Filename: "{app}\hexwerk-desktop.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\hexwerk-desktop.exe"; Description: "{cm:LaunchProgram,Hexwerk}"; Flags: nowait postinstall skipifsilent
