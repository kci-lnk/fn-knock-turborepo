[CmdletBinding()]
param(
  [ValidateSet("Prepare", "Test", "Build")]
  [string]$Mode = "Build",
  [string]$GoRepository = "",
  [switch]$SkipDesktopBundle,
  [switch]$BundleInstaller,
  [switch]$RequireCleanTree,
  [string]$OutputDirectory = ""
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$Root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
if (-not $GoRepository) {
  $GoRepository = Join-Path (Split-Path $Root -Parent) "Go-Reauth-Proxy"
}
$GoRepository = (Resolve-Path $GoRepository).Path
$GoCommit = (git -C $GoRepository rev-parse HEAD).Trim()
if ($env:FN_KNOCK_GO_SOURCE_COMMIT) {
  $ExpectedGoCommit = $env:FN_KNOCK_GO_SOURCE_COMMIT.Trim().ToLowerInvariant()
  if ($ExpectedGoCommit -notmatch '^[0-9a-f]{40}$' -or $GoCommit.ToLowerInvariant() -ne $ExpectedGoCommit) {
    throw "Go source checkout $GoCommit does not match locked commit $ExpectedGoCommit"
  }
}
$VersionDocument = Get-Content -Raw (Join-Path $Root "version.json") | ConvertFrom-Json
$Version = [string]$VersionDocument.version
$Commit = (git -C $Root rev-parse HEAD).Trim()
$Target = "x86_64-pc-windows-msvc"
$DesktopRoot = Join-Path $Root "apps\fn-knock-desktop"
$DesktopTauri = Join-Path $DesktopRoot "src-tauri"
$BundleRoot = Join-Path $DesktopRoot "bundle\windows"
$RuntimeRoot = Join-Path $BundleRoot "runtime"

if ($SkipDesktopBundle -and $BundleInstaller) {
  throw "SkipDesktopBundle and BundleInstaller cannot be used together"
}
if ($BundleInstaller -and $Mode -ne "Build") {
  throw "BundleInstaller is only valid in Build mode"
}

function Assert-CleanReleaseTrees([string]$Phase) {
  if (git -C $Root status --porcelain) {
    throw "Windows release builds require a clean fn-knock source tree ($Phase)"
  }
  if (git -C $GoRepository status --porcelain) {
    throw "Windows release builds require a clean Go gateway source tree ($Phase)"
  }
}

if ($Mode -eq "Build" -and $RequireCleanTree) {
  Assert-CleanReleaseTrees "before version synchronization"
}

function Assert-LastExitCode([string]$Operation) {
  if ($LASTEXITCODE -ne 0) {
    throw "$Operation failed with exit code $LASTEXITCODE"
  }
}

function Set-JsonVersion([string]$Path) {
  $document = Get-Content -Raw $Path | ConvertFrom-Json
  if ([string]$document.version -eq $Version) {
    return
  }
  $document.version = $Version
  $json = $document | ConvertTo-Json -Depth 30
  [System.IO.File]::WriteAllText($Path, "$json`n", [System.Text.UTF8Encoding]::new($false))
}

function Set-CargoVersion([string]$Path) {
  $content = Get-Content -Raw $Path
  $pattern = [regex]::new('(?ms)(\[package\].*?^version\s*=\s*)"([^"]+)"')
  $match = $pattern.Match($content)
  if (-not $match.Success) {
    throw "Unable to locate the Cargo package version in $Path"
  }
  if ($match.Groups[2].Value -eq $Version) {
    return
  }
  $replacement = '${1}"' + $Version + '"'
  $updated = $pattern.Replace($content, $replacement, 1)
  [System.IO.File]::WriteAllText($Path, $updated, [System.Text.UTF8Encoding]::new($false))
}

function Sync-Versions {
  Set-JsonVersion (Join-Path $DesktopRoot "package.json")
  Set-JsonVersion (Join-Path $DesktopTauri "tauri.conf.json")
  Set-CargoVersion (Join-Path $DesktopTauri "Cargo.toml")
  Set-CargoVersion (Join-Path $Root "apps\server-admin-rs\Cargo.toml")
}

function Invoke-FrontendBuilds {
  npm run build --workspace server-admin-view
  Assert-LastExitCode "server-admin-view build"
  npm run build --workspace server-auth-view
  Assert-LastExitCode "server-auth-view build"
  npm run build --workspace fn-knock-desktop
  Assert-LastExitCode "fn-knock-desktop build"
}

function Invoke-GoChecksAndBuild {
  Push-Location $GoRepository
  try {
    $env:GOOS = "windows"
    $env:GOARCH = "amd64"
    $env:CGO_ENABLED = "0"
    go test ./...
    Assert-LastExitCode "Go Windows tests"
    New-Item -ItemType Directory -Force (Join-Path $GoRepository "build") | Out-Null
    $output = Join-Path $GoRepository "build\go-reauth-proxy-windows-amd64.exe"
    go build -trimpath -ldflags "-s -w -X go-reauth-proxy/pkg/version.Version=$Version -X go-reauth-proxy/pkg/version.Commit=$GoCommit" -o $output ./cmd/server
    Assert-LastExitCode "Go Windows build"
  } finally {
    Pop-Location
  }
}

function Invoke-RustChecksAndBuild {
  $manifest = Join-Path $Root "apps\server-admin-rs\Cargo.toml"
  $env:FN_KNOCK_DEPLOYMENT_TARGET = "windows"
  $env:FN_KNOCK_COMMIT = $Commit
  $env:FN_KNOCK_GATEWAY_COMMIT = $GoCommit
  cargo test --manifest-path $manifest
  Assert-LastExitCode "Rust unit tests"
  cargo check --manifest-path $manifest --target $Target
  Assert-LastExitCode "Rust Windows check"
  cargo build --release --manifest-path $manifest --target $Target
  Assert-LastExitCode "Rust Windows release build"
}

function Copy-DirectoryContents([string]$Source, [string]$Destination) {
  New-Item -ItemType Directory -Force $Destination | Out-Null
  Get-ChildItem -Force $Destination | Remove-Item -Recurse -Force
  Copy-Item -Path (Join-Path $Source "*") -Destination $Destination -Recurse -Force
}

function Stage-WindowsBundle {
  if (Test-Path $BundleRoot) {
    Remove-Item -Recurse -Force $BundleRoot
  }
  New-Item -ItemType Directory -Force $RuntimeRoot | Out-Null

  Copy-Item (Join-Path $GoRepository "build\go-reauth-proxy-windows-amd64.exe") (Join-Path $BundleRoot "fn-knock-gateway.exe")
  Copy-Item (Join-Path $Root "apps\server-admin-rs\target\$Target\release\fn-knock-service.exe") (Join-Path $BundleRoot "fn-knock-service.exe")

  Copy-DirectoryContents (Join-Path $Root "apps\server-admin-view\dist") (Join-Path $RuntimeRoot "ui\www")
  Copy-DirectoryContents (Join-Path $Root "apps\server-auth-view\dist") (Join-Path $RuntimeRoot "server-auth-view\dist")

  $identity = @{
    version = $Version
    commit = $Commit
    gateway_commit = $GoCommit
    control_api_version = 1
    target = "windows-x86_64"
    files = @(
      "fn-knock.exe",
      "fn-knock-service.exe",
      "fn-knock-gateway.exe",
      "ui/www",
      "server-auth-view/dist"
    )
  } | ConvertTo-Json -Depth 10
  [System.IO.File]::WriteAllText(
    (Join-Path $RuntimeRoot "bundle.json"),
    "$identity`n",
    [System.Text.UTF8Encoding]::new($false)
  )
}

function New-TauriBuildConfig {
  $temporaryRoot = $null
  $publicKey = ""
  $endpoint = "https://cdn.fnknock.cn/windows/stable/latest.json"
  $path = Join-Path $DesktopTauri "tauri.ci.conf.json"

  if (-not [string]::IsNullOrWhiteSpace($env:FN_KNOCK_UPDATER_PUBLIC_KEY)) {
    $publicKey = $env:FN_KNOCK_UPDATER_PUBLIC_KEY.Trim()
  } else {
    if (-not $BundleInstaller) {
      throw "FN_KNOCK_UPDATER_PUBLIC_KEY is required for a Windows release build"
    }
    $temporaryRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("fn-knock-tauri-" + [guid]::NewGuid().ToString("N"))
    New-Item -ItemType Directory -Force $temporaryRoot | Out-Null
    $keyPath = Join-Path $temporaryRoot "updater.key"
    try {
      Push-Location $DesktopRoot
      try {
        npm run tauri -- signer generate --ci --write-keys $keyPath *> $null
        Assert-LastExitCode "temporary Tauri updater key generation"
      } finally {
        Pop-Location
      }
      $publicKeyPath = "$keyPath.pub"
      if (-not (Test-Path -LiteralPath $publicKeyPath -PathType Leaf)) {
        throw "Temporary Tauri updater public key was not created"
      }
      $publicKey = (Get-Content -Raw -LiteralPath $publicKeyPath).Trim()
      if (-not $publicKey) {
        throw "Temporary Tauri updater public key is empty"
      }
      Remove-Item -LiteralPath $keyPath, $publicKeyPath -Force
    } catch {
      Remove-Item -LiteralPath $temporaryRoot -Recurse -Force -ErrorAction SilentlyContinue
      throw
    }
    $endpoint = "https://updates.invalid/fn-knock/windows/latest.json"
    $path = Join-Path $temporaryRoot "tauri.local.conf.json"
    Write-Warning "FN_KNOCK_UPDATER_PUBLIC_KEY is not set; this unsigned local build uses an ephemeral updater public key"
  }

  $config = @{
    plugins = @{
      updater = @{
        pubkey = $publicKey
        endpoints = @($endpoint)
        windows = @{ installMode = "passive" }
      }
    }
  } | ConvertTo-Json -Depth 10
  try {
    [System.IO.File]::WriteAllText($path, "$config`n", [System.Text.UTF8Encoding]::new($false))
  } catch {
    if ($temporaryRoot) {
      Remove-Item -LiteralPath $temporaryRoot -Recurse -Force -ErrorAction SilentlyContinue
    }
    throw
  }
  return [pscustomobject]@{
    Path = $path
    TemporaryRoot = $temporaryRoot
  }
}

function Publish-UnsignedInstaller([string]$SetupPath) {
  if (-not $OutputDirectory) {
    $script:OutputDirectory = Join-Path $Root "dist\windows"
  } elseif (-not [System.IO.Path]::IsPathRooted($OutputDirectory)) {
    $script:OutputDirectory = Join-Path $Root $OutputDirectory
  }
  New-Item -ItemType Directory -Force $OutputDirectory | Out-Null
  $artifactPath = Join-Path $OutputDirectory "fn-knock-$Version-windows-x86_64-unsigned-setup.exe"
  Copy-Item -LiteralPath $SetupPath -Destination $artifactPath -Force
  return (Resolve-Path -LiteralPath $artifactPath).Path
}

Sync-Versions

if ($Mode -eq "Build" -and $RequireCleanTree) {
  Assert-CleanReleaseTrees "after version synchronization; commit synchronized version metadata before release"
}

if ($Mode -eq "Prepare") {
  Invoke-FrontendBuilds
  Invoke-GoChecksAndBuild
  Invoke-RustChecksAndBuild
  Stage-WindowsBundle
  Write-Host "Prepared $BundleRoot"
  exit 0
}

if ($Mode -eq "Test") {
  npm run check-types --workspace server-admin-view
  Assert-LastExitCode "server-admin-view type check"
  npm run check-types --workspace server-auth-view
  Assert-LastExitCode "server-auth-view type check"
  npm run check-types --workspace fn-knock-desktop
  Assert-LastExitCode "fn-knock-desktop type check"
  Invoke-GoChecksAndBuild
  Invoke-RustChecksAndBuild
  exit 0
}

if (-not $IsWindows -or $env:PROCESSOR_ARCHITECTURE -notmatch "AMD64") {
  throw "Release packaging must run on a native Windows x86_64 runner"
}

Invoke-FrontendBuilds
Invoke-GoChecksAndBuild
Invoke-RustChecksAndBuild
Stage-WindowsBundle

if (-not $SkipDesktopBundle) {
  $buildConfig = New-TauriBuildConfig
  try {
    Push-Location $DesktopRoot
    try {
      npm run tauri -- build --target $Target --no-bundle --no-sign --config $buildConfig.Path --ci
      Assert-LastExitCode "Tauri Windows executable build"

      if ($BundleInstaller) {
        $bundleStartedAt = [DateTime]::UtcNow.AddSeconds(-2)
        npm run tauri -- bundle --target $Target --bundles nsis --no-sign --config $buildConfig.Path --ci
        Assert-LastExitCode "Tauri unsigned NSIS bundle"
        $setupRoot = Join-Path $DesktopTauri "target\$Target\release\bundle\nsis"
        $setups = @(Get-ChildItem -LiteralPath $setupRoot -Filter "*-setup.exe" -File |
          Where-Object LastWriteTimeUtc -ge $bundleStartedAt |
          Sort-Object LastWriteTimeUtc -Descending)
        if ($setups.Count -ne 1) {
          throw "Expected exactly one newly generated NSIS setup in $setupRoot, found $($setups.Count)"
        }
        $publishedSetup = Publish-UnsignedInstaller $setups[0].FullName
        Write-Host "Unsigned NSIS installer: $publishedSetup"
      }
    } finally {
      Pop-Location
    }
  } finally {
    if ($buildConfig.TemporaryRoot) {
      Remove-Item -LiteralPath $buildConfig.TemporaryRoot -Recurse -Force -ErrorAction SilentlyContinue
    }
  }
}

if ($BundleInstaller) {
  Write-Host "The local installer is unsigned and intended for development/testing only."
} else {
  Write-Host "Windows binaries and runtime are ready for Authenticode signing."
  Write-Host "After signing all three EXEs, run: npm run tauri -- bundle --target $Target --bundles nsis --config <ci-config>"
}
