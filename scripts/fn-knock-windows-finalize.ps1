[CmdletBinding()]
param(
  [Parameter(Mandatory = $true)]
  [string]$SetupPath,
  [string]$OutputDirectory = "",
  [string]$ReleaseNotesPath = ""
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function Assert-WindowsInstaller([string]$Path) {
  $item = Get-Item -LiteralPath $Path
  if ($item.Length -lt 64) {
    throw "Windows installer is too small to contain a valid PE header: $Path"
  }

  $stream = [System.IO.File]::OpenRead($item.FullName)
  try {
    $dosHeader = [byte[]]::new(64)
    if ($stream.Read($dosHeader, 0, $dosHeader.Length) -ne $dosHeader.Length -or
        $dosHeader[0] -ne [byte][char]'M' -or $dosHeader[1] -ne [byte][char]'Z') {
      throw "Windows installer does not have a valid MZ header: $Path"
    }
    $peOffset = [BitConverter]::ToUInt32($dosHeader, 0x3c)
    if ([uint64]$peOffset + 4 -gt [uint64]$item.Length) {
      throw "Windows installer PE header is outside the file: $Path"
    }
    $stream.Position = $peOffset
    $peSignature = [byte[]]::new(4)
    if ($stream.Read($peSignature, 0, $peSignature.Length) -ne $peSignature.Length -or
        $peSignature[0] -ne [byte][char]'P' -or $peSignature[1] -ne [byte][char]'E' -or
        $peSignature[2] -ne 0 -or $peSignature[3] -ne 0) {
      throw "Windows installer does not have a valid PE signature: $Path"
    }
  } finally {
    $stream.Dispose()
  }

  $signature = Get-AuthenticodeSignature -LiteralPath $item.FullName
  if ($signature.Status -ne "Valid") {
    throw "Windows installer Authenticode signature is invalid: $($signature.Status) ($Path)"
  }
}

$Root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$Version = [string](Get-Content -Raw (Join-Path $Root "version.json") | ConvertFrom-Json).version
$BundleIdentityPath = Join-Path $Root "apps\fn-knock-desktop\bundle\windows\runtime\bundle.json"
$BundleIdentity = Get-Content -Raw $BundleIdentityPath | ConvertFrom-Json
if ([string]$BundleIdentity.version -ne $Version) {
  throw "Staged bundle identity does not match release version $Version"
}
foreach ($property in @("commit", "gateway_commit")) {
  $value = [string]$BundleIdentity.$property
  if ($value -notmatch '^[0-9a-fA-F]{40}$') {
    throw "Staged bundle identity has an invalid or missing $property"
  }
}
if ([int]$BundleIdentity.control_api_version -ne 1) {
  throw "Staged bundle identity has an invalid or missing control_api_version"
}
$SetupPath = (Resolve-Path $SetupPath).Path
if (-not $OutputDirectory) {
  $OutputDirectory = Join-Path $Root "dist\fn-knock-artifacts\windows\x86_64"
}
New-Item -ItemType Directory -Force $OutputDirectory | Out-Null

$ArtifactName = "fn-knock-$Version-windows-x86_64-setup.exe"
$ArtifactPath = Join-Path $OutputDirectory $ArtifactName
Copy-Item -Force $SetupPath $ArtifactPath
Assert-WindowsInstaller $ArtifactPath

$Sha256 = (Get-FileHash -Algorithm SHA256 $ArtifactPath).Hash.ToLowerInvariant()
$ShaPath = "$ArtifactPath.sha256"
[System.IO.File]::WriteAllText(
  $ShaPath,
  "$Sha256  $ArtifactName`n",
  [System.Text.UTF8Encoding]::new($false)
)

$Url = "https://cdn.fnknock.cn/files/$Version/windows/x86_64/$ArtifactName"
$PublishedAt = [DateTimeOffset]::UtcNow.ToString("o")
if (-not $ReleaseNotesPath) {
  $ReleaseNotesPath = Join-Path $Root "release-notes\$Version.md"
}
if (-not (Test-Path -LiteralPath $ReleaseNotesPath -PathType Leaf)) {
  throw "Windows release notes are required: $ReleaseNotesPath"
}
$ReleaseNotes = (Get-Content -Raw -LiteralPath $ReleaseNotesPath).Trim()
if ([string]::IsNullOrWhiteSpace($ReleaseNotes)) {
  throw "Windows release notes must not be empty"
}

$Release = @{
  version = $Version
  commit = [string]$BundleIdentity.commit
  gateway_commit = [string]$BundleIdentity.gateway_commit
  runtime_target = "windows"
  architecture = "x86_64"
  channel = "stable"
  published_at = $PublishedAt
  release_notes = $ReleaseNotes
  file_name = $ArtifactName
  sha256 = $Sha256
  size = (Get-Item $ArtifactPath).Length
  packages = @{
    windows = @{
      x86_64 = @{
        url = $Url
        sha256 = $Sha256
        size = (Get-Item $ArtifactPath).Length
      }
    }
  }
} | ConvertTo-Json -Depth 12

$Updater = @{
  version = $Version
  notes = $ReleaseNotes
  pub_date = $PublishedAt
  platforms = @{
    "windows-x86_64" = @{
      url = $Url
      sha256 = $Sha256
      size = (Get-Item $ArtifactPath).Length
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
