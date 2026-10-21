!include "MUI2.nsh"

!define PRODUCT_NAME "Esotereel"
!define PRODUCT_VERSION "1.0.0"
!define PRODUCT_PUBLISHER "Esotereel"

Name "${PRODUCT_NAME}"
OutFile "..\..\dist\esotereel-installer.exe"
InstallDir "$PROGRAMFILES64\Esotereel"
InstallDirRegKey HKLM "Software\Esotereel" "InstallLocation"
RequestExecutionLevel admin

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "Japanese"

Section "Esotereel" SecMain
    SetOutPath "$INSTDIR"

    ; Copy the complete prepared runtime tree: EXE, Rust/Qt/FFmpeg/MinGW DLLs,
    ; and Qt plugin subdirectories created in build\win_dist by PowerShell.
    File /r "..\..\build\win_dist\*"

    ; Esotereel's own plugins are kept separate from Qt's runtime plugins.
    SetOutPath "$INSTDIR\std_plugins"
    File /r "..\..\plugins\*"

    WriteUninstaller "$INSTDIR\uninstall.exe"

    CreateDirectory "$SMPROGRAMS\Esotereel"
    CreateShortcut "$SMPROGRAMS\Esotereel\Esotereel.lnk" "$INSTDIR\esotereel_gui.exe"
    CreateShortcut "$SMPROGRAMS\Esotereel\Uninstall.lnk" "$INSTDIR\uninstall.exe"
    CreateShortcut "$DESKTOP\Esotereel.lnk" "$INSTDIR\esotereel_gui.exe"

    WriteRegStr HKLM "Software\Esotereel" "InstallLocation" "$INSTDIR"
    WriteRegStr HKLM "Software\Esotereel" "Version" "${PRODUCT_VERSION}"

    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Esotereel" "DisplayName" "${PRODUCT_NAME}"
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Esotereel" "DisplayPublisher" "${PRODUCT_PUBLISHER}"
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Esotereel" "InstallLocation" "$INSTDIR"
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Esotereel" "UninstallString" "$\"$INSTDIR\uninstall.exe$\""
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Esotereel" "DisplayVersion" "${PRODUCT_VERSION}"
SectionEnd

Section "Uninstall"
    ; The runtime tree contains more DLLs than the EXE and Rust helper now, so
    ; remove the complete installation directory rather than maintaining a list.
    RMDir /r "$INSTDIR"

    Delete "$SMPROGRAMS\Esotereel\Esotereel.lnk"
    Delete "$SMPROGRAMS\Esotereel\Uninstall.lnk"
    RMDir "$SMPROGRAMS\Esotereel"
    Delete "$DESKTOP\Esotereel.lnk"

    DeleteRegKey HKLM "Software\Esotereel"
    DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Esotereel"
SectionEnd
