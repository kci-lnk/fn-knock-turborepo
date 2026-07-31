function Get-FnKnockControlApiVersion {
  [CmdletBinding()]
  param([Parameter(Mandatory = $true)][string]$Root)

  $contractPath = Join-Path $Root "packages\grpc-contracts\proto\fnknock\v1\gateway.proto"
  if (-not (Test-Path -LiteralPath $contractPath -PathType Leaf)) {
    throw "Control API contract is missing: $contractPath"
  }
  $source = Get-Content -Raw -LiteralPath $contractPath
  $match = [regex]::Match(
    $source,
    '(?m)^\s*CONTROL_API_VERSION_CURRENT\s*=\s*([0-9]+)\s*;'
  )
  if (-not $match.Success) {
    throw "Unable to read CONTROL_API_VERSION_CURRENT from $contractPath"
  }
  $version = [uint64]$match.Groups[1].Value
  if ($version -eq 0) {
    throw "CONTROL_API_VERSION_CURRENT must be positive"
  }
  return $version
}

function Assert-FnKnockGoControlApiContract {
  [CmdletBinding()]
  param(
    [Parameter(Mandatory = $true)][string]$Root,
    [Parameter(Mandatory = $true)][string]$GoRepository
  )

  $expected = Get-FnKnockControlApiVersion -Root $Root
  $generatedPath = Join-Path $GoRepository "pkg\grpc\pb\gateway.pb.go"
  if (-not (Test-Path -LiteralPath $generatedPath -PathType Leaf)) {
    throw "Generated Go control API contract is missing: $generatedPath"
  }
  $source = Get-Content -Raw -LiteralPath $generatedPath
  $match = [regex]::Match(
    $source,
    '(?m)^\s*ControlApiVersion_CONTROL_API_VERSION_CURRENT\s+ControlApiVersion\s*=\s*([0-9]+)\s*$'
  )
  if (-not $match.Success) {
    throw "Generated Go control API contract is missing CONTROL_API_VERSION_CURRENT"
  }
  $actual = [uint64]$match.Groups[1].Value
  if ($actual -ne $expected) {
    throw "Generated Go control API version $actual does not match gateway.proto $expected; run npm run fn-knock:grpc:sync-go"
  }
}
