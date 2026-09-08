; Inno Setup Script para CastScreen
; Genera el instalador profesional CastScreen_Setup.exe para Windows (x64)

#define MyAppName "CastScreen"
#define MyAppVersion "0.2.0"
#define MyAppPublisher "gabjesus15"
#define MyAppURL "https://github.com/gabjesus15/CastScreen"
#define MyAppExeName "CastScreen.exe"

[Setup]
; Identificador único de la aplicación (usado para detectar versiones previas)
AppId={{D81E9A4B-3F5B-4C92-A23B-9876543210AB}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}/releases
VersionInfoVersion={#MyAppVersion}

; Detección y actualización automática de versiones anteriores
UsePreviousAppDir=yes
UsePreviousGroup=yes
DisableDirPage=auto
CloseApplications=yes
CloseApplicationsFilter=CastScreen.exe,castscreen-sender.exe,castscreen-receiver.exe
RestartApplications=no

; Directorio por defecto e iconos
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

; Identidad visual del instalador y desinstalador
SetupIconFile=..\assets\icon.ico
UninstallDisplayName={#MyAppName}
UninstallDisplayIcon={app}\assets\icon.ico
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

; Recursos gráficos e iconos del sistema
Source: "..\assets\*"; DestDir: "{app}\assets"; Flags: ignoreversion recursesubdirs createallsubdirs

; Documentación y licencia
Source: "..\README.md"; DestDir: "{app}"; Flags: isreadme
Source: "..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\docs\*"; DestDir: "{app}\docs"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
; Lanzador general en Menú Inicio con icono oficial
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; IconFilename: "{app}\assets\icon.ico"
; Acceso directo a Emisor (PC Gaming) con icono oficial
Name: "{group}\{#MyAppName} - Emisor (PC Gaming)"; Filename: "{app}\castscreen-sender.exe"; IconFilename: "{app}\assets\icon.ico"
; Acceso directo a Receptor (Laptop Stream) con icono oficial
Name: "{group}\{#MyAppName} - Receptor (Laptop)"; Filename: "{app}\castscreen-receiver.exe"; IconFilename: "{app}\assets\icon.ico"
; Acceso directo al Desinstalador
Name: "{group}\{cm:UninstallProgram,{#MyAppName}}"; Filename: "{uninstallexe}"; IconFilename: "{app}\assets\icon.ico"

; Acceso directo en el Escritorio con icono oficial
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon; IconFilename: "{app}\assets\icon.ico"

[Run]
; Opción de ejecutar CastScreen al terminar la instalación
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#StringChange(MyAppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent
