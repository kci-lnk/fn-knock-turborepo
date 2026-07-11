!include "LogicLib.nsh"

!define FNKNOCK_SERVICE "FnKnock"
!define FNKNOCK_TRANSACTION_SCRIPT "$PLUGINSDIR\fnknock-transaction.ps1"

Function FnKnockProtectPluginDirectory
  StrCpy $0 0
  StrCpy $1 ""

  ; Take ownership first so a pre-created explicit deny ACE cannot prevent the
  ; elevated installer from replacing the complete DACL.
  nsExec::ExecToStack '"$SYSDIR\icacls.exe" "$PLUGINSDIR" /setowner "*S-1-5-32-544" /T /L /Q'
  Pop $0
  Pop $1
  ${If} $0 != 0
    StrCpy $1 "unable to take ownership of the installer transaction directory"
    Return
  ${EndIf}

  ; /reset removes every pre-existing explicit ACE. The following protected
  ; DACL is therefore an allowlist, rather than /grant:r layered over attacker
  ; controlled rules.
  nsExec::ExecToStack '"$SYSDIR\icacls.exe" "$PLUGINSDIR" /reset /T /L /Q'
  Pop $0
  Pop $1
  ${If} $0 != 0
    StrCpy $1 "unable to reset the installer transaction directory ACL"
    Return
  ${EndIf}

  nsExec::ExecToStack '"$SYSDIR\icacls.exe" "$PLUGINSDIR" /inheritance:r /grant:r "*S-1-5-18:(OI)(CI)F" "*S-1-5-32-544:(OI)(CI)F" /T /L /Q'
  Pop $0
  Pop $1
  ${If} $0 != 0
    StrCpy $1 "unable to apply the installer transaction directory allowlist"
    Return
  ${EndIf}

  nsExec::ExecToStack '"$SYSDIR\icacls.exe" "$PLUGINSDIR" /setowner "*S-1-5-18" /T /L /Q'
  Pop $0
  Pop $1
  ${If} $0 != 0
    StrCpy $1 "unable to transfer ownership of the installer transaction directory"
    Return
  ${EndIf}
  StrCpy $0 0
  StrCpy $1 ""
FunctionEnd

; Keep the PowerShell command line short enough for NSIS' string limit. The
; transaction implementation is emitted into $PLUGINSDIR in small chunks and
; reused by PREINSTALL, POSTINSTALL and the installer failure callbacks.
Function FnKnockWriteTransactionScript
  ; The elevated installer must not execute a helper from a directory still
  ; writable by the unelevated user. Lock the unique NSIS plug-in directory
  ; before creating or replacing the PowerShell transaction script.
  Call FnKnockProtectPluginDirectory
  ${If} $0 != 0
    Return
  ${EndIf}

  ClearErrors
  FileOpen $R7 "${FNKNOCK_TRANSACTION_SCRIPT}" w
  ${If} ${Errors}
    StrCpy $0 1
    StrCpy $1 "unable to create the installer transaction helper"
    Return
  ${EndIf}

  FileWrite $R7 "[CmdletBinding()]$\r$\n"
  FileWrite $R7 "param([Parameter(Mandatory=$$true)][ValidateSet('begin','stop','snapshot','rollback','wait-ready')][string]$$Action, [Parameter(Mandatory=$$true)][string]$$InstallDir)$\r$\n"
  FileWrite $R7 "$$ErrorActionPreference = 'Stop'$\r$\n"
  FileWrite $R7 "function Assert-FnKnockInstallerAcl($$Acl, [string]$$Path, [string[]]$$AllowedSids) {$\r$\n"
  FileWrite $R7 "  if (-not $$Acl.AreAccessRulesProtected) { throw ('installer ACL inherits permissions: ' + $$Path) }$\r$\n"
  FileWrite $R7 "  if ($$Acl.GetOwner([System.Security.Principal.SecurityIdentifier]).Value -ne 'S-1-5-18') { throw ('installer object is not owned by SYSTEM: ' + $$Path) }$\r$\n"
  FileWrite $R7 "  $$rules = @($$Acl.GetAccessRules($$true, $$false, [System.Security.Principal.SecurityIdentifier]))$\r$\n"
  FileWrite $R7 "  if ($$rules.Count -ne $$AllowedSids.Count) { throw ('installer ACL has an unexpected rule count: ' + $$Path) }$\r$\n"
  FileWrite $R7 "  foreach ($$rule in $$rules) { if ($$AllowedSids -notcontains $$rule.IdentityReference.Value -or $$rule.AccessControlType -ne [System.Security.AccessControl.AccessControlType]::Allow -or $$rule.FileSystemRights -ne [System.Security.AccessControl.FileSystemRights]::FullControl) { throw ('installer ACL contains an unexpected rule: ' + $$Path) } }$\r$\n"
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
  FileWrite $R7 "Assert-FnKnockInstallerTreeAcl $$PSScriptRoot$\r$\n"
  FileWrite $R7 "$$serviceName = '${FNKNOCK_SERVICE}'$\r$\n"
  FileWrite $R7 "$$install = [IO.Path]::GetFullPath($$InstallDir)$\r$\n"
  FileWrite $R7 "$$programData = [Environment]::GetFolderPath([Environment+SpecialFolder]::CommonApplicationData)$\r$\n"
  FileWrite $R7 "if ([string]::IsNullOrWhiteSpace($$programData) -or -not [IO.Path]::IsPathRooted($$programData) -or $$programData.StartsWith('\\')) { throw 'the CommonApplicationData known folder is invalid' }$\r$\n"
  FileWrite $R7 "$$programData = [IO.Path]::GetFullPath($$programData)$\r$\n"
  FileWrite $R7 "$$root = Join-Path $$programData 'FnKnock'$\r$\n"
  FileWrite $R7 "$$pending = Join-Path $$root 'rollback\pending'$\r$\n"
  FileWrite $R7 "$$marker = Join-Path $$pending 'transaction.pending'$\r$\n"
  FileWrite $R7 "$$snapshotReady = Join-Path $$pending 'snapshot.ready'$\r$\n"
  FileWrite $R7 "$$serviceExe = Join-Path $$install 'fn-knock-service.exe'$\r$\n"
  FileWrite $R7 "$$systemDirectory = [Environment]::GetFolderPath([Environment+SpecialFolder]::System)$\r$\n"
  FileWrite $R7 "if ([string]::IsNullOrWhiteSpace($$systemDirectory) -or -not [IO.Path]::IsPathRooted($$systemDirectory) -or $$systemDirectory.StartsWith('\\')) { throw 'the System known folder is invalid' }$\r$\n"
  FileWrite $R7 "$$systemDirectory = [IO.Path]::GetFullPath($$systemDirectory)$\r$\n"
  FileWrite $R7 "$$sc = Join-Path $$systemDirectory 'sc.exe'$\r$\n"
  FileWrite $R7 "$$icacls = Join-Path $$systemDirectory 'icacls.exe'$\r$\n"
  FileWrite $R7 "if (-not (Test-Path -LiteralPath $$sc -PathType Leaf) -or -not (Test-Path -LiteralPath $$icacls -PathType Leaf)) { throw 'required System32 tools are missing' }$\r$\n"
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

  FileWrite $R7 "function New-FnKnockDirectoryAcl($$SystemSid, $$AdministratorsSid, $$ServiceSid) {$\r$\n"
  FileWrite $R7 "  $$acl = [System.Security.AccessControl.DirectorySecurity]::new()$\r$\n"
  FileWrite $R7 "  $$acl.SetAccessRuleProtection($$true, $$false)$\r$\n"
  FileWrite $R7 "  $$inherit = [System.Security.AccessControl.InheritanceFlags]::ContainerInherit -bor [System.Security.AccessControl.InheritanceFlags]::ObjectInherit$\r$\n"
  FileWrite $R7 "  $$none = [System.Security.AccessControl.PropagationFlags]::None$\r$\n"
  FileWrite $R7 "  $$allow = [System.Security.AccessControl.AccessControlType]::Allow$\r$\n"
  FileWrite $R7 "  [void]$$acl.AddAccessRule([System.Security.AccessControl.FileSystemAccessRule]::new($$SystemSid, [System.Security.AccessControl.FileSystemRights]::FullControl, $$inherit, $$none, $$allow))$\r$\n"
  FileWrite $R7 "  [void]$$acl.AddAccessRule([System.Security.AccessControl.FileSystemAccessRule]::new($$AdministratorsSid, [System.Security.AccessControl.FileSystemRights]::FullControl, $$inherit, $$none, $$allow))$\r$\n"
  FileWrite $R7 "  if ($$null -ne $$ServiceSid) { [void]$$acl.AddAccessRule([System.Security.AccessControl.FileSystemAccessRule]::new($$ServiceSid, [System.Security.AccessControl.FileSystemRights]::Modify, $$inherit, $$none, $$allow)) }$\r$\n"
  FileWrite $R7 "  return $$acl$\r$\n"
  FileWrite $R7 "}$\r$\n"
  FileWrite $R7 "$\r$\n"

  FileWrite $R7 "function New-FnKnockFileAcl($$SystemSid, $$AdministratorsSid, $$ServiceSid) {$\r$\n"
  FileWrite $R7 "  $$acl = [System.Security.AccessControl.FileSecurity]::new()$\r$\n"
  FileWrite $R7 "  $$acl.SetAccessRuleProtection($$true, $$false)$\r$\n"
  FileWrite $R7 "  $$allow = [System.Security.AccessControl.AccessControlType]::Allow$\r$\n"
  FileWrite $R7 "  [void]$$acl.AddAccessRule([System.Security.AccessControl.FileSystemAccessRule]::new($$SystemSid, [System.Security.AccessControl.FileSystemRights]::FullControl, $$allow))$\r$\n"
  FileWrite $R7 "  [void]$$acl.AddAccessRule([System.Security.AccessControl.FileSystemAccessRule]::new($$AdministratorsSid, [System.Security.AccessControl.FileSystemRights]::FullControl, $$allow))$\r$\n"
  FileWrite $R7 "  if ($$null -ne $$ServiceSid) { [void]$$acl.AddAccessRule([System.Security.AccessControl.FileSystemAccessRule]::new($$ServiceSid, [System.Security.AccessControl.FileSystemRights]::Modify, $$allow)) }$\r$\n"
  FileWrite $R7 "  return $$acl$\r$\n"
  FileWrite $R7 "}$\r$\n"
  FileWrite $R7 "$\r$\n"

  FileWrite $R7 "function Set-FnKnockDataTreeAcl([string]$$Path, $$SystemSid, $$AdministratorsSid, $$ServiceSid) {$\r$\n"
  FileWrite $R7 "  $$queue = [System.Collections.Generic.Queue[string]]::new()$\r$\n"
  FileWrite $R7 "  $$queue.Enqueue([IO.Path]::GetFullPath($$Path))$\r$\n"
  FileWrite $R7 "  while ($$queue.Count -gt 0) {$\r$\n"
  FileWrite $R7 "    $$directory = $$queue.Dequeue()$\r$\n"
  FileWrite $R7 "    $$attributes = [IO.File]::GetAttributes($$directory)$\r$\n"
  FileWrite $R7 "    if (($$attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) { throw ('refusing a reparse point in ProgramData: ' + $$directory) }$\r$\n"
  FileWrite $R7 "    [IO.Directory]::SetAccessControl($$directory, (New-FnKnockDirectoryAcl $$SystemSid $$AdministratorsSid $$ServiceSid))$\r$\n"
  FileWrite $R7 "    foreach ($$entry in [IO.Directory]::EnumerateFileSystemEntries($$directory)) {$\r$\n"
  FileWrite $R7 "      $$entryAttributes = [IO.File]::GetAttributes($$entry)$\r$\n"
  FileWrite $R7 "      if (($$entryAttributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) { throw ('refusing a reparse point in ProgramData: ' + $$entry) }$\r$\n"
  FileWrite $R7 "      if (($$entryAttributes -band [IO.FileAttributes]::Directory) -ne 0) { $$queue.Enqueue($$entry) } else { [IO.File]::SetAccessControl($$entry, (New-FnKnockFileAcl $$SystemSid $$AdministratorsSid $$ServiceSid)) }$\r$\n"
  FileWrite $R7 "    }$\r$\n"
  FileWrite $R7 "  }$\r$\n"
  FileWrite $R7 "}$\r$\n"
  FileWrite $R7 "$\r$\n"

  FileWrite $R7 "function Set-FnKnockDataTreeOwner([string]$$Path, [string]$$Icacls) {$\r$\n"
  FileWrite $R7 "  $$ownerArgs = @($$Path, '/setowner', '*S-1-5-18', '/T', '/L', '/Q')$\r$\n"
  FileWrite $R7 "  & $$Icacls @ownerArgs | Out-Null$\r$\n"
  FileWrite $R7 "  $$ownerExitCode = $$LASTEXITCODE$\r$\n"
  FileWrite $R7 "  if ($$ownerExitCode -ne 0) { throw ('unable to transfer ProgramData\FnKnock ownership to SYSTEM; icacls exited with code ' + $$ownerExitCode) }$\r$\n"
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

  FileWrite $R7 "function Wait-FnKnockReady {$\r$\n"
  FileWrite $R7 "  $$port = 7991$\r$\n"
  FileWrite $R7 "  $$runtime = Join-Path $$root 'config\runtime.json'$\r$\n"
  FileWrite $R7 "  if (Test-Path -LiteralPath $$runtime) {$\r$\n"
  FileWrite $R7 "    try { $$configured = Get-Content -Raw -LiteralPath $$runtime | ConvertFrom-Json; if ($$configured.admin_port) { $$port = [int]$$configured.admin_port } } catch {}$\r$\n"
  FileWrite $R7 "  }$\r$\n"
  FileWrite $R7 "  $$deadline = [DateTime]::UtcNow.AddSeconds(60)$\r$\n"
  FileWrite $R7 "  while ([DateTime]::UtcNow -lt $$deadline) {$\r$\n"
  FileWrite $R7 "    try {$\r$\n"
  FileWrite $R7 "      $$response = Invoke-WebRequest -UseBasicParsing -TimeoutSec 2 -Uri ('http://127.0.0.1:' + $$port + '/__fn-knock/readyz')$\r$\n"
  FileWrite $R7 "      $$body = $$response.Content | ConvertFrom-Json$\r$\n"
  FileWrite $R7 "      $$components = $$body.components$\r$\n"
  FileWrite $R7 "      if ($$response.StatusCode -eq 200 -and $$body.ready -eq $$true -and $$body.control_api_version -eq 1 -and $$components.storage -eq $$true -and $$components.gateway_bundle -eq $$true -and $$components.gateway_process -eq $$true -and $$components.gateway_dataplane -eq $$true -and $$components.auth_bridge -eq $$true) { return }$\r$\n"
  FileWrite $R7 "    } catch {}$\r$\n"
  FileWrite $R7 "    Start-Sleep -Milliseconds 750$\r$\n"
  FileWrite $R7 "  }$\r$\n"
  FileWrite $R7 "  throw 'the complete FnKnock runtime did not become ready within 60 seconds'$\r$\n"
  FileWrite $R7 "}$\r$\n"
  FileWrite $R7 "$\r$\n"

  FileWrite $R7 "function Install-And-StartFnKnock {$\r$\n"
  FileWrite $R7 "  if (-not (Test-Path -LiteralPath $$serviceExe -PathType Leaf)) { throw 'fn-knock-service.exe is missing' }$\r$\n"
  FileWrite $R7 "  & $$serviceExe install$\r$\n"
  FileWrite $R7 "  if ($$LASTEXITCODE -ne 0) { throw ('service registration failed with exit code ' + $$LASTEXITCODE) }$\r$\n"
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
  FileWrite $R7 "  if (Test-Path -LiteralPath $$serviceExe -PathType Leaf) { try { & $$serviceExe uninstall | Out-Null } catch {} }$\r$\n"
  FileWrite $R7 "  $$service = Get-Service -Name $$serviceName -ErrorAction SilentlyContinue$\r$\n"
  FileWrite $R7 "  if ($$null -ne $$service) { & $$sc delete $$serviceName | Out-Null; if ($$LASTEXITCODE -ne 0 -and $$LASTEXITCODE -ne 1072) { throw ('SCM service deletion failed with exit code ' + $$LASTEXITCODE) } }$\r$\n"
  FileWrite $R7 "  Get-NetFirewallRule -DisplayName 'FnKnock Gateway' -ErrorAction SilentlyContinue | Remove-NetFirewallRule -ErrorAction Stop$\r$\n"
  FileWrite $R7 "  if (Test-Path -LiteralPath $$install) { Get-ChildItem -LiteralPath $$install -Force -ErrorAction SilentlyContinue | Remove-Item -Recurse -Force }$\r$\n"
  FileWrite $R7 "}$\r$\n"
  FileWrite $R7 "$\r$\n"

  FileWrite $R7 "function Restore-FnKnockUpgrade {$\r$\n"
  FileWrite $R7 "  Stop-FnKnock$\r$\n"
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
  FileWrite $R7 "  foreach ($$name in @('config','data')) {$\r$\n"
  FileWrite $R7 "    $$current = Join-Path $$root $$name$\r$\n"
  FileWrite $R7 "    Remove-Item -LiteralPath $$current -Recurse -Force -ErrorAction SilentlyContinue$\r$\n"
  FileWrite $R7 "    $$snapshot = Join-Path (Join-Path $$pending 'data') $$name$\r$\n"
  FileWrite $R7 "    if (Test-Path -LiteralPath $$snapshot) { Copy-Item -LiteralPath $$snapshot -Destination $$current -Recurse -Force; Assert-TreeCopy $$snapshot $$current }$\r$\n"
  FileWrite $R7 "  }$\r$\n"
  FileWrite $R7 "  Install-And-StartFnKnock$\r$\n"
  FileWrite $R7 "}$\r$\n"
  FileWrite $R7 "$\r$\n"

  FileWrite $R7 "function Rollback-FnKnockTransaction {$\r$\n"
  FileWrite $R7 "  if (-not (Test-Path -LiteralPath $$marker -PathType Leaf)) { return }$\r$\n"
  FileWrite $R7 "  $$kind = (Get-Content -Raw -LiteralPath $$marker).Trim()$\r$\n"
  FileWrite $R7 "  if ($$kind -eq 'upgrade') { Restore-FnKnockUpgrade } elseif ($$kind -eq 'first') { Remove-PartialFnKnockInstall } else { throw ('unknown installer transaction kind ' + $$kind) }$\r$\n"
  FileWrite $R7 "  Remove-Item -LiteralPath $$marker -Force$\r$\n"
  FileWrite $R7 "  Remove-Item -LiteralPath $$pending -Recurse -Force -ErrorAction SilentlyContinue$\r$\n"
  FileWrite $R7 "}$\r$\n"
  FileWrite $R7 "$\r$\n"

  FileWrite $R7 "switch ($$Action) {$\r$\n"
  FileWrite $R7 "  'begin' {$\r$\n"
  ; Do not inspect a stale transaction until ProgramData and its existing tree
  ; have an installer-owned ACL. Preserve the live service SID during upgrades.
  FileWrite $R7 "    New-Item -ItemType Directory -Force -Path $$root | Out-Null$\r$\n"
  FileWrite $R7 "    $$systemSid = [System.Security.Principal.SecurityIdentifier]::new('S-1-5-18')$\r$\n"
  FileWrite $R7 "    $$administratorsSid = [System.Security.Principal.SecurityIdentifier]::new('S-1-5-32-544')$\r$\n"
  FileWrite $R7 "    $$serviceSid = $$null$\r$\n"
  FileWrite $R7 "    if ($$null -ne (Get-Service -Name $$serviceName -ErrorAction SilentlyContinue)) { $$serviceSid = ([System.Security.Principal.NTAccount]::new('NT SERVICE', $$serviceName)).Translate([System.Security.Principal.SecurityIdentifier]) }$\r$\n"
  FileWrite $R7 "    Set-FnKnockDataTreeAcl $$root $$systemSid $$administratorsSid $$serviceSid$\r$\n"
  FileWrite $R7 "    Set-FnKnockDataTreeOwner $$root $$icacls$\r$\n"
  FileWrite $R7 "    foreach ($$directory in @('config','data','certificates','waf','logs','state','rollback')) { New-Item -ItemType Directory -Force -Path (Join-Path $$root $$directory) | Out-Null }$\r$\n"
  FileWrite $R7 "    Set-FnKnockDataTreeAcl $$root $$systemSid $$administratorsSid $$serviceSid$\r$\n"
  FileWrite $R7 "    Set-FnKnockDataTreeOwner $$root $$icacls$\r$\n"
  FileWrite $R7 "    Rollback-FnKnockTransaction$\r$\n"
  FileWrite $R7 "    Remove-Item -LiteralPath $$pending -Recurse -Force -ErrorAction SilentlyContinue$\r$\n"
  FileWrite $R7 "    if (Test-Path -LiteralPath $$pending) { throw 'a stale installer transaction could not be removed' }$\r$\n"
  FileWrite $R7 "    New-Item -ItemType Directory -Force -Path (Join-Path $$pending 'bundle') | Out-Null$\r$\n"
  FileWrite $R7 "    New-Item -ItemType Directory -Force -Path (Join-Path $$pending 'data') | Out-Null$\r$\n"
  FileWrite $R7 "    Set-FnKnockDataTreeAcl $$pending $$systemSid $$administratorsSid $$serviceSid$\r$\n"
  FileWrite $R7 "    Set-FnKnockDataTreeOwner $$pending $$icacls$\r$\n"
  FileWrite $R7 "    $$kind = if (Test-Path -LiteralPath $$serviceExe -PathType Leaf) { 'upgrade' } else { 'first' }$\r$\n"
  FileWrite $R7 "    [IO.File]::WriteAllText($$marker, $$kind)$\r$\n"
  FileWrite $R7 "    Set-FnKnockDataTreeAcl $$pending $$systemSid $$administratorsSid $$serviceSid$\r$\n"
  FileWrite $R7 "    Set-FnKnockDataTreeOwner $$pending $$icacls$\r$\n"
  FileWrite $R7 "  }$\r$\n"
  FileWrite $R7 "  'stop' { Stop-FnKnock }$\r$\n"
  FileWrite $R7 "  'snapshot' {$\r$\n"
  FileWrite $R7 "    if (-not (Test-Path -LiteralPath $$marker -PathType Leaf)) { throw 'the installer transaction marker is missing' }$\r$\n"
  FileWrite $R7 "    $$kind = (Get-Content -Raw -LiteralPath $$marker).Trim()$\r$\n"
  FileWrite $R7 "    if ($$kind -eq 'upgrade') {$\r$\n"
  FileWrite $R7 "      $$bundle = Join-Path $$pending 'bundle'$\r$\n"
  FileWrite $R7 "      Get-ChildItem -LiteralPath $$install -Force | Copy-Item -Destination $$bundle -Recurse -Force$\r$\n"
  FileWrite $R7 "      Assert-TreeCopy $$install $$bundle$\r$\n"
  FileWrite $R7 "      foreach ($$required in @('fn-knock-service.exe','fn-knock-gateway.exe','bundle.json')) { if (-not (Test-Path -LiteralPath (Join-Path $$bundle $$required) -PathType Leaf)) { throw ('rollback snapshot is missing ' + $$required) } }$\r$\n"
  FileWrite $R7 "      foreach ($$name in @('config','data')) { $$source = Join-Path $$root $$name; if (Test-Path -LiteralPath $$source) { $$copy = Join-Path (Join-Path $$pending 'data') $$name; Copy-Item -LiteralPath $$source -Destination $$copy -Recurse -Force; Assert-TreeCopy $$source $$copy } }$\r$\n"
  FileWrite $R7 "    } elseif ($$kind -ne 'first') { throw ('unknown installer transaction kind ' + $$kind) }$\r$\n"
  FileWrite $R7 "    [IO.File]::WriteAllText($$snapshotReady, 'ready')$\r$\n"
  FileWrite $R7 "    if ($$kind -eq 'upgrade') { Get-ChildItem -LiteralPath $$install -Force | Remove-Item -Recurse -Force }$\r$\n"
  FileWrite $R7 "  }$\r$\n"
  FileWrite $R7 "  'rollback' { Rollback-FnKnockTransaction }$\r$\n"
  FileWrite $R7 "  'wait-ready' { Wait-FnKnockReady }$\r$\n"
  FileWrite $R7 "}$\r$\n"

  ${If} ${Errors}
    FileClose $R7
    StrCpy $0 1
    StrCpy $1 "unable to write the installer transaction helper"
    Return
  ${EndIf}
  FileClose $R7
  ; File creation assigns the elevated user's default owner. Re-apply the
  ; exact tree ACL and SYSTEM ownership before the helper can be executed.
  Call FnKnockProtectPluginDirectory
FunctionEnd

!macro FNKNOCK_RUN_TRANSACTION ACTION
  Call FnKnockWriteTransactionScript
  ${If} $0 == 0
    nsExec::ExecToStack '"$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "${FNKNOCK_TRANSACTION_SCRIPT}" -Action ${ACTION} -InstallDir "$INSTDIR"'
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
  !insertmacro FNKNOCK_RUN_TRANSACTION rollback
  ${If} $0 != 0
    MessageBox MB_OK|MB_ICONSTOP "FnKnock could not roll back the cancelled setup. The recovery snapshot was preserved under ProgramData\FnKnock\rollback\pending: $1"
    Abort
  ${EndIf}
FunctionEnd

; File extraction and registry failures happen between Tauri's PREINSTALL and
; POSTINSTALL hooks. These callbacks make those failures transactional too.
Function .onInstFailed
  !insertmacro FNKNOCK_RUN_TRANSACTION rollback
  ${If} $0 != 0
    MessageBox MB_OK|MB_ICONSTOP "FnKnock automatic rollback failed. The recovery snapshot was preserved under ProgramData\FnKnock\rollback\pending: $1"
  ${EndIf}
FunctionEnd

Function .onGUIEnd
  ; Idempotent final safety net. A successful install has already removed the
  ; transaction marker, so this is a no-op on the normal path.
  !insertmacro FNKNOCK_RUN_TRANSACTION rollback
FunctionEnd

!macro NSIS_HOOK_PREINSTALL
  ; Keep every versioned component together under one replaceable bundle.
  StrCpy $INSTDIR "$PROGRAMFILES64\FnKnock\current"

  ; Tauri's stock process check runs after PREINSTALL. Perform the same check
  ; before stopping SCM so cancelling the prompt cannot take the gateway down.
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
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ; Tauri's stock check also runs after this hook. Check (and close) the GUI
  ; first so a user cancellation cannot occur after SCM state was removed.
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
