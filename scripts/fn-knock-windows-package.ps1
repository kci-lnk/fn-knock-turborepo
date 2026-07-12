[CmdletBinding()]
param(
  [string]$OutputPath = ""
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$Root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$Version = [string](Get-Content -Raw (Join-Path $Root "version.json") | ConvertFrom-Json).version
$Target = "x86_64-pc-windows-msvc"
$DesktopNative = Join-Path $Root "apps\fn-knock-desktop\native"
$BundleRoot = Join-Path $Root "apps\fn-knock-desktop\bundle\windows"
$RuntimeRoot = Join-Path $BundleRoot "runtime"
$ReleaseRoot = Join-Path $DesktopNative "target\$Target\release"

function Resolve-MakeNsis {
  if ($env:FN_KNOCK_MAKENSIS) {
    if (Test-Path -LiteralPath $env:FN_KNOCK_MAKENSIS -PathType Leaf) {
      return (Resolve-Path -LiteralPath $env:FN_KNOCK_MAKENSIS).Path
    }
    throw "FN_KNOCK_MAKENSIS does not point to makensis.exe: $env:FN_KNOCK_MAKENSIS"
  }
  $command = Get-Command makensis.exe -ErrorAction SilentlyContinue
  if ($command) { return $command.Source }
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

foreach ($required in @(
  (Join-Path $ReleaseRoot "fn-knock.exe"),
  (Join-Path $BundleRoot "fn-knock-service.exe"),
  (Join-Path $BundleRoot "fn-knock-gateway.exe"),
  (Join-Path $BundleRoot "rust-acmesh.exe"),
  (Join-Path $RuntimeRoot "bundle.json")
)) {
  if (-not (Test-Path -LiteralPath $required -PathType Leaf)) {
    throw "Missing signed Windows package input: $required"
  }
}

if (-not $OutputPath) {
  $OutputPath = Join-Path $ReleaseRoot "installer\Knock 敲门_${Version}_x64-setup.exe"
} elseif (-not [System.IO.Path]::IsPathRooted($OutputPath)) {
  $OutputPath = Join-Path $Root $OutputPath
}
$OutputPath = [System.IO.Path]::GetFullPath($OutputPath)
New-Item -ItemType Directory -Force (Split-Path $OutputPath -Parent) | Out-Null
Remove-Item -LiteralPath $OutputPath -Force -ErrorAction SilentlyContinue

$parts = @($Version.Split('.'))
if ($parts.Count -gt 4 -or @($parts | Where-Object { $_ -notmatch '^\d+$' }).Count -gt 0) {
  throw "Version cannot be represented as a Windows file version: $Version"
}
while ($parts.Count -lt 4) { $parts += "0" }
$NumericVersion = $parts -join "."

$makeNsis = Resolve-MakeNsis
$installerScript = Join-Path $DesktopNative "installer\installer.nsi"
$desktopExe = Join-Path $ReleaseRoot "fn-knock.exe"
$icon = Join-Path $DesktopNative "assets\icon.ico"
& $makeNsis "/INPUTCHARSET" "UTF8" "/DVERSION=$Version" "/DNUMERIC_VERSION=$NumericVersion" "/DOUTPUT_FILE=$OutputPath" "/DDESKTOP_EXE=$desktopExe" "/DBUNDLE_ROOT=$BundleRoot" "/DRUNTIME_ROOT=$RuntimeRoot" "/DICON_FILE=$icon" $installerScript
if ($LASTEXITCODE -ne 0) {
  throw "native NSIS bundle failed with exit code $LASTEXITCODE"
}
if (-not (Test-Path -LiteralPath $OutputPath -PathType Leaf)) {
  throw "NSIS did not produce the expected setup: $OutputPath"
}
(Resolve-Path -LiteralPath $OutputPath).Path
