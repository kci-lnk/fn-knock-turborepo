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
$ProductRoot = Join-Path ([Environment]::GetFolderPath([Environment+SpecialFolder]::ProgramFiles)) "Knock 敲门"
$InstallRoot = $ProductRoot
$ServiceExecutable = Join-Path $InstallRoot "fn-knock-service.exe"
$GatewayExecutable = Join-Path $InstallRoot "fn-knock-gateway.exe"
$RustAcmeshExecutable = Join-Path $InstallRoot "rust-acmesh.exe"
$DesktopExecutable = Join-Path $InstallRoot "fn-knock.exe"
$BundleIdentityPath = Join-Path $InstallRoot "bundle.json"
$RuntimeConfigPath = Join-Path $ProgramDataRoot "config\runtime.json"
$HmacSecretPath = Join-Path $ProgramDataRoot "secrets\hmac-secret"
$AltchaHmacKeyPath = Join-Path $ProgramDataRoot "secrets\altcha-hmac-key"
$RegistryPaths = @(
  "Registry::HKEY_LOCAL_MACHINE\Software\KCI-LNK Corporation\Knock 敲门",
  "Registry::HKEY_LOCAL_MACHINE\Software\Microsoft\Windows\CurrentVersion\Uninstall\Knock 敲门"
)
$ShortcutPaths = @(
  (Join-Path ([Environment]::GetFolderPath([Environment+SpecialFolder]::CommonPrograms)) "Knock 敲门.lnk"),
  (Join-Path ([Environment]::GetFolderPath([Environment+SpecialFolder]::CommonDesktopDirectory)) "Knock 敲门.lnk")
)
$ClassificationFixtureRoot = Join-Path ([IO.Path]::GetTempPath()) "FnKnock-installer-smoke-$PID"
$script:CleanupAuthorized = $false
$script:InstallAttempted = $false
$script:TestSucceeded = $false
$script:ExpectedAltchaHmacKey = $null

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

function Read-RequiredSecret {
  param(
    [Parameter(Mandatory = $true)][string]$Path,
    [Parameter(Mandatory = $true)][string]$Label
  )
  Assert-File $Path
  $secret = (Get-Content -Raw -LiteralPath $Path).Trim()
  Assert-Condition ($secret.Length -ge 32) "$Label is missing or invalid"
  return $secret
}

function Get-HmacSha256Hex {
  param(
    [Parameter(Mandatory = $true)][string]$Secret,
    [Parameter(Mandatory = $true)][string]$Message
  )
  $hmac = [Security.Cryptography.HMACSHA256]::new(
    [Text.Encoding]::UTF8.GetBytes($Secret)
  )
  try {
    $digest = $hmac.ComputeHash([Text.Encoding]::UTF8.GetBytes($Message))
    return [BitConverter]::ToString($digest).Replace("-", "").ToLowerInvariant()
  } finally {
    $hmac.Dispose()
  }
}

function Assert-AuthCaptchaRuntime {
  param(
    [Parameter(Mandatory = $true)][int]$AuthPort,
    [string]$ExpectedAltchaHmacKey = ""
  )
  $hmacSecret = Read-RequiredSecret -Path $HmacSecretPath -Label "Runtime HMAC secret"
  $altchaHmacKey = Read-RequiredSecret -Path $AltchaHmacKeyPath -Label "ALTCHA HMAC secret"
  if (-not [string]::IsNullOrEmpty($ExpectedAltchaHmacKey)) {
    Assert-Condition ($altchaHmacKey -eq $ExpectedAltchaHmacKey) `
      "ALTCHA HMAC secret changed across an installer lifecycle"
  }

  $timestamp = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds().ToString(
    [Globalization.CultureInfo]::InvariantCulture
  )
  $nonce = [guid]::NewGuid().ToString("N")
  $signature = Get-HmacSha256Hex -Secret $hmacSecret -Message "${timestamp}:${nonce}"
  $bootstrap = Invoke-RestMethod `
    -Method Get `
    -Uri "http://127.0.0.1:$AuthPort/api/auth/bootstrap" `
    -Headers @{
      "x-timestamp" = $timestamp
      "x-nonce" = $nonce
      "x-signature" = $signature
    } `
    -TimeoutSec 5 `
    -NoProxy `
    -ErrorAction Stop
  Assert-Condition ($bootstrap.success -eq $true) "Auth bootstrap did not succeed"
  Assert-Condition ([string]$bootstrap.data.captcha.provider -eq "pow") `
    "Auth bootstrap did not select the PoW captcha provider"
  Assert-Condition ($bootstrap.data.captcha.available -eq $true) `
    "Auth bootstrap reported that the PoW captcha provider is unavailable"
  Assert-Condition `
    ([string]::IsNullOrWhiteSpace([string]$bootstrap.data.captcha.unavailable_reason)) `
    "Auth bootstrap returned a captcha unavailability reason"

  $challenge = Invoke-RestMethod `
    -Method Get `
    -Uri "http://127.0.0.1:$AuthPort/api/auth/challenge" `
    -TimeoutSec 5 `
    -NoProxy `
    -ErrorAction Stop
  Assert-Condition ([string]$challenge.algorithm -eq "SHA-256") `
    "PoW challenge did not use SHA-256"
  Assert-Condition (-not [string]::IsNullOrWhiteSpace([string]$challenge.challenge)) `
    "PoW challenge is missing its challenge value"
  $expectedChallengeSignature = Get-HmacSha256Hex `
    -Secret $altchaHmacKey `
    -Message ([string]$challenge.challenge)
  Assert-Condition ([string]$challenge.signature -eq $expectedChallengeSignature) `
    "PoW challenge was not signed by the persisted ALTCHA HMAC secret"
  return $altchaHmacKey
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

function Get-AuthPort {
  if (-not (Test-Path -LiteralPath $RuntimeConfigPath -PathType Leaf)) {
    return 7997
  }
  try {
    $runtime = Get-Content -Raw -LiteralPath $RuntimeConfigPath | ConvertFrom-Json
    if ($null -ne $runtime.auth_port) {
      return [int]$runtime.auth_port
    }
  } catch {
    Write-Verbose "Unable to read the installed auth port: $($_.Exception.Message)"
  }
  return 7997
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

function Assert-InstalledRuntime {
  param(
    [Parameter(Mandatory = $true)][int]$TimeoutSeconds
  )

  foreach ($file in @(
    $DesktopExecutable,
    $ServiceExecutable,
    $GatewayExecutable,
    $RustAcmeshExecutable,
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
  Assert-Condition ([int]$bundleIdentity.control_api_version -eq 2) `
    "The installed bundle does not use control API version 2"
  foreach ($path in $RegistryPaths) {
    Assert-Condition (Test-Path -LiteralPath $path) `
      "Installer did not create required 64-bit registry metadata: $path"
  }
  Assert-File $ShortcutPaths[0]

  Wait-ServiceState -State "Running" -TimeoutSeconds $TimeoutSeconds
  Wait-FnKnockReady `
    -ExpectedVersion ([string]$bundleIdentity.version) `
    -ExpectedControlApiVersion ([int]$bundleIdentity.control_api_version) `
    -TimeoutSeconds $TimeoutSeconds
  $expectedAltchaHmacKey = if ($null -eq $script:ExpectedAltchaHmacKey) {
    ""
  } else {
    [string]$script:ExpectedAltchaHmacKey
  }
  $observedAltchaHmacKey = Assert-AuthCaptchaRuntime `
    -AuthPort (Get-AuthPort) `
    -ExpectedAltchaHmacKey $expectedAltchaHmacKey
  if ($null -eq $script:ExpectedAltchaHmacKey) {
    $script:ExpectedAltchaHmacKey = $observedAltchaHmacKey
  }
  Assert-Condition ((Get-FnKnockFirewallRules).Count -gt 0) `
    "Installer did not create the '$FirewallRuleName' firewall rule"
}

function Assert-InstallerMetadataAbsent {
  foreach ($path in $RegistryPaths) {
    Assert-Condition (-not (Test-Path -LiteralPath $path)) `
      "Installer registry metadata was not removed: $path"
  }
  foreach ($path in $ShortcutPaths) {
    Assert-Condition (-not (Test-Path -LiteralPath $path)) `
      "Installer left a broken all-users shortcut: $path"
  }
}

function Invoke-NativeExpectFailure {
  param(
    [Parameter(Mandatory = $true)][string]$FilePath,
    [string[]]$Arguments = @()
  )
  $output = & $FilePath @Arguments 2>&1
  $exitCode = $LASTEXITCODE
  if ($output) {
    $output | ForEach-Object { Write-Host $_ }
  }
  if ($exitCode -eq 0) {
    throw "$FilePath unexpectedly succeeded"
  }
}

function Assert-UninstalledRuntime {
  param(
    [Parameter(Mandatory = $true)][int]$TimeoutSeconds
  )

  Wait-ServiceAbsent -TimeoutSeconds $TimeoutSeconds
  Wait-FirewallRulesAbsent -TimeoutSeconds $TimeoutSeconds
  Assert-Condition (-not (Test-Path -LiteralPath $ProductRoot)) `
    "NSIS uninstall left the Program Files install root behind"
  Assert-Condition (Test-Path -LiteralPath $ProgramDataRoot -PathType Container) `
    "NSIS uninstall removed ProgramData instead of retaining it"
  Assert-InstallerMetadataAbsent
}

function Save-BundleClassificationFixture {
  if (Test-Path -LiteralPath $ClassificationFixtureRoot) {
    throw "Classification fixture already exists: $ClassificationFixtureRoot"
  }
  New-Item -ItemType Directory -Path $ClassificationFixtureRoot | Out-Null
  Copy-Item -LiteralPath $ServiceExecutable -Destination $ClassificationFixtureRoot
  Copy-Item -LiteralPath $GatewayExecutable -Destination $ClassificationFixtureRoot
}

function Assert-InstallerRejectsMalformedCompleteBundle {
  New-Item -ItemType Directory -Force -Path $ProductRoot | Out-Null
  foreach ($name in @("fn-knock-service.exe", "fn-knock-gateway.exe")) {
    Copy-Item -LiteralPath (Join-Path $ClassificationFixtureRoot $name) -Destination $ProductRoot
  }
  $malformedIdentity = '{"version":"","control_api_version":0,"target":"not-windows"}'
  [IO.File]::WriteAllText($BundleIdentityPath, $malformedIdentity)
  $beforeHashes = @{}
  foreach ($path in @($ServiceExecutable, $GatewayExecutable)) {
    $beforeHashes[$path] = (Get-FileHash -Algorithm SHA256 -LiteralPath $path).Hash
  }

  Write-Host "Verifying that upgrade classification rejects and preserves an invalid complete bundle"
  Invoke-NativeExpectFailure -FilePath $SetupPath -Arguments @("/S")
  foreach ($path in @($ServiceExecutable, $GatewayExecutable)) {
    Assert-File $path
    Assert-Condition ((Get-FileHash -Algorithm SHA256 -LiteralPath $path).Hash -eq $beforeHashes[$path]) `
      "The rejected invalid bundle was modified: $path"
  }
  Assert-Condition ((Get-Content -Raw -LiteralPath $BundleIdentityPath) -eq $malformedIdentity) `
    "The rejected invalid bundle identity was modified"
  Assert-Condition ($null -eq (Get-FnKnockService)) `
    "The rejected invalid bundle unexpectedly registered the service"
  Assert-Condition ((Get-FnKnockFirewallRules).Count -eq 0) `
    "The rejected invalid bundle unexpectedly created a firewall rule"
  Assert-InstallerMetadataAbsent
  Assert-Condition (-not (Test-Path -LiteralPath (Join-Path $ProgramDataRoot "rollback\pending\transaction.pending") -PathType Leaf)) `
    "The rejected invalid bundle left an armed transaction marker"

  Remove-TestOwnedDirectory -Path $ProductRoot
}

function Assert-InstallerUpgradesLegacyBundleIdentity {
  New-Item -ItemType Directory -Force -Path $ProductRoot | Out-Null
  foreach ($name in @("fn-knock-service.exe", "fn-knock-gateway.exe")) {
    Copy-Item -LiteralPath (Join-Path $ClassificationFixtureRoot $name) -Destination $ProductRoot
  }
  # Releases produced before control_api_version was added used this shape.
  # They already implement control API v1 and must remain upgradeable.
  $legacyIdentity = @{
    version = "2.0.1"
    target = "windows-x86_64"
    files = @("fn-knock-service.exe", "fn-knock-gateway.exe")
  } | ConvertTo-Json -Depth 5
  [IO.File]::WriteAllText($BundleIdentityPath, $legacyIdentity)

  Write-Host "Verifying upgrade compatibility with a legacy bundle identity"
  Invoke-NativeChecked -FilePath $SetupPath -Arguments @("/S")
  Assert-InstalledRuntime -TimeoutSeconds $ReadyTimeoutSeconds

  Write-Host "Uninstalling the legacy-identity upgrade fixture"
  Invoke-NativeChecked -FilePath (Join-Path $InstallRoot "uninstall.exe") -Arguments @("/S")
  Assert-UninstalledRuntime -TimeoutSeconds $UninstallTimeoutSeconds
}

function Add-DanglingServiceFixture {
  Assert-Condition (-not (Test-Path -LiteralPath $ServiceExecutable)) `
    "The repair-first fixture requires the service executable to be absent"
  $expected = "`"$ServiceExecutable`" --service"
  New-Service `
    -Name $ServiceName `
    -BinaryPathName $expected `
    -DisplayName "fn-knock Gateway" `
    -StartupType Automatic | Out-Null
  Invoke-NativeChecked -FilePath "$env:SystemRoot\System32\sc.exe" -Arguments @(
    "sidtype", $ServiceName, "unrestricted"
  )
  $record = Get-CimInstance -ClassName Win32_Service -Filter "Name='FnKnock'" -ErrorAction Stop
  Assert-Condition ([string]$record.PathName -eq $expected) `
    "The repair-first fixture registered an unexpected service command: $($record.PathName)"
  Write-Host "Prepared a matching dangling SCM service for repair-first coverage"
}

function Add-RestrictedProgramDataFixture {
  $fixture = Join-Path $ProgramDataRoot "logs\installer-acl-regression"
  New-Item -ItemType Directory -Force -Path $fixture | Out-Null
  [IO.File]::WriteAllText(
    (Join-Path $fixture "system-owned.txt"),
    "The installer must recover this deliberately restricted stale entry."
  )

  # Reproduce state left by an older SYSTEM-owned install. The service has
  # already been uninstalled, so the next setup must recover this tree before
  # its transaction helper can enumerate or replace ACLs.
  Invoke-NativeChecked -FilePath "$env:SystemRoot\System32\icacls.exe" -Arguments @(
    $fixture,
    "/setowner", "*S-1-5-18",
    "/T", "/L", "/Q"
  )
  Invoke-NativeChecked -FilePath "$env:SystemRoot\System32\icacls.exe" -Arguments @(
    $fixture,
    "/inheritance:r",
    "/grant:r", "*S-1-5-18:(OI)(CI)F",
    "/T", "/L", "/Q"
  )
  Write-Host "Prepared a SYSTEM-only ProgramData ACL regression fixture"
}

function Assert-FirstInstallPreservesNonEmptyDirectory {
  $legacyRoot = Join-Path $ProductRoot "current"
  $sentinel = Join-Path $legacyRoot "pre-existing-sentinel.txt"
  $sentinelContent = "This pre-existing legacy content must never be deleted."
  New-Item -ItemType Directory -Force -Path $legacyRoot | Out-Null
  [IO.File]::WriteAllText($sentinel, $sentinelContent)

  Write-Host "Verifying that first install rejects and preserves a non-empty/legacy install root"
  Invoke-NativeExpectFailure -FilePath $SetupPath -Arguments @("/S")
  Assert-File $sentinel
  Assert-Condition ((Get-Content -Raw -LiteralPath $sentinel) -eq $sentinelContent) `
    "The rejected first install modified the pre-existing sentinel"
  Assert-Condition ($null -eq (Get-FnKnockService)) `
    "The rejected first install unexpectedly registered the service"
  Assert-Condition ((Get-FnKnockFirewallRules).Count -eq 0) `
    "The rejected first install unexpectedly created a firewall rule"
  Assert-InstallerMetadataAbsent
  Assert-Condition (-not (Test-Path -LiteralPath (Join-Path $ProgramDataRoot "rollback\pending\transaction.pending") -PathType Leaf)) `
    "The rejected first install left an armed transaction marker"

  Remove-TestOwnedDirectory -Path $ProductRoot
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
  $takeownHelp = (& takeown.exe /? 2>&1 | Out-String)
  if ($LASTEXITCODE -ne 0 -or $takeownHelp -notmatch '(?im)(^|\s)/SKIPSL(\s|$)') {
    throw "Refusing recursive cleanup because takeown.exe lacks /SKIPSL"
  }
  Invoke-NativeChecked -FilePath "$env:SystemRoot\System32\takeown.exe" -Arguments @(
    "/F", $Path, "/R", "/D", "Y", "/SKIPSL"
  )
  Invoke-NativeChecked -FilePath "$env:SystemRoot\System32\icacls.exe" -Arguments @(
    $Path, "/grant", "*S-1-5-32-544:(OI)(CI)F", "/T", "/C", "/L"
  )
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

  foreach ($path in $ShortcutPaths) {
    try {
      if (Test-Path -LiteralPath $path -PathType Leaf) {
        Remove-Item -LiteralPath $path -Force -ErrorAction Stop
      }
    } catch {
      Write-Warning "Cleanup could not remove shortcut ${path}: $($_.Exception.Message)"
    }
  }
  foreach ($path in $RegistryPaths) {
    try {
      Remove-Item -LiteralPath $path -Recurse -Force -ErrorAction SilentlyContinue
    } catch {
      Write-Warning "Cleanup could not remove registry metadata ${path}: $($_.Exception.Message)"
    }
  }
  try {
    if (Test-Path -LiteralPath $ClassificationFixtureRoot) {
      Remove-Item -LiteralPath $ClassificationFixtureRoot -Recurse -Force -ErrorAction Stop
    }
  } catch {
    Write-Warning "Cleanup could not remove classification fixture: $($_.Exception.Message)"
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
    foreach ($path in @($RegistryPaths + $ShortcutPaths)) {
      if (Test-Path -LiteralPath $path) {
        $leftovers.Add("installer metadata: $path")
      }
    }
    if (Test-Path -LiteralPath $ClassificationFixtureRoot) {
      $leftovers.Add("classification fixture")
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
  Assert-InstallerMetadataAbsent

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
  Assert-InstallerMetadataAbsent
  $script:CleanupAuthorized = $true

  Write-Host "Installing NSIS package silently: $SetupPath"
  $script:InstallAttempted = $true
  Invoke-NativeChecked -FilePath $SetupPath -Arguments @("/S")
  Assert-InstalledRuntime -TimeoutSeconds $ReadyTimeoutSeconds
  Save-BundleClassificationFixture

  Write-Host "Uninstalling NSIS package silently"
  Invoke-NativeChecked -FilePath (Join-Path $InstallRoot "uninstall.exe") -Arguments @("/S")
  Assert-UninstalledRuntime -TimeoutSeconds $UninstallTimeoutSeconds

  Assert-InstallerRejectsMalformedCompleteBundle
  Assert-InstallerUpgradesLegacyBundleIdentity
  Assert-FirstInstallPreservesNonEmptyDirectory
  Add-DanglingServiceFixture
  Add-RestrictedProgramDataFixture

  Write-Host "Reinstalling NSIS package over retained ProgramData"
  Invoke-NativeChecked -FilePath $SetupPath -Arguments @("/S")
  Assert-InstalledRuntime -TimeoutSeconds $ReadyTimeoutSeconds

  Write-Host "Uninstalling NSIS package after retained-ProgramData reinstall"
  Invoke-NativeChecked -FilePath (Join-Path $InstallRoot "uninstall.exe") -Arguments @("/S")
  Assert-UninstalledRuntime -TimeoutSeconds $UninstallTimeoutSeconds

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
