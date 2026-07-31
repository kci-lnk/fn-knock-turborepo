#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
RUNTIME_SMOKE_SCRIPT="${ROOT_DIR}/scripts/fn-knock-windows-smoke.ps1"
INSTALLER_SMOKE_SCRIPT="${ROOT_DIR}/scripts/fn-knock-windows-installer-smoke.ps1"
WINDOWS_BUILD_SCRIPT="${ROOT_DIR}/scripts/fn-knock-windows.ps1"
WINDOWS_FINALIZE_SCRIPT="${ROOT_DIR}/scripts/fn-knock-windows-finalize.ps1"
INSTALLER_HOOK="${ROOT_DIR}/apps/fn-knock-desktop/native/installer/hooks.nsh"
CONTROL_API_HELPER="${ROOT_DIR}/scripts/fn-knock-control-api.ps1"
SERVICE_GO_BACKEND="${ROOT_DIR}/apps/server-admin-rs/src/infra/go_backend.rs"
DESKTOP_RUNTIME="${ROOT_DIR}/apps/fn-knock-desktop/native/src/runtime.rs"
DESKTOP_BUILD="${ROOT_DIR}/apps/fn-knock-desktop/native/build.rs"
RELEASE_WORKFLOW="${ROOT_DIR}/.github/workflows/release.yml"
WINDOWS_WORKFLOW="${ROOT_DIR}/.github/workflows/windows-x86_64.yml"

fail() {
  printf '[test-windows-smoke-exit-status] ERROR: %s\n' "$*" >&2
  exit 1
}

assert_success_exit_status() {
  local script_path="$1"
  local cleanup_call="$2"
  local label="$3"
  local last_statement
  local cleanup_line
  local reset_line

  last_statement="$(
    awk '
      /^[[:space:]]*($|#)/ { next }
      { statement = $0 }
      END { print statement }
    ' "${script_path}"
  )"
  [ "${last_statement}" = '$global:LASTEXITCODE = 0' ] || \
    fail "${label} success path must clear native-command exit status after cleanup"

  cleanup_line="$(grep -n "^[[:space:]]*${cleanup_call}[[:space:]]*$" "${script_path}" | tail -1 | cut -d: -f1)"
  reset_line="$(grep -n '^\$global:LASTEXITCODE = 0$' "${script_path}" | tail -1 | cut -d: -f1)"
  [ -n "${cleanup_line}" ] || fail "${label} script does not invoke final cleanup"
  [ -n "${reset_line}" ] || fail "${label} script does not reset LASTEXITCODE"
  [ "${reset_line}" -gt "${cleanup_line}" ] || \
    fail "${label} LASTEXITCODE reset must follow successful final cleanup"
}

assert_success_exit_status "${RUNTIME_SMOKE_SCRIPT}" "Invoke-SmokeCleanup" "runtime smoke"
assert_success_exit_status "${INSTALLER_SMOKE_SCRIPT}" "Invoke-InstallerCleanup" "installer smoke"

control_api_version="$(bash "${ROOT_DIR}/scripts/control-api-version.sh")"
case "${control_api_version}" in
  ''|0|*[!0-9]*) fail "protobuf control API version must be a positive integer" ;;
esac
grep -Fq 'GATEWAY_CONTROL_API_VERSION: u64 = ControlApiVersion::Current as u64' \
  "${SERVICE_GO_BACKEND}" || \
  fail "Rust service must derive the control API version from generated protobuf code"
grep -Fq 'CONTROL_API_VERSION_CURRENT' "${CONTROL_API_HELPER}" || \
  fail "Windows tools must read the protobuf control API contract"
grep -Fq 'ControlApiVersion_CONTROL_API_VERSION_CURRENT' "${CONTROL_API_HELPER}" || \
  fail "Windows tools must validate the generated Go control API contract"
grep -Fq 'Assert-FnKnockGoControlApiContract -Root $Root -GoRepository $GoRepository' \
  "${WINDOWS_BUILD_SCRIPT}" || \
  fail "Windows bundle staging must reject stale generated Go protobuf code"
grep -Fq 'control_api_version = $ControlApiVersion' "${WINDOWS_BUILD_SCRIPT}" || \
  fail "Windows bundle staging must write the shared control API version"
grep -Fq 'control_api_version -ne $ControlApiVersion' "${WINDOWS_FINALIZE_SCRIPT}" || \
  fail "Windows finalization must use the shared control API version"
grep -Fq 'control_api_version -eq $ExpectedControlApiVersion' "${RUNTIME_SMOKE_SCRIPT}" || \
  fail "Windows runtime smoke must use the shared control API version"
grep -Fq 'control_api_version -eq $ExpectedControlApiVersion' "${INSTALLER_SMOKE_SCRIPT}" || \
  fail "Windows installer smoke must use the shared control API version"
grep -Fq '== Some(EXPECTED_CONTROL_API_VERSION)' "${DESKTOP_RUNTIME}" || \
  fail "Windows desktop readiness must use its generated control API version"
grep -Fq 'strip_prefix("CONTROL_API_VERSION_CURRENT")' "${DESKTOP_BUILD}" || \
  fail "Windows desktop build must read the protobuf control API version"
grep -Fq 'Some(expected_control_api_version)' "${DESKTOP_BUILD}" || \
  fail "Windows desktop build validation must use the shared control API version"
grep -Fq 'bash ./scripts/sync-go-grpc-contract.sh Go-Reauth-Proxy' "${RELEASE_WORKFLOW}" || \
  fail "Release CI must regenerate Go protobuf code from the shared contract"

unsafe_count_calls="$(
  grep -E '\(Get-FnKnock(FirewallRules|Processes)\)\.Count' "${INSTALLER_SMOKE_SCRIPT}" |
    grep -v '@(Get-FnKnock' || true
)"
[ -z "${unsafe_count_calls}" ] || \
  fail "installer smoke collection counts must handle zero results with @(...): ${unsafe_count_calls}"

grep -Fq '[Diagnostics.ProcessStartInfo]::new()' "${INSTALLER_SMOKE_SCRIPT}" || \
  fail "installer smoke native runner must use ProcessStartInfo"
grep -Fq '$process.WaitForExit()' "${INSTALLER_SMOKE_SCRIPT}" || \
  fail "installer smoke native runner must wait for GUI installers"
grep -Fq 'ExitCode = [int]$process.ExitCode' "${INSTALLER_SMOKE_SCRIPT}" || \
  fail "installer smoke native runner must capture the process ExitCode"

unsafe_native_exit_reads="$(
  awk '
    /^function Invoke-NativeChecked / { in_helper = 1 }
    /^function Wait-ServiceState / { in_helper = 0 }
    /^function Invoke-NativeExpectFailure / { in_helper = 1 }
    /^function Assert-UninstalledRuntime / { in_helper = 0 }
    in_helper && /\$LASTEXITCODE/ { print }
  ' "${INSTALLER_SMOKE_SCRIPT}"
)"
[ -z "${unsafe_native_exit_reads}" ] || \
  fail "installer native helpers must not rely on optional LASTEXITCODE: ${unsafe_native_exit_reads}"

if grep -Eq "['\"]/SKIPSL" "${INSTALLER_HOOK}" "${INSTALLER_SMOKE_SCRIPT}"; then
  fail "Windows installer paths must not depend on undocumented takeown /SKIPSL support"
fi
grep -Fq "\$\$rootTakeownArgs = @('/F', \$\$Path, '/A')" "${INSTALLER_HOOK}" || \
  fail "installer bootstrap must limit portable takeown to the validated root"
grep -Fq '& $$Icacls $$Path /reset /L /Q | Out-Null' "${INSTALLER_HOOK}" || \
  fail "installer bootstrap must neutralize a stale root deny without recursive link traversal"
grep -Fq "\$\$ownerArgs = @(\$\$Path, '/setowner', '*S-1-5-32-544', '/T', '/L', '/Q')" "${INSTALLER_HOOK}" || \
  fail "installer bootstrap must use documented icacls /L for recursive ownership repair"
grep -Fq '"*S-1-5-18:(OI)(CI)F" "*S-1-5-32-544:(OI)(CI)F" /T /L /Q' "${INSTALLER_HOOK}" || \
  fail "installer transaction ACL must grant one inheritable FullControl rule to SYSTEM and Administrators"
if grep -Fq '"*S-1-5-18:F"' "${INSTALLER_HOOK}" || \
    grep -Fq '"*S-1-5-32-544:F"' "${INSTALLER_HOOK}"; then
  fail "installer transaction ACL must not add duplicate non-inheriting SID rules"
fi
grep -Fq 'Set-FnKnockDataTreeAcl $$PSScriptRoot $$systemSid $$administratorsSid $$null' "${INSTALLER_HOOK}" || \
  fail "installer helper must replace inherited platform ACLs through the exact ACL API"
grep -Fq 'Set-FnKnockDataTreeOwner $$PSScriptRoot $$icacls' "${INSTALLER_HOOK}" || \
  fail "installer helper must restore SYSTEM ownership after canonicalizing its ACL"
grep -Fq 'Assert-FnKnockInstallerTreeAcl $$PSScriptRoot' "${INSTALLER_HOOK}" || \
  fail "installer helper must verify its canonicalized ACL before dispatch"
grep -Fq "\$\$platformReadSids = @('S-1-15-2-1','S-1-15-2-2')" "${INSTALLER_HOOK}" || \
  fail "installer ACL validation must recognize Windows application-package identities"
grep -Fq '($$rightsMask -band [uint32]0x530D0146) -ne 0' "${INSTALLER_HOOK}" || \
  fail "installer ACL validation must reject application-package mutation rights"
acl_normalize_line="$(grep -nF 'Set-FnKnockDataTreeAcl $$PSScriptRoot $$systemSid $$administratorsSid $$null' "${INSTALLER_HOOK}" | cut -d: -f1)"
acl_verify_line="$(grep -nF 'Assert-FnKnockInstallerTreeAcl $$PSScriptRoot' "${INSTALLER_HOOK}" | cut -d: -f1)"
action_dispatch_line="$(grep -nF 'switch ($$Action)' "${INSTALLER_HOOK}" | cut -d: -f1)"
[ "${acl_normalize_line}" -lt "${acl_verify_line}" ] && [ "${acl_verify_line}" -lt "${action_dispatch_line}" ] || \
  fail "installer helper must normalize and verify its ACL before action dispatch"
grep -Fq "[Console]::Error.WriteLine(('FnKnock installer ' + \$\$Action + ' failed: ' + \$\$_.Exception.Message))" "${INSTALLER_HOOK}" || \
  fail "installer transaction failures must emit a concise actionable error"

workflow_smoke_calls="$(
  grep -HnE 'fn-knock-windows-(smoke|installer-smoke)\.ps1' \
    "${RELEASE_WORKFLOW}" "${WINDOWS_WORKFLOW}" || true
)"
[ -z "${workflow_smoke_calls}" ] || \
  fail "GitHub Actions must not run Windows smoke/install lifecycle tests: ${workflow_smoke_calls}"

grep -Fq './scripts/fn-knock-windows-finalize.ps1 -SetupPath $setup -SignaturePolicy Unsigned' \
  "${RELEASE_WORKFLOW}" || \
  fail "release workflow must still finalize the unsigned installer metadata"

printf '[test-windows-smoke-exit-status] success exit status contract passed\n'
