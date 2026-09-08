; Inno Setup Script para CastScreen
; Genera el instalador profesional CastScreen_Setup.exe para Windows (x64)

#define MyAppName "CastScreen"
#define MyAppVersion "0.1.0"
#define MyAppPublisher "gabjesus15"
#define MyAppURL "https://github.com/gabjesus15/CastScreen"
#define MyAppExeName "CastScreen.exe"

[Setup]
; Identificador de la aplicación
AppId={{D81E9A4B-3F5B-4C92-A23B-9876543210AB}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}/releases
DefaultDirName={autopf}\{#MyAppName}
DefaultGroupName={#MyAppName}
DisableProgramGroupPage=yes
LicenseFile=..\LICENSE
OutputDir=..\dist
OutputBaseFilename=CastScreen_Setup
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
ArchitecturesInstallIn64BitMode=x64
ArchitecturesAllowed=x64
UninstallDisplayIcon={app}\{#MyAppExeName}
PrivilegesRequired=lowest

[Languages]
Name: "spanish"; MessagesFile: "compiler:Languages\Spanish.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"

[Files]
; Binarios principales compilados
Source: "..\target\release\CastScreen.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\target\release\castscreen-sender.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\target\release\castscreen-receiver.exe"; DestDir: "{app}"; Flags: ignoreversion

; Documentación y licencia
Source: "..\README.md"; DestDir: "{app}"; Flags: isreadme
Source: "..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\docs\*"; DestDir: "{app}\docs"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
; Lanzador general
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"
; Acceso directo directo a Emisor (PC Gaming)
Name: "{group}\{#MyAppName} - Emisor (PC Gaming)"; Filename: "{app}\castscreen-sender.exe"
; Acceso directo directo a Receptor (Laptop Stream)
Name: "{group}\{#MyAppName} - Receptor (Laptop)"; Filename: "{app}\castscreen-receiver.exe"
; Desinstalador
Name: "{group}\{cm:UninstallProgram,{#MyAppName}}"; Filename: "{uninstallexe}"

; Acceso directo en el Escritorio
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon

[Run]
; Opción de ejecutar CastScreen al terminar la instalación
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#StringChange(MyAppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent
