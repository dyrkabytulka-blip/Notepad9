; Notepad9 installer (Inno Setup 6)
;
; Build:
;   1) dotnet publish ..\Notepad9\Notepad9.csproj -c Release -r win-x64 --self-contained true -o ..\publish
;   2) "%LOCALAPPDATA%\Programs\Inno Setup 6\ISCC.exe" Notepad9.iss
; or just run build-installer.cmd next to this file.
;
; The app is published self-contained: the target PC does not need .NET installed.

#define AppName      "Notepad9"
#define AppVersion   "1.0.0"
#define AppPublisher "Notepad9 authors"
#define AppExe       "Notepad9.exe"
#define PublishDir   "..\publish"

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
LicenseFile=..\LICENSE
SetupIconFile=..\Notepad9\Assets\app.ico
UninstallDisplayIcon={app}\{#AppExe}
UninstallDisplayName={#AppName}
OutputDir=output
OutputBaseFilename=Notepad9-Setup-{#AppVersion}
Compression=lzma2/ultra64
SolidCompression=yes
LZMANumBlockThreads=4
WizardStyle=modern
ChangesAssociations=yes
CloseApplications=yes
RestartApplications=no

[Languages]
Name: "en"; MessagesFile: "compiler:Default.isl"
Name: "ru"; MessagesFile: "compiler:Languages\Russian.isl"

[CustomMessages]
en.ContextMenu=Add "Edit in Notepad9" to the right-click menu of every file
ru.ContextMenu=Добавить «Edit in Notepad9» в меню правой кнопки для любых файлов
en.OpenWith=Show Notepad9 in "Open with" for text and source files
ru.OpenWith=Показывать Notepad9 в «Открыть с помощью» для текстовых файлов и исходников
en.Integration=Windows integration:
ru.Integration=Интеграция с Windows:
en.EditIn=Edit in Notepad9
ru.EditIn=Edit in Notepad9
en.AutoStart=Start Notepad9 in the background when I sign in (files then open instantly)
ru.AutoStart=Запускать Notepad9 в фоне при входе в Windows (файлы будут открываться мгновенно)
en.LaunchApp=Launch Notepad9
ru.LaunchApp=Запустить Notepad9

[Tasks]
Name: "contextmenu"; Description: "{cm:ContextMenu}"; GroupDescription: "{cm:Integration}"
Name: "openwith"; Description: "{cm:OpenWith}"; GroupDescription: "{cm:Integration}"
Name: "autostart"; Description: "{cm:AutoStart}"; GroupDescription: "{cm:Integration}"
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#PublishDir}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "..\LICENSE"; DestDir: "{app}"; DestName: "LICENSE.txt"; Flags: ignoreversion
Source: "..\THIRD-PARTY-NOTICES.md"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\{#AppExe}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExe}"; Tasks: desktopicon

[Registry]
; ---- "Edit in Notepad9" for every file type, binaries included ("*" = all files)
Root: HKA; Subkey: "Software\Classes\*\shell\Notepad9"; ValueType: string; ValueName: ""; ValueData: "{cm:EditIn}"; Flags: uninsdeletekey; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\*\shell\Notepad9"; ValueType: string; ValueName: "Icon"; ValueData: """{app}\{#AppExe}"",0"; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\*\shell\Notepad9\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#AppExe}"" ""%1"""; Tasks: contextmenu

; ---- start hidden at sign-in, so the first window opens instantly
Root: HKA; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "Notepad9"; ValueData: """{app}\{#AppExe}"" /background"; Flags: uninsdeletevalue; Tasks: autostart

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
; the background copy keeps its session on disk, so stopping it loses nothing
Filename: "{sys}\taskkill.exe"; Parameters: "/im {#AppExe} /f"; Flags: runhidden; RunOnceId: "StopNotepad9"

[Run]
Filename: "{app}\{#AppExe}"; Description: "{cm:LaunchApp}"; Flags: nowait postinstall skipifsilent
