; Lyapunov installer for Windows (Inno Setup 6). CI builds it with:
;   iscc /DAppVersion=0.1.0 packaging\windows\lyapunov.iss
; dist\Lyapunov (SourceDir) holds lyapunov.exe, lyapunov-desktop.exe and the Qt files from windeployqt.
; Silent install for scripts: Lyapunov-Setup-x64.exe /VERYSILENT /TASKS="desktopicon"
#ifndef AppVersion
  #define AppVersion "0.1.0"
#endif
#ifndef SourceDir
  #define SourceDir "..\..\dist\Lyapunov"
#endif

[Setup]
; Never change AppId: Windows recognises updates and the uninstaller by it.
AppId={{4A804861-67AA-4B21-BE24-34863CEE97D7}
AppName=Lyapunov
AppVersion={#AppVersion}
AppPublisher=Chaos-Tic
AppPublisherURL=https://github.com/Chaos-Tic/Lyapunov
AppSupportURL=https://github.com/Chaos-Tic/Lyapunov/issues
; Per-user: no administrator rights.
PrivilegesRequired=lowest
DefaultDirName={localappdata}\Programs\Lyapunov
DisableDirPage=yes
DisableProgramGroupPage=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0.17763
OutputDir=..\..\dist
OutputBaseFilename=Lyapunov-Setup-x64
SetupIconFile=..\..\assets\lyapunov.ico
UninstallDisplayIcon={app}\lyapunov-desktop.exe
UninstallDisplayName=Lyapunov
WizardStyle=modern
Compression=lzma2/max
SolidCompression=yes
CloseApplications=force
RestartApplications=no
; Offer French or English, preselected from the Windows display language.
ShowLanguageDialog=yes
LanguageDetectionMethod=uilanguage

[Languages]
Name: "fr"; MessagesFile: "compiler:Languages\French.isl"
Name: "en"; MessagesFile: "compiler:Default.isl"

[CustomMessages]
fr.GroupShortcuts=Raccourcis :
en.GroupShortcuts=Shortcuts:
fr.TaskDesktop=Créer un raccourci sur le Bureau
en.TaskDesktop=Create a desktop shortcut
fr.ShortcutComment=Enregistrer une activité et la reprendre plus tard
en.ShortcutComment=Save an activity and pick it up later
fr.LaunchLyapunov=Lancer Lyapunov
en.LaunchLyapunov=Start Lyapunov

[Tasks]
Name: "desktopicon"; Description: "{cm:TaskDesktop}"; GroupDescription: "{cm:GroupShortcuts}"

[Files]
Source: "{#SourceDir}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "..\..\LICENSE"; DestDir: "{app}"; DestName: "LICENSE.txt"; Flags: ignoreversion
; Qt is bundled under the LGPL; THIRD-PARTY-NOTICES.txt (in SourceDir) lists it
; and the crates linked into lyapunov.exe (packaging/licenses/notices.sh).
Source: "..\licenses\LGPL-3.0.txt"; DestDir: "{app}"; Flags: ignoreversion

[INI]
; The language picked here is the app's language too ("Language" in
; lyapunov.conf; "auto" there would follow the system instead).
Filename: "{localappdata}\lyapunov\lyapunov.conf"; Section: "General"; Key: "Language"; String: "{language}"

[Icons]
Name: "{userprograms}\Lyapunov"; Filename: "{app}\lyapunov-desktop.exe"; WorkingDir: "{app}"; Comment: "{cm:ShortcutComment}"
Name: "{userdesktop}\Lyapunov"; Filename: "{app}\lyapunov-desktop.exe"; WorkingDir: "{app}"; Comment: "{cm:ShortcutComment}"; Tasks: desktopicon

[Run]
Filename: "{app}\lyapunov-desktop.exe"; Description: "{cm:LaunchLyapunov}"; Flags: postinstall nowait skipifsilent

[UninstallRun]
Filename: "{sys}\taskkill.exe"; Parameters: "/F /IM lyapunov-desktop.exe"; Flags: runhidden; RunOnceId: "StopLyapunov"
