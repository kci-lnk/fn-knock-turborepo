!include "LogicLib.nsh"

; Public installer branding and transactional service lifecycle hooks.
Caption "Knock 敲门"
UninstallCaption "卸载 Knock 敲门"
!define MUI_WELCOMEPAGE_TITLE "欢迎使用 Knock 敲门 安装程序"
!define MUI_WELCOMEPAGE_TEXT "此向导将安装 Knock 敲门。$\r$\n$\r$\n单击“下一步”继续。"
!define MUI_DIRECTORYPAGE_TEXT_TOP "请选择 Knock 敲门 的安装位置。"
!define MUI_FINISHPAGE_TITLE "Knock 敲门 安装完成"
!define MUI_FINISHPAGE_TEXT "Knock 敲门 已成功安装。"
!define MUI_UNWELCOMEPAGE_TITLE "卸载 Knock 敲门"
!define MUI_UNWELCOMEPAGE_TEXT "此向导将从计算机中卸载 Knock 敲门。"

!define FNKNOCK_SERVICE "FnKnock"
Var FnKnockTransactionDir
Var FnKnockTransactionScript
Var FnKnockTransactionReady
Var FnKnockTransactionCreated

Function FnKnockProtectTransactionDirectory
  StrCpy $0 0
  StrCpy $1 ""

  ; Program Files is the protected parent. GetFileAttributes failure also
  ; produces all bits set and is rejected by the same mask.
  Push $2
  System::Call 'kernel32::GetFileAttributesW(w "$FnKnockTransactionDir") i.r2'
  IntOp $2 $2 & 0x400
  ${If} $2 != 0
    StrCpy $0 1
    StrCpy $1 "the installer transaction directory is a reparse point or could not be inspected"
    Pop $2
    Return
  ${EndIf}
  Pop $2

  ; This directory was just created below Program Files and was already checked
  ; for reparse-point substitution. Only claim the root here. The recursive
  ; ACL operations below use icacls /L so they never need takeown's
  ; version-dependent, undocumented /SKIPSL option.
  nsExec::ExecToStack '"$SYSDIR\takeown.exe" /F "$FnKnockTransactionDir" /A'
  Pop $0
  Pop $1
  ${If} $0 != 0
    StrCpy $1 "unable to take ownership of the installer transaction directory"
    Return
  ${EndIf}

  nsExec::ExecToStack '"$SYSDIR\icacls.exe" "$FnKnockTransactionDir" /reset /T /L /Q'
  Pop $0
  Pop $1
  ${If} $0 != 0
    StrCpy $1 "unable to reset the installer transaction directory ACL"
    Return
  ${EndIf}

  ; Establish a safe write allowlist before the helper exists. Some Windows 10
  ; builds preserve read-only application-package ACEs here; once loaded from
  ; this non-writable directory, the helper replaces the DACL through the .NET
  ; ACL API and verifies the exact canonical result.
  nsExec::ExecToStack '"$SYSDIR\icacls.exe" "$FnKnockTransactionDir" /inheritance:r /grant:r "*S-1-5-18:(OI)(CI)F" "*S-1-5-32-544:(OI)(CI)F" /T /L /Q'
  Pop $0
  Pop $1
  ${If} $0 != 0
    StrCpy $1 "unable to apply the installer transaction directory allowlist"
    Return
  ${EndIf}

  ; Some Windows 11 configurations reject the owner transfer when icacls is a
  ; direct NSIS child, while the same elevated token succeeds when PowerShell
  ; dispatches icacls. Use the latter path before PowerShell reads the helper;
  ; the helper independently reapplies and verifies the exact owner/DACL.
  nsExec::ExecToStack '"$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -NonInteractive -ExecutionPolicy Bypass -Command "$$ErrorActionPreference = $\'Stop$\'; & $\'$SYSDIR\icacls.exe$\' $\'$FnKnockTransactionDir$\' /setowner $\'*S-1-5-18$\' /T /L /Q; if ($$LASTEXITCODE -ne 0) { exit $$LASTEXITCODE }"'
  Pop $0
  Pop $1
  ${If} $0 != 0
    StrCpy $1 "unable to transfer the installer transaction directory ownership through PowerShell (exit $0): $1"
    Return
  ${EndIf}
  StrCpy $0 0
  StrCpy $1 ""
FunctionEnd

Function FnKnockCleanupTransactionDirectory
  ${If} $FnKnockTransactionCreated == 1
    RMDir /r "$FnKnockTransactionDir"
    StrCpy $FnKnockTransactionCreated 0
    StrCpy $FnKnockTransactionReady 0
  ${EndIf}
FunctionEnd

; Keep the PowerShell command line short enough for NSIS' string limit. The
; transaction implementation is emitted below protected Program Files in small
; chunks and reused by PREINSTALL, POSTINSTALL and failure callbacks.
Function FnKnockWriteTransactionScript
  StrCpy $0 0
  StrCpy $1 ""

  ; The initialized helper is immutable and self-verifying. Never reset its ACL
  ; again between transaction actions.
  ${If} $FnKnockTransactionReady == 1
    Return
  ${EndIf}

  Push $2
  System::Call 'kernel32::GetCurrentProcessId() i.r2'
  StrCpy $FnKnockTransactionDir "$PROGRAMFILES64\.FnKnock-installer-$2"
  StrCpy $FnKnockTransactionScript "$FnKnockTransactionDir\transaction.ps1"
  Pop $2

  ; A PID collision or previous residue must never be reused. Prove absence;
  ; access denied and every error except FILE/PATH_NOT_FOUND fail closed.
  Push $2
  Push $3
  System::Call 'kernel32::GetFileAttributesW(w "$FnKnockTransactionDir") i.r2 ?e'
  Pop $3
  ${If} $2 != -1
    StrCpy $0 1
    StrCpy $1 "the installer transaction directory existed before secure initialization"
    Pop $3
    Pop $2
    Return
  ${EndIf}
  ${If} $3 != 2
  ${AndIf} $3 != 3
    StrCpy $0 1
    StrCpy $1 "unable to prove that the installer transaction directory is absent"
    Pop $3
    Pop $2
    Return
  ${EndIf}
  Pop $3
  Pop $2

  ClearErrors
  CreateDirectory "$FnKnockTransactionDir"
  ${If} ${Errors}
    StrCpy $0 1
    StrCpy $1 "unable to create the installer transaction directory"
    Return
  ${EndIf}
  StrCpy $FnKnockTransactionCreated 1

  Call FnKnockProtectTransactionDirectory
  ${If} $0 != 0
    Return
  ${EndIf}

  ClearErrors
  FileOpen $R7 "$FnKnockTransactionScript" w
  ${If} ${Errors}
    StrCpy $0 1
    StrCpy $1 "unable to create the installer transaction helper"
    Return
  ${EndIf}

  FileWrite $R7 "[CmdletBinding()]$\r$\n"
  FileWrite $R7 "param([Parameter(Mandatory=$$true)][ValidateSet('begin','stop','snapshot','rollback','protect-rollback','wait-ready')][string]$$Action, [Parameter(Mandatory=$$true)][string]$$InstallDir, [Parameter(Mandatory=$$true)][string]$$ProgramFilesDir)$\r$\n"
  FileWrite $R7 "$$ErrorActionPreference = 'Stop'$\r$\n"
  ; Keep every initialization and action error within the concise diagnostic
  ; boundary. This must begin after param(), which PowerShell requires first.
  FileWrite $R7 "try {$\r$\n"
  FileWrite $R7 "function Assert-FnKnockInstallerAcl($$Acl, [string]$$Path, [string[]]$$AllowedSids) {$\r$\n"
  FileWrite $R7 "  if (-not $$Acl.AreAccessRulesProtected) { throw ('installer ACL inherits permissions: ' + $$Path) }$\r$\n"
  FileWrite $R7 "  if ($$Acl.GetOwner([System.Security.Principal.SecurityIdentifier]).Value -ne 'S-1-5-18') { throw ('installer object is not owned by SYSTEM: ' + $$Path) }$\r$\n"
  FileWrite $R7 "  $$rules = @($$Acl.GetAccessRules($$true, $$false, [System.Security.Principal.SecurityIdentifier]))$\r$\n"
  FileWrite $R7 "  $$isDirectory = $$Acl -is [System.Security.AccessControl.DirectorySecurity]$\r$\n"
  ; Windows enforces read-compatible Program Files ACEs for packaged apps even
  ; after SetAccessControl replaces a protected DACL. Accept those two
  ; well-known identities only when their access mask cannot mutate contents,
  ; delete objects, change the DACL/owner, or request generic write/all access.
  FileWrite $R7 "  $$platformReadSids = @('S-1-15-2-1','S-1-15-2-2'); $$knownSids = @($$AllowedSids) + $$platformReadSids$\r$\n"
  FileWrite $R7 "  if (@($$rules | Where-Object { $$knownSids -notcontains $$_.IdentityReference.Value }).Count -ne 0) { throw ('installer ACL has unexpected identities: ' + $$Path) }$\r$\n"
  FileWrite $R7 "  $$full = [System.Security.AccessControl.FileSystemRights]::FullControl; $$allow = [System.Security.AccessControl.AccessControlType]::Allow; $$noInheritance = [System.Security.AccessControl.InheritanceFlags]::None; $$container = [System.Security.AccessControl.InheritanceFlags]::ContainerInherit; $$both = $$container -bor [System.Security.AccessControl.InheritanceFlags]::ObjectInherit; $$noPropagation = [System.Security.AccessControl.PropagationFlags]::None; $$inheritOnly = [System.Security.AccessControl.PropagationFlags]::InheritOnly$\r$\n"
  FileWrite $R7 "  foreach ($$rule in @($$rules | Where-Object { $$AllowedSids -contains $$_.IdentityReference.Value })) { if ($$rule.AccessControlType -ne $$allow -or $$rule.FileSystemRights -ne $$full -or $$rule.PropagationFlags -ne $$noPropagation -or ($$rule.InheritanceFlags -ne $$noInheritance -and $$rule.InheritanceFlags -ne $$both)) { throw ('installer ACL contains a non-exact trusted rule: ' + $$Path) } }$\r$\n"
  FileWrite $R7 "  foreach ($$rule in @($$rules | Where-Object { $$platformReadSids -contains $$_.IdentityReference.Value })) { $$rightsMask = [BitConverter]::ToUInt32([BitConverter]::GetBytes([int32]$$rule.FileSystemRights), 0); if ($$rule.AccessControlType -ne $$allow -or $$rightsMask -eq 0 -or ($$rightsMask -band [uint32]0x530D0146) -ne 0) { throw ('installer ACL gives an application package unsafe access: ' + $$Path) }; if ((-not $$isDirectory -and ($$rule.InheritanceFlags -ne $$noInheritance -or $$rule.PropagationFlags -ne $$noPropagation)) -or ($$isDirectory -and (($$rule.InheritanceFlags -band (-bnot $$both)) -ne $$noInheritance -or ($$rule.PropagationFlags -ne $$noPropagation -and $$rule.PropagationFlags -ne $$inheritOnly) -or ($$rule.PropagationFlags -eq $$inheritOnly -and $$rule.InheritanceFlags -eq $$noInheritance)))) { throw ('installer ACL has unexpected application-package inheritance: ' + $$Path) } }$\r$\n"
  FileWrite $R7 "  foreach ($$sid in $$AllowedSids) { $$sidRules = @($$rules | Where-Object { $$_.IdentityReference.Value -eq $$sid }); if ($$sidRules.Count -ne 1) { throw ('installer ACL has an unexpected trusted SID rule count: ' + $$Path) }; $$expectedInheritance = if ($$isDirectory) { $$both } else { $$noInheritance }; if ($$sidRules[0].InheritanceFlags -ne $$expectedInheritance) { throw ('installer ACL has unexpected trusted inheritance flags: ' + $$Path) } }$\r$\n"
  FileWrite $R7 "}$\r$\n"
  FileWrite $R7 "function Assert-FnKnockInstallerTreeAcl([string]$$Path) {$\r$\n"
  FileWrite $R7 "  $$allowedSids = @('S-1-5-18','S-1-5-32-544')$\r$\n"
  FileWrite $R7 "  $$queue = [System.Collections.Generic.Queue[string]]::new()$\r$\n"
  FileWrite $R7 "  $$queue.Enqueue([IO.Path]::GetFullPath($$Path))$\r$\n"
  FileWrite $R7 "  while ($$queue.Count -gt 0) {$\r$\n"
  FileWrite $R7 "    $$directory = $$queue.Dequeue()$\r$\n"
  FileWrite $R7 "    $$attributes = [IO.File]::GetAttributes($$directory)$\r$\n"
  FileWrite $R7 "    if (($$attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) { throw ('installer tree contains a reparse point: ' + $$directory) }$\r$\n"
  FileWrite $R7 "    Assert-FnKnockInstallerAcl ([IO.Directory]::GetAccessControl($$directory)) $$directory $$allowedSids$\r$\n"
  FileWrite $R7 "    foreach ($$entry in [IO.Directory]::EnumerateFileSystemEntries($$directory)) {$\r$\n"
  FileWrite $R7 "      $$entryAttributes = [IO.File]::GetAttributes($$entry)$\r$\n"
  FileWrite $R7 "      if (($$entryAttributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) { throw ('installer tree contains a reparse point: ' + $$entry) }$\r$\n"
  FileWrite $R7 "      if (($$entryAttributes -band [IO.FileAttributes]::Directory) -ne 0) { $$queue.Enqueue($$entry) } else { Assert-FnKnockInstallerAcl ([IO.File]::GetAccessControl($$entry)) $$entry $$allowedSids }$\r$\n"
  FileWrite $R7 "    }$\r$\n"
  FileWrite $R7 "  }$\r$\n"
  FileWrite $R7 "}$\r$\n"
  FileWrite $R7 "$$serviceName = '${FNKNOCK_SERVICE}'$\r$\n"
  FileWrite $R7 "$$systemSid = [System.Security.Principal.SecurityIdentifier]::new('S-1-5-18')$\r$\n"
  FileWrite $R7 "$$administratorsSid = [System.Security.Principal.SecurityIdentifier]::new('S-1-5-32-544')$\r$\n"
  ; NSIS is a 32-bit executable even for x64 bundles. Pass its trusted
  ; $PROGRAMFILES64 value instead of resolving ProgramFiles inside WOW64 PS.
  FileWrite $R7 "$$programFiles = $$ProgramFilesDir$\r$\n"
  FileWrite $R7 "if ([string]::IsNullOrWhiteSpace($$programFiles) -or -not [IO.Path]::IsPathRooted($$programFiles) -or $$programFiles.StartsWith('\\')) { throw 'the 64-bit ProgramFiles known folder is invalid' }$\r$\n"
  FileWrite $R7 "$$programFiles = [IO.Path]::GetFullPath($$programFiles).TrimEnd([char]'\')$\r$\n"
  FileWrite $R7 "$$allowedInstallRoot = [IO.Path]::GetFullPath((Join-Path $$programFiles 'Knock 敲门')).TrimEnd([char]'\')$\r$\n"
  FileWrite $R7 "$$legacyInstallRoot = [IO.Path]::GetFullPath((Join-Path $$allowedInstallRoot 'current')).TrimEnd([char]'\')$\r$\n"
  FileWrite $R7 "$$install = [IO.Path]::GetFullPath($$InstallDir).TrimEnd([char]'\')$\r$\n"
  FileWrite $R7 "$$isCurrentInstallRoot = $$install.Equals($$allowedInstallRoot, [StringComparison]::OrdinalIgnoreCase)$\r$\n"
  FileWrite $R7 "$$isLegacyInstallRoot = $$install.Equals($$legacyInstallRoot, [StringComparison]::OrdinalIgnoreCase)$\r$\n"
  FileWrite $R7 "if (-not $$isCurrentInstallRoot -and -not $$isLegacyInstallRoot) { throw ('InstallDir must be Program Files\Knock 敲门: ' + $$install) }$\r$\n"
  ; Enumerate each trusted parent so access-denied entries cannot be mistaken
  ; for absent paths by Directory.Exists.
  FileWrite $R7 "$$fnKnockEntry = [IO.DirectoryInfo]::new($$programFiles).EnumerateFileSystemInfos('Knock 敲门', [IO.SearchOption]::TopDirectoryOnly) | Select-Object -First 1$\r$\n"
  FileWrite $R7 "if ($$null -ne $$fnKnockEntry -and (($$fnKnockEntry.Attributes -band [IO.FileAttributes]::Directory) -eq 0 -or ($$fnKnockEntry.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0)) { throw 'Program Files\Knock 敲门 is not a trusted ordinary directory' }$\r$\n"
  FileWrite $R7 "$$legacyEntry = $$null$\r$\n"
  FileWrite $R7 "if ($$isLegacyInstallRoot -and $$null -ne $$fnKnockEntry) { $$legacyEntry = [IO.DirectoryInfo]::new($$allowedInstallRoot).EnumerateFileSystemInfos('current', [IO.SearchOption]::TopDirectoryOnly) | Select-Object -First 1; if ($$null -ne $$legacyEntry -and (($$legacyEntry.Attributes -band [IO.FileAttributes]::Directory) -eq 0 -or ($$legacyEntry.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0)) { throw 'Program Files\Knock 敲门\current is not a trusted ordinary directory' } }$\r$\n"
  FileWrite $R7 "$$installEntry = if ($$isCurrentInstallRoot) { $$fnKnockEntry } else { $$legacyEntry }$\r$\n"
  FileWrite $R7 "$$programData = [Environment]::GetFolderPath([Environment+SpecialFolder]::CommonApplicationData)$\r$\n"
  FileWrite $R7 "if ([string]::IsNullOrWhiteSpace($$programData) -or -not [IO.Path]::IsPathRooted($$programData) -or $$programData.StartsWith('\\')) { throw 'the CommonApplicationData known folder is invalid' }$\r$\n"
  FileWrite $R7 "$$programData = [IO.Path]::GetFullPath($$programData)$\r$\n"
  FileWrite $R7 "$$root = Join-Path $$programData 'FnKnock'$\r$\n"
  FileWrite $R7 "$$state = Join-Path $$root 'state'$\r$\n"
  FileWrite $R7 "$$rollback = Join-Path $$root 'rollback'$\r$\n"
  FileWrite $R7 "$$pending = Join-Path $$rollback 'pending'$\r$\n"
  FileWrite $R7 "$$persistentDataTrees = @('config','data','gateway','waf','certificates','secrets')$\r$\n"
  FileWrite $R7 "$$marker = Join-Path $$pending 'transaction.pending'$\r$\n"
  FileWrite $R7 "$$snapshotReady = Join-Path $$pending 'snapshot.ready'$\r$\n"
  FileWrite $R7 "$$serviceExe = Join-Path $$install 'fn-knock-service.exe'$\r$\n"
  FileWrite $R7 "$$gatewayExe = Join-Path $$install 'fn-knock-gateway.exe'$\r$\n"
  FileWrite $R7 "$$bundleIdentityPath = Join-Path $$install 'bundle.json'$\r$\n"
  FileWrite $R7 "$$registrySnapshot = Join-Path $$pending 'registry'$\r$\n"
  FileWrite $R7 "$$shortcutSnapshot = Join-Path $$pending 'shortcuts'$\r$\n"
  FileWrite $R7 "$$systemDirectory = [Environment]::GetFolderPath([Environment+SpecialFolder]::System)$\r$\n"
  FileWrite $R7 "if ([string]::IsNullOrWhiteSpace($$systemDirectory) -or -not [IO.Path]::IsPathRooted($$systemDirectory) -or $$systemDirectory.StartsWith('\\')) { throw 'the System known folder is invalid' }$\r$\n"
  FileWrite $R7 "$$systemDirectory = [IO.Path]::GetFullPath($$systemDirectory)$\r$\n"
  FileWrite $R7 "$$sc = Join-Path $$systemDirectory 'sc.exe'$\r$\n"
  FileWrite $R7 "$$icacls = Join-Path $$systemDirectory 'icacls.exe'$\r$\n"
  FileWrite $R7 "$$takeown = Join-Path $$systemDirectory 'takeown.exe'$\r$\n"
  FileWrite $R7 "$$reg = Join-Path $$systemDirectory 'reg.exe'$\r$\n"
  FileWrite $R7 "if (-not (Test-Path -LiteralPath $$sc -PathType Leaf) -or -not (Test-Path -LiteralPath $$icacls -PathType Leaf) -or -not (Test-Path -LiteralPath $$takeown -PathType Leaf) -or -not (Test-Path -LiteralPath $$reg -PathType Leaf)) { throw 'required System32 tools are missing' }$\r$\n"
  FileWrite $R7 "$$registryEntries = @([pscustomobject]@{ Name = 'product'; SubKey = 'Software\KCI-LNK Corporation\Knock 敲门'; RegKey = 'HKLM\Software\KCI-LNK Corporation\Knock 敲门' }, [pscustomobject]@{ Name = 'uninstall'; SubKey = 'Software\Microsoft\Windows\CurrentVersion\Uninstall\Knock 敲门'; RegKey = 'HKLM\Software\Microsoft\Windows\CurrentVersion\Uninstall\Knock 敲门' })$\r$\n"
  FileWrite $R7 "$$commonPrograms = [Environment]::GetFolderPath([Environment+SpecialFolder]::CommonPrograms)$\r$\n"
  FileWrite $R7 "$$commonDesktop = [Environment]::GetFolderPath([Environment+SpecialFolder]::CommonDesktopDirectory)$\r$\n"
  FileWrite $R7 "foreach ($$knownFolder in @($$commonPrograms, $$commonDesktop)) { if ([string]::IsNullOrWhiteSpace($$knownFolder) -or -not [IO.Path]::IsPathRooted($$knownFolder) -or $$knownFolder.StartsWith('\\')) { throw 'an all-users shortcut known folder is invalid' } }$\r$\n"
  FileWrite $R7 "$$shortcutEntries = @([pscustomobject]@{ Name = 'start-menu'; Path = Join-Path $$commonPrograms 'Knock 敲门.lnk' }, [pscustomobject]@{ Name = 'desktop'; Path = Join-Path $$commonDesktop 'Knock 敲门.lnk' })$\r$\n"
  ; A service with our name but a different binary must not receive temporary
  ; ProgramData access or be stopped/deleted by this installer. Older builds
  ; registered the same exact executable without the explicit --service flag.
  FileWrite $R7 "$$existingService = Get-Service -Name $$serviceName -ErrorAction SilentlyContinue$\r$\n"
  FileWrite $R7 "$$serviceRecord = Get-CimInstance -ClassName Win32_Service -Filter 'Name=''FnKnock''' -ErrorAction Stop$\r$\n"
  FileWrite $R7 "if (($$null -eq $$existingService) -ne ($$null -eq $$serviceRecord)) { throw 'SCM and Win32_Service returned inconsistent FnKnock service state' }$\r$\n"
  FileWrite $R7 "$$expectedServiceCommand = ([char]34 + $$serviceExe + [char]34 + ' --service'); $$legacyServiceCommand = ([char]34 + $$serviceExe + [char]34)$\r$\n"
  FileWrite $R7 "if ($$null -ne $$serviceRecord) { $$registeredCommand = ([string]$$serviceRecord.PathName).Trim(); if (-not $$registeredCommand.Equals($$expectedServiceCommand, [StringComparison]::OrdinalIgnoreCase) -and -not $$registeredCommand.Equals($$legacyServiceCommand, [StringComparison]::OrdinalIgnoreCase)) { throw ('the existing FnKnock service points outside the selected install root or has unexpected arguments: ' + $$registeredCommand) } }$\r$\n"
  FileWrite $R7 "$\r$\n"

  FileWrite $R7 "function Stop-FnKnock {$\r$\n"
  FileWrite $R7 "  $$service = Get-Service -Name $$serviceName -ErrorAction SilentlyContinue$\r$\n"
  FileWrite $R7 "  if ($$null -eq $$service) { return }$\r$\n"
  FileWrite $R7 "  if ($$service.Status -ne 'Stopped' -and $$service.Status -ne 'StopPending') { Stop-Service -Name $$serviceName -ErrorAction Stop }$\r$\n"
  FileWrite $R7 "  $$service.Refresh()$\r$\n"
  FileWrite $R7 "  if ($$service.Status -ne 'Stopped') { $$service.WaitForStatus('Stopped', [TimeSpan]::FromSeconds(20)) }$\r$\n"
  FileWrite $R7 "  $$service.Refresh()$\r$\n"
  FileWrite $R7 "  if ($$service.Status -ne 'Stopped') { throw 'FnKnock did not stop within 20 seconds' }$\r$\n"
  FileWrite $R7 "}$\r$\n"
  FileWrite $R7 "$\r$\n"

  FileWrite $R7 "function New-FnKnockDirectoryAcl($$SystemSid, $$AdministratorsSid, $$ServiceSid, $$ReadSid = $$null) {$\r$\n"
  FileWrite $R7 "  $$acl = [System.Security.AccessControl.DirectorySecurity]::new()$\r$\n"
  FileWrite $R7 "  $$acl.SetAccessRuleProtection($$true, $$false)$\r$\n"
  FileWrite $R7 "  $$inherit = [System.Security.AccessControl.InheritanceFlags]::ContainerInherit -bor [System.Security.AccessControl.InheritanceFlags]::ObjectInherit$\r$\n"
  FileWrite $R7 "  $$none = [System.Security.AccessControl.PropagationFlags]::None$\r$\n"
  FileWrite $R7 "  $$allow = [System.Security.AccessControl.AccessControlType]::Allow$\r$\n"
  FileWrite $R7 "  [void]$$acl.AddAccessRule([System.Security.AccessControl.FileSystemAccessRule]::new($$SystemSid, [System.Security.AccessControl.FileSystemRights]::FullControl, $$inherit, $$none, $$allow))$\r$\n"
  FileWrite $R7 "  [void]$$acl.AddAccessRule([System.Security.AccessControl.FileSystemAccessRule]::new($$AdministratorsSid, [System.Security.AccessControl.FileSystemRights]::FullControl, $$inherit, $$none, $$allow))$\r$\n"
  FileWrite $R7 "  if ($$null -ne $$ServiceSid) { [void]$$acl.AddAccessRule([System.Security.AccessControl.FileSystemAccessRule]::new($$ServiceSid, [System.Security.AccessControl.FileSystemRights]::Modify, $$inherit, $$none, $$allow)) }$\r$\n"
  FileWrite $R7 "  if ($$null -ne $$ReadSid) { [void]$$acl.AddAccessRule([System.Security.AccessControl.FileSystemAccessRule]::new($$ReadSid, [System.Security.AccessControl.FileSystemRights]::ReadAndExecute, $$inherit, $$none, $$allow)) }$\r$\n"
  FileWrite $R7 "  return $$acl$\r$\n"
  FileWrite $R7 "}$\r$\n"
  FileWrite $R7 "$\r$\n"

  FileWrite $R7 "function New-FnKnockFileAcl($$SystemSid, $$AdministratorsSid, $$ServiceSid, $$ReadSid = $$null) {$\r$\n"
  FileWrite $R7 "  $$acl = [System.Security.AccessControl.FileSecurity]::new()$\r$\n"
  FileWrite $R7 "  $$acl.SetAccessRuleProtection($$true, $$false)$\r$\n"
  FileWrite $R7 "  $$allow = [System.Security.AccessControl.AccessControlType]::Allow$\r$\n"
  FileWrite $R7 "  [void]$$acl.AddAccessRule([System.Security.AccessControl.FileSystemAccessRule]::new($$SystemSid, [System.Security.AccessControl.FileSystemRights]::FullControl, $$allow))$\r$\n"
  FileWrite $R7 "  [void]$$acl.AddAccessRule([System.Security.AccessControl.FileSystemAccessRule]::new($$AdministratorsSid, [System.Security.AccessControl.FileSystemRights]::FullControl, $$allow))$\r$\n"
  FileWrite $R7 "  if ($$null -ne $$ServiceSid) { [void]$$acl.AddAccessRule([System.Security.AccessControl.FileSystemAccessRule]::new($$ServiceSid, [System.Security.AccessControl.FileSystemRights]::Modify, $$allow)) }$\r$\n"
  FileWrite $R7 "  if ($$null -ne $$ReadSid) { [void]$$acl.AddAccessRule([System.Security.AccessControl.FileSystemAccessRule]::new($$ReadSid, [System.Security.AccessControl.FileSystemRights]::ReadAndExecute, $$allow)) }$\r$\n"
  FileWrite $R7 "  return $$acl$\r$\n"
  FileWrite $R7 "}$\r$\n"
  FileWrite $R7 "$\r$\n"

  FileWrite $R7 "function Set-FnKnockDataTreeAcl([string]$$Path, $$SystemSid, $$AdministratorsSid, $$ServiceSid, $$ReadSid = $$null) {$\r$\n"
  FileWrite $R7 "  $$queue = [System.Collections.Generic.Queue[string]]::new()$\r$\n"
  FileWrite $R7 "  $$queue.Enqueue([IO.Path]::GetFullPath($$Path))$\r$\n"
  FileWrite $R7 "  while ($$queue.Count -gt 0) {$\r$\n"
  FileWrite $R7 "    $$directory = $$queue.Dequeue()$\r$\n"
  FileWrite $R7 "    $$attributes = [IO.File]::GetAttributes($$directory)$\r$\n"
  FileWrite $R7 "    if (($$attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) { throw ('refusing a reparse point in ProgramData: ' + $$directory) }$\r$\n"
  FileWrite $R7 "    [IO.Directory]::SetAccessControl($$directory, (New-FnKnockDirectoryAcl $$SystemSid $$AdministratorsSid $$ServiceSid $$ReadSid))$\r$\n"
  FileWrite $R7 "    foreach ($$entry in [IO.Directory]::EnumerateFileSystemEntries($$directory)) {$\r$\n"
  FileWrite $R7 "      $$entryAttributes = [IO.File]::GetAttributes($$entry)$\r$\n"
  FileWrite $R7 "      if (($$entryAttributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) { throw ('refusing a reparse point in ProgramData: ' + $$entry) }$\r$\n"
  FileWrite $R7 "      if (($$entryAttributes -band [IO.FileAttributes]::Directory) -ne 0) { $$queue.Enqueue($$entry) } else { [IO.File]::SetAccessControl($$entry, (New-FnKnockFileAcl $$SystemSid $$AdministratorsSid $$ServiceSid $$ReadSid)) }$\r$\n"
  FileWrite $R7 "    }$\r$\n"
  FileWrite $R7 "  }$\r$\n"
  FileWrite $R7 "}$\r$\n"
  FileWrite $R7 "$\r$\n"

  FileWrite $R7 "function Assert-NoReparseTree([string]$$Path) {$\r$\n"
  FileWrite $R7 "  $$fullPath = [IO.Path]::GetFullPath($$Path)$\r$\n"
  FileWrite $R7 "  try { $$rootAttributes = [IO.File]::GetAttributes($$fullPath) } catch [IO.FileNotFoundException] { return } catch [IO.DirectoryNotFoundException] { return }$\r$\n"
  FileWrite $R7 "  if (($$rootAttributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) { throw ('refusing a reparse point before a recursive installer operation: ' + $$fullPath) }$\r$\n"
  FileWrite $R7 "  if (($$rootAttributes -band [IO.FileAttributes]::Directory) -eq 0) { return }$\r$\n"
  FileWrite $R7 "  $$queue = [System.Collections.Generic.Queue[string]]::new(); $$queue.Enqueue($$fullPath)$\r$\n"
  FileWrite $R7 "  while ($$queue.Count -gt 0) { $$directory = $$queue.Dequeue(); foreach ($$entry in [IO.Directory]::EnumerateFileSystemEntries($$directory)) { $$attributes = [IO.File]::GetAttributes($$entry); if (($$attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) { throw ('refusing a reparse point before a recursive installer operation: ' + $$entry) }; if (($$attributes -band [IO.FileAttributes]::Directory) -ne 0) { $$queue.Enqueue($$entry) } } }$\r$\n"
  FileWrite $R7 "}$\r$\n"
  FileWrite $R7 "$\r$\n"

  FileWrite $R7 "function Set-FnKnockDataTreeOwner([string]$$Path, [string]$$Icacls) {$\r$\n"
  FileWrite $R7 "  $$ownerArgs = @($$Path, '/setowner', '*S-1-5-18', '/T', '/L', '/Q')$\r$\n"
  FileWrite $R7 "  & $$Icacls @ownerArgs | Out-Null$\r$\n"
  FileWrite $R7 "  $$ownerExitCode = $$LASTEXITCODE$\r$\n"
  FileWrite $R7 "  if ($$ownerExitCode -ne 0) { throw ('unable to transfer ProgramData\FnKnock ownership to SYSTEM; icacls exited with code ' + $$ownerExitCode) }$\r$\n"
  FileWrite $R7 "}$\r$\n"
  FileWrite $R7 "$\r$\n"
  FileWrite $R7 "function Bootstrap-FnKnockDataTreeAccess([string]$$Path, [string]$$Takeown, [string]$$Icacls, [string]$$ServiceGrant) {$\r$\n"
  ; takeown's recursive no-follow switch is not documented and is absent on
  ; otherwise supported Windows 10 builds. Claim only the already validated
  ; root with portable takeown syntax. Every recursive operation is delegated
  ; to documented icacls /L, which operates on links rather than their targets.
  FileWrite $R7 "  $$rootAttributes = [IO.File]::GetAttributes($$Path)$\r$\n"
  FileWrite $R7 "  if (($$rootAttributes -band [IO.FileAttributes]::Directory) -eq 0 -or ($$rootAttributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) { throw ('refusing an untrusted ProgramData root: ' + $$Path) }$\r$\n"
  FileWrite $R7 "  $$rootTakeownArgs = @('/F', $$Path, '/A')$\r$\n"
  FileWrite $R7 "  & $$Takeown @rootTakeownArgs | Out-Null$\r$\n"
  FileWrite $R7 "  $$rootTakeownExitCode = $$LASTEXITCODE$\r$\n"
  FileWrite $R7 "  if ($$rootTakeownExitCode -ne 0) { throw ('unable to take ownership of the ProgramData\FnKnock root; takeown exited with code ' + $$rootTakeownExitCode) }$\r$\n"
  ; Neutralize a stale explicit deny on the now-owned root, then make it
  ; traversable before the recursive owner repair. Keep the live service SID
  ; while begin runs so a failed preflight does not strand the existing runtime
  ; without its normal ProgramData access.
  FileWrite $R7 "  & $$Icacls $$Path /reset /L /Q | Out-Null$\r$\n"
  FileWrite $R7 "  $$rootResetExitCode = $$LASTEXITCODE$\r$\n"
  FileWrite $R7 "  if ($$rootResetExitCode -ne 0) { throw ('unable to reset the ProgramData\FnKnock root ACL; icacls exited with code ' + $$rootResetExitCode) }$\r$\n"
  FileWrite $R7 "  $$rootGrantArgs = @($$Path, '/inheritance:r', '/grant:r', '*S-1-5-18:(OI)(CI)F', '*S-1-5-32-544:(OI)(CI)F')$\r$\n"
  FileWrite $R7 "  if (-not [string]::IsNullOrWhiteSpace($$ServiceGrant)) { $$rootGrantArgs += $$ServiceGrant }$\r$\n"
  FileWrite $R7 "  $$rootGrantArgs += @('/L', '/Q')$\r$\n"
  FileWrite $R7 "  & $$Icacls @rootGrantArgs | Out-Null$\r$\n"
  FileWrite $R7 "  $$rootGrantExitCode = $$LASTEXITCODE$\r$\n"
  FileWrite $R7 "  if ($$rootGrantExitCode -ne 0) { throw ('unable to grant bootstrap access to the ProgramData\FnKnock root; icacls exited with code ' + $$rootGrantExitCode) }$\r$\n"
  FileWrite $R7 "  $$ownerArgs = @($$Path, '/setowner', '*S-1-5-32-544', '/T', '/L', '/Q')$\r$\n"
  FileWrite $R7 "  & $$Icacls @ownerArgs | Out-Null$\r$\n"
  FileWrite $R7 "  $$ownerExitCode = $$LASTEXITCODE$\r$\n"
  FileWrite $R7 "  if ($$ownerExitCode -ne 0) { throw ('unable to repair ProgramData\FnKnock ownership; icacls exited with code ' + $$ownerExitCode) }$\r$\n"
  ; Ownership alone does not neutralize an explicit deny ACE left by an old or
  ; interrupted install. Reset the complete DACL before applying the allowlist.
  ; Do not use /C: a skipped child would fail later with an opaque .NET
  ; SetAccessControl UnauthorizedAccessException.
  FileWrite $R7 "  & $$Icacls $$Path /reset /T /L /Q | Out-Null$\r$\n"
  FileWrite $R7 "  $$resetExitCode = $$LASTEXITCODE$\r$\n"
  FileWrite $R7 "  if ($$resetExitCode -ne 0) { throw ('unable to reset ProgramData\FnKnock ACLs; icacls exited with code ' + $$resetExitCode) }$\r$\n"
  FileWrite $R7 "  $$grantArgs = @($$Path, '/inheritance:r', '/grant:r', '*S-1-5-18:(OI)(CI)F', '*S-1-5-32-544:(OI)(CI)F')$\r$\n"
  FileWrite $R7 "  if (-not [string]::IsNullOrWhiteSpace($$ServiceGrant)) { $$grantArgs += $$ServiceGrant }$\r$\n"
  FileWrite $R7 "  $$grantArgs += @('/T', '/L', '/Q')$\r$\n"
  FileWrite $R7 "  & $$Icacls @grantArgs | Out-Null$\r$\n"
  FileWrite $R7 "  $$grantExitCode = $$LASTEXITCODE$\r$\n"
  FileWrite $R7 "  if ($$grantExitCode -ne 0) { throw ('unable to grant bootstrap access to ProgramData\FnKnock; icacls exited with code ' + $$grantExitCode) }$\r$\n"
  FileWrite $R7 "}$\r$\n"
  FileWrite $R7 "$\r$\n"

  FileWrite $R7 "function Assert-TreeCopy([string]$$Source, [string]$$Destination) {$\r$\n"
  FileWrite $R7 "  $$base = (Get-Item -LiteralPath $$Source).FullName.TrimEnd([char]'\')$\r$\n"
  FileWrite $R7 "  foreach ($$file in Get-ChildItem -LiteralPath $$Source -File -Recurse -Force) {$\r$\n"
  FileWrite $R7 "    $$relative = $$file.FullName.Substring($$base.Length).TrimStart([char]'\')$\r$\n"
  FileWrite $R7 "    $$copy = Join-Path $$Destination $$relative$\r$\n"
  FileWrite $R7 "    if (-not (Test-Path -LiteralPath $$copy -PathType Leaf)) { throw ('rollback snapshot is missing ' + $$relative) }$\r$\n"
  FileWrite $R7 "    if ((Get-Item -LiteralPath $$copy).Length -ne $$file.Length) { throw ('rollback snapshot size mismatch for ' + $$relative) }$\r$\n"
  FileWrite $R7 "  }$\r$\n"
  FileWrite $R7 "}$\r$\n"
  FileWrite $R7 "$\r$\n"

  FileWrite $R7 "function Test-FnKnockRegistryKey([string]$$SubKey) {$\r$\n"
  FileWrite $R7 "  $$baseKey = [Microsoft.Win32.RegistryKey]::OpenBaseKey([Microsoft.Win32.RegistryHive]::LocalMachine, [Microsoft.Win32.RegistryView]::Registry64)$\r$\n"
  FileWrite $R7 "  try { $$key = $$baseKey.OpenSubKey($$SubKey, $$false); try { return $$null -ne $$key } finally { if ($$null -ne $$key) { $$key.Dispose() } } } finally { $$baseKey.Dispose() }$\r$\n"
  FileWrite $R7 "}$\r$\n"
  FileWrite $R7 "$\r$\n"

  FileWrite $R7 "function Get-FnKnockShortcutState([string]$$Path) {$\r$\n"
  FileWrite $R7 "  try { $$attributes = [IO.File]::GetAttributes($$Path) } catch [IO.FileNotFoundException] { return 'missing' } catch [IO.DirectoryNotFoundException] { return 'missing' }$\r$\n"
  FileWrite $R7 "  if (($$attributes -band [IO.FileAttributes]::Directory) -ne 0) { throw ('the FnKnock shortcut path is a directory: ' + $$Path) }$\r$\n"
  FileWrite $R7 "  if (($$attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) { throw ('the FnKnock shortcut path is a reparse point: ' + $$Path) }$\r$\n"
  FileWrite $R7 "  return 'file'$\r$\n"
  FileWrite $R7 "}$\r$\n"
  FileWrite $R7 "$\r$\n"

  FileWrite $R7 "function Remove-FnKnockShortcut([string]$$Path) {$\r$\n"
  FileWrite $R7 "  try { $$attributes = [IO.File]::GetAttributes($$Path) } catch [IO.FileNotFoundException] { return } catch [IO.DirectoryNotFoundException] { return }$\r$\n"
  FileWrite $R7 "  if (($$attributes -band [IO.FileAttributes]::Directory) -ne 0) { throw ('refusing to remove a directory at the FnKnock shortcut path: ' + $$Path) }$\r$\n"
  FileWrite $R7 "  [IO.File]::Delete($$Path)$\r$\n"
  FileWrite $R7 "  if ([IO.File]::Exists($$Path)) { throw ('unable to remove the FnKnock shortcut: ' + $$Path) }$\r$\n"
  FileWrite $R7 "}$\r$\n"
  FileWrite $R7 "$\r$\n"

  FileWrite $R7 "function Snapshot-FnKnockInstallerMetadata {$\r$\n"
  FileWrite $R7 "  New-Item -ItemType Directory -Force -Path $$registrySnapshot | Out-Null$\r$\n"
  FileWrite $R7 "  New-Item -ItemType Directory -Force -Path $$shortcutSnapshot | Out-Null$\r$\n"
  FileWrite $R7 "  foreach ($$entry in $$registryEntries) {$\r$\n"
  FileWrite $R7 "    $$existsMarker = Join-Path $$registrySnapshot ($$entry.Name + '.exists'); $$missingMarker = Join-Path $$registrySnapshot ($$entry.Name + '.missing'); $$exportPath = Join-Path $$registrySnapshot ($$entry.Name + '.reg')$\r$\n"
  FileWrite $R7 "    if (Test-FnKnockRegistryKey $$entry.SubKey) { & $$reg export $$entry.RegKey $$exportPath /y /reg:64 | Out-Null; if ($$LASTEXITCODE -ne 0 -or -not (Test-Path -LiteralPath $$exportPath -PathType Leaf)) { throw ('unable to export registry key ' + $$entry.RegKey) }; [IO.File]::WriteAllText($$existsMarker, 'exists') } else { [IO.File]::WriteAllText($$missingMarker, 'missing') }$\r$\n"
  FileWrite $R7 "  }$\r$\n"
  FileWrite $R7 "  foreach ($$entry in $$shortcutEntries) {$\r$\n"
  FileWrite $R7 "    $$existsMarker = Join-Path $$shortcutSnapshot ($$entry.Name + '.exists'); $$missingMarker = Join-Path $$shortcutSnapshot ($$entry.Name + '.missing'); $$copyPath = Join-Path $$shortcutSnapshot ($$entry.Name + '.lnk')$\r$\n"
  FileWrite $R7 "    if ((Get-FnKnockShortcutState $$entry.Path) -eq 'file') { [IO.File]::Copy($$entry.Path, $$copyPath, $$false); if ((Get-FileHash -Algorithm SHA256 -LiteralPath $$entry.Path).Hash -ne (Get-FileHash -Algorithm SHA256 -LiteralPath $$copyPath).Hash) { throw ('shortcut snapshot verification failed for ' + $$entry.Path) }; [IO.File]::WriteAllText($$existsMarker, 'exists') } else { [IO.File]::WriteAllText($$missingMarker, 'missing') }$\r$\n"
  FileWrite $R7 "  }$\r$\n"
  FileWrite $R7 "}$\r$\n"
  FileWrite $R7 "$\r$\n"

  FileWrite $R7 "function Restore-FnKnockInstallerMetadata([string]$$Kind) {$\r$\n"
  FileWrite $R7 "  foreach ($$entry in $$registryEntries) { $$existsMarker = Join-Path $$registrySnapshot ($$entry.Name + '.exists'); $$missingMarker = Join-Path $$registrySnapshot ($$entry.Name + '.missing'); $$exportPath = Join-Path $$registrySnapshot ($$entry.Name + '.reg'); $$exists = Test-Path -LiteralPath $$existsMarker -PathType Leaf; $$missing = Test-Path -LiteralPath $$missingMarker -PathType Leaf; if ($$exists -eq $$missing -or ($$exists -and -not (Test-Path -LiteralPath $$exportPath -PathType Leaf))) { throw ('registry transaction snapshot is incomplete for ' + $$entry.RegKey) } }$\r$\n"
  FileWrite $R7 "  foreach ($$entry in $$shortcutEntries) { $$existsMarker = Join-Path $$shortcutSnapshot ($$entry.Name + '.exists'); $$missingMarker = Join-Path $$shortcutSnapshot ($$entry.Name + '.missing'); $$copyPath = Join-Path $$shortcutSnapshot ($$entry.Name + '.lnk'); $$exists = Test-Path -LiteralPath $$existsMarker -PathType Leaf; $$missing = Test-Path -LiteralPath $$missingMarker -PathType Leaf; if ($$exists -eq $$missing -or ($$exists -and -not (Test-Path -LiteralPath $$copyPath -PathType Leaf))) { throw ('shortcut transaction snapshot is incomplete for ' + $$entry.Path) } }$\r$\n"
  FileWrite $R7 "  foreach ($$entry in $$registryEntries) {$\r$\n"
  FileWrite $R7 "    & $$reg delete $$entry.RegKey /f /reg:64 2>&1 | Out-Null$\r$\n"
  FileWrite $R7 "    if (Test-FnKnockRegistryKey $$entry.SubKey) { throw ('unable to remove installer registry key before rollback restore: ' + $$entry.RegKey) }$\r$\n"
  FileWrite $R7 "    if ($$Kind -eq 'upgrade' -and (Test-Path -LiteralPath (Join-Path $$registrySnapshot ($$entry.Name + '.exists')) -PathType Leaf)) { $$exportPath = Join-Path $$registrySnapshot ($$entry.Name + '.reg'); & $$reg import $$exportPath /reg:64 | Out-Null; if ($$LASTEXITCODE -ne 0 -or -not (Test-FnKnockRegistryKey $$entry.SubKey)) { throw ('unable to restore installer registry key ' + $$entry.RegKey) } }$\r$\n"
  FileWrite $R7 "  }$\r$\n"
  FileWrite $R7 "  foreach ($$entry in $$shortcutEntries) {$\r$\n"
  FileWrite $R7 "    Remove-FnKnockShortcut $$entry.Path$\r$\n"
  FileWrite $R7 "    if ($$Kind -eq 'upgrade' -and (Test-Path -LiteralPath (Join-Path $$shortcutSnapshot ($$entry.Name + '.exists')) -PathType Leaf)) { $$copyPath = Join-Path $$shortcutSnapshot ($$entry.Name + '.lnk'); [IO.File]::Copy($$copyPath, $$entry.Path, $$false); if ((Get-FileHash -Algorithm SHA256 -LiteralPath $$copyPath).Hash -ne (Get-FileHash -Algorithm SHA256 -LiteralPath $$entry.Path).Hash) { throw ('shortcut rollback verification failed for ' + $$entry.Path) } }$\r$\n"
  FileWrite $R7 "  }$\r$\n"
  FileWrite $R7 "}$\r$\n"
  FileWrite $R7 "$\r$\n"

  FileWrite $R7 "function Repair-FnKnockDataTreeAfterServiceInstall {$\r$\n"
  FileWrite $R7 "  $$installedService = Get-Service -Name $$serviceName -ErrorAction Stop$\r$\n"
  FileWrite $R7 "  $$installedServiceSid = ([System.Security.Principal.NTAccount]::new('NT SERVICE', $$serviceName)).Translate([System.Security.Principal.SecurityIdentifier])$\r$\n"
  FileWrite $R7 "  $$installedServiceGrant = ('*' + $$installedServiceSid.Value + ':(OI)(CI)M')$\r$\n"
  FileWrite $R7 "  Bootstrap-FnKnockDataTreeAccess $$root $$takeown $$icacls $$installedServiceGrant$\r$\n"
  FileWrite $R7 "  Set-FnKnockDataTreeAcl $$root $$systemSid $$administratorsSid $$installedServiceSid$\r$\n"
  FileWrite $R7 "  Set-FnKnockDataTreeOwner $$root $$icacls$\r$\n"
  FileWrite $R7 "  $$usersSid = [System.Security.Principal.SecurityIdentifier]::new('S-1-5-32-545')$\r$\n"
  FileWrite $R7 "  Set-FnKnockDataTreeAcl $$state $$systemSid $$administratorsSid $$installedServiceSid $$usersSid$\r$\n"
  FileWrite $R7 "  Set-FnKnockDataTreeOwner $$state $$icacls$\r$\n"
  FileWrite $R7 "  Bootstrap-FnKnockDataTreeAccess $$rollback $$takeown $$icacls $$null$\r$\n"
  FileWrite $R7 "  Set-FnKnockDataTreeAcl $$rollback $$systemSid $$administratorsSid $$null$\r$\n"
  FileWrite $R7 "  Set-FnKnockDataTreeOwner $$rollback $$icacls$\r$\n"
  FileWrite $R7 "}$\r$\n"
  FileWrite $R7 "$\r$\n"

  FileWrite $R7 "function Wait-FnKnockReady {$\r$\n"
  FileWrite $R7 "  if (-not (Test-Path -LiteralPath $$bundleIdentityPath -PathType Leaf)) { throw 'the installed bundle identity is missing' }$\r$\n"
  FileWrite $R7 "  $$bundleIdentity = Get-Content -Raw -LiteralPath $$bundleIdentityPath | ConvertFrom-Json$\r$\n"
  FileWrite $R7 "  $$expectedVersion = [string]$$bundleIdentity.version$\r$\n"
  ; Bundles produced before control_api_version was added already implement
  ; version 1 of the local control contract. Keep that one legacy shape
  ; upgradeable, while still rejecting an explicitly invalid value.
  FileWrite $R7 "  $$controlApiProperty = $$bundleIdentity.PSObject.Properties['control_api_version']$\r$\n"
  FileWrite $R7 "  $$expectedControlApiVersion = if ($$null -eq $$controlApiProperty) { 1 } else { [int]$$controlApiProperty.Value }$\r$\n"
  FileWrite $R7 "  if ([string]::IsNullOrWhiteSpace($$expectedVersion) -or $$expectedControlApiVersion -le 0) { throw 'the installed bundle identity is invalid' }$\r$\n"
  FileWrite $R7 "  $$port = 7991$\r$\n"
  FileWrite $R7 "  $$runtime = Join-Path $$root 'config\runtime.json'$\r$\n"
  FileWrite $R7 "  if (Test-Path -LiteralPath $$runtime) {$\r$\n"
  FileWrite $R7 "    try { $$configured = Get-Content -Raw -LiteralPath $$runtime | ConvertFrom-Json; if ($$configured.admin_port) { $$port = [int]$$configured.admin_port } } catch {}$\r$\n"
  FileWrite $R7 "  }$\r$\n"
  FileWrite $R7 "  $$deadline = [DateTime]::UtcNow.AddSeconds(60)$\r$\n"
  FileWrite $R7 "  $$consecutiveReady = 0$\r$\n"
  FileWrite $R7 "  $$lastDetail = 'the service has not exposed its readiness endpoint'$\r$\n"
  FileWrite $R7 "  while ([DateTime]::UtcNow -lt $$deadline) {$\r$\n"
  FileWrite $R7 "    $$service = Get-Service -Name $$serviceName -ErrorAction SilentlyContinue$\r$\n"
  FileWrite $R7 "    if ($$null -eq $$service) { throw 'the FnKnock service disappeared during startup' }$\r$\n"
  FileWrite $R7 "    $$service.Refresh()$\r$\n"
  FileWrite $R7 "    if ($$service.Status -eq 'Stopped') {$\r$\n"
  FileWrite $R7 "      $$detail = 'the FnKnock service stopped before the runtime became ready'$\r$\n"
  FileWrite $R7 "      $$statusPath = Join-Path $$root 'state\status.json'$\r$\n"
  FileWrite $R7 "      try { $$status = Get-Content -Raw -LiteralPath $$statusPath | ConvertFrom-Json; if ($$status.error) { $$detail += ': ' + [string]$$status.error } } catch {}$\r$\n"
  FileWrite $R7 "      throw $$detail$\r$\n"
  FileWrite $R7 "    }$\r$\n"
  FileWrite $R7 "    try {$\r$\n"
  FileWrite $R7 "      $$runtimeService = Get-CimInstance -ClassName Win32_Service -Filter 'Name=''FnKnock''' -ErrorAction Stop$\r$\n"
  FileWrite $R7 "      if ($$service.Status -ne 'Running' -or $$null -eq $$runtimeService -or $$runtimeService.State -ne 'Running' -or [uint32]$$runtimeService.ProcessId -eq 0) { $$lastDetail = 'service state is ' + [string]$$service.Status; $$consecutiveReady = 0; Start-Sleep -Milliseconds 750; continue }$\r$\n"
  FileWrite $R7 "      $$serviceProcessId = [uint32]$$runtimeService.ProcessId$\r$\n"
  FileWrite $R7 "      $$listener = Get-NetTCPConnection -State Listen -LocalAddress '127.0.0.1' -LocalPort $$port -ErrorAction Stop | Where-Object { [uint32]$$_.OwningProcess -eq $$serviceProcessId } | Select-Object -First 1$\r$\n"
  FileWrite $R7 "      if ($$null -eq $$listener) { $$lastDetail = 'the running service process is not listening on 127.0.0.1:' + $$port; $$consecutiveReady = 0; Start-Sleep -Milliseconds 750; continue }$\r$\n"
  FileWrite $R7 "      $$response = Invoke-WebRequest -UseBasicParsing -TimeoutSec 2 -Uri ('http://127.0.0.1:' + $$port + '/__fn-knock/readyz')$\r$\n"
  FileWrite $R7 "      $$body = $$response.Content | ConvertFrom-Json$\r$\n"
  FileWrite $R7 "      $$components = $$body.components$\r$\n"
  FileWrite $R7 "      $$contractReady = $$response.StatusCode -eq 200 -and $$body.ready -eq $$true -and ([string]$$body.version).Equals($$expectedVersion, [StringComparison]::Ordinal) -and [int]$$body.control_api_version -eq $$expectedControlApiVersion -and $$components.storage -eq $$true -and $$components.gateway_bundle -eq $$true -and $$components.gateway_process -eq $$true -and $$components.gateway_dataplane -eq $$true -and $$components.auth_bridge -eq $$true$\r$\n"
  FileWrite $R7 "      $$service.Refresh(); $$confirmedService = Get-CimInstance -ClassName Win32_Service -Filter 'Name=''FnKnock''' -ErrorAction Stop$\r$\n"
  FileWrite $R7 "      $$confirmedListener = Get-NetTCPConnection -State Listen -LocalAddress '127.0.0.1' -LocalPort $$port -ErrorAction Stop | Where-Object { [uint32]$$_.OwningProcess -eq $$serviceProcessId } | Select-Object -First 1$\r$\n"
  FileWrite $R7 "      if ($$contractReady -and $$service.Status -eq 'Running' -and $$null -ne $$confirmedService -and $$confirmedService.State -eq 'Running' -and [uint32]$$confirmedService.ProcessId -eq $$serviceProcessId -and $$null -ne $$confirmedListener) { $$consecutiveReady += 1; if ($$consecutiveReady -ge 2) { return } } else { $$lastDetail = 'readyz contract mismatch: ' + ($$body | ConvertTo-Json -Compress -Depth 5); $$consecutiveReady = 0 }$\r$\n"
  FileWrite $R7 "    } catch { $$lastDetail = $$_.Exception.Message; $$consecutiveReady = 0 }$\r$\n"
  FileWrite $R7 "    Start-Sleep -Milliseconds 750$\r$\n"
  FileWrite $R7 "  }$\r$\n"
  FileWrite $R7 "  $$statusPath = Join-Path $$root 'state\status.json'; try { $$status = Get-Content -Raw -LiteralPath $$statusPath | ConvertFrom-Json; if ($$status.error) { $$lastDetail += '; service status: ' + [string]$$status.error } elseif ($$status.message) { $$lastDetail += '; service status: ' + [string]$$status.message } } catch {}$\r$\n"
  FileWrite $R7 "  throw ('the complete FnKnock runtime did not become ready within 60 seconds: ' + $$lastDetail)$\r$\n"
  FileWrite $R7 "}$\r$\n"
  FileWrite $R7 "$\r$\n"

  FileWrite $R7 "function Install-And-StartFnKnock {$\r$\n"
  FileWrite $R7 "  if (-not (Test-Path -LiteralPath $$serviceExe -PathType Leaf)) { throw 'fn-knock-service.exe is missing' }$\r$\n"
  FileWrite $R7 "  & $$serviceExe install$\r$\n"
  FileWrite $R7 "  if ($$LASTEXITCODE -ne 0) { throw ('service registration failed with exit code ' + $$LASTEXITCODE) }$\r$\n"
  FileWrite $R7 "  Repair-FnKnockDataTreeAfterServiceInstall$\r$\n"
  FileWrite $R7 "  $$service = Get-Service -Name $$serviceName -ErrorAction Stop$\r$\n"
  FileWrite $R7 "  if ($$service.Status -ne 'Running' -and $$service.Status -ne 'StartPending') {$\r$\n"
  FileWrite $R7 "    & $$serviceExe start$\r$\n"
  FileWrite $R7 "    $$startCode = $$LASTEXITCODE$\r$\n"
  FileWrite $R7 "    $$service = Get-Service -Name $$serviceName -ErrorAction Stop$\r$\n"
  FileWrite $R7 "    if ($$startCode -ne 0 -and $$service.Status -ne 'Running') { throw ('service start failed with exit code ' + $$startCode) }$\r$\n"
  FileWrite $R7 "  }$\r$\n"
  FileWrite $R7 "  Wait-FnKnockReady$\r$\n"
  FileWrite $R7 "}$\r$\n"
  FileWrite $R7 "$\r$\n"

  FileWrite $R7 "function Remove-PartialFnKnockInstall {$\r$\n"
  FileWrite $R7 "  Stop-FnKnock$\r$\n"
  FileWrite $R7 "  Assert-NoReparseTree $$root; Assert-NoReparseTree $$pending; Assert-NoReparseTree $$install$\r$\n"
  FileWrite $R7 "  if (Test-Path -LiteralPath $$serviceExe -PathType Leaf) { try { & $$serviceExe uninstall | Out-Null } catch {} }$\r$\n"
  FileWrite $R7 "  $$service = Get-Service -Name $$serviceName -ErrorAction SilentlyContinue$\r$\n"
  FileWrite $R7 "  if ($$null -ne $$service) { & $$sc delete $$serviceName | Out-Null; if ($$LASTEXITCODE -ne 0 -and $$LASTEXITCODE -ne 1072) { throw ('SCM service deletion failed with exit code ' + $$LASTEXITCODE) } }$\r$\n"
  FileWrite $R7 "  Get-NetFirewallRule -DisplayName 'FnKnock Gateway' -ErrorAction SilentlyContinue | Remove-NetFirewallRule -ErrorAction Stop$\r$\n"
  FileWrite $R7 "  if ((Test-Path -LiteralPath $$snapshotReady -PathType Leaf) -and (Test-Path -LiteralPath $$install -PathType Container)) { Get-ChildItem -LiteralPath $$install -Force -ErrorAction Stop | Remove-Item -Recurse -Force }$\r$\n"
  FileWrite $R7 "}$\r$\n"
  FileWrite $R7 "$\r$\n"

  FileWrite $R7 "function Restore-FnKnockUpgrade {$\r$\n"
  FileWrite $R7 "  Stop-FnKnock$\r$\n"
  FileWrite $R7 "  Assert-NoReparseTree $$root; Assert-NoReparseTree $$pending; Assert-NoReparseTree $$install$\r$\n"
  FileWrite $R7 "  $$bundle = Join-Path $$pending 'bundle'$\r$\n"
  FileWrite $R7 "  if (-not (Test-Path -LiteralPath $$snapshotReady -PathType Leaf)) {$\r$\n"
  FileWrite $R7 "    Install-And-StartFnKnock$\r$\n"
  FileWrite $R7 "    return$\r$\n"
  FileWrite $R7 "  }$\r$\n"
  FileWrite $R7 "  if (-not (Test-Path -LiteralPath (Join-Path $$bundle 'fn-knock-service.exe') -PathType Leaf)) { throw 'the rollback bundle is incomplete' }$\r$\n"
  FileWrite $R7 "  New-Item -ItemType Directory -Force -Path $$install | Out-Null$\r$\n"
  FileWrite $R7 "  Get-ChildItem -LiteralPath $$install -Force -ErrorAction SilentlyContinue | Remove-Item -Recurse -Force$\r$\n"
  FileWrite $R7 "  Get-ChildItem -LiteralPath $$bundle -Force | Copy-Item -Destination $$install -Recurse -Force$\r$\n"
  FileWrite $R7 "  Assert-TreeCopy $$bundle $$install$\r$\n"
  FileWrite $R7 "  foreach ($$name in $$persistentDataTrees) {$\r$\n"
  FileWrite $R7 "    $$current = Join-Path $$root $$name$\r$\n"
  FileWrite $R7 "    Remove-Item -LiteralPath $$current -Recurse -Force -ErrorAction SilentlyContinue$\r$\n"
  FileWrite $R7 "    $$snapshot = Join-Path (Join-Path $$pending 'data') $$name$\r$\n"
  FileWrite $R7 "    if (Test-Path -LiteralPath $$snapshot) { Copy-Item -LiteralPath $$snapshot -Destination $$current -Recurse -Force; Assert-TreeCopy $$snapshot $$current }$\r$\n"
  FileWrite $R7 "  }$\r$\n"
  ; Restore the old externally visible identity before attempting to start the
  ; old runtime. If startup still fails, Add/Remove Programs and shortcuts no
  ; longer advertise the failed new version while the snapshot is preserved.
  FileWrite $R7 "  Restore-FnKnockInstallerMetadata 'upgrade'$\r$\n"
  FileWrite $R7 "  Install-And-StartFnKnock$\r$\n"
  FileWrite $R7 "}$\r$\n"
  FileWrite $R7 "$\r$\n"

  FileWrite $R7 "function Rollback-FnKnockTransaction {$\r$\n"
  FileWrite $R7 "  if (-not (Test-Path -LiteralPath $$marker -PathType Leaf)) { return }$\r$\n"
  FileWrite $R7 "  Assert-NoReparseTree $$pending$\r$\n"
  FileWrite $R7 "  $$kind = (Get-Content -Raw -LiteralPath $$marker).Trim()$\r$\n"
  ; Before snapshot.ready is committed, extraction and installer metadata
  ; mutation have not started. A crash in this window may leave only a partial
  ; metadata snapshot, so preserve the live registry/shortcuts and discard the
  ; unarmed transaction after making an existing upgrade runtime ready again.
  FileWrite $R7 "  if (-not (Test-Path -LiteralPath $$snapshotReady -PathType Leaf)) { if ($$kind -eq 'upgrade') { Install-And-StartFnKnock } elseif ($$kind -ne 'first' -and $$kind -ne 'repair-first') { throw ('unknown installer transaction kind ' + $$kind) }; Remove-Item -LiteralPath $$marker -Force; Remove-Item -LiteralPath $$pending -Recurse -Force -ErrorAction SilentlyContinue; return }$\r$\n"
  FileWrite $R7 "  if ($$kind -eq 'upgrade') { Restore-FnKnockUpgrade } elseif ($$kind -eq 'first' -or $$kind -eq 'repair-first') { if (Test-Path -LiteralPath $$snapshotReady -PathType Leaf) { Remove-PartialFnKnockInstall } } else { throw ('unknown installer transaction kind ' + $$kind) }$\r$\n"
  FileWrite $R7 "  Restore-FnKnockInstallerMetadata $$kind$\r$\n"
  FileWrite $R7 "  Remove-Item -LiteralPath $$marker -Force$\r$\n"
  FileWrite $R7 "  Remove-Item -LiteralPath $$pending -Recurse -Force -ErrorAction SilentlyContinue$\r$\n"
  FileWrite $R7 "}$\r$\n"
  FileWrite $R7 "$\r$\n"

  ; icacls retains the Program Files application-package ACEs on supported
  ; Windows 10 builds even after /inheritance:r. The helper was loaded from a
  ; directory where those identities cannot write its contents; now replace
  ; every DACL with the same exact ACL constructors used for protected runtime
  ; data, restore the SYSTEM owner, and fail closed unless verification agrees.
  FileWrite $R7 "  Set-FnKnockDataTreeAcl $$PSScriptRoot $$systemSid $$administratorsSid $$null$\r$\n"
  FileWrite $R7 "  Set-FnKnockDataTreeOwner $$PSScriptRoot $$icacls$\r$\n"
  FileWrite $R7 "  Assert-FnKnockInstallerTreeAcl $$PSScriptRoot$\r$\n"
  FileWrite $R7 "  switch ($$Action) {$\r$\n"
  FileWrite $R7 "    'begin' {$\r$\n"
  ; Do not inspect a stale transaction until ProgramData and its existing tree
  ; have an installer-owned ACL. Preserve the live service SID during upgrades.
  ; Directory.Exists returns false for some access-denied trees. Enumerate the
  ; trusted ProgramData parent so an inaccessible existing root is recovered
  ; instead of being mistaken for a missing directory.
  FileWrite $R7 "    $$rootEntry = [IO.DirectoryInfo]::new($$programData).EnumerateFileSystemInfos('FnKnock', [IO.SearchOption]::TopDirectoryOnly) | Select-Object -First 1$\r$\n"
  FileWrite $R7 "    if ($$null -eq $$rootEntry) { [void][IO.Directory]::CreateDirectory($$root) } else {$\r$\n"
  FileWrite $R7 "      if (($$rootEntry.Attributes -band [IO.FileAttributes]::Directory) -eq 0) { throw 'ProgramData\FnKnock exists but is not a directory' }$\r$\n"
  FileWrite $R7 "      if (($$rootEntry.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) { throw 'refusing a reparse point at ProgramData\FnKnock' }$\r$\n"
  FileWrite $R7 "    }$\r$\n"
  FileWrite $R7 "    $$serviceSid = $$null$\r$\n"
  FileWrite $R7 "    if ($$null -ne (Get-Service -Name $$serviceName -ErrorAction SilentlyContinue)) { $$serviceSid = ([System.Security.Principal.NTAccount]::new('NT SERVICE', $$serviceName)).Translate([System.Security.Principal.SecurityIdentifier]) }$\r$\n"
  FileWrite $R7 "    $$serviceGrant = $$null$\r$\n"
  FileWrite $R7 "    if ($$null -ne $$serviceSid) { $$serviceGrant = ('*' + $$serviceSid.Value + ':(OI)(CI)M') }$\r$\n"
  FileWrite $R7 "    Bootstrap-FnKnockDataTreeAccess $$root $$takeown $$icacls $$serviceGrant$\r$\n"
  FileWrite $R7 "    Set-FnKnockDataTreeAcl $$root $$systemSid $$administratorsSid $$serviceSid$\r$\n"
  FileWrite $R7 "    Set-FnKnockDataTreeOwner $$root $$icacls$\r$\n"
  FileWrite $R7 "    foreach ($$directory in @('config','data','gateway','certificates','waf','secrets','logs','state','rollback')) { New-Item -ItemType Directory -Force -Path (Join-Path $$root $$directory) | Out-Null }$\r$\n"
  FileWrite $R7 "    Set-FnKnockDataTreeAcl $$root $$systemSid $$administratorsSid $$serviceSid$\r$\n"
  FileWrite $R7 "    Set-FnKnockDataTreeOwner $$root $$icacls$\r$\n"
  FileWrite $R7 "    Set-FnKnockDataTreeAcl $$rollback $$systemSid $$administratorsSid $$null$\r$\n"
  FileWrite $R7 "    Set-FnKnockDataTreeOwner $$rollback $$icacls$\r$\n"
  FileWrite $R7 "    Rollback-FnKnockTransaction$\r$\n"
  FileWrite $R7 "    Remove-Item -LiteralPath $$pending -Recurse -Force -ErrorAction SilentlyContinue$\r$\n"
  FileWrite $R7 "    if (Test-Path -LiteralPath $$pending) { throw 'a stale installer transaction could not be removed' }$\r$\n"
  FileWrite $R7 "    New-Item -ItemType Directory -Force -Path (Join-Path $$pending 'bundle') | Out-Null$\r$\n"
  FileWrite $R7 "    New-Item -ItemType Directory -Force -Path (Join-Path $$pending 'data') | Out-Null$\r$\n"
  FileWrite $R7 "    Set-FnKnockDataTreeAcl $$pending $$systemSid $$administratorsSid $$null$\r$\n"
  FileWrite $R7 "    Set-FnKnockDataTreeOwner $$pending $$icacls$\r$\n"
  ; Upgrade is accepted only for a complete old runtime. A matching dangling
  ; SCM entry with no files is an explicit repair-first case; every partial or
  ; foreign state is preserved and rejected.
  FileWrite $R7 "    $$requiredBundlePaths = @($$serviceExe, $$gatewayExe, $$bundleIdentityPath); $$presentRequiredBundleFiles = 0$\r$\n"
  FileWrite $R7 "    foreach ($$requiredPath in $$requiredBundlePaths) { $$requiredExists = $$true; try { $$requiredAttributes = [IO.File]::GetAttributes($$requiredPath) } catch [IO.FileNotFoundException] { $$requiredExists = $$false } catch [IO.DirectoryNotFoundException] { $$requiredExists = $$false }; if ($$requiredExists) { if (($$requiredAttributes -band [IO.FileAttributes]::Directory) -ne 0 -or ($$requiredAttributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) { throw ('a required bundle path is not an ordinary file: ' + $$requiredPath) }; $$presentRequiredBundleFiles += 1 } }$\r$\n"
  FileWrite $R7 "    if ($$presentRequiredBundleFiles -gt 0 -and $$presentRequiredBundleFiles -lt $$requiredBundlePaths.Count) { throw 'the existing FnKnock bundle is partial; all existing content was preserved' }$\r$\n"
  FileWrite $R7 "    $$currentService = Get-Service -Name $$serviceName -ErrorAction SilentlyContinue; $$currentServiceRecord = Get-CimInstance -ClassName Win32_Service -Filter 'Name=''FnKnock''' -ErrorAction Stop$\r$\n"
  FileWrite $R7 "    if (($$null -eq $$currentService) -ne ($$null -eq $$currentServiceRecord)) { throw 'SCM and Win32_Service returned inconsistent FnKnock service state during classification' }$\r$\n"
  FileWrite $R7 "    if ($$null -ne $$currentServiceRecord) { $$currentRegisteredCommand = ([string]$$currentServiceRecord.PathName).Trim(); if (-not $$currentRegisteredCommand.Equals($$expectedServiceCommand, [StringComparison]::OrdinalIgnoreCase) -and -not $$currentRegisteredCommand.Equals($$legacyServiceCommand, [StringComparison]::OrdinalIgnoreCase)) { throw ('the FnKnock service changed to an unexpected command during transaction recovery: ' + $$currentRegisteredCommand) } }$\r$\n"
  FileWrite $R7 "    if ($$presentRequiredBundleFiles -eq $$requiredBundlePaths.Count) {$\r$\n"
  FileWrite $R7 "      Assert-NoReparseTree $$install$\r$\n"
  FileWrite $R7 "      try {$\r$\n"
  FileWrite $R7 "        $$existingBundleIdentity = Get-Content -Raw -LiteralPath $$bundleIdentityPath | ConvertFrom-Json$\r$\n"
  FileWrite $R7 "        $$existingBundleVersion = [string]$$existingBundleIdentity.version$\r$\n"
  FileWrite $R7 "        $$existingControlApiProperty = $$existingBundleIdentity.PSObject.Properties['control_api_version']$\r$\n"
  FileWrite $R7 "        $$existingControlApiVersion = if ($$null -eq $$existingControlApiProperty) { 1 } else { [int]$$existingControlApiProperty.Value }$\r$\n"
  FileWrite $R7 "        $$existingTarget = [string]$$existingBundleIdentity.target$\r$\n"
  FileWrite $R7 "      } catch { throw ('the existing FnKnock bundle identity is malformed and was preserved: ' + $$_.Exception.Message) }$\r$\n"
  FileWrite $R7 "      if ([string]::IsNullOrWhiteSpace($$existingBundleVersion) -or $$existingControlApiVersion -le 0 -or -not $$existingTarget.Equals('windows-x86_64', [StringComparison]::Ordinal)) { throw 'the existing FnKnock bundle identity is invalid and was preserved' }$\r$\n"
  FileWrite $R7 "      $$kind = 'upgrade'$\r$\n"
  FileWrite $R7 "    } elseif ($$null -ne $$currentService) { $$kind = 'repair-first' } else { $$kind = 'first' }$\r$\n"
  FileWrite $R7 "    if (($$kind -eq 'first' -or $$kind -eq 'repair-first') -and $$null -ne $$installEntry) { $$unexpectedInstallEntry = Get-ChildItem -LiteralPath $$install -Force -ErrorAction Stop | Select-Object -First 1; if ($$null -ne $$unexpectedInstallEntry) { throw ('refusing ' + $$kind + ' installation into a non-empty directory; existing content was preserved: ' + $$unexpectedInstallEntry.FullName) } }$\r$\n"
  FileWrite $R7 "    Snapshot-FnKnockInstallerMetadata$\r$\n"
  FileWrite $R7 "    [IO.File]::WriteAllText($$marker, $$kind)$\r$\n"
  FileWrite $R7 "    Set-FnKnockDataTreeAcl $$pending $$systemSid $$administratorsSid $$null$\r$\n"
  FileWrite $R7 "    Set-FnKnockDataTreeOwner $$pending $$icacls$\r$\n"
  FileWrite $R7 "  }$\r$\n"
  FileWrite $R7 "    'stop' { Stop-FnKnock }$\r$\n"
  FileWrite $R7 "    'snapshot' {$\r$\n"
  FileWrite $R7 "    if (-not (Test-Path -LiteralPath $$marker -PathType Leaf)) { throw 'the installer transaction marker is missing' }$\r$\n"
  FileWrite $R7 "    Assert-NoReparseTree $$root; Assert-NoReparseTree $$pending; Assert-NoReparseTree $$install$\r$\n"
  FileWrite $R7 "    $$kind = (Get-Content -Raw -LiteralPath $$marker).Trim()$\r$\n"
  FileWrite $R7 "    if ($$kind -eq 'upgrade') {$\r$\n"
  FileWrite $R7 "      $$bundle = Join-Path $$pending 'bundle'$\r$\n"
  FileWrite $R7 "      Get-ChildItem -LiteralPath $$install -Force | Copy-Item -Destination $$bundle -Recurse -Force$\r$\n"
  FileWrite $R7 "      Assert-TreeCopy $$install $$bundle$\r$\n"
  FileWrite $R7 "      foreach ($$required in @('fn-knock-service.exe','fn-knock-gateway.exe','bundle.json')) { if (-not (Test-Path -LiteralPath (Join-Path $$bundle $$required) -PathType Leaf)) { throw ('rollback snapshot is missing ' + $$required) } }$\r$\n"
  FileWrite $R7 "      foreach ($$name in $$persistentDataTrees) { $$source = Join-Path $$root $$name; if (Test-Path -LiteralPath $$source) { $$copy = Join-Path (Join-Path $$pending 'data') $$name; Copy-Item -LiteralPath $$source -Destination $$copy -Recurse -Force; Assert-TreeCopy $$source $$copy } }$\r$\n"
  FileWrite $R7 "      [IO.File]::WriteAllText($$snapshotReady, 'ready')$\r$\n"
  FileWrite $R7 "      Get-ChildItem -LiteralPath $$install -Force -ErrorAction Stop | Remove-Item -Recurse -Force$\r$\n"
  FileWrite $R7 "      & $$icacls $$install /reset /Q | Out-Null$\r$\n"
  FileWrite $R7 "      if ($$LASTEXITCODE -ne 0) { throw ('unable to reset the install directory ACL; icacls exited with code ' + $$LASTEXITCODE) }$\r$\n"
  FileWrite $R7 "    } elseif ($$kind -eq 'first' -or $$kind -eq 'repair-first') {$\r$\n"
  FileWrite $R7 "      if ($$null -ne $$installEntry) {$\r$\n"
  FileWrite $R7 "        $$unexpectedInstallEntry = Get-ChildItem -LiteralPath $$install -Force -ErrorAction Stop | Select-Object -First 1$\r$\n"
  FileWrite $R7 "        if ($$null -ne $$unexpectedInstallEntry) { Remove-Item -LiteralPath $$marker -Force -ErrorAction Stop; throw ('the ' + $$kind + ' directory became non-empty before extraction; existing content was preserved: ' + $$unexpectedInstallEntry.FullName) }$\r$\n"
  FileWrite $R7 "        & $$icacls $$install /reset /Q | Out-Null$\r$\n"
  FileWrite $R7 "        if ($$LASTEXITCODE -ne 0) { throw ('unable to reset the empty install directory ACL; icacls exited with code ' + $$LASTEXITCODE) }$\r$\n"
  FileWrite $R7 "      }$\r$\n"
  FileWrite $R7 "      [IO.File]::WriteAllText($$snapshotReady, 'ready')$\r$\n"
  FileWrite $R7 "    } else { throw ('unknown installer transaction kind ' + $$kind) }$\r$\n"
  FileWrite $R7 "  }$\r$\n"
  FileWrite $R7 "    'rollback' { Rollback-FnKnockTransaction }$\r$\n"
  FileWrite $R7 "    'protect-rollback' { Repair-FnKnockDataTreeAfterServiceInstall }$\r$\n"
  FileWrite $R7 "    'wait-ready' { Wait-FnKnockReady }$\r$\n"
  FileWrite $R7 "  }$\r$\n"
  FileWrite $R7 "} catch {$\r$\n"
  ; PowerShell's default non-interactive error record is much longer than an
  ; NSIS string and used to hide the actionable exception behind formatting.
  FileWrite $R7 "  [Console]::Error.WriteLine(('FnKnock installer ' + $$Action + ' failed: ' + $$_.Exception.Message))$\r$\n"
  FileWrite $R7 "  exit 1$\r$\n"
  FileWrite $R7 "}$\r$\n"

  ${If} ${Errors}
    FileClose $R7
    StrCpy $0 1
    StrCpy $1 "unable to write the installer transaction helper"
    Return
  ${EndIf}
  FileClose $R7
  ; File creation gives the elevated administrator ownership. Reapply the exact
  ; allowlist and SYSTEM owner before PowerShell reads the helper.
  Call FnKnockProtectTransactionDirectory
  ${If} $0 == 0
    StrCpy $FnKnockTransactionReady 1
  ${EndIf}
FunctionEnd

!macro FNKNOCK_RUN_TRANSACTION ACTION
  Call FnKnockWriteTransactionScript
  ${If} $0 == 0
    nsExec::ExecToStack '"$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$FnKnockTransactionScript" -Action ${ACTION} -InstallDir "$INSTDIR" -ProgramFilesDir "$PROGRAMFILES64"'
    Pop $0
    Pop $1
  ${EndIf}
!macroend

; Stop is cooperative: Rust asks Go to drain for up to 15 seconds. Never touch
; the bundle or SQLite until SCM confirms the whole runtime group is stopped.
!macro FNKNOCK_STOP_AND_WAIT
  nsExec::ExecToStack '"$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -NonInteractive -ExecutionPolicy Bypass -Command "$$ErrorActionPreference = $\'Stop$\'; $$service = Get-Service -Name $\'${FNKNOCK_SERVICE}$\' -ErrorAction SilentlyContinue; if ($$null -ne $$service -and $$service.Status -ne $\'Stopped$\') { Stop-Service -Name $\'${FNKNOCK_SERVICE}$\' -ErrorAction Stop; $$service.WaitForStatus($\'Stopped$\', [TimeSpan]::FromSeconds(20)) }; if ($$null -ne (Get-Service -Name $\'${FNKNOCK_SERVICE}$\' -ErrorAction SilentlyContinue) -and (Get-Service -Name $\'${FNKNOCK_SERVICE}$\').Status -ne $\'Stopped$\') { throw $\'FnKnock did not stop within 20 seconds$\' }"'
  Pop $0
  Pop $1
!macroend

; MUI owns .onUserAbort. This callback is invoked from MUI before the installer
; exits, so a normal cancel is covered by the same idempotent transaction path.
!define MUI_CUSTOMFUNCTION_ABORT FnKnockInstallerUserAbort

Function FnKnockInstallerUserAbort
  ${If} $FnKnockTransactionReady == 1
    SetOutPath "$PROGRAMFILES64"
    !insertmacro FNKNOCK_RUN_TRANSACTION rollback
    ${If} $0 != 0
      MessageBox MB_OK|MB_ICONSTOP "FnKnock could not roll back the cancelled setup. The recovery snapshot was preserved under ProgramData\FnKnock\rollback\pending: $1"
      Abort
    ${EndIf}
  ${EndIf}
FunctionEnd

; File extraction and registry failures happen between PREINSTALL and
; POSTINSTALL. These callbacks make those failures transactional too.
Function .onInstFailed
  ${If} $FnKnockTransactionReady == 1
    SetOutPath "$PROGRAMFILES64"
    !insertmacro FNKNOCK_RUN_TRANSACTION rollback
    ${If} $0 != 0
      MessageBox MB_OK|MB_ICONSTOP "FnKnock automatic rollback failed. The recovery snapshot was preserved under ProgramData\FnKnock\rollback\pending: $1"
    ${EndIf}
  ${EndIf}
FunctionEnd

Function .onGUIEnd
  ; Idempotent final safety net. A successful install has already removed the
  ; transaction marker, so this is a no-op on the normal path.
  ${If} $FnKnockTransactionReady == 1
    SetOutPath "$PROGRAMFILES64"
    !insertmacro FNKNOCK_RUN_TRANSACTION rollback
  ${EndIf}
  Call FnKnockCleanupTransactionDirectory
FunctionEnd

!macro NSIS_HOOK_PREINSTALL
  ; Reject /D= and registry-derived paths before launching any elevated helper.
  ${If} $INSTDIR != "$PROGRAMFILES64\Knock 敲门"
  ${AndIf} $INSTDIR != "$PROGRAMFILES64\Knock 敲门\current"
    Abort "Knock 敲门只能安装到 64 位 Program Files\Knock 敲门。未更改任何文件。"
  ${EndIf}
  ; The installer enters $INSTDIR before this hook. Use a trusted working directory for
  ; every external process, then restore the extraction directory at the end.
  SetOutPath "$PROGRAMFILES64"

  ; Check the GUI before stopping SCM so cancellation cannot take the gateway down.
  !insertmacro CheckIfAppIsRunning "fn-knock.exe" "FnKnock"

  !insertmacro FNKNOCK_RUN_TRANSACTION begin
  ${If} $0 != 0
    Abort "Unable to initialize the FnKnock installer transaction. Any existing recovery snapshot was preserved under ProgramData\FnKnock\rollback\pending: $1"
  ${EndIf}

  DetailPrint "Stopping the FnKnock runtime group"
  !insertmacro FNKNOCK_RUN_TRANSACTION stop
  ${If} $0 != 0
    StrCpy $R9 "FnKnock did not stop safely: $1"
    !insertmacro FNKNOCK_RUN_TRANSACTION rollback
    ${If} $0 != 0
      Abort "$R9 Automatic recovery also failed; ProgramData\FnKnock\rollback\pending was preserved: $1"
    ${EndIf}
    Abort "$R9 The previous installation was restored."
  ${EndIf}

  ; The helper copies and verifies the complete old bundle plus config/SQLite,
  ; writes snapshot.ready, and only then removes files from $INSTDIR.
  DetailPrint "Creating and verifying the FnKnock rollback snapshot"
  !insertmacro FNKNOCK_RUN_TRANSACTION snapshot
  ${If} $0 != 0
    StrCpy $R9 "FnKnock rollback snapshot or old-bundle cleanup failed: $1"
    !insertmacro FNKNOCK_RUN_TRANSACTION rollback
    ${If} $0 != 0
      Abort "$R9 Automatic recovery also failed; ProgramData\FnKnock\rollback\pending was preserved: $1"
    ${EndIf}
    Abort "$R9 The previous installation was restored."
  ${EndIf}

  SetOutPath "$INSTDIR"
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ; Per-machine SetShellVarContext maps $APPDATA to %ProgramData%.
  CreateDirectory "$APPDATA\FnKnock\config"
  CreateDirectory "$APPDATA\FnKnock\data"
  CreateDirectory "$APPDATA\FnKnock\certificates"
  CreateDirectory "$APPDATA\FnKnock\waf"
  CreateDirectory "$APPDATA\FnKnock\logs"
  CreateDirectory "$APPDATA\FnKnock\state"
  CreateDirectory "$APPDATA\FnKnock\rollback"

  ; The signed service binary is the single owner of SCM recovery, ACL and
  ; program-level Domain/Private firewall configuration.
  nsExec::ExecToStack '"$INSTDIR\fn-knock-service.exe" install'
  Pop $0
  Pop $1
  ${If} $0 != 0
    StrCpy $R9 "service registration failed: $1"
    Goto fnknock_rollback
  ${EndIf}

  ; Both legacy and current service installers may recursively touch
  ; ProgramData ACLs. Reassert the rollback allowlist before any service start.
  !insertmacro FNKNOCK_RUN_TRANSACTION protect-rollback
  ${If} $0 != 0
    StrCpy $R9 "rollback snapshot ACL protection failed: $1"
    Goto fnknock_rollback
  ${EndIf}

  DetailPrint "Starting the FnKnock service"
  nsExec::ExecToStack '"$INSTDIR\fn-knock-service.exe" start'
  Pop $0
  Pop $1
  ${If} $0 != 0
    StrCpy $R9 "service start failed: $1"
    Goto fnknock_rollback
  ${EndIf}

  !insertmacro FNKNOCK_RUN_TRANSACTION wait-ready
  ${If} $0 != 0
    StrCpy $R9 "the complete runtime did not become ready within 60 seconds: $1"
    Goto fnknock_rollback
  ${EndIf}

  ; Removing the marker commits the transaction. Delete it before best-effort
  ; snapshot cleanup so an antivirus lock cannot roll back an already-ready app.
  ClearErrors
  Delete "$APPDATA\FnKnock\rollback\pending\transaction.pending"
  ${If} ${Errors}
    StrCpy $R9 "the installer transaction could not be committed"
    Goto fnknock_rollback
  ${EndIf}
  RMDir /r "$APPDATA\FnKnock\rollback\pending"
  ClearErrors
  Goto fnknock_postinstall_done

  fnknock_rollback:
  DetailPrint "FnKnock setup failed ($R9); restoring the previous state"
  StrCpy $R8 "first-install"
  ${If} ${FileExists} "$APPDATA\FnKnock\rollback\pending\bundle\fn-knock-service.exe"
    StrCpy $R8 "upgrade"
  ${EndIf}
  !insertmacro FNKNOCK_RUN_TRANSACTION rollback
  ${If} $0 != 0
    Abort "FnKnock setup failed ($R9), and automatic rollback failed. The snapshot is preserved at ProgramData\FnKnock\rollback\pending: $1"
  ${EndIf}
  ${If} $R8 == "upgrade"
    Abort "FnKnock setup failed ($R9). The previous bundle and data were restored and verified ready."
  ${Else}
    Abort "FnKnock first install failed ($R9). Partial service and program files were removed; ProgramData was retained for diagnostics."
  ${EndIf}

  fnknock_postinstall_done:
  ; Write normalized product branding and shortcuts.
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Knock 敲门" "DisplayName" "Knock 敲门"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Knock 敲门" "Publisher" "KCI-LNK Corporation"
  Delete "$SMPROGRAMS\fn-knock.lnk"
  Delete "$DESKTOP\fn-knock.lnk"
  Delete "$DESKTOP\Knock 敲门.lnk"
  CreateShortCut "$SMPROGRAMS\Knock 敲门.lnk" "$INSTDIR\fn-knock.exe"
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ; Check the GUI before SCM state is removed.
  !insertmacro CheckIfAppIsRunning "fn-knock.exe" "FnKnock"

  DetailPrint "Removing the FnKnock service (ProgramData is preserved)"
  !insertmacro FNKNOCK_STOP_AND_WAIT
  ${If} $0 != 0
    Abort "FnKnock could not be stopped safely; uninstall was cancelled: $1"
  ${EndIf}
  nsExec::ExecToStack '"$INSTDIR\fn-knock-service.exe" uninstall'
  Pop $0
  Pop $1
  ${If} $0 != 0
    nsExec::ExecToStack '"$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -NonInteractive -ExecutionPolicy Bypass -Command "$$ErrorActionPreference = $\'Stop$\'; $$sc = $\'$SYSDIR\sc.exe$\'; if (-not (Test-Path -LiteralPath $$sc -PathType Leaf)) { throw $\'System32 sc.exe is missing$\' }; $$service = Get-Service -Name $\'${FNKNOCK_SERVICE}$\' -ErrorAction SilentlyContinue; if ($$null -ne $$service) { & $$sc delete $\'${FNKNOCK_SERVICE}$\' | Out-Null; if ($$LASTEXITCODE -ne 0 -and $$LASTEXITCODE -ne 1072) { throw $\'SCM service deletion failed$\' } }; Get-NetFirewallRule -DisplayName $\'FnKnock Gateway$\' -ErrorAction SilentlyContinue | Remove-NetFirewallRule -ErrorAction Stop"'
    Pop $0
    Pop $1
    ${If} $0 != 0
      Abort "FnKnock service removal failed; program files were retained to avoid a dangling SCM entry: $1"
    ${EndIf}
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ; Intentionally preserve %ProgramData%\FnKnock for reinstall and recovery.
!macroend
