#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ENTRYPOINT="${ROOT_DIR}/apps/fn-knock-synology/package/bin/fn-knock-entrypoint"
WORK_DIR="$(mktemp -d "${ROOT_DIR}/dist/synology-entrypoint-test.XXXXXX")"
APP_HOME="${WORK_DIR}/target"
DATA_DIR="${WORK_DIR}/var"
LOG_FILE="${WORK_DIR}/entrypoint.log"
SUPERVISOR_PID=""
GATEWAY_PID=""
MANAGEMENT_PID=""

process_is_alive() {
  local pid="$1"
  kill -0 "${pid}" 2>/dev/null
}

terminate_if_running() {
  local pid="${1:-}"
  if [ -n "${pid}" ] && process_is_alive "${pid}"; then
    kill -TERM "${pid}" 2>/dev/null || true
  fi
}

cleanup() {
  trap - EXIT INT TERM
  terminate_if_running "${SUPERVISOR_PID}"
  terminate_if_running "${MANAGEMENT_PID}"
  terminate_if_running "${GATEWAY_PID}"
  [ -n "${SUPERVISOR_PID}" ] && wait "${SUPERVISOR_PID}" 2>/dev/null || true
  rm -rf "${WORK_DIR}"
}
trap cleanup EXIT INT TERM

fail() {
  printf '[test-synology-entrypoint] ERROR: %s\n' "$*" >&2
  if [ -f "${LOG_FILE}" ]; then
    sed -n '1,160p' "${LOG_FILE}" >&2
  fi
  exit 1
}

read_running_pid() {
  local path="$1"
  local label="$2"
  local pid

  [ -s "${path}" ] || fail "${label} PID file was not created"
  pid="$(tr -d '\r\n' < "${path}")"
  case "${pid}" in
    ''|*[!0-9]*) fail "${label} PID file is invalid: ${pid:-<empty>}" ;;
  esac
  process_is_alive "${pid}" || fail "${label} process ${pid} is not running"
  printf '%s\n' "${pid}"
}

wait_for_processes_started() {
  local attempts=0
  while [ "${attempts}" -lt 80 ]; do
    if grep -Fq '[fn-knock] service processes started; waiting for application readiness' "${LOG_FILE}" 2>/dev/null; then
      return 0
    fi
    process_is_alive "${SUPERVISOR_PID}" || fail 'entrypoint exited before reporting child startup'
    attempts=$((attempts + 1))
    sleep 0.1
  done
  fail 'entrypoint did not report child startup within 8 seconds'
}

wait_for_exit() {
  local pid="$1"
  local label="$2"
  local attempts=0
  while process_is_alive "${pid}" && [ "${attempts}" -lt 80 ]; do
    attempts=$((attempts + 1))
    sleep 0.1
  done
  ! process_is_alive "${pid}" || fail "${label} process ${pid} did not stop"
}

mkdir -p \
  "${APP_HOME}/bin" \
  "${APP_HOME}/ui/www" \
  "${APP_HOME}/server-auth-view/dist" \
  "${APP_HOME}/server/server-admin/resources" \
  "${DATA_DIR}"
: > "${APP_HOME}/server/server-admin/resources/acmesh.zip"

cat > "${WORK_DIR}/fake-service" <<'SH'
#!/bin/sh
trap 'exit 0' INT TERM
while :; do
  sleep 1
done
SH
chmod 755 "${WORK_DIR}/fake-service"
cp "${WORK_DIR}/fake-service" "${APP_HOME}/bin/go-reauth-proxy"
cp "${WORK_DIR}/fake-service" "${APP_HOME}/bin/server-admin-rs"

FN_KNOCK_APP_HOME="${APP_HOME}" \
FN_KNOCK_DATA_DIR="${DATA_DIR}" \
FN_KNOCK_GATEWAY_CONFIG_DIR="${DATA_DIR}/gateway" \
  bash "${ENTRYPOINT}" > "${LOG_FILE}" 2>&1 &
SUPERVISOR_PID=$!

wait_for_processes_started
if grep -Fq '[fn-knock] services are ready' "${LOG_FILE}"; then
  fail 'entrypoint reported application readiness from process liveness alone'
fi
GATEWAY_PID="$(read_running_pid "${DATA_DIR}/runtime/pids/gateway.pid" 'gateway')"
MANAGEMENT_PID="$(read_running_pid "${DATA_DIR}/runtime/pids/management.pid" 'management')"

kill -TERM "${SUPERVISOR_PID}"
set +e
wait "${SUPERVISOR_PID}"
exit_status=$?
set -e
SUPERVISOR_PID=""

[ "${exit_status}" -eq 0 ] || fail "entrypoint exited with status ${exit_status} after SIGTERM"
wait_for_exit "${GATEWAY_PID}" 'gateway'
wait_for_exit "${MANAGEMENT_PID}" 'management'
[ ! -e "${DATA_DIR}/runtime/pids/gateway.pid" ] || fail 'gateway PID file remained after shutdown'
[ ! -e "${DATA_DIR}/runtime/pids/management.pid" ] || fail 'management PID file remained after shutdown'
grep -Fq '"event":"stop_requested"' "${DATA_DIR}/runtime/logs/supervisor.jsonl" || \
  fail 'supervisor stop event was not recorded'
if grep -Eq 'unbound variable|missing argument to .?-exec|no terminating' "${LOG_FILE}"; then
  fail 'entrypoint log contains a shell or find execution error'
fi

printf '[test-synology-entrypoint] real entrypoint startup, PID tracking, and shutdown passed\n'
