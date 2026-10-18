; Windows installer for LVGLab (Inno Setup 6).
;
;   iscc /DAppVersion=0.5.0 /DSourceDir=..\target\release /DOutputDir=..\dist installer\lvglab.iss
;
; SourceDir must contain lvglab.exe. The installer installs for all users
; (Program Files, needs administrator rights) or, if chosen, for the current
; user only.

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
#ifndef SourceDir
  #define SourceDir "..\target\release"
#endif
#ifndef OutputDir
  #define OutputDir "..\dist"
#endif

#define AppName "LVGLab"
#define AppExe "lvglab.exe"
#define AppUrl "https://github.com/bluecraank/lvglab"

[Setup]
AppId={{FB82365D-BD6C-4420-9CCA-BEE17958D28A}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher=blueCraanK
AppPublisherURL={#AppUrl}
AppSupportURL={#AppUrl}/issues
AppUpdatesURL={#AppUrl}/releases
DefaultDirName={autopf}\{#AppName}
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
LicenseFile=..\LICENSE
SetupIconFile=..\crates\app\assets\icon.ico
UninstallDisplayIcon={app}\{#AppExe}
UninstallDisplayName={#AppName}
OutputDir={#OutputDir}
OutputBaseFilename=LVGLab-{#AppVersion}-windows-x64-setup
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequired=admin
PrivilegesRequiredOverridesAllowed=dialog
ChangesAssociations=yes
CloseApplications=yes
VersionInfoVersion={#AppVersion}
VersionInfoProductName={#AppName}
VersionInfoDescription={#AppName} Setup

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "german"; MessagesFile: "compiler:Languages\German.isl"
Name: "spanish"; MessagesFile: "compiler:Languages\Spanish.isl"
Name: "french"; MessagesFile: "compiler:Languages\French.isl"

[CustomMessages]
english.AssociateFiles=Open .lvglab projects with {#AppName}
german.AssociateFiles=.lvglab-Projekte mit {#AppName} öffnen
spanish.AssociateFiles=Abrir proyectos .lvglab con {#AppName}
french.AssociateFiles=Ouvrir les projets .lvglab avec {#AppName}
english.ProjectFile={#AppName} project
german.ProjectFile={#AppName}-Projekt
spanish.ProjectFile=Proyecto de {#AppName}
french.ProjectFile=Projet {#AppName}
english.Example=Example project
german.Example=Beispielprojekt
spanish.Example=Proyecto de ejemplo
french.Example=Projet d'exemple

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
Name: "associate"; Description: "{cm:AssociateFiles}"; GroupDescription: "{cm:AdditionalIcons}"

[Files]
Source: "{#SourceDir}\{#AppExe}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\examples\showcase\*"; DestDir: "{app}\examples\showcase"; Flags: ignoreversion recursesubdirs

[Icons]
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\{#AppExe}"
Name: "{autoprograms}\{#AppName} - {cm:Example}"; Filename: "{app}\{#AppExe}"; Parameters: """{app}\examples\showcase\showcase.lvglab"""
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExe}"; Tasks: desktopicon

[Registry]
; .lvglab files open in LVGLab (per user or machine, following the install mode).
Root: HKA; Subkey: "Software\Classes\.lvglab"; ValueType: string; ValueName: ""; ValueData: "LVGLab.Project"; Flags: uninsdeletevalue; Tasks: associate
Root: HKA; Subkey: "Software\Classes\.lvglab\OpenWithProgids"; ValueType: string; ValueName: "LVGLab.Project"; ValueData: ""; Flags: uninsdeletevalue; Tasks: associate
Root: HKA; Subkey: "Software\Classes\LVGLab.Project"; ValueType: string; ValueName: ""; ValueData: "{cm:ProjectFile}"; Flags: uninsdeletekey; Tasks: associate
Root: HKA; Subkey: "Software\Classes\LVGLab.Project\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\{#AppExe},0"; Tasks: associate
Root: HKA; Subkey: "Software\Classes\LVGLab.Project\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#AppExe}"" ""%1"""; Tasks: associate
; SquareLine projects can be opened with LVGLab too ("Open with").
Root: HKA; Subkey: "Software\Classes\.spj\OpenWithProgids"; ValueType: string; ValueName: "LVGLab.Project"; ValueData: ""; Flags: uninsdeletevalue; Tasks: associate
Root: HKA; Subkey: "Software\Classes\Applications\{#AppExe}\SupportedTypes"; ValueType: string; ValueName: ".lvglab"; ValueData: ""; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\Applications\{#AppExe}\SupportedTypes"; ValueType: string; ValueName: ".spj"; ValueData: ""

[Run]
Filename: "{app}\{#AppExe}"; Description: "{cm:LaunchProgram,{#AppName}}"; Flags: nowait postinstall skipifsilent
