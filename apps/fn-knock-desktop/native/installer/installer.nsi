Unicode true
RequestExecutionLevel admin
ManifestDPIAware true
SetCompressor /SOLID lzma
SetCompressorDictSize 32

!include "MUI2.nsh"
!include "LogicLib.nsh"
!include "x64.nsh"

!ifndef VERSION
  !error "VERSION is required"
!endif
!ifndef NUMERIC_VERSION
  !error "NUMERIC_VERSION is required"
!endif
!ifndef OUTPUT_FILE
  !error "OUTPUT_FILE is required"
!endif
!ifndef DESKTOP_EXE
  !error "DESKTOP_EXE is required"
!endif
!ifndef BUNDLE_ROOT
  !error "BUNDLE_ROOT is required"
!endif
!ifndef RUNTIME_ROOT
  !error "RUNTIME_ROOT is required"
!endif
!ifndef ICON_FILE
  !error "ICON_FILE is required"
!endif

!macro CheckIfAppIsRunning APP_NAME PRODUCT_NAME
  nsExec::ExecToStack '"$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -NonInteractive -ExecutionPolicy Bypass -Command "if (Get-Process -Name $\'fn-knock$\' -ErrorAction SilentlyContinue) { exit 2 }"'
  Pop $0
  Pop $1
  ${If} $0 == 2
    MessageBox MB_YESNO|MB_ICONQUESTION "Knock 敲门管理程序正在运行。是否立即关闭并继续？" IDYES +2
    Abort "用户取消了操作，未更改任何文件。"
    nsExec::ExecToStack '"$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -NonInteractive -ExecutionPolicy Bypass -Command "Get-Process -Name $\'fn-knock$\' -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction Stop; Start-Sleep -Milliseconds 300; if (Get-Process -Name $\'fn-knock$\' -ErrorAction SilentlyContinue) { exit 1 }"'
    Pop $0
    Pop $1
    ${If} $0 != 0
      Abort "无法关闭 Knock 敲门管理程序，未继续更改系统。"
    ${EndIf}
  ${ElseIf} $0 != 0
    Abort "无法确认 Knock 敲门是否正在运行，未更改任何文件。"
  ${EndIf}
!macroend

!include "hooks.nsh"

Name "Knock 敲门"
BrandingText "KCI-LNK Corporation"
OutFile "${OUTPUT_FILE}"
InstallDir "$PROGRAMFILES64\Knock 敲门"
InstallDirRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Knock 敲门" "InstallLocation"
Icon "${ICON_FILE}"
UninstallIcon "${ICON_FILE}"
ShowInstDetails show
ShowUninstDetails show

VIProductVersion "${NUMERIC_VERSION}"
VIAddVersionKey /LANG=2052 "ProductName" "Knock 敲门"
VIAddVersionKey /LANG=2052 "CompanyName" "KCI-LNK Corporation"
VIAddVersionKey /LANG=2052 "FileDescription" "Knock 敲门 安装程序"
VIAddVersionKey /LANG=2052 "FileVersion" "${VERSION}"
VIAddVersionKey /LANG=2052 "ProductVersion" "${VERSION}"
VIAddVersionKey /LANG=2052 "LegalCopyright" "Copyright © KCI-LNK Corporation"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_RUN "$INSTDIR\fn-knock.exe"
!define MUI_FINISHPAGE_RUN_TEXT "立即启动 Knock 敲门"
!define MUI_FINISHPAGE_RUN_FUNCTION FnKnockLaunch
!define MUI_FINISHPAGE_SHOWREADME "$INSTDIR\fn-knock.exe"
!define MUI_FINISHPAGE_SHOWREADME_TEXT "创建桌面快捷方式"
!define MUI_FINISHPAGE_SHOWREADME_FUNCTION FnKnockCreateDesktopShortcut
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_WELCOME
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_UNPAGE_FINISH

!insertmacro MUI_LANGUAGE "SimpChinese"
!insertmacro MUI_LANGUAGE "English"

Function .onInit
  SetShellVarContext all
  SetRegView 64
  ${IfNot} ${RunningX64}
    Abort "Knock 敲门仅支持 64 位 Windows。"
  ${EndIf}
FunctionEnd

Function un.onInit
  SetShellVarContext all
  SetRegView 64
FunctionEnd

Function FnKnockLaunch
  Exec '"$INSTDIR\fn-knock.exe"'
FunctionEnd

Function FnKnockCreateDesktopShortcut
  SetShellVarContext all
  CreateShortCut "$DESKTOP\Knock 敲门.lnk" "$INSTDIR\fn-knock.exe"
FunctionEnd

Section "Knock 敲门" SEC_MAIN
  SectionIn RO
  SetShellVarContext all
  SetRegView 64
  !insertmacro NSIS_HOOK_PREINSTALL

  SetOutPath "$INSTDIR"
  File /oname=fn-knock.exe "${DESKTOP_EXE}"
  File /oname=fn-knock-service.exe "${BUNDLE_ROOT}\fn-knock-service.exe"
  File /oname=fn-knock-gateway.exe "${BUNDLE_ROOT}\fn-knock-gateway.exe"

  SetOutPath "$INSTDIR\ui"
  File /r "${RUNTIME_ROOT}\ui\*"
  SetOutPath "$INSTDIR\server-auth-view"
  File /r "${RUNTIME_ROOT}\server-auth-view\*"
  SetOutPath "$INSTDIR"
  File /oname=bundle.json "${RUNTIME_ROOT}\bundle.json"
  WriteUninstaller "$INSTDIR\uninstall.exe"

  WriteRegStr HKLM "Software\KCI-LNK Corporation\Knock 敲门" "InstallLocation" "$INSTDIR"
  WriteRegStr HKLM "Software\KCI-LNK Corporation\Knock 敲门" "Version" "${VERSION}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Knock 敲门" "DisplayName" "Knock 敲门"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Knock 敲门" "DisplayVersion" "${VERSION}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Knock 敲门" "Publisher" "KCI-LNK Corporation"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Knock 敲门" "InstallLocation" "$INSTDIR"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Knock 敲门" "DisplayIcon" "$INSTDIR\fn-knock.exe"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Knock 敲门" "UninstallString" '$\"$INSTDIR\uninstall.exe$\"'
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Knock 敲门" "QuietUninstallString" '$\"$INSTDIR\uninstall.exe$\" /S'
  WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Knock 敲门" "NoModify" 1
  WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Knock 敲门" "NoRepair" 1

  !insertmacro NSIS_HOOK_POSTINSTALL
SectionEnd

Section "Uninstall"
  SetShellVarContext all
  SetRegView 64
  !insertmacro NSIS_HOOK_PREUNINSTALL

  Delete "$SMPROGRAMS\Knock 敲门.lnk"
  Delete "$DESKTOP\Knock 敲门.lnk"
  Delete "$INSTDIR\fn-knock.exe"
  Delete "$INSTDIR\fn-knock-service.exe"
  Delete "$INSTDIR\fn-knock-gateway.exe"
  Delete "$INSTDIR\bundle.json"
  RMDir /r "$INSTDIR\ui"
  RMDir /r "$INSTDIR\server-auth-view"
  Delete "$INSTDIR\uninstall.exe"
  DeleteRegKey HKLM "Software\KCI-LNK Corporation\Knock 敲门"
  DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Knock 敲门"

  !insertmacro NSIS_HOOK_POSTUNINSTALL
  RMDir "$INSTDIR"
SectionEnd
