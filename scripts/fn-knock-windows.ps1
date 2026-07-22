[CmdletBinding()]
param(
  [ValidateSet("Prepare", "Test", "Build")]
  [string]$Mode = "Build",
  [string]$GoRepository = "",
  [switch]$SkipDesktopBundle,
  [switch]$BundleInstaller,
  [switch]$SkipChecks,
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
$DesktopNative = Join-Path $DesktopRoot "native"
$BundleRoot = Join-Path $DesktopRoot "bundle\windows"
$RuntimeRoot = Join-Path $BundleRoot "runtime"
$RustAcmeshExecutable = Join-Path $Root "apps\server-admin-rs\resources\rust-acmesh.exe"

if ($SkipDesktopBundle -and $BundleInstaller) {
  throw "SkipDesktopBundle and BundleInstaller cannot be used together"
}
if ($BundleInstaller -and $Mode -ne "Build") {
  throw "BundleInstaller is only valid in Build mode"
}
if ($SkipChecks -and $Mode -ne "Build") {
  throw "SkipChecks is only valid in Build mode"
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
  Set-CargoVersion (Join-Path $DesktopNative "Cargo.toml")
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
    if (-not $SkipChecks) {
      go test -mod=readonly ./...
      Assert-LastExitCode "Go Windows tests"
    }
    New-Item -ItemType Directory -Force (Join-Path $GoRepository "build") | Out-Null
    $output = Join-Path $GoRepository "build\go-reauth-proxy-windows-amd64.exe"
    go build -mod=readonly -trimpath -ldflags "-s -w -X go-reauth-proxy/pkg/version.Version=$Version -X go-reauth-proxy/pkg/version.Commit=$GoCommit" -o $output ./cmd/server
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
  if (-not $SkipChecks) {
    cargo test --locked --manifest-path $manifest
    Assert-LastExitCode "Rust unit tests"
    cargo check --locked --manifest-path $manifest --target $Target
    Assert-LastExitCode "Rust Windows check"
  }
  cargo build --locked --release --manifest-path $manifest --target $Target
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

  if (-not (Test-Path -LiteralPath $RustAcmeshExecutable -PathType Leaf)) {
    throw "Bundled rust-acmesh.exe is missing: $RustAcmeshExecutable"
  }

  Copy-Item (Join-Path $GoRepository "build\go-reauth-proxy-windows-amd64.exe") (Join-Path $BundleRoot "fn-knock-gateway.exe")
  Copy-Item (Join-Path $Root "apps\server-admin-rs\target\$Target\release\fn-knock-service.exe") (Join-Path $BundleRoot "fn-knock-service.exe")
  Copy-Item $RustAcmeshExecutable (Join-Path $BundleRoot "rust-acmesh.exe")

  Copy-DirectoryContents (Join-Path $Root "apps\server-admin-view\dist") (Join-Path $RuntimeRoot "ui\www")
  Copy-DirectoryContents (Join-Path $Root "apps\server-auth-view\dist") (Join-Path $RuntimeRoot "server-auth-view\dist")

  $identity = @{
    version = $Version
    commit = $Commit
    gateway_commit = $GoCommit
    control_api_version = 2
    target = "windows-x86_64"
    files = @(
      "fn-knock.exe",
      "fn-knock-service.exe",
      "fn-knock-gateway.exe",
      "rust-acmesh.exe",
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

function Resolve-MakeNsis {
  if ($env:FN_KNOCK_MAKENSIS) {
    $candidate = $env:FN_KNOCK_MAKENSIS
    if (Test-Path -LiteralPath $candidate -PathType Leaf) {
      return (Resolve-Path -LiteralPath $candidate).Path
    }
    throw "FN_KNOCK_MAKENSIS does not point to makensis.exe: $candidate"
  }
  $command = Get-Command makensis.exe -ErrorAction SilentlyContinue
  if ($command) {
    return $command.Source
  }
  foreach ($candidate in @(
    (Join-Path ${env:ProgramFiles(x86)} "NSIS\makensis.exe"),
    (Join-Path $env:ProgramFiles "NSIS\makensis.exe")
  )) {
    if ($candidate -and (Test-Path -LiteralPath $candidate -PathType Leaf)) {
      return (Resolve-Path -LiteralPath $candidate).Path
    }
  }
  throw "NSIS 3 is required. Install NSIS or set FN_KNOCK_MAKENSIS to makensis.exe."
}

function New-NativeNsisInstaller {
  $makeNsis = Resolve-MakeNsis
  $releaseRoot = Join-Path $DesktopNative "target\$Target\release"
  $installerRoot = Join-Path $releaseRoot "installer"
  New-Item -ItemType Directory -Force $installerRoot | Out-Null
  $setupPath = Join-Path $installerRoot "Knock 敲门_${Version}_x64-setup.exe"
  Remove-Item -LiteralPath $setupPath -Force -ErrorAction SilentlyContinue
  $parts = @($Version.Split('.'))
  if ($parts.Count -gt 4 -or @($parts | Where-Object { $_ -notmatch '^\d+$' }).Count -gt 0) {
    throw "Version cannot be represented as a Windows file version: $Version"
  }
  while ($parts.Count -lt 4) { $parts += "0" }
  $numericVersion = $parts -join "."
  $script = Join-Path $DesktopNative "installer\installer.nsi"
  $desktopExe = Join-Path $releaseRoot "fn-knock.exe"
  $icon = Join-Path $DesktopNative "assets\icon.ico"
  $nsisOutput = @(& $makeNsis "/INPUTCHARSET" "UTF8" "/DVERSION=$Version" "/DNUMERIC_VERSION=$numericVersion" "/DOUTPUT_FILE=$setupPath" "/DDESKTOP_EXE=$desktopExe" "/DBUNDLE_ROOT=$BundleRoot" "/DRUNTIME_ROOT=$RuntimeRoot" "/DICON_FILE=$icon" $script 2>&1)
  $nsisOutput | ForEach-Object { Write-Host $_ }
  Assert-LastExitCode "native NSIS bundle"
  if (-not (Test-Path -LiteralPath $setupPath -PathType Leaf)) {
    throw "NSIS did not produce the expected setup: $setupPath"
  }
  return (Resolve-Path -LiteralPath $setupPath).Path
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
  cargo build --locked --release --manifest-path (Join-Path $DesktopNative "Cargo.toml") --target $Target
  Assert-LastExitCode "native Win32 controller release build"

  if ($BundleInstaller) {
    $setup = New-NativeNsisInstaller
    $publishedSetup = Publish-UnsignedInstaller $setup
    Write-Host "Unsigned NSIS installer: $publishedSetup"
  }
}

if ($BundleInstaller) {
  Write-Host "The local installer is unsigned and intended for development/testing only."
} else {
  Write-Host "Windows binaries and runtime are ready for Authenticode signing."
  Write-Host "After signing all three EXEs, run the native NSIS packaging step to create the installer."
}
