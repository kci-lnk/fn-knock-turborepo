[CmdletBinding()]
param(
  [Parameter(Mandatory = $true)]
  [string]$SetupPath,
  [string]$OutputDirectory = ""
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$Root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$Version = [string](Get-Content -Raw (Join-Path $Root "version.json") | ConvertFrom-Json).version
$BundleIdentityPath = Join-Path $Root "apps\fn-knock-desktop\bundle\windows\runtime\bundle.json"
$BundleIdentity = Get-Content -Raw $BundleIdentityPath | ConvertFrom-Json
if ([string]$BundleIdentity.version -ne $Version) {
  throw "Staged bundle identity does not match release version $Version"
}
$SetupPath = (Resolve-Path $SetupPath).Path
if (-not $OutputDirectory) {
  $OutputDirectory = Join-Path $Root "dist\fn-knock-artifacts\windows\x86_64"
}
New-Item -ItemType Directory -Force $OutputDirectory | Out-Null

$ArtifactName = "fn-knock-$Version-windows-x86_64-setup.exe"
$ArtifactPath = Join-Path $OutputDirectory $ArtifactName
Copy-Item -Force $SetupPath $ArtifactPath

$SignatureSource = "$SetupPath.sig"
if (-not (Test-Path $SignatureSource)) {
  throw "Missing Tauri signature $SignatureSource. Generate it only after Authenticode signing the setup."
}
$SignaturePath = "$ArtifactPath.sig"
Copy-Item -Force $SignatureSource $SignaturePath

$Sha256 = (Get-FileHash -Algorithm SHA256 $ArtifactPath).Hash.ToLowerInvariant()
$ShaPath = "$ArtifactPath.sha256"
[System.IO.File]::WriteAllText(
  $ShaPath,
  "$Sha256  $ArtifactName`n",
  [System.Text.UTF8Encoding]::new($false)
)

$Signature = (Get-Content -Raw $SignaturePath).Trim()
$Url = "https://cdn.fnknock.cn/files/$Version/windows/x86_64/$ArtifactName"
$PublishedAt = [DateTimeOffset]::UtcNow.ToString("o")

$Release = @{
  version = $Version
  commit = [string]$BundleIdentity.commit
  gateway_commit = [string]$BundleIdentity.gateway_commit
  runtime_target = "windows"
  architecture = "x86_64"
  channel = "stable"
  published_at = $PublishedAt
  file_name = $ArtifactName
  sha256 = $Sha256
  signature = $Signature
  size = (Get-Item $ArtifactPath).Length
  packages = @{
    windows = @{
      x86_64 = @{
        url = $Url
        sha256 = $Sha256
        signature = $Signature
        size = (Get-Item $ArtifactPath).Length
      }
    }
  }
} | ConvertTo-Json -Depth 12

$Updater = @{
  version = $Version
  notes = "FnKnock $Version"
  pub_date = $PublishedAt
  platforms = @{
    "windows-x86_64" = @{
      url = $Url
      signature = $Signature
    }
  }
} | ConvertTo-Json -Depth 12

[System.IO.File]::WriteAllText(
  (Join-Path $OutputDirectory "release.json"),
  "$Release`n",
  [System.Text.UTF8Encoding]::new($false)
)
[System.IO.File]::WriteAllText(
  (Join-Path $OutputDirectory "updater.json"),
  "$Updater`n",
  [System.Text.UTF8Encoding]::new($false)
)

Write-Host "Finalized Windows release artifacts in $OutputDirectory"
