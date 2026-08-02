#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
GO_REPOSITORY="${FN_KNOCK_GO_REAUTH_PROXY_DIR:-${FN_KNOCK_GO_REAUTH_PROXY_REPO:-${ROOT_DIR}/../Go-Reauth-Proxy}}"

fail() {
  printf '[test-runtime-health-platform-contract] ERROR: %s\n' "$*" >&2
  exit 1
}

assert_contains() {
  local file="$1" expected="$2" label="$3"
  grep -Fq -- "${expected}" "${file}" || fail "${label}: ${file} is missing ${expected}"
}

[ -d "${GO_REPOSITORY}" ] || fail "Go repository not found: ${GO_REPOSITORY}"

PROTO="${ROOT_DIR}/packages/grpc-contracts/proto/fnknock/v1/gateway.proto"
RUNTIME="${ROOT_DIR}/apps/server-admin-rs/src/runtime_health.rs"
ROUTES="${ROOT_DIR}/apps/server-admin-rs/src/runtime_health/routes.rs"
VIEW="${ROOT_DIR}/apps/server-admin-view/src/views/event-center/RuntimeComponentCard.vue"
TAB="${ROOT_DIR}/apps/server-admin-view/src/views/event-center/RuntimeTab.vue"
API="${ROOT_DIR}/apps/server-admin-view/src/lib/api/runtime-health.ts"
WINDOWS="${ROOT_DIR}/apps/server-admin-rs/src/windows_service.rs"

assert_contains "${PROTO}" 'uint64 rss_bytes = 9;' 'shared RSS field'
for target in linux macos; do
  assert_contains "${RUNTIME}" "target_os = \"${target}\"" "Rust ${target} RSS implementation"
done
assert_contains "${RUNTIME}" '#[cfg(windows)]' 'Rust windows RSS implementation'
for target in linux darwin windows other; do
  file="${GO_REPOSITORY}/pkg/admin/runtime_rss_${target}.go"
  [ -f "${file}" ] || fail "Go RSS implementation is missing: ${file}"
  assert_contains "${file}" 'func currentProcessRSSBytes() uint64' "Go ${target} RSS implementation"
done
assert_contains "${VIEW}" 'component.rss_bytes != null' 'RSS UI visibility'
assert_contains "${ROUTES}" 'get(runtime_logs).delete(clear_runtime_logs)' 'clear-log API route'
assert_contains "${API}" 'apiClient.delete' 'clear-log frontend client'
assert_contains "${TAB}" 'ConfirmDangerPopover' 'clear-log confirmation UI'

for launcher in \
  "${ROOT_DIR}/apps/fn-knock/cmd/main" \
  "${ROOT_DIR}/apps/fn-knock-lite/cmd/main" \
  "${ROOT_DIR}/deploy/linux/fn-knock-entrypoint" \
  "${ROOT_DIR}/deploy/docker/entrypoint.sh" \
  "${ROOT_DIR}/deploy/openwrt/etc/init.d/fn-knock" \
  "${ROOT_DIR}/apps/fn-knock-synology/package/bin/fn-knock-entrypoint"
do
  assert_contains "${launcher}" 'runtime/logs' 'platform runtime log directory'
  assert_contains "${launcher}" 'FN_KNOCK_DATA_DIR' 'platform shared data directory'
done
assert_contains "${WINDOWS}" 'FN_KNOCK_RUNTIME_TARGET", "windows"' 'Windows runtime target'
assert_contains "${WINDOWS}" 'paths.data.join("runtime/logs")' 'Windows runtime log directory'
assert_contains "${RUNTIME}" 'let mut exit_code = 0u32;' 'Windows process exit-code type'
assert_contains "${RUNTIME}" 'exit_code == STILL_ACTIVE as u32' 'Windows active-process status type'

assert_contains "${RUNTIME}" 'RUNTIME_STATE_TTL_SECONDS: i64 = 7 * 24 * 60 * 60' 'runtime state TTL'
assert_contains "${RUNTIME}" 'PENDING_EVENT_TTL: Duration = Duration::from_secs(60 * 60)' 'pending event TTL'
assert_contains "${RUNTIME}" 'SUPERVISOR_HINT_TTL: Duration = Duration::from_secs(7 * 24 * 60 * 60)' 'supervisor hint TTL'
assert_contains "${RUNTIME}" 'SUPERVISOR_TEMP_TTL: Duration = Duration::from_secs(24 * 60 * 60)' 'supervisor temp TTL'
assert_contains "${RUNTIME}" 'LOG_REPEAT_TTL: Duration = Duration::from_secs(5 * 60)' 'repeat aggregation TTL'
assert_contains "${ROUTES}" 'LOG_CUTOFF_MS: i64 = 24 * 60 * 60 * 1000' 'visible log TTL'

printf '[test-runtime-health-platform-contract] cross-platform RSS, logs, clear API, and TTL contract passed\n'
