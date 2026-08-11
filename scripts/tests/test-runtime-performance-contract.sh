#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
PACKAGE_MANIFEST="${ROOT_DIR}/package.json"
HARNESS="${ROOT_DIR}/scripts/runtime-test-harness.mjs"
MEASURE="${ROOT_DIR}/scripts/runtime-performance.mjs"
COMPARE="${ROOT_DIR}/scripts/check-runtime-performance.mjs"
WORKFLOW="${ROOT_DIR}/.github/workflows/ci.yml"

fail() {
  printf '[test-runtime-performance-contract] ERROR: %s\n' "$*" >&2
  exit 1
}

assert_contains() {
  local file="$1" expected="$2" label="$3"
  grep -Fq -- "${expected}" "${file}" || fail "${label}: ${file} is missing ${expected}"
}

for file in "${HARNESS}" "${MEASURE}" "${COMPARE}" "${WORKFLOW}"; do
  [ -f "${file}" ] || fail "missing runtime performance contract file: ${file}"
done

node -e '
  const manifest = require(process.argv[1]);
  if (manifest.scripts["runtime:measure"] !== "node ./scripts/runtime-performance.mjs") process.exit(1);
  if (manifest.scripts["runtime:measure:check"] !== "node ./scripts/check-runtime-performance.mjs") process.exit(1);
' "${PACKAGE_MANIFEST}" || fail 'runtime measurement commands are missing from package scripts'

assert_contains "${HARNESS}" 'const gatewayMetricTimeoutMs = 7_000' 'bounded gateway RSS wait'
assert_contains "${HARNESS}" 'const readinessMs = Math.round(performance.now() - startedAt)' 'readiness measured before RSS stabilization'
assert_contains "${HARNESS}" 'gateway_rss_bytes: gatewayRSS' 'Go gateway RSS collection'
assert_contains "${HARNESS}" 'collectMetrics = !protectedAdmin' 'protected runtime metric isolation'
assert_contains "${MEASURE}" 'FN_KNOCK_RUNTIME_PERF_RUNS' 'repeatable sample count'
assert_contains "${COMPARE}" '0.1' 'default readiness regression tolerance'
assert_contains "${COMPARE}" '0.05' 'default RSS regression tolerance'
assert_contains "${WORKFLOW}" 'Measure runtime readiness and idle RSS' 'scheduled runtime performance measurement'
assert_contains "${WORKFLOW}" 'FN_KNOCK_RUNTIME_PERF_RUNS: "5"' 'scheduled sample count'
assert_contains "${WORKFLOW}" 'runtime-performance-${{ github.run_id }}' 'runtime performance artifact'
assert_contains "${WORKFLOW}" 'runtime-performance:' 'PR runtime performance job'
assert_contains "${WORKFLOW}" "github.event.pull_request.base.sha" 'PR base revision'
assert_contains "${WORKFLOW}" 'git worktree add --detach' 'base source worktree'
assert_contains "${WORKFLOW}" 'Go-Reauth-Proxy worktree add --detach' 'base gateway worktree'
assert_contains "${WORKFLOW}" 'GATEWAY_COMMIT_BEFORE_MANIFEST_PIN: "a0e9ff7e34d4ae683b47011cda43578885bd75a3"' 'legacy gateway revision'
assert_contains "${WORKFLOW}" '"__before_manifest_pin__"' 'legacy gateway manifest fallback'
assert_contains "${WORKFLOW}" 'base_manifest="${{ steps.baseline.outputs.root }}/apps/server-admin-rs/Cargo.toml"' 'base Rust manifest path'
assert_contains "${WORKFLOW}" 'base_profile="runtime-test"' 'matching base runtime profile'
assert_contains "${WORKFLOW}" 'base_profile="release"' 'legacy base profile fallback'
assert_contains "${WORKFLOW}" 'CARGO_PROFILE_RELEASE_LTO="thin"' 'equivalent legacy base LTO'
assert_contains "${WORKFLOW}" 'install -m 0755 "${base_target}/${base_profile}/server-admin-rs"' 'stable base Rust binary path'
assert_contains "${WORKFLOW}" 'CARGO_TARGET_DIR="${base_target}" cargo build --locked' 'isolated base Rust target'
assert_contains "${WORKFLOW}" '--max-readiness-regression 0.10 --max-rss-regression 0.05' 'PR performance tolerances'
assert_contains "${WORKFLOW}" 'runtime-performance-pr-${{ github.event.pull_request.number }}' 'PR comparison artifact'

node --test "${ROOT_DIR}/scripts/tests/runtime-performance.test.mjs"

printf '[test-runtime-performance-contract] runtime readiness and RSS measurement contract passed\n'
