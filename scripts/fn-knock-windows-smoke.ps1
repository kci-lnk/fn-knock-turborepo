[CmdletBinding()]
param(
  [string]$BundleRoot = "",
  [string]$DesktopExecutable = "",
  [ValidateRange(60, 300)]
  [int]$ReadyTimeoutSeconds = 90,
  [ValidateRange(90, 300)]
  [int]$RecoveryTimeoutSeconds = 150
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

if (-not $IsWindows -or -not [Environment]::Is64BitProcess) {
  throw "FnKnock runtime smoke tests require native 64-bit Windows"
}

$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$principal = [Security.Principal.WindowsPrincipal]::new($identity)
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
  throw "FnKnock runtime smoke tests require an elevated administrator token"
}

$Root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
if ([string]::IsNullOrWhiteSpace($BundleRoot)) {
  $BundleRoot = Join-Path $Root "apps\fn-knock-desktop\bundle\windows"
}
if ([string]::IsNullOrWhiteSpace($DesktopExecutable)) {
  $DesktopExecutable = Join-Path $Root "apps\fn-knock-desktop\native\target\x86_64-pc-windows-msvc\release\fn-knock.exe"
}
$BundleRoot = (Resolve-Path -LiteralPath $BundleRoot).Path
$DesktopExecutable = (Resolve-Path -LiteralPath $DesktopExecutable).Path

$ServiceName = "FnKnock"
$FirewallRuleName = "FnKnock Gateway"
$ProgramDataRoot = Join-Path $env:PROGRAMDATA "FnKnock"
$StatusPath = Join-Path $ProgramDataRoot "state\status.json"
$RuntimeConfigPath = Join-Path $ProgramDataRoot "config\runtime.json"
$HmacSecretPath = Join-Path $ProgramDataRoot "secrets\hmac-secret"
$AltchaHmacKeyPath = Join-Path $ProgramDataRoot "secrets\altcha-hmac-key"
$RetainMarker = Join-Path $ProgramDataRoot "state\windows-smoke-retain.marker"
$TempParent = Join-Path $env:SystemDrive "FnKnockSmoke"
$TestRoot = Join-Path $TempParent ([guid]::NewGuid().ToString("N"))
$CurrentRoot = Join-Path $TestRoot "current"
$ServiceExecutable = Join-Path $CurrentRoot "fn-knock-service.exe"
$GatewayExecutable = Join-Path $CurrentRoot "fn-knock-gateway.exe"
$ConflictListener = $null
$CleanupAuthorized = $false
$OwnsProgramData = $false
$TestSucceeded = $false

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
    throw "Required smoke-test file is missing: $Path"
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
      "ALTCHA HMAC secret changed across a service restart"
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

function Copy-DirectoryContents {
  param(
    [Parameter(Mandatory = $true)][string]$Source,
    [Parameter(Mandatory = $true)][string]$Destination
  )
  if (-not (Test-Path -LiteralPath $Source -PathType Container)) {
    throw "Required smoke-test directory is missing: $Source"
  }
  New-Item -ItemType Directory -Force -Path $Destination | Out-Null
  Get-ChildItem -LiteralPath $Source -Force |
    Copy-Item -Destination $Destination -Recurse -Force
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

function Read-ServiceStatusDocument {
  if (-not (Test-Path -LiteralPath $StatusPath -PathType Leaf)) {
    return $null
  }
  try {
    return Get-Content -Raw -LiteralPath $StatusPath | ConvertFrom-Json
  } catch {
    return $null
  }
}

function Test-ProcessExists {
  param([Parameter(Mandatory = $true)][int]$ProcessId)
  return $null -ne (Get-Process -Id $ProcessId -ErrorAction SilentlyContinue)
}

function Wait-ProcessExit {
  param(
    [Parameter(Mandatory = $true)][int]$ProcessId,
    [Parameter(Mandatory = $true)][int]$TimeoutSeconds,
    [Parameter(Mandatory = $true)][string]$Description
  )
  $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
  while ([DateTime]::UtcNow -lt $deadline) {
    if (-not (Test-ProcessExists -ProcessId $ProcessId)) {
      return
    }
    Start-Sleep -Milliseconds 250
  }
  throw "$Description (PID $ProcessId) did not exit within $TimeoutSeconds seconds"
}

function Wait-ServiceState {
  param(
    [Parameter(Mandatory = $true)][string]$State,
    [Parameter(Mandatory = $true)][int]$TimeoutSeconds
  )
  $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
  $lastState = "missing"
  while ([DateTime]::UtcNow -lt $deadline) {
    $service = Get-Service -Name $ServiceName -ErrorAction SilentlyContinue
    if ($null -ne $service) {
      $lastState = [string]$service.Status
      if ($lastState -eq $State) {
        return
      }
    }
    Start-Sleep -Milliseconds 250
  }
  throw "$ServiceName did not reach $State within $TimeoutSeconds seconds (last state: $lastState)"
}

function Wait-ServiceAbsent {
  param([Parameter(Mandatory = $true)][int]$TimeoutSeconds)
  $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
  while ([DateTime]::UtcNow -lt $deadline) {
    if ($null -eq (Get-Service -Name $ServiceName -ErrorAction SilentlyContinue)) {
      return
    }
    Start-Sleep -Milliseconds 250
  }
  throw "$ServiceName still exists after $TimeoutSeconds seconds"
}

function Test-TcpPortAvailable {
  param([Parameter(Mandatory = $true)][int]$Port)
  $listeners = [System.Collections.Generic.List[System.Net.Sockets.TcpListener]]::new()
  try {
    foreach ($address in @([Net.IPAddress]::Loopback, [Net.IPAddress]::IPv6Loopback)) {
      $listener = [Net.Sockets.TcpListener]::new($address, $Port)
      $listener.Server.ExclusiveAddressUse = $true
      if ($address.AddressFamily -eq [Net.Sockets.AddressFamily]::InterNetworkV6) {
        $listener.Server.DualMode = $false
      }
      $listener.Start()
      $listeners.Add($listener)
    }
    return $true
  } catch {
    return $false
  } finally {
    foreach ($listener in $listeners) {
      $listener.Stop()
    }
  }
}

function Get-AvailableTcpPort {
  param(
    [Parameter(Mandatory = $true)]
    [AllowEmptyCollection()]
    [System.Collections.Generic.HashSet[int]]$UsedPorts
  )
  for ($attempt = 0; $attempt -lt 100; $attempt++) {
    $probe = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback, 0)
    try {
      $probe.Server.ExclusiveAddressUse = $true
      $probe.Start()
      $candidate = ([Net.IPEndPoint]$probe.LocalEndpoint).Port
    } finally {
      $probe.Stop()
    }
    if ($candidate -ge 1024 -and -not $UsedPorts.Contains($candidate) -and
        (Test-TcpPortAvailable -Port $candidate)) {
      [void]$UsedPorts.Add($candidate)
      return $candidate
    }
  }
  throw "Unable to reserve a dual-stack TCP port for the smoke test"
}

function Assert-ProcessImage {
  param(
    [Parameter(Mandatory = $true)][int]$ProcessId,
    [Parameter(Mandatory = $true)][string]$ExpectedPath
  )
  $process = Get-CimInstance Win32_Process -Filter "ProcessId=$ProcessId" -ErrorAction SilentlyContinue
  if ($null -eq $process) {
    throw "Expected process $ProcessId is not running"
  }
  if ([string]::IsNullOrWhiteSpace([string]$process.ExecutablePath)) {
    throw "Unable to inspect executable path for process $ProcessId"
  }
  $actual = [IO.Path]::GetFullPath([string]$process.ExecutablePath)
  $expected = [IO.Path]::GetFullPath($ExpectedPath)
  if (-not $actual.Equals($expected, [StringComparison]::OrdinalIgnoreCase)) {
    throw "Process $ProcessId runs $actual instead of $expected"
  }
}

function Assert-ReadyContract {
  param(
    [Parameter(Mandatory = $true)]$Document,
    [Parameter(Mandatory = $true)][string]$ExpectedVersion,
    [Parameter(Mandatory = $true)][int]$ExpectedControlApiVersion
  )
  Assert-Condition ($Document.ready -eq $true) "readyz did not report ready=true"
  Assert-Condition ([string]$Document.version -eq $ExpectedVersion) `
    "readyz version $($Document.version) does not match $ExpectedVersion"
  Assert-Condition ([int]$Document.control_api_version -eq $ExpectedControlApiVersion) `
    "readyz control_api_version $($Document.control_api_version) does not match $ExpectedControlApiVersion"
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
    [Parameter(Mandatory = $true)][int]$AdminPort,
    [Parameter(Mandatory = $true)][string]$ExpectedVersion,
    [Parameter(Mandatory = $true)][int]$ExpectedControlApiVersion,
    [int]$PreviousServiceProcessId = 0,
    [int]$PreviousGatewayProcessId = 0,
    [Parameter(Mandatory = $true)][int]$TimeoutSeconds
  )
  $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
  $lastDetail = "service has not reached Running"
  while ([DateTime]::UtcNow -lt $deadline) {
    $service = Get-Service -Name $ServiceName -ErrorAction SilentlyContinue
    $status = Read-ServiceStatusDocument
    if ($null -ne $service -and $service.Status -eq "Running" -and
        $null -ne $status -and $status.state -eq "running" -and
        $null -ne $status.gateway_pid) {
      $serviceProcessId = [int]$status.service_pid
      $gatewayProcessId = [int]$status.gateway_pid
      $serviceRecord = Get-CimInstance Win32_Service -Filter "Name='$ServiceName'" -ErrorAction SilentlyContinue
      if ($null -eq $serviceRecord -or [int]$serviceRecord.ProcessId -ne $serviceProcessId) {
        $lastDetail = "status.json contains a stale service PID"
      } elseif (($PreviousServiceProcessId -ne 0 -and $serviceProcessId -eq $PreviousServiceProcessId) -or
                ($PreviousGatewayProcessId -ne 0 -and $gatewayProcessId -eq $PreviousGatewayProcessId)) {
        $lastDetail = "SCM has not replaced both runtime processes"
      } elseif (-not (Test-ProcessExists -ProcessId $gatewayProcessId)) {
        $lastDetail = "status.json contains a stale gateway PID"
      } else {
        try {
          $document = Invoke-RestMethod `
            -Method Get `
            -Uri "http://127.0.0.1:$AdminPort/__fn-knock/readyz" `
            -TimeoutSec 3 `
            -NoProxy `
            -ErrorAction Stop
          Assert-ReadyContract `
            -Document $document `
            -ExpectedVersion $ExpectedVersion `
            -ExpectedControlApiVersion $ExpectedControlApiVersion
          return [pscustomobject]@{
            ServiceProcessId = $serviceProcessId
            GatewayProcessId = $gatewayProcessId
            Status = $status
            Ready = $document
          }
        } catch {
          $lastDetail = $_.Exception.Message
        }
      }
    } elseif ($null -ne $status) {
      $serviceState = if ($null -eq $service) { "missing" } else { [string]$service.Status }
      $lastDetail = "service=$serviceState; runtime=$($status.state); message=$($status.message)"
    }
    Start-Sleep -Milliseconds 500
  }
  throw "FnKnock did not satisfy the full readyz contract within $TimeoutSeconds seconds: $lastDetail"
}

function Wait-DeterministicPortFailure {
  param([Parameter(Mandatory = $true)][int]$TimeoutSeconds)
  $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
  $lastDetail = "status.json was not written"
  while ([DateTime]::UtcNow -lt $deadline) {
    $status = Read-ServiceStatusDocument
    $service = Get-Service -Name $ServiceName -ErrorAction SilentlyContinue
    if ($null -ne $status) {
      $lastDetail = "state=$($status.state); message=$($status.message)"
      if ($status.state -eq "faulted" -and
          [string]$status.message -like "*port preflight failed*" -and
          $null -ne $service -and $service.Status -eq "Stopped") {
        return $status
      }
    }
    Start-Sleep -Milliseconds 250
  }
  throw "Port conflict was not classified as a deterministic stopped failure: $lastDetail"
}

function Test-ManagedGatewayPresent {
  $expected = [IO.Path]::GetFullPath($GatewayExecutable)
  $processes = @(Get-CimInstance Win32_Process -Filter "Name='fn-knock-gateway.exe'" -ErrorAction SilentlyContinue)
  foreach ($process in $processes) {
    if (-not [string]::IsNullOrWhiteSpace([string]$process.ExecutablePath)) {
      $actual = [IO.Path]::GetFullPath([string]$process.ExecutablePath)
      if ($actual.Equals($expected, [StringComparison]::OrdinalIgnoreCase)) {
        return $true
      }
    }
  }
  return $false
}

function Write-SmokeDiagnostics {
  Write-Warning "FnKnock Windows smoke test diagnostics follow"
  try {
    & sc.exe queryex $ServiceName 2>&1 | ForEach-Object { Write-Warning $_ }
  } catch {
    Write-Warning "Unable to query service: $($_.Exception.Message)"
  }
  if (Test-Path -LiteralPath $StatusPath -PathType Leaf) {
    try {
      Write-Warning (Get-Content -Raw -LiteralPath $StatusPath)
    } catch {
      Write-Warning "Unable to read status.json: $($_.Exception.Message)"
    }
  }
  $logRoot = Join-Path $ProgramDataRoot "logs"
  if (Test-Path -LiteralPath $logRoot -PathType Container) {
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

function Invoke-SmokeCleanup {
  if ($null -ne $script:ConflictListener) {
    try { $script:ConflictListener.Stop() } catch { Write-Verbose $_.Exception.Message }
    $script:ConflictListener = $null
  }

  if (-not $CleanupAuthorized) {
    if (Test-Path -LiteralPath $TestRoot) {
      try { Remove-Item -LiteralPath $TestRoot -Recurse -Force } catch { Write-Verbose $_.Exception.Message }
    }
    if ((Test-Path -LiteralPath $TempParent -PathType Container) -and
        @(Get-ChildItem -LiteralPath $TempParent -Force).Count -eq 0) {
      try { Remove-Item -LiteralPath $TempParent -Force } catch { Write-Verbose $_.Exception.Message }
    }
    return
  }

  $service = Get-Service -Name $ServiceName -ErrorAction SilentlyContinue
  if ($null -ne $service) {
    try {
      if ($service.Status -ne "Stopped") {
        Stop-Service -Name $ServiceName -Force -ErrorAction SilentlyContinue
        Wait-ServiceState -State "Stopped" -TimeoutSeconds 25
      }
    } catch {
      Write-Warning "Cleanup could not stop $ServiceName cleanly: $($_.Exception.Message)"
    }
    if (Test-Path -LiteralPath $ServiceExecutable -PathType Leaf) {
      try {
        Invoke-NativeChecked -FilePath $ServiceExecutable -Arguments @("uninstall")
      } catch {
        Write-Warning "Cleanup uninstall failed: $($_.Exception.Message)"
      }
    }
    if ($null -ne (Get-Service -Name $ServiceName -ErrorAction SilentlyContinue)) {
      try { & sc.exe delete $ServiceName | Out-Null } catch { Write-Verbose $_.Exception.Message }
    }
  }

  try {
    Get-CimInstance Win32_Process -ErrorAction SilentlyContinue |
      Where-Object {
        if ([string]::IsNullOrWhiteSpace([string]$_.ExecutablePath)) {
          return $false
        }
        $actual = [IO.Path]::GetFullPath([string]$_.ExecutablePath)
        return $actual.Equals(
          [IO.Path]::GetFullPath($GatewayExecutable),
          [StringComparison]::OrdinalIgnoreCase
        ) -or $actual.Equals(
          [IO.Path]::GetFullPath($ServiceExecutable),
          [StringComparison]::OrdinalIgnoreCase
        )
      } |
      ForEach-Object { Stop-Process -Id ([int]$_.ProcessId) -Force -ErrorAction SilentlyContinue }
  } catch {
    Write-Warning "Cleanup could not terminate a staged runtime process: $($_.Exception.Message)"
  }

  try {
    & netsh.exe advfirewall firewall delete rule "name=$FirewallRuleName" | Out-Null
  } catch {
    Write-Warning "Cleanup could not remove the firewall rule: $($_.Exception.Message)"
  }
  if ($OwnsProgramData -and (Test-Path -LiteralPath $ProgramDataRoot)) {
    try {
      Remove-Item -LiteralPath $ProgramDataRoot -Recurse -Force
    } catch {
      Write-Warning "Cleanup could not remove ${ProgramDataRoot}: $($_.Exception.Message)"
    }
  }
  if (Test-Path -LiteralPath $TestRoot) {
    try {
      Remove-Item -LiteralPath $TestRoot -Recurse -Force
    } catch {
      Write-Warning "Cleanup could not remove ${TestRoot}: $($_.Exception.Message)"
    }
  }
  if ((Test-Path -LiteralPath $TempParent -PathType Container) -and
      @(Get-ChildItem -LiteralPath $TempParent -Force).Count -eq 0) {
    try { Remove-Item -LiteralPath $TempParent -Force } catch { Write-Verbose $_.Exception.Message }
  }

  if ($TestSucceeded) {
    $leftovers = [System.Collections.Generic.List[string]]::new()
    if ($null -ne (Get-Service -Name $ServiceName -ErrorAction SilentlyContinue)) {
      $leftovers.Add("SCM service")
    }
    if (@(Get-NetFirewallRule -DisplayName $FirewallRuleName -ErrorAction SilentlyContinue).Count -ne 0) {
      $leftovers.Add("firewall rule")
    }
    if (Test-Path -LiteralPath $ProgramDataRoot) {
      $leftovers.Add("ProgramData")
    }
    if (Test-Path -LiteralPath $TestRoot) {
      $leftovers.Add("staged runtime")
    }
    if ($leftovers.Count -ne 0) {
      throw "Smoke test passed, but final cleanup left: $($leftovers -join ', ')"
    }
  }
}

try {
  Assert-File (Join-Path $BundleRoot "fn-knock-service.exe")
  Assert-File (Join-Path $BundleRoot "fn-knock-gateway.exe")
  Assert-File (Join-Path $BundleRoot "rust-acmesh.exe")
  $rustAcmeshVersion = & (Join-Path $BundleRoot "rust-acmesh.exe") version
  Assert-Condition ($LASTEXITCODE -eq 0 -and $rustAcmeshVersion -match '^rust-acmesh ') `
    "The bundled rust-acmesh.exe version check failed"
  Assert-File (Join-Path $BundleRoot "runtime\bundle.json")
  Assert-File $DesktopExecutable

  if ($null -ne (Get-Service -Name $ServiceName -ErrorAction SilentlyContinue)) {
    throw "Refusing to run: a pre-existing $ServiceName service is installed"
  }
  if (Test-Path -LiteralPath $ProgramDataRoot) {
    throw "Refusing to run: pre-existing FnKnock ProgramData would be at risk ($ProgramDataRoot)"
  }
  if (@(Get-NetFirewallRule -DisplayName $FirewallRuleName -ErrorAction SilentlyContinue).Count -ne 0) {
    throw "Refusing to run: a pre-existing '$FirewallRuleName' firewall rule exists"
  }

  $bundleIdentity = Get-Content -Raw -LiteralPath (Join-Path $BundleRoot "runtime\bundle.json") |
    ConvertFrom-Json
  Assert-Condition ([string]$bundleIdentity.target -eq "windows-x86_64") `
    "The staged bundle target is not windows-x86_64"
  Assert-Condition ([int]$bundleIdentity.control_api_version -eq 2) `
    "The staged bundle does not use control API version 2"

  Write-Host "Assembling flattened smoke-test runtime at $CurrentRoot"
  New-Item -ItemType Directory -Force -Path $CurrentRoot | Out-Null
  Copy-Item -LiteralPath (Join-Path $BundleRoot "fn-knock-service.exe") -Destination $ServiceExecutable
  Copy-Item -LiteralPath (Join-Path $BundleRoot "fn-knock-gateway.exe") -Destination $GatewayExecutable
  Copy-Item -LiteralPath (Join-Path $BundleRoot "rust-acmesh.exe") `
    -Destination (Join-Path $CurrentRoot "rust-acmesh.exe")
  Copy-Item -LiteralPath $DesktopExecutable -Destination (Join-Path $CurrentRoot "fn-knock.exe")
  Copy-Item -LiteralPath (Join-Path $BundleRoot "runtime\bundle.json") `
    -Destination (Join-Path $CurrentRoot "bundle.json")
  Copy-DirectoryContents `
    -Source (Join-Path $BundleRoot "runtime\ui") `
    -Destination (Join-Path $CurrentRoot "ui")
  Copy-DirectoryContents `
    -Source (Join-Path $BundleRoot "runtime\server-auth-view") `
    -Destination (Join-Path $CurrentRoot "server-auth-view")
  & icacls.exe $TestRoot /inheritance:e /grant "*S-1-5-32-545:(OI)(CI)RX" /T /C | Out-Null
  if ($LASTEXITCODE -ne 0) {
    throw "Failed to grant the FnKnock virtual service account read/execute access to the staged runtime"
  }

  if ($null -ne (Get-Service -Name $ServiceName -ErrorAction SilentlyContinue) -or
      (Test-Path -LiteralPath $ProgramDataRoot) -or
      @(Get-NetFirewallRule -DisplayName $FirewallRuleName -ErrorAction SilentlyContinue).Count -ne 0) {
    throw "Refusing to install: FnKnock state appeared after the initial safety check"
  }
  # From this point onward any service, firewall rule, or ProgramData tree with
  # the fixed FnKnock identity was created by this test and may be removed.
  $CleanupAuthorized = $true
  $OwnsProgramData = $true

  Write-Host "Installing the temporary $ServiceName service"
  Invoke-NativeChecked -FilePath $ServiceExecutable -Arguments @("install")
  & icacls.exe $TestRoot /grant "NT SERVICE\FnKnock:(OI)(CI)RX" /T /C | Out-Null
  if ($LASTEXITCODE -ne 0) {
    throw "Failed to grant NT SERVICE\FnKnock read/execute access to the staged runtime"
  }
  Wait-ServiceState -State "Stopped" -TimeoutSeconds 15
  $serviceRecord = Get-CimInstance Win32_Service -Filter "Name='$ServiceName'"
  Assert-Condition ([string]$serviceRecord.PathName -like "*$ServiceExecutable*") `
    "SCM points $ServiceName at an unexpected executable"
  Assert-Condition ([string]$serviceRecord.StartName -eq "NT SERVICE\FnKnock") `
    "SCM did not install $ServiceName under the low-privilege virtual service account"
  Assert-Condition ([string]$serviceRecord.StartMode -eq "Auto") `
    "SCM did not configure $ServiceName for automatic startup"
  $firewallRules = @(
    Get-NetFirewallRule -DisplayName $FirewallRuleName -ErrorAction SilentlyContinue
  )
  Assert-Condition ($firewallRules.Count -gt 0) `
    "Service installation did not create the gateway firewall rule"
  foreach ($rule in $firewallRules) {
    Assert-Condition ([string]$rule.Enabled -eq "True") `
      "The gateway firewall rule is disabled"
    Assert-Condition ([string]$rule.Direction -eq "Inbound") `
      "The gateway firewall rule is not inbound-only"
    # MSFT_NetFirewallRule.Profiles is a uint16 bitmask: Domain=0x1,
    # Private=0x2, Public=0x4. Exact equality also rejects Any and Public.
    $expectedProfileMask = [uint16](0x1 -bor 0x2)
    $actualProfileMask = [uint16]$rule.Profile
    Assert-Condition ($actualProfileMask -eq $expectedProfileMask) `
      "The gateway firewall rule must target Domain and Private profiles only"
    $applicationFilters = @($rule | Get-NetFirewallApplicationFilter)
    Assert-Condition ($applicationFilters.Count -eq 1) `
      "The gateway firewall rule does not have exactly one program filter"
    $firewallProgram = [IO.Path]::GetFullPath([string]$applicationFilters[0].Program)
    Assert-Condition `
      ($firewallProgram.Equals(
        [IO.Path]::GetFullPath($GatewayExecutable),
        [StringComparison]::OrdinalIgnoreCase
      )) `
      "The inbound firewall rule targets $firewallProgram instead of the Go gateway"
  }

  $usedPorts = [System.Collections.Generic.HashSet[int]]::new()
  $runtimeConfig = [ordered]@{
    schema_version = 1
    admin_port = $(Get-AvailableTcpPort -UsedPorts $usedPorts)
    backend_port = $(Get-AvailableTcpPort -UsedPorts $usedPorts)
    auth_port = $(Get-AvailableTcpPort -UsedPorts $usedPorts)
    grpc_port = $(Get-AvailableTcpPort -UsedPorts $usedPorts)
    proxy_port = $(Get-AvailableTcpPort -UsedPorts $usedPorts)
    listener_scope = "all"
  }
  $runtimeJson = $runtimeConfig | ConvertTo-Json -Depth 5
  [IO.File]::WriteAllText($RuntimeConfigPath, "$runtimeJson`n", [Text.UTF8Encoding]::new($false))
  [IO.File]::WriteAllText($RetainMarker, "retain-after-uninstall`n", [Text.UTF8Encoding]::new($false))

  Write-Host "Verifying deterministic port-conflict handling without SCM recovery"
  Remove-Item -LiteralPath $StatusPath -Force -ErrorAction SilentlyContinue
  $ConflictListener = [Net.Sockets.TcpListener]::new(
    [Net.IPAddress]::Loopback,
    [int]$runtimeConfig.admin_port
  )
  $ConflictListener.Server.ExclusiveAddressUse = $true
  $ConflictListener.Start()
  try {
    Invoke-NativeChecked -FilePath $ServiceExecutable -Arguments @("start")
    $fault = Wait-DeterministicPortFailure -TimeoutSeconds 60
    $faultSignature = "$($fault.service_pid)|$($fault.updated_at)|$($fault.message)"
    for ($second = 0; $second -lt 12; $second++) {
      Start-Sleep -Seconds 1
      $service = Get-Service -Name $ServiceName
      $currentFault = Read-ServiceStatusDocument
      Assert-Condition ($service.Status -eq "Stopped") `
        "SCM retried a deterministic port-conflict failure"
      Assert-Condition ($null -ne $currentFault) "The deterministic fault status disappeared"
      $currentSignature = "$($currentFault.service_pid)|$($currentFault.updated_at)|$($currentFault.message)"
      Assert-Condition ($currentSignature -eq $faultSignature) `
        "The deterministic failure was unexpectedly retried or rewritten"
    }
  } finally {
    if ($null -ne $ConflictListener) {
      $ConflictListener.Stop()
      $ConflictListener = $null
    }
  }

  Write-Host "Starting the complete runtime and verifying readyz"
  Invoke-NativeChecked -FilePath $ServiceExecutable -Arguments @("start")
  $baseline = Wait-FnKnockReady `
    -AdminPort ([int]$runtimeConfig.admin_port) `
    -ExpectedVersion ([string]$bundleIdentity.version) `
    -ExpectedControlApiVersion ([int]$bundleIdentity.control_api_version) `
    -TimeoutSeconds $ReadyTimeoutSeconds
  $altchaHmacKey = Assert-AuthCaptchaRuntime -AuthPort ([int]$runtimeConfig.auth_port)
  Assert-ProcessImage -ProcessId $baseline.ServiceProcessId -ExpectedPath $ServiceExecutable
  Assert-ProcessImage -ProcessId $baseline.GatewayProcessId -ExpectedPath $GatewayExecutable
  $proxyListener = Get-NetTCPConnection `
    -State Listen `
    -LocalAddress "0.0.0.0" `
    -LocalPort ([int]$runtimeConfig.proxy_port) `
    -ErrorAction SilentlyContinue |
    Where-Object { [uint32]$_.OwningProcess -eq $baseline.GatewayProcessId } |
    Select-Object -First 1
  Assert-Condition ($null -ne $proxyListener) `
    "The Go gateway is not listening on 0.0.0.0:$($runtimeConfig.proxy_port)"

  Write-Host "Crashing Go and verifying whole-group SCM recovery"
  Stop-Process -Id $baseline.GatewayProcessId -Force
  $afterGoCrash = Wait-FnKnockReady `
    -AdminPort ([int]$runtimeConfig.admin_port) `
    -ExpectedVersion ([string]$bundleIdentity.version) `
    -ExpectedControlApiVersion ([int]$bundleIdentity.control_api_version) `
    -PreviousServiceProcessId $baseline.ServiceProcessId `
    -PreviousGatewayProcessId $baseline.GatewayProcessId `
    -TimeoutSeconds $RecoveryTimeoutSeconds
  Wait-ProcessExit `
    -ProcessId $baseline.ServiceProcessId `
    -TimeoutSeconds 10 `
    -Description "The Rust supervisor from the Go-crashed runtime group"
  Assert-ProcessImage -ProcessId $afterGoCrash.ServiceProcessId -ExpectedPath $ServiceExecutable
  Assert-ProcessImage -ProcessId $afterGoCrash.GatewayProcessId -ExpectedPath $GatewayExecutable
  Assert-AuthCaptchaRuntime `
    -AuthPort ([int]$runtimeConfig.auth_port) `
    -ExpectedAltchaHmacKey $altchaHmacKey | Out-Null

  Write-Host "Crashing Rust and verifying Job Object cleanup plus SCM recovery"
  Stop-Process -Id $afterGoCrash.ServiceProcessId -Force
  Wait-ProcessExit `
    -ProcessId $afterGoCrash.GatewayProcessId `
    -TimeoutSeconds 10 `
    -Description "The gateway owned by the crashed Rust Job Object"
  $afterRustCrash = Wait-FnKnockReady `
    -AdminPort ([int]$runtimeConfig.admin_port) `
    -ExpectedVersion ([string]$bundleIdentity.version) `
    -ExpectedControlApiVersion ([int]$bundleIdentity.control_api_version) `
    -PreviousServiceProcessId $afterGoCrash.ServiceProcessId `
    -PreviousGatewayProcessId $afterGoCrash.GatewayProcessId `
    -TimeoutSeconds $RecoveryTimeoutSeconds
  Assert-ProcessImage -ProcessId $afterRustCrash.ServiceProcessId -ExpectedPath $ServiceExecutable
  Assert-ProcessImage -ProcessId $afterRustCrash.GatewayProcessId -ExpectedPath $GatewayExecutable
  Assert-AuthCaptchaRuntime `
    -AuthPort ([int]$runtimeConfig.auth_port) `
    -ExpectedAltchaHmacKey $altchaHmacKey | Out-Null

  Write-Host "Stopping normally and verifying the gateway does not survive"
  Invoke-NativeChecked -FilePath $ServiceExecutable -Arguments @("stop")
  Wait-ServiceState -State "Stopped" -TimeoutSeconds 30
  Wait-ProcessExit `
    -ProcessId $afterRustCrash.GatewayProcessId `
    -TimeoutSeconds 20 `
    -Description "The gateway after a normal SCM stop"
  Assert-Condition (-not (Test-ManagedGatewayPresent)) `
    "A staged fn-knock-gateway.exe survived normal service stop"

  Write-Host "Uninstalling and verifying service/firewall cleanup with ProgramData retention"
  Invoke-NativeChecked -FilePath $ServiceExecutable -Arguments @("uninstall")
  Wait-ServiceAbsent -TimeoutSeconds 30
  $firewallDeadline = [DateTime]::UtcNow.AddSeconds(15)
  while ([DateTime]::UtcNow -lt $firewallDeadline -and
         @(Get-NetFirewallRule -DisplayName $FirewallRuleName -ErrorAction SilentlyContinue).Count -ne 0) {
    Start-Sleep -Milliseconds 250
  }
  Assert-Condition `
    (@(Get-NetFirewallRule -DisplayName $FirewallRuleName -ErrorAction SilentlyContinue).Count -eq 0) `
    "The gateway firewall rule survived uninstall"
  Assert-Condition (Test-Path -LiteralPath $ProgramDataRoot -PathType Container) `
    "Uninstall removed ProgramData instead of retaining it"
  Assert-Condition (Test-Path -LiteralPath $RetainMarker -PathType Leaf) `
    "Uninstall did not preserve the ProgramData retention marker"

  $TestSucceeded = $true
  Write-Host "FnKnock Windows runtime smoke test passed"
} catch {
  if ($CleanupAuthorized) {
    Write-SmokeDiagnostics
  }
  throw
} finally {
  Invoke-SmokeCleanup
  if (-not $TestSucceeded) {
    Write-Warning "FnKnock Windows runtime smoke test failed; test-owned state was cleaned up"
  }
}
