#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# Load only this pure helper; sourcing the entrypoint would start/stop services.
source /dev/stdin <<EOF
$(awk '/^backend_glibc_tunables\(\) \{/{capture=1} capture {print} capture && /^}/{exit}' "${ROOT_DIR}/apps/fn-knock/cmd/main")
EOF

check() {
    local input="$1" expected="$2" actual
    actual="$(GLIBC_TUNABLES="${input}" backend_glibc_tunables)"
    if [ "${actual}" != "${expected}" ]; then
        printf 'Unexpected allocator tunables: %s\n' "${actual}" >&2
        exit 1
    fi
}

unset GLIBC_TUNABLES
[ "$(backend_glibc_tunables)" = 'glibc.malloc.tcache_count=0' ]
check '' 'glibc.malloc.tcache_count=0'
check 'glibc.malloc.arena_max=2' 'glibc.malloc.arena_max=2:glibc.malloc.tcache_count=0'
check 'glibc.malloc.tcache_count=7' 'glibc.malloc.tcache_count=7'
check 'glibc.malloc.arena_max=2:glibc.malloc.tcache_count=3:glibc.malloc.trim_threshold=65536' \
    'glibc.malloc.arena_max=2:glibc.malloc.tcache_count=3:glibc.malloc.trim_threshold=65536'
check 'custom.glibc.malloc.tcache_count=7' 'custom.glibc.malloc.tcache_count=7:glibc.malloc.tcache_count=0'
[ "${GLIBC_TUNABLES+x}" != x ]

# Execute the real startup function against a temporary backend. This catches
# regressions where the helper still passes but its result is no longer passed
# to the process, or accidentally exported into the supervisor environment.
WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/fn-knock-allocator.XXXXXX")"
TEST_BACKEND_PID=""
cleanup() {
    if [ -n "${TEST_BACKEND_PID}" ]; then
        kill "${TEST_BACKEND_PID}" 2>/dev/null || true
        wait "${TEST_BACKEND_PID}" 2>/dev/null || true
    fi
    rm -rf "${WORK_DIR}"
}
trap cleanup EXIT
awk '/^start_backend\(\) \{/{capture=1} capture {print} capture && /^}/{exit}' \
    "${ROOT_DIR}/apps/fn-knock/cmd/main" > "${WORK_DIR}/startup.sh"
source "${WORK_DIR}/startup.sh"
cat > "${WORK_DIR}/backend" <<'EOF'
#!/bin/bash
printf '%s\n%s\n' "${GLIBC_TUNABLES-}" "${FN_KNOCK_RUNTIME_TARGET-}" > "${FN_KNOCK_DATA_DIR}/captured"
sleep 2
EOF
chmod +x "${WORK_DIR}/backend"
check_process() { kill -0 "$1" 2>/dev/null; }
log_msg() { :; }
supervisor_log() { :; }
fail_backend_start() { printf '%s\n' "$*" >&2; return 1; }
resolve_altcha_hmac_key() { :; }
resolve_hmac_secret() { :; }
resolve_internal_rpc_token() { :; }
resolve_admin_proxy_secret() { :; }
resolve_root_share_dir() { :; }
BACKEND_ENTRY="${WORK_DIR}/backend"
APP_HOME="${WORK_DIR}"
PKG_VAR_DIR="${WORK_DIR}"
ADMIN_STATIC_PATH="${WORK_DIR}"
AUTH_STATIC_PATH="${WORK_DIR}"
BACKEND_PID_FILE="${WORK_DIR}/backend.pid"
GATEWAY_PID_FILE="${WORK_DIR}/gateway.pid"
READINESS_MARKER="${WORK_DIR}/ready"
LOG_FILE="${WORK_DIR}/log"
ADMIN_VIEW_PORT=7991 BACKEND_PORT=7998 AUTH_PORT=7997
GO_BACKEND_PORT=7996 GO_REPROXY_PORT=7999 START_TIMEOUT_SECONDS=30
GATEWAY_CONFIG_DIR="${WORK_DIR}" ROOT_SHARE_DIR="${WORK_DIR}" ACME_BUNDLE_ZIP=""
FN_KNOCK_INTERNAL_RPC_TOKEN=test ALTCHA_HMAC_KEY=test HMAC_SECRET=test ADMIN_PROXY_SECRET=test
for input in '' 'glibc.malloc.arena_max=2' 'glibc.malloc.tcache_count=7'; do
    if [ -z "${input}" ]; then unset GLIBC_TUNABLES; else export GLIBC_TUNABLES="${input}"; fi
    parent_state="${GLIBC_TUNABLES+x}:${GLIBC_TUNABLES-}"
    expected="$(backend_glibc_tunables)"
    start_backend
    TEST_BACKEND_PID="$(cat "${BACKEND_PID_FILE}")"
    [ "$(cat "${WORK_DIR}/captured")" = "$(printf '%s\nfpk' "${expected}")" ]
    [ "${GLIBC_TUNABLES+x}:${GLIBC_TUNABLES-}" = "${parent_state}" ]
    wait "${TEST_BACKEND_PID}"
    TEST_BACKEND_PID=""
    rm "${BACKEND_PID_FILE}"
done
printf '[test-fpk-allocator-tunables] ok: defaults, overrides, startup environment, supervisor scope\n'
