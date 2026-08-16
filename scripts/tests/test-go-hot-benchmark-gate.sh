#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
GO_REPOSITORY="${FN_KNOCK_GO_REAUTH_PROXY_DIR:-${FN_KNOCK_GO_REAUTH_PROXY_REPO:-${ROOT_DIR}/../Go-Reauth-Proxy}}"

fail() {
  printf '[test-go-hot-benchmark-gate] ERROR: %s\n' "$*" >&2
  exit 1
}

assert_contains() {
  local file="$1" expected="$2" label="$3"
  grep -Fq -- "${expected}" "${file}" || fail "${label}: ${file} is missing ${expected}"
}

[ -d "${GO_REPOSITORY}" ] || fail "Go repository not found: ${GO_REPOSITORY}"

WORKFLOW="${GO_REPOSITORY}/.github/workflows/ci.yml"
COMPARATOR="${GO_REPOSITORY}/tools/benchcheck/main.go"
README="${GO_REPOSITORY}/README.md"

[ -f "${WORKFLOW}" ] || fail "Go CI workflow not found: ${WORKFLOW}"
[ -f "${COMPARATOR}" ] || fail "Go benchmark comparator not found: ${COMPARATOR}"

assert_contains "${COMPARATOR}" 'func parseBenchmarkSamples' 'benchmark sample parser'
assert_contains "${COMPARATOR}" 'func compareBenchmarks' 'benchmark comparator'
assert_contains "${COMPARATOR}" 'max-latency-regression' 'latency regression threshold'
assert_contains "${COMPARATOR}" 'max-bytes-regression' 'memory regression threshold'
assert_contains "${COMPARATOR}" 'max-allocs-regression' 'allocation regression threshold'
assert_contains "${COMPARATOR}" 'max-allocs-absolute-regression' 'allocation rounding threshold'
assert_contains "${WORKFLOW}" 'hot-benchmarks:' 'PR hot benchmark job'
assert_contains "${WORKFLOW}" "github.event_name == 'pull_request'" 'PR-only benchmark gate'
assert_contains "${WORKFLOW}" 'github.event.pull_request.base.sha' 'PR baseline commit'
assert_contains "${WORKFLOW}" 'git worktree add --detach' 'isolated baseline worktree'
assert_contains "${WORKFLOW}" '-count=1' 'isolated benchmark process samples'
assert_contains "${WORKFLOW}" 'for iteration in $(seq 1 6)' 'multiple benchmark samples'
assert_contains "${WORKFLOW}" '-benchmem' 'allocation benchmark metrics'
assert_contains "${WORKFLOW}" '--max-latency-regression 0.05' 'latency tolerance'
assert_contains "${WORKFLOW}" '--max-bytes-regression 0.05' 'memory tolerance'
assert_contains "${WORKFLOW}" '--max-allocs-regression 0.05' 'allocation tolerance'
assert_contains "${WORKFLOW}" '--max-allocs-absolute-regression 1' 'allocation rounding tolerance'
assert_contains "${README}" '每个 PR 会在同一 CI runner 上将热路径 benchmark' 'benchmark gate documentation'

(
  cd "${GO_REPOSITORY}"
  go test ./tools/benchcheck
)

printf '[test-go-hot-benchmark-gate] Go hot-path benchmark regression gate passed\n'
