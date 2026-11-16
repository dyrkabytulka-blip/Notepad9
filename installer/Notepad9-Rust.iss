; Notepad9 (Rust version) installer (Inno Setup 6)
;
; Build:
;   1) cargo build --release   (in ..\Notepad9Rust); example plugins: cargo build --release in
;      ..\Notepad9Rust\plugins, then copy plugins\target\release\*.dll to target\release\plugins
;   2) "%LOCALAPPDATA%\Programs\Inno Setup 6\ISCC.exe" Notepad9-Rust.iss
; or just run build-installer-rust.cmd next to this file.
;
; One self-contained exe (plus optional example plugins): nothing else is needed on the target PC.

#define AppName      "Notepad9"
#define AppVersion   "2.1.0"
#define AppPublisher "Notepad9 authors"
#define AppExe       "Notepad9.exe"
#define BuildDir     "..\Notepad9Rust\target\release"

[Setup]
AppId={{8316B2A1-EE7B-42AD-99C6-04AFC74B3C91}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher={#AppPublisher}
AppCopyright=Copyright (c) 2026 {#AppPublisher}. MIT License.
VersionInfoVersion={#AppVersion}
DefaultDirName={autopf}\{#AppName}
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
; installs for the current user without admin rights, or for everyone if the user chooses
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0.17763
LicenseFile=..\Notepad9Rust\LICENSE
SetupIconFile=..\Notepad9Rust\assets\app.ico
UninstallDisplayIcon={app}\{#AppExe}
UninstallDisplayName={#AppName}
OutputDir=output
OutputBaseFilename=Notepad9-Setup-{#AppVersion}
Compression=lzma2/ultra64
SolidCompression=yes
; same AppId as 1.x: installing 2.0 replaces the C# version
LZMANumBlockThreads=4
WizardStyle=modern
ChangesAssociations=yes
CloseApplications=yes
RestartApplications=no

[Languages]
Name: "en"; MessagesFile: "compiler:Default.isl"
Name: "ru"; MessagesFile: "compiler:Languages\Russian.isl"

[CustomMessages]
en.ContextMenu=Add "Edit in Notepad9" to the right-click menu of files and folders
ru.ContextMenu=Добавить Notepad9 в меню правой кнопки для файлов и папок
en.OpenWith=Show Notepad9 in "Open with" for text and source files
ru.OpenWith=Показывать Notepad9 в «Открыть с помощью» для текстовых файлов и исходников
en.Integration=Windows integration:
ru.Integration=Интеграция с Windows:
en.EditIn=Edit in Notepad9
ru.EditIn=Edit in Notepad9
en.OpenFolderIn=Open in Notepad9
ru.OpenFolderIn=Открыть в Notepad9
en.LaunchApp=Launch Notepad9
ru.LaunchApp=Запустить Notepad9
en.Plugins=Install example plugins (Word Stats, TODO List, Text Tools)
ru.Plugins=Установить примеры плагинов (Word Stats, TODO List, Text Tools)
en.Extras=Extras:
ru.Extras=Дополнительно:

[Tasks]
Name: "contextmenu"; Description: "{cm:ContextMenu}"; GroupDescription: "{cm:Integration}"
Name: "openwith"; Description: "{cm:OpenWith}"; GroupDescription: "{cm:Integration}"
Name: "plugins"; Description: "{cm:Plugins}"; GroupDescription: "{cm:Extras}"
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[InstallDelete]
; files of the 1.x (.NET) version, removed only when that version is installed here
Type: files; Name: "{app}\*.dll"; Check: IsDotNetVersion
Type: files; Name: "{app}\*.json"; Check: IsDotNetVersion
Type: files; Name: "{app}\*.pdb"; Check: IsDotNetVersion
Type: files; Name: "{app}\createdump.exe"; Check: IsDotNetVersion
Type: filesandordirs; Name: "{app}\runtimes"; Check: IsDotNetVersion
Type: filesandordirs; Name: "{app}\ru"; Check: IsDotNetVersion
Type: filesandordirs; Name: "{app}\en"; Check: IsDotNetVersion

[Files]
Source: "{#BuildDir}\{#AppExe}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\Notepad9Rust\LICENSE"; DestDir: "{app}"; DestName: "LICENSE.txt"; Flags: ignoreversion
Source: "..\Notepad9Rust\THIRD-PARTY-NOTICES.md"; DestDir: "{app}"; Flags: ignoreversion
; plugins: DLLs loaded from {app}\plugins (and %APPDATA%\Notepad9\plugins); the guide sits next to them
Source: "{#BuildDir}\plugins\*.dll"; DestDir: "{app}\plugins"; Flags: ignoreversion; Tasks: plugins
Source: "..\Notepad9Rust\plugin-sdk\README.md"; DestDir: "{app}\plugins"; DestName: "README-plugins.md"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\{#AppExe}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExe}"; Tasks: desktopicon

[Registry]
; ---- "Edit in Notepad9" for every file type, binaries included ("*" = all files)
Root: HKA; Subkey: "Software\Classes\*\shell\Notepad9"; ValueType: string; ValueName: ""; ValueData: "{cm:EditIn}"; Flags: uninsdeletekey; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\*\shell\Notepad9"; ValueType: string; ValueName: "Icon"; ValueData: """{app}\{#AppExe}"",0"; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\*\shell\Notepad9\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#AppExe}"" ""%1"""; Tasks: contextmenu
; folders: open the folder in the sidebar
Root: HKA; Subkey: "Software\Classes\Directory\shell\Notepad9"; ValueType: string; ValueName: ""; ValueData: "{cm:OpenFolderIn}"; Flags: uninsdeletekey; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\Directory\shell\Notepad9"; ValueType: string; ValueName: "Icon"; ValueData: """{app}\{#AppExe}"",0"; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\Directory\shell\Notepad9\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#AppExe}"" ""%1"""; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\Directory\Background\shell\Notepad9"; ValueType: string; ValueName: ""; ValueData: "{cm:OpenFolderIn}"; Flags: uninsdeletekey; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\Directory\Background\shell\Notepad9"; ValueType: string; ValueName: "Icon"; ValueData: """{app}\{#AppExe}"",0"; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\Directory\Background\shell\Notepad9\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#AppExe}"" ""%V"""; Tasks: contextmenu

; ---- the 1.x version could start hidden at sign-in; 2.0 does not need it
Root: HKA; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: none; ValueName: "Notepad9"; Flags: deletevalue

; ---- Notepad9 as an application Windows knows about (Open with, Run dialog, "notepad9" in a console)
Root: HKA; Subkey: "Software\Microsoft\Windows\CurrentVersion\App Paths\{#AppExe}"; ValueType: string; ValueName: ""; ValueData: "{app}\{#AppExe}"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Microsoft\Windows\CurrentVersion\App Paths\{#AppExe}"; ValueType: string; ValueName: "Path"; ValueData: "{app}"
Root: HKA; Subkey: "Software\Classes\Applications\{#AppExe}"; ValueType: string; ValueName: "FriendlyAppName"; ValueData: "{#AppName}"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\Applications\{#AppExe}\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: """{app}\{#AppExe}"",0"
Root: HKA; Subkey: "Software\Classes\Applications\{#AppExe}\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#AppExe}"" ""%1"""

; ---- file type used by "Open with" (does not take over the default program)
Root: HKA; Subkey: "Software\Classes\Notepad9.Document"; ValueType: string; ValueName: ""; ValueData: "Text Document (Notepad9)"; Flags: uninsdeletekey; Tasks: openwith
Root: HKA; Subkey: "Software\Classes\Notepad9.Document\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: """{app}\{#AppExe}"",0"; Tasks: openwith
Root: HKA; Subkey: "Software\Classes\Notepad9.Document\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#AppExe}"" ""%1"""; Tasks: openwith
Root: HKA; Subkey: "Software\Classes\.txt\OpenWithProgids"; ValueType: none; ValueName: "Notepad9.Document"; Flags: uninsdeletevalue; Tasks: openwith
Root: HKA; Subkey: "Software\Classes\.md\OpenWithProgids"; ValueType: none; ValueName: "Notepad9.Document"; Flags: uninsdeletevalue; Tasks: openwith
Root: HKA; Subkey: "Software\Classes\.markdown\OpenWithProgids"; ValueType: none; ValueName: "Notepad9.Document"; Flags: uninsdeletevalue; Tasks: openwith
Root: HKA; Subkey: "Software\Classes\.log\OpenWithProgids"; ValueType: none; ValueName: "Notepad9.Document"; Flags: uninsdeletevalue; Tasks: openwith
Root: HKA; Subkey: "Software\Classes\.ini\OpenWithProgids"; ValueType: none; ValueName: "Notepad9.Document"; Flags: uninsdeletevalue; Tasks: openwith
Root: HKA; Subkey: "Software\Classes\.cfg\OpenWithProgids"; ValueType: none; ValueName: "Notepad9.Document"; Flags: uninsdeletevalue; Tasks: openwith
Root: HKA; Subkey: "Software\Classes\.json\OpenWithProgids"; ValueType: none; ValueName: "Notepad9.Document"; Flags: uninsdeletevalue; Tasks: openwith
Root: HKA; Subkey: "Software\Classes\.xml\OpenWithProgids"; ValueType: none; ValueName: "Notepad9.Document"; Flags: uninsdeletevalue; Tasks: openwith
Root: HKA; Subkey: "Software\Classes\.yml\OpenWithProgids"; ValueType: none; ValueName: "Notepad9.Document"; Flags: uninsdeletevalue; Tasks: openwith
Root: HKA; Subkey: "Software\Classes\.yaml\OpenWithProgids"; ValueType: none; ValueName: "Notepad9.Document"; Flags: uninsdeletevalue; Tasks: openwith
Root: HKA; Subkey: "Software\Classes\.csv\OpenWithProgids"; ValueType: none; ValueName: "Notepad9.Document"; Flags: uninsdeletevalue; Tasks: openwith
Root: HKA; Subkey: "Software\Classes\.cs\OpenWithProgids"; ValueType: none; ValueName: "Notepad9.Document"; Flags: uninsdeletevalue; Tasks: openwith
Root: HKA; Subkey: "Software\Classes\.py\OpenWithProgids"; ValueType: none; ValueName: "Notepad9.Document"; Flags: uninsdeletevalue; Tasks: openwith
Root: HKA; Subkey: "Software\Classes\.js\OpenWithProgids"; ValueType: none; ValueName: "Notepad9.Document"; Flags: uninsdeletevalue; Tasks: openwith
Root: HKA; Subkey: "Software\Classes\.ts\OpenWithProgids"; ValueType: none; ValueName: "Notepad9.Document"; Flags: uninsdeletevalue; Tasks: openwith
Root: HKA; Subkey: "Software\Classes\.html\OpenWithProgids"; ValueType: none; ValueName: "Notepad9.Document"; Flags: uninsdeletevalue; Tasks: openwith
Root: HKA; Subkey: "Software\Classes\.css\OpenWithProgids"; ValueType: none; ValueName: "Notepad9.Document"; Flags: uninsdeletevalue; Tasks: openwith
Root: HKA; Subkey: "Software\Classes\.bat\OpenWithProgids"; ValueType: none; ValueName: "Notepad9.Document"; Flags: uninsdeletevalue; Tasks: openwith
Root: HKA; Subkey: "Software\Classes\.cmd\OpenWithProgids"; ValueType: none; ValueName: "Notepad9.Document"; Flags: uninsdeletevalue; Tasks: openwith
Root: HKA; Subkey: "Software\Classes\.ps1\OpenWithProgids"; ValueType: none; ValueName: "Notepad9.Document"; Flags: uninsdeletevalue; Tasks: openwith
Root: HKA; Subkey: "Software\Classes\.c\OpenWithProgids"; ValueType: none; ValueName: "Notepad9.Document"; Flags: uninsdeletevalue; Tasks: openwith
Root: HKA; Subkey: "Software\Classes\.cpp\OpenWithProgids"; ValueType: none; ValueName: "Notepad9.Document"; Flags: uninsdeletevalue; Tasks: openwith
Root: HKA; Subkey: "Software\Classes\.h\OpenWithProgids"; ValueType: none; ValueName: "Notepad9.Document"; Flags: uninsdeletevalue; Tasks: openwith
Root: HKA; Subkey: "Software\Classes\.java\OpenWithProgids"; ValueType: none; ValueName: "Notepad9.Document"; Flags: uninsdeletevalue; Tasks: openwith
Root: HKA; Subkey: "Software\Classes\.sql\OpenWithProgids"; ValueType: none; ValueName: "Notepad9.Document"; Flags: uninsdeletevalue; Tasks: openwith

[UninstallRun]
; stop a running Notepad9 (its session with unsaved text is already on disk)
Filename: "{sys}\taskkill.exe"; Parameters: "/im {#AppExe} /f"; Flags: runhidden; RunOnceId: "StopNotepad9"

[Run]
Filename: "{app}\{#AppExe}"; Description: "{cm:LaunchApp}"; Flags: nowait postinstall skipifsilent

[Code]
// The 1.x version was a .NET app: Notepad9.dll next to Notepad9.exe.
function IsDotNetVersion(): Boolean;
begin
  Result := FileExists(ExpandConstant('{app}\Notepad9.dll'));
end;
