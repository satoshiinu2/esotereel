!include "MUI2.nsh"

!define PRODUCT_VERSION "1.0.0"

Name "Esotereel"
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

	; 実行ファイルおよびライブラリをコピー
	File "..\..\build\cmake\esotereel_gui.exe"

	File "..\..\build\target\release\esotereel_gui_helper.dll"

	; アンインストーラーを生成
	WriteUninstaller "$INSTDIR\uninstall.exe"

	; スタートメニュー・デスクトップショートカットの作成
	CreateDirectory "$SMPROGRAMS\Esotereel"
	CreateShortcut "$SMPROGRAMS\Esotereel\Esotereel.lnk" "$INSTDIR\esotereel_gui.exe"
	CreateShortcut "$SMPROGRAMS\Esotereel\Uninstall.lnk" "$INSTDIR\uninstall.exe"
	CreateShortcut "$DESKTOP\Esotereel.lnk" "$INSTDIR\esotereel_gui.exe"

	; レジストリ登録
	WriteRegStr HKLM "Software\Esotereel" "InstallLocation" "$INSTDIR"
	WriteRegStr HKLM "Software\Esotereel" "Version" "${PRODUCT_VERSION}"

	; コントロールパネル「プログラムと機能」への登録
	WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Esotereel" "DisplayName" "Esotereel"
	WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Esotereel" "InstallLocation" "$INSTDIR"
	WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Esotereel" "UninstallString" \
		"$INSTDIR\uninstall.exe"
	WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Esotereel" "DisplayVersion" "${PRODUCT_VERSION}"
SectionEnd

Section "Uninstall"
	Delete "$INSTDIR\esotereel_gui.exe"
	Delete "$INSTDIR\esotereel_gui_helper.dll"
	Delete "$INSTDIR\uninstall.exe"
	RMDir /r "$INSTDIR"

	Delete "$SMPROGRAMS\Esotereel\Esotereel.lnk"
	Delete "$SMPROGRAMS\Esotereel\Uninstall.lnk"
	RMDir "$SMPROGRAMS\Esotereel"
	Delete "$DESKTOP\Esotereel.lnk"

	DeleteRegKey HKLM "Software\Esotereel"
	DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Esotereel"
SectionEnd
