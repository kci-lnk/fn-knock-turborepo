#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
RUNTIME_SMOKE_SCRIPT="${ROOT_DIR}/scripts/fn-knock-windows-smoke.ps1"
INSTALLER_SMOKE_SCRIPT="${ROOT_DIR}/scripts/fn-knock-windows-installer-smoke.ps1"
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
