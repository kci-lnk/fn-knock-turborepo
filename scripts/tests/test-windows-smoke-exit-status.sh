#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SMOKE_SCRIPT="${ROOT_DIR}/scripts/fn-knock-windows-smoke.ps1"

fail() {
  printf '[test-windows-smoke-exit-status] ERROR: %s\n' "$*" >&2
  exit 1
}

last_statement="$(
  awk '
    /^[[:space:]]*($|#)/ { next }
    { statement = $0 }
    END { print statement }
  ' "${SMOKE_SCRIPT}"
)"

[ "${last_statement}" = '$global:LASTEXITCODE = 0' ] || \
  fail "successful smoke runs must clear native-command exit status after cleanup"

cleanup_line="$(grep -n '^[[:space:]]*Invoke-SmokeCleanup[[:space:]]*$' "${SMOKE_SCRIPT}" | tail -1 | cut -d: -f1)"
reset_line="$(grep -n '^\$global:LASTEXITCODE = 0$' "${SMOKE_SCRIPT}" | tail -1 | cut -d: -f1)"
[ -n "${cleanup_line}" ] || fail "smoke script does not invoke final cleanup"
[ -n "${reset_line}" ] || fail "smoke script does not reset LASTEXITCODE"
[ "${reset_line}" -gt "${cleanup_line}" ] || \
  fail "LASTEXITCODE must be reset only after final cleanup succeeds"

printf '[test-windows-smoke-exit-status] success exit status contract passed\n'
