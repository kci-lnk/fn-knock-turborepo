[CmdletBinding()]
param(
  [ValidateSet("Prepare", "Test", "Build")]
  [string]$Mode = "Build",
  [string]$GoRepository = "",
  [switch]$SkipDesktopBundle
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

if ($Mode -eq "Build") {
  if (git -C $Root status --porcelain) {
    throw "Windows release builds require a clean fn-knock source tree"
  }
  if (git -C $GoRepository status --porcelain) {
    throw "Windows release builds require a clean Go gateway source tree"
  }
}

function Assert-LastExitCode([string]$Operation) {
  if ($LASTEXITCODE -ne 0) {
    throw "$Operation failed with exit code $LASTEXITCODE"
  }
}

function Set-JsonVersion([string]$Path) {
  $document = Get-Content -Raw $Path | ConvertFrom-Json
  $document.version = $Version
  $json = $document | ConvertTo-Json -Depth 30
  [System.IO.File]::WriteAllText($Path, "$json`n", [System.Text.UTF8Encoding]::new($false))
}

function Set-CargoVersion([string]$Path) {
  $content = Get-Content -Raw $Path
  $pattern = [regex]::new('(?ms)(\[package\].*?^version\s*=\s*)"[^"]+"')
  $replacement = '${1}"' + $Version + '"'
  $updated = $pattern.Replace($content, $replacement, 1)
  if ($updated -eq $content -and $content -notmatch "version\s*=\s*`"$([regex]::Escape($Version))`"") {
    throw "Unable to synchronize Cargo version in $Path"
  }
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

function New-TauriCiConfig {
  if (-not $env:FN_KNOCK_UPDATER_PUBLIC_KEY) {
    throw "FN_KNOCK_UPDATER_PUBLIC_KEY is required for a Windows release build"
  }
  $config = @{
    plugins = @{
      updater = @{
        pubkey = $env:FN_KNOCK_UPDATER_PUBLIC_KEY
        endpoints = @("https://cdn.fnknock.cn/windows/stable/latest.json")
        windows = @{ installMode = "passive" }
      }
    }
  } | ConvertTo-Json -Depth 10
  $path = Join-Path $DesktopTauri "tauri.ci.conf.json"
  [System.IO.File]::WriteAllText($path, "$config`n", [System.Text.UTF8Encoding]::new($false))
  return $path
}

Sync-Versions

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
  $ciConfig = New-TauriCiConfig
  Push-Location $DesktopRoot
  try {
    npm run tauri -- build --target $Target --no-bundle --config $ciConfig
    Assert-LastExitCode "Tauri Windows executable build"
  } finally {
    Pop-Location
  }
}

Write-Host "Windows binaries and runtime are ready for Authenticode signing."
Write-Host "After signing all three EXEs, run: npm run tauri -- bundle --target $Target --bundles nsis --config <ci-config>"
