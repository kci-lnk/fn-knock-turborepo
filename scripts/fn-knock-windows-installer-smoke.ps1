[CmdletBinding()]
param(
  [Parameter(Mandatory = $true)]
  [string]$SetupPath,
  [ValidateRange(30, 180)]
  [int]$ReadyTimeoutSeconds = 90,
  [ValidateRange(15, 90)]
  [int]$UninstallTimeoutSeconds = 45
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

if (-not $IsWindows -or -not [Environment]::Is64BitProcess) {
  throw "FnKnock installer smoke tests require native 64-bit Windows"
}

$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$principal = [Security.Principal.WindowsPrincipal]::new($identity)
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
  throw "FnKnock installer smoke tests require an elevated administrator token"
}

$SetupPath = (Resolve-Path -LiteralPath $SetupPath).Path
if (-not (Test-Path -LiteralPath $SetupPath -PathType Leaf) -or
    -not $SetupPath.EndsWith(".exe", [StringComparison]::OrdinalIgnoreCase)) {
  throw "SetupPath must resolve to an NSIS .exe installer: $SetupPath"
}

$ServiceName = "FnKnock"
$FirewallRuleName = "FnKnock Gateway"
$ProgramDataRoot = Join-Path ([Environment]::GetFolderPath([Environment+SpecialFolder]::CommonApplicationData)) "FnKnock"
$ProductRoot = Join-Path ([Environment]::GetFolderPath([Environment+SpecialFolder]::ProgramFiles)) "FnKnock"
# hooks.nsh fixes the per-machine install directory at Program Files\FnKnock\current.
$InstallRoot = Join-Path $ProductRoot "current"
$ServiceExecutable = Join-Path $InstallRoot "fn-knock-service.exe"
$GatewayExecutable = Join-Path $InstallRoot "fn-knock-gateway.exe"
$DesktopExecutable = Join-Path $InstallRoot "fn-knock.exe"
$BundleIdentityPath = Join-Path $InstallRoot "bundle.json"
$RuntimeConfigPath = Join-Path $ProgramDataRoot "config\runtime.json"
$script:CleanupAuthorized = $false
$script:InstallAttempted = $false
$script:TestSucceeded = $false

function Assert-Condition {
  param(
    [Parameter(Mandatory = $true)]
    [bool]$Condition,
    [Parameter(Mandatory = $true)]
    [string]$Message
  )
  if (-not $Condition) {
    throw $Message
  }
}

function Assert-File {
  param([Parameter(Mandatory = $true)][string]$Path)
  if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
    throw "Required installed file is missing: $Path"
  }
}

function Get-FnKnockService {
  return Get-Service -Name $ServiceName -ErrorAction SilentlyContinue
}

function Get-FnKnockFirewallRules {
  return @(Get-NetFirewallRule -DisplayName $FirewallRuleName -ErrorAction SilentlyContinue)
}

function Get-FnKnockProcesses {
  return @(
    Get-CimInstance Win32_Process -ErrorAction SilentlyContinue |
      Where-Object { $_.Name -in @("fn-knock.exe", "fn-knock-service.exe", "fn-knock-gateway.exe") }
  )
}

function Invoke-NativeChecked {
  param(
    [Parameter(Mandatory = $true)][string]$FilePath,
    [string[]]$Arguments = @()
  )
  $output = & $FilePath @Arguments 2>&1
  $exitCode = $LASTEXITCODE
  if ($output) {
    $output | ForEach-Object { Write-Host $_ }
  }
  if ($exitCode -ne 0) {
    throw "$FilePath $($Arguments -join ' ') failed with exit code $exitCode"
  }
}

function Wait-ServiceState {
  param(
    [Parameter(Mandatory = $true)][string]$State,
    [Parameter(Mandatory = $true)][int]$TimeoutSeconds
  )
  $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
  $lastState = "missing"
  while ([DateTime]::UtcNow -lt $deadline) {
    $service = Get-FnKnockService
    if ($null -ne $service) {
      $lastState = [string]$service.Status
      if ($lastState -eq $State) {
        return
      }
    }
    Start-Sleep -Milliseconds 500
  }
  throw "$ServiceName did not reach $State within $TimeoutSeconds seconds (last state: $lastState)"
}

function Wait-ServiceAbsent {
  param([Parameter(Mandatory = $true)][int]$TimeoutSeconds)
  $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
  while ([DateTime]::UtcNow -lt $deadline) {
    if ($null -eq (Get-FnKnockService)) {
      return
    }
    Start-Sleep -Milliseconds 500
  }
  throw "$ServiceName still exists after $TimeoutSeconds seconds"
}

function Wait-FirewallRulesAbsent {
  param([Parameter(Mandatory = $true)][int]$TimeoutSeconds)
  $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
  while ([DateTime]::UtcNow -lt $deadline) {
    if ((Get-FnKnockFirewallRules).Count -eq 0) {
      return
    }
    Start-Sleep -Milliseconds 500
  }
  throw "The '$FirewallRuleName' firewall rule still exists after $TimeoutSeconds seconds"
}

function Get-AdminPort {
  if (-not (Test-Path -LiteralPath $RuntimeConfigPath -PathType Leaf)) {
    return 7991
  }
  try {
    $runtime = Get-Content -Raw -LiteralPath $RuntimeConfigPath | ConvertFrom-Json
    if ($null -ne $runtime.admin_port) {
      return [int]$runtime.admin_port
    }
  } catch {
    Write-Verbose "Unable to read the installed runtime configuration: $($_.Exception.Message)"
  }
  return 7991
}

function Assert-ReadyContract {
  param(
    [Parameter(Mandatory = $true)]$Document,
    [Parameter(Mandatory = $true)][string]$ExpectedVersion,
    [Parameter(Mandatory = $true)][int]$ExpectedControlApiVersion
  )
  Assert-Condition ($Document.ready -eq $true) "readyz did not report ready=true"
  Assert-Condition ([string]$Document.version -eq $ExpectedVersion) `
    "readyz version $($Document.version) does not match installed bundle version $ExpectedVersion"
  Assert-Condition ([int]$Document.control_api_version -eq $ExpectedControlApiVersion) `
    "readyz control API version $($Document.control_api_version) does not match the installed bundle"
  foreach ($component in @(
    "storage",
    "gateway_bundle",
    "gateway_process",
    "gateway_dataplane",
    "auth_bridge"
  )) {
    $property = $Document.components.PSObject.Properties[$component]
    if ($null -eq $property -or $property.Value -ne $true) {
      throw "readyz component $component is missing or not ready"
    }
  }
}

function Wait-FnKnockReady {
  param(
    [Parameter(Mandatory = $true)][string]$ExpectedVersion,
    [Parameter(Mandatory = $true)][int]$ExpectedControlApiVersion,
    [Parameter(Mandatory = $true)][int]$TimeoutSeconds
  )
  $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
  $lastDetail = "service has not reached Running"
  while ([DateTime]::UtcNow -lt $deadline) {
    $service = Get-FnKnockService
    if ($null -ne $service -and $service.Status -eq "Running") {
      try {
        $adminPort = Get-AdminPort
        $document = Invoke-RestMethod `
          -Method Get `
          -Uri "http://127.0.0.1:$adminPort/__fn-knock/readyz" `
          -TimeoutSec 3 `
          -NoProxy `
          -ErrorAction Stop
        Assert-ReadyContract `
          -Document $document `
          -ExpectedVersion $ExpectedVersion `
          -ExpectedControlApiVersion $ExpectedControlApiVersion
        return
      } catch {
        $lastDetail = $_.Exception.Message
      }
    } elseif ($null -ne $service) {
      $lastDetail = "service state is $($service.Status)"
    }
    Start-Sleep -Milliseconds 750
  }
  throw "FnKnock did not satisfy the readyz contract within $TimeoutSeconds seconds: $lastDetail"
}

function Write-InstallerDiagnostics {
  Write-Warning "FnKnock installer smoke test diagnostics follow"
  try {
    & sc.exe queryex $ServiceName 2>&1 | ForEach-Object { Write-Warning $_ }
  } catch {
    Write-Warning "Unable to query ${ServiceName}: $($_.Exception.Message)"
  }
  if (Test-Path -LiteralPath $ProgramDataRoot -PathType Container) {
    $logRoot = Join-Path $ProgramDataRoot "logs"
    Get-ChildItem -LiteralPath $logRoot -File -Recurse -ErrorAction SilentlyContinue |
      Sort-Object LastWriteTime -Descending |
      Select-Object -First 3 |
      ForEach-Object {
        Write-Warning "Tail of $($_.FullName)"
        Get-Content -LiteralPath $_.FullName -Tail 40 -ErrorAction SilentlyContinue |
          ForEach-Object { Write-Warning $_ }
      }
  }
}

function Remove-TestOwnedDirectory {
  param([Parameter(Mandatory = $true)][string]$Path)
  if (-not (Test-Path -LiteralPath $Path)) {
    return
  }

  # The installer deliberately makes its trees SYSTEM-owned. Retake only the
  # two fixed locations whose absence was verified before this test began.
  try { & takeown.exe /F $Path /R /D Y | Out-Null } catch { Write-Verbose $_.Exception.Message }
  try { & icacls.exe $Path /grant "*S-1-5-32-544:(OI)(CI)F" /T /C | Out-Null } catch { Write-Verbose $_.Exception.Message }
  Remove-Item -LiteralPath $Path -Recurse -Force -ErrorAction Stop
}

function Invoke-InstallerCleanup {
  if (-not $script:CleanupAuthorized) {
    return
  }

  $uninstaller = Join-Path $InstallRoot "uninstall.exe"
  if (Test-Path -LiteralPath $uninstaller -PathType Leaf) {
    try {
      Invoke-NativeChecked -FilePath $uninstaller -Arguments @("/S")
    } catch {
      Write-Warning "Cleanup uninstaller failed: $($_.Exception.Message)"
    }
  }

  $service = Get-FnKnockService
  if ($null -ne $service) {
    try {
      if ($service.Status -ne "Stopped") {
        Stop-Service -Name $ServiceName -Force -ErrorAction SilentlyContinue
        Wait-ServiceState -State "Stopped" -TimeoutSeconds 20
      }
    } catch {
      Write-Warning "Cleanup could not stop $ServiceName cleanly: $($_.Exception.Message)"
    }
    try { & sc.exe delete $ServiceName | Out-Null } catch { Write-Verbose $_.Exception.Message }
  }

  try {
    Get-FnKnockFirewallRules | Remove-NetFirewallRule -ErrorAction Stop
  } catch {
    Write-Warning "Cleanup could not remove '$FirewallRuleName': $($_.Exception.Message)"
  }

  foreach ($path in @($ProductRoot, $ProgramDataRoot)) {
    try {
      Remove-TestOwnedDirectory -Path $path
    } catch {
      Write-Warning "Cleanup could not remove ${path}: $($_.Exception.Message)"
    }
  }

  if ($script:TestSucceeded) {
    $leftovers = [System.Collections.Generic.List[string]]::new()
    if ($null -ne (Get-FnKnockService)) {
      $leftovers.Add("SCM service")
    }
    if ((Get-FnKnockFirewallRules).Count -ne 0) {
      $leftovers.Add("firewall rule")
    }
    if (Test-Path -LiteralPath $ProductRoot) {
      $leftovers.Add("Program Files install root")
    }
    if (Test-Path -LiteralPath $ProgramDataRoot) {
      $leftovers.Add("ProgramData")
    }
    if ($leftovers.Count -ne 0) {
      throw "Installer smoke test passed, but final cleanup left: $($leftovers -join ', ')"
    }
  }
}

try {
  foreach ($path in @($ProductRoot, $ProgramDataRoot)) {
    if (Test-Path -LiteralPath $path) {
      throw "Refusing to run: pre-existing FnKnock state would be at risk ($path)"
    }
  }
  if ($null -ne (Get-FnKnockService)) {
    throw "Refusing to run: a pre-existing $ServiceName service is installed"
  }
  if ((Get-FnKnockFirewallRules).Count -ne 0) {
    throw "Refusing to run: a pre-existing '$FirewallRuleName' firewall rule exists"
  }
  if ((Get-FnKnockProcesses).Count -ne 0) {
    throw "Refusing to run: an existing FnKnock process is running"
  }

  # Re-check immediately before allowing cleanup. This prevents a stale
  # preflight from granting deletion rights over a concurrently-created install.
  foreach ($path in @($ProductRoot, $ProgramDataRoot)) {
    if (Test-Path -LiteralPath $path) {
      throw "Refusing to install: FnKnock state appeared after the initial safety check ($path)"
    }
  }
  if ($null -ne (Get-FnKnockService) -or
      (Get-FnKnockFirewallRules).Count -ne 0 -or
      (Get-FnKnockProcesses).Count -ne 0) {
    throw "Refusing to install: FnKnock state appeared after the initial safety check"
  }
  $script:CleanupAuthorized = $true

  Write-Host "Installing NSIS package silently: $SetupPath"
  $script:InstallAttempted = $true
  Invoke-NativeChecked -FilePath $SetupPath -Arguments @("/S")

  foreach ($file in @(
    $DesktopExecutable,
    $ServiceExecutable,
    $GatewayExecutable,
    $BundleIdentityPath,
    (Join-Path $InstallRoot "uninstall.exe")
  )) {
    Assert-File $file
  }

  $bundleIdentity = Get-Content -Raw -LiteralPath $BundleIdentityPath | ConvertFrom-Json
  Assert-Condition ([string]$bundleIdentity.target -eq "windows-x86_64") `
    "The installed bundle target is not windows-x86_64"
  Assert-Condition (-not [string]::IsNullOrWhiteSpace([string]$bundleIdentity.version)) `
    "The installed bundle has no version"
  Assert-Condition ([int]$bundleIdentity.control_api_version -eq 1) `
    "The installed bundle does not use control API version 1"

  Wait-ServiceState -State "Running" -TimeoutSeconds $ReadyTimeoutSeconds
  Wait-FnKnockReady `
    -ExpectedVersion ([string]$bundleIdentity.version) `
    -ExpectedControlApiVersion ([int]$bundleIdentity.control_api_version) `
    -TimeoutSeconds $ReadyTimeoutSeconds
  Assert-Condition ((Get-FnKnockFirewallRules).Count -gt 0) `
    "Installer did not create the '$FirewallRuleName' firewall rule"

  Write-Host "Uninstalling NSIS package silently"
  Invoke-NativeChecked -FilePath (Join-Path $InstallRoot "uninstall.exe") -Arguments @("/S")
  Wait-ServiceAbsent -TimeoutSeconds $UninstallTimeoutSeconds
  Wait-FirewallRulesAbsent -TimeoutSeconds $UninstallTimeoutSeconds
  Assert-Condition (-not (Test-Path -LiteralPath $ProductRoot)) `
    "NSIS uninstall left the Program Files install root behind"
  Assert-Condition (Test-Path -LiteralPath $ProgramDataRoot -PathType Container) `
    "NSIS uninstall removed ProgramData instead of retaining it"

  $script:TestSucceeded = $true
  Write-Host "FnKnock NSIS installer smoke test passed"
} catch {
  if ($script:CleanupAuthorized) {
    Write-InstallerDiagnostics
  }
  throw
} finally {
  Invoke-InstallerCleanup
  if (-not $script:TestSucceeded) {
    Write-Warning "FnKnock installer smoke test failed; test-owned state was cleaned up"
  }
}
