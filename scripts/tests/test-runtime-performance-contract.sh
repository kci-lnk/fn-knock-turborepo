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
assert_contains "${MEASURE}" 'schema_version: 2' 'six-checkpoint measurement schema'
assert_contains "${MEASURE}" 'post_load_30s' 'post-load retention checkpoint'
assert_contains "${MEASURE}" 'post_reclaim' 'explicit reclaim checkpoint'
assert_contains "${MEASURE}" 'Brotli HEAD response failed the static asset contract' 'static compression and HEAD acceptance'
assert_contains "${MEASURE}" 'static compression did not honor Accept-Encoding quality' 'weighted static compression acceptance'
assert_contains "${MEASURE}" 'static asset If-None-Match did not return an empty 304' 'static ETag acceptance'
assert_contains "${MEASURE}" 'admin SPA fallback accepted a non-read method' 'static method boundary acceptance'
assert_contains "${MEASURE}" 'exact API root escaped the JSON not-found boundary' 'exact API boundary acceptance'
assert_contains "${MEASURE}" 'auth index did not set the configured locale cookie' 'locale cookie acceptance'
assert_contains "${COMPARE}" '0.1' 'default readiness regression tolerance'
assert_contains "${COMPARE}" '0.05' 'default RSS regression tolerance'
assert_contains "${WORKFLOW}" 'Measure runtime readiness and idle RSS' 'scheduled runtime performance measurement'
assert_contains "${WORKFLOW}" 'FN_KNOCK_RUNTIME_PERF_RUNS: "5"' 'scheduled sample count'
assert_contains "${WORKFLOW}" 'runtime-performance-${{ github.run_id }}' 'runtime performance artifact'
assert_contains "${WORKFLOW}" 'runtime-performance:' 'PR runtime performance job'
assert_contains "${WORKFLOW}" "github.event.pull_request.base.sha" 'PR base revision'
assert_contains "${WORKFLOW}" 'git worktree add --detach' 'base source worktree'
assert_contains "${WORKFLOW}" 'Go-Reauth-Proxy worktree add --detach' 'base gateway worktree'
assert_contains "${WORKFLOW}" 'base_gateway_commit="$(jq -er .gatewayCommit "${base_root}/version.json")"' 'version-locked base gateway'
assert_contains "${WORKFLOW}" 'locked_commit="$(jq -er .gatewayCommit version.json)"' 'version-locked current gateway'
assert_contains "${WORKFLOW}" 'FN_KNOCK_RUNTIME_PERF_EXERCISE_MEMORY_CONFIG="0"' 'legacy baseline memory API compatibility'
assert_contains "${WORKFLOW}" 'base_manifest="${{ steps.baseline.outputs.root }}/apps/server-admin-rs/Cargo.toml"' 'base Rust manifest path'
assert_contains "${WORKFLOW}" 'base_profile="runtime-test"' 'matching base runtime profile'
assert_contains "${WORKFLOW}" 'base_profile="release"' 'legacy base profile fallback'
assert_contains "${WORKFLOW}" 'CARGO_PROFILE_RELEASE_LTO="thin"' 'equivalent legacy base LTO'
assert_contains "${WORKFLOW}" 'install -m 0755 "${base_target}/${base_profile}/server-admin-rs"' 'stable base Rust binary path'
assert_contains "${WORKFLOW}" 'CARGO_TARGET_DIR="${base_target}" cargo build --locked' 'isolated base Rust target'
assert_contains "${WORKFLOW}" '--max-readiness-regression 0.10 --max-rss-regression 0.05' 'PR performance tolerances'
assert_contains "${WORKFLOW}" '--memory=512m --memory-swap=512m' '512 MiB cgroup acceptance'
assert_contains "${WORKFLOW}" 'gateway_memory_limit_bytes == 134217728' '512 MiB auto memory limit assertion'
assert_contains "${WORKFLOW}" 'FN_KNOCK_RUNTIME_PERF_REJECT_MEMORY_LIMIT_MIB=257' '512 MiB manual limit rejection'
assert_contains "${WORKFLOW}" '/artifacts/go-stream-tests -test.v' '512 MiB UDP and stream test suite'
assert_contains "${WORKFLOW}" 'runtime-performance-pr-${{ github.event.pull_request.number }}' 'PR comparison artifact'

node --test "${ROOT_DIR}/scripts/tests/runtime-performance.test.mjs"

printf '[test-runtime-performance-contract] runtime readiness and RSS measurement contract passed\n'
