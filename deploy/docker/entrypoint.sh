#!/usr/bin/env bash
set -euo pipefail

APP_HOME="/opt/fn-knock"
DATA_DIR="${FN_KNOCK_DATA_DIR:-/var/lib/fn-knock}"
GATEWAY_CONFIG_DIR="${FN_KNOCK_GATEWAY_CONFIG_DIR:-/usr/local/etc/fn-knock}"
BACKEND_PORT="${BACKEND_PORT:-7998}"
AUTH_PORT="${AUTH_PORT:-7997}"
ADMIN_VIEW_PORT="${ADMIN_VIEW_PORT:-}"
GO_BACKEND_PORT="${GO_BACKEND_PORT:-7996}"
GO_REPROXY_PORT="${GO_REPROXY_PORT:-7999}"
BACKEND_HOST="${BACKEND_HOST:-127.0.0.1}"
AUTH_HOST="${AUTH_HOST:-127.0.0.1}"
ADMIN_VIEW_HOST="${ADMIN_VIEW_HOST:-${BACKEND_HOST}}"
GO_BACKEND_GRPC_ADDR="${GO_BACKEND_GRPC_ADDR:-127.0.0.1:${GO_BACKEND_PORT}}"
RUST_BACKEND_BIN="${RUST_BACKEND_BIN:-${APP_HOME}/bin/server-admin-rs}"
GATEWAY_BIN="${APP_HOME}/bin/go-reauth-proxy"
ADMIN_STATIC_PATH="${APP_HOME}/ui/www"
AUTH_STATIC_PATH="${APP_HOME}/server-auth-view/dist"
ACME_BUNDLE_ZIP="${APP_HOME}/server/server-admin/resources/acmesh.zip"
ALTCHA_HMAC_KEY_FILE="${DATA_DIR}/altcha_hmac_key"
HMAC_SECRET_FILE="${DATA_DIR}/hmac_secret"
INTERNAL_RPC_TOKEN_FILE="${DATA_DIR}/internal_rpc_token"
ADMIN_PROXY_SECRET_FILE="${DATA_DIR}/admin_proxy_secret"
NOFILE_LIMIT="${FN_KNOCK_NOFILE_LIMIT:-1048576}"
RUNTIME_LOG_DIR="${DATA_DIR}/runtime/logs"
SUPERVISOR_LOG="${RUNTIME_LOG_DIR}/supervisor.jsonl"
SUPERVISOR_EVENTS_DIR="${DATA_DIR}/runtime/supervisor-events"

supervisor_log() {
  local level="$1" component="$2" event="$3" reason="$4" exit_code="${5:-null}"
  local signal="null"
  if [[ "${exit_code}" =~ ^[0-9]+$ ]] && [ "${exit_code}" -gt 128 ]; then
    signal="$((exit_code - 128))"
  fi
  mkdir -p "${RUNTIME_LOG_DIR}"
  chmod 700 "${DATA_DIR}/runtime" "${RUNTIME_LOG_DIR}" 2>/dev/null || true
  if [ -f "${SUPERVISOR_LOG}" ] && [ "$(wc -c < "${SUPERVISOR_LOG}")" -ge 516096 ]; then
    mv -f "${SUPERVISOR_LOG}" "${SUPERVISOR_LOG}.1"
  fi
  printf '{"time":"%s","level":"%s","component":"%s","event":"%s","reason_code":"%s","fields":{"exit_code":%s,"signal":%s}}\n' \
    "$(date -u '+%Y-%m-%dT%H:%M:%SZ')" "${level}" "${component}" "${event}" "${reason}" "${exit_code}" "${signal}" >> "${SUPERVISOR_LOG}"
  chmod 600 "${SUPERVISOR_LOG}" 2>/dev/null || true
  mkdir -p "${SUPERVISOR_EVENTS_DIR}"
  chmod 700 "${SUPERVISOR_EVENTS_DIR}" 2>/dev/null || true
  local hint_tmp="${SUPERVISOR_EVENTS_DIR}/.hint-$$-${RANDOM:-0}.tmp"
  local hint_path="${SUPERVISOR_EVENTS_DIR}/$(date -u '+%s')-$$-${RANDOM:-0}-${event}.json"
  printf '{"time":"%s","level":"%s","component":"%s","event":"%s","reason_code":"%s","fields":{"exit_code":%s,"signal":%s}}\n' \
    "$(date -u '+%Y-%m-%dT%H:%M:%SZ')" "${level}" "${component}" "${event}" "${reason}" "${exit_code}" "${signal}" > "${hint_tmp}"
  chmod 600 "${hint_tmp}" 2>/dev/null || true
  mv -f "${hint_tmp}" "${hint_path}"
  find "${SUPERVISOR_EVENTS_DIR}" -type f -name '*.json' -mtime +7 -exec rm -f {} \; 2>/dev/null || true
  find "${SUPERVISOR_EVENTS_DIR}" -type f -name '.hint-*.tmp' -mtime +1 -exec rm -f {} \; 2>/dev/null || true
  local hints=("${SUPERVISOR_EVENTS_DIR}"/*.json)
  while [ "${#hints[@]}" -gt 32 ]; do rm -f "${hints[0]}"; hints=("${SUPERVISOR_EVENTS_DIR}"/*.json); done
  local temp_hints=("${SUPERVISOR_EVENTS_DIR}"/.hint-*.tmp)
  while [ "${#temp_hints[@]}" -gt 32 ]; do rm -f "${temp_hints[0]}"; temp_hints=("${SUPERVISOR_EVENTS_DIR}"/.hint-*.tmp); done
}

generate_random_hex() {
  if command -v openssl >/dev/null 2>&1; then
    openssl rand -hex 32
    return 0
  fi
  od -An -N32 -tx1 /dev/urandom | tr -d ' \n'
}

load_or_create_secret() {
  local __var_name="$1"
  local file_path="$2"
  local current_value="${!__var_name:-}"

  if [ -n "${current_value}" ]; then
    export "${__var_name}=${current_value}"
    return 0
  fi

  if [ -f "${file_path}" ]; then
    current_value="$(tr -d '\r\n' < "${file_path}")"
  fi

  if [ -z "${current_value}" ]; then
    current_value="$(generate_random_hex)"
    printf '%s' "${current_value}" > "${file_path}"
    chmod 600 "${file_path}" 2>/dev/null || true
  fi

  export "${__var_name}=${current_value}"
}

ensure_runtime_layout() {
  mkdir -p \
    "${DATA_DIR}" \
    "${DATA_DIR}/frp" \
    "${DATA_DIR}/frp/instances" \
    "${DATA_DIR}/cloudflared" \
    "${DATA_DIR}/updates" \
    "${GATEWAY_CONFIG_DIR}"
}

raise_nofile_limit() {
  local limit="${1:-${NOFILE_LIMIT}}"
  local inherited_hard=""

  if ulimit -Hn "${limit}" >/dev/null 2>&1 && ulimit -Sn "${limit}" >/dev/null 2>&1; then
    echo "[fn-knock] Raised nofile limit to soft=${limit}, hard=${limit}"
    return 0
  fi

  inherited_hard="$(ulimit -Hn 2>/dev/null || true)"
  if [ -n "${inherited_hard}" ] && [ "${inherited_hard}" != "unlimited" ]; then
    if ulimit -Sn "${inherited_hard}" >/dev/null 2>&1; then
      echo "[fn-knock] Unable to raise nofile hard limit to ${limit}; raised soft limit to inherited hard=${inherited_hard}" >&2
      return 0
    fi
  fi

  echo "[fn-knock] Unable to raise nofile limit to soft=${limit}, hard=${limit}" >&2
  return 1
}

wait_for_process_or_fail() {
  local pid="$1"
  local name="$2"

  sleep 1
  if ! kill -0 "${pid}" 2>/dev/null; then
    echo "[fn-knock] ${name} exited early" >&2
    set +e
    wait "${pid}"
    local status=$?
    set -e
    if [ "${name}" = "gateway" ]; then
      supervisor_log ERROR gateway_process exited startup_exit "${status}" || true
    else
      supervisor_log ERROR management exited startup_exit "${status}" || true
    fi
    exit 1
  fi
}

cleanup() {
  local exit_code=$?
  trap - EXIT INT TERM

  if [ -n "${BACKEND_PID:-}" ] && kill -0 "${BACKEND_PID}" 2>/dev/null; then
    kill "${BACKEND_PID}" 2>/dev/null || true
  fi
  if [ -n "${GATEWAY_PID:-}" ] && kill -0 "${GATEWAY_PID}" 2>/dev/null; then
    kill "${GATEWAY_PID}" 2>/dev/null || true
  fi

  wait "${BACKEND_PID:-}" 2>/dev/null || true
  wait "${GATEWAY_PID:-}" 2>/dev/null || true
  exit "${exit_code}"
}

stop_gracefully() {
  trap '' INT TERM
  supervisor_log INFO supervisor stop_requested signal null || true
  exit 0
}

trap cleanup EXIT
trap stop_gracefully INT TERM

ensure_runtime_layout
raise_nofile_limit "${NOFILE_LIMIT}" || true
load_or_create_secret ALTCHA_HMAC_KEY "${ALTCHA_HMAC_KEY_FILE}"
load_or_create_secret HMAC_SECRET "${HMAC_SECRET_FILE}"
load_or_create_secret FN_KNOCK_INTERNAL_RPC_TOKEN "${INTERNAL_RPC_TOKEN_FILE}"
load_or_create_secret ADMIN_PROXY_SECRET "${ADMIN_PROXY_SECRET_FILE}"

[ -x "${GATEWAY_BIN}" ] || {
  echo "[fn-knock] gateway is not executable: ${GATEWAY_BIN}" >&2
  exit 1
}
[ -x "${RUST_BACKEND_BIN}" ] || {
  echo "[fn-knock] Rust backend is not executable: ${RUST_BACKEND_BIN}" >&2
  exit 1
}
[ -d "${ADMIN_STATIC_PATH}" ] || {
  echo "[fn-knock] admin static path is missing: ${ADMIN_STATIC_PATH}" >&2
  exit 1
}
[ -d "${AUTH_STATIC_PATH}" ] || {
  echo "[fn-knock] auth static path is missing: ${AUTH_STATIC_PATH}" >&2
  exit 1
}

echo "[fn-knock] Starting gateway on admin ${GO_BACKEND_PORT}, proxy ${GO_REPROXY_PORT}"
BACKEND_PORT="${BACKEND_PORT}" \
  FN_KNOCK_DATA_DIR="${DATA_DIR}" \
  FN_KNOCK_INTERNAL_RPC_TOKEN="${FN_KNOCK_INTERNAL_RPC_TOKEN}" \
  "${GATEWAY_BIN}" \
    -c "${GATEWAY_CONFIG_DIR}" \
    -admin-port "${GO_BACKEND_PORT}" \
    -proxy-port "${GO_REPROXY_PORT}" &
GATEWAY_PID=$!
wait_for_process_or_fail "${GATEWAY_PID}" "gateway"
supervisor_log INFO gateway_process started supervisor_start null || true

if [ -n "${ADMIN_VIEW_PORT}" ]; then
  echo "[fn-knock] Starting Rust backend on ${BACKEND_HOST}:${BACKEND_PORT} (admin view ${ADMIN_VIEW_HOST}:${ADMIN_VIEW_PORT})"
else
  echo "[fn-knock] Starting Rust backend on ${BACKEND_HOST}:${BACKEND_PORT}"
fi
(
  cd "${APP_HOME}" && \
  ADMIN_STATIC_PATH="${ADMIN_STATIC_PATH}" \
  AUTH_STATIC_PATH="${AUTH_STATIC_PATH}" \
  FN_KNOCK_DATA_DIR="${DATA_DIR}" \
  FN_KNOCK_GATEWAY_CONFIG_DIR="${GATEWAY_CONFIG_DIR}" \
  FN_KNOCK_RUNTIME_TARGET="docker" \
  FN_KNOCK_BACKEND_IMPL="rust" \
  ACME_BUNDLE_ZIP="${ACME_BUNDLE_ZIP}" \
  ADMIN_VIEW_PORT="${ADMIN_VIEW_PORT}" \
  BACKEND_PORT="${BACKEND_PORT}" \
  AUTH_PORT="${AUTH_PORT}" \
  GO_BACKEND_PORT="${GO_BACKEND_PORT}" \
  GO_REPROXY_PORT="${GO_REPROXY_PORT}" \
  GO_BACKEND_GRPC_ADDR="${GO_BACKEND_GRPC_ADDR}" \
  FN_KNOCK_INTERNAL_RPC_TOKEN="${FN_KNOCK_INTERNAL_RPC_TOKEN}" \
  ADMIN_VIEW_HOST="${ADMIN_VIEW_HOST}" \
  BACKEND_HOST="${BACKEND_HOST}" \
  AUTH_HOST="${AUTH_HOST}" \
  ALTCHA_HMAC_KEY="${ALTCHA_HMAC_KEY}" \
  HMAC_SECRET="${HMAC_SECRET}" \
  ADMIN_PROXY_SECRET="${ADMIN_PROXY_SECRET}" \
  "${RUST_BACKEND_BIN}"
) &
BACKEND_PID=$!
wait_for_process_or_fail "${BACKEND_PID}" "Rust backend"
supervisor_log INFO management started supervisor_start null || true

echo "[fn-knock] Services are up"
set +e
wait -n "${GATEWAY_PID}" "${BACKEND_PID}"
exited_status=$?
set -e
if kill -0 "${GATEWAY_PID}" 2>/dev/null; then
  supervisor_log ERROR management exited unexpected_exit "${exited_status}" || true
else
  supervisor_log ERROR gateway_process exited unexpected_exit "${exited_status}" || true
fi
exit 1
