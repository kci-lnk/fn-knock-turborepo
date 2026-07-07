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
GO_BACKEND_BASE_URL="${GO_BACKEND_BASE_URL:-http://127.0.0.1:${GO_BACKEND_PORT}}"
REDIS_HOST="${REDIS_HOST:-redis}"
REDIS_PORT="${REDIS_PORT:-6379}"
REDIS_PASSWORD="${REDIS_PASSWORD:-}"
REDIS_STARTUP_WAIT_SECONDS="${REDIS_STARTUP_WAIT_SECONDS:-120}"
REDIS_STARTUP_RETRY_DELAY_SECONDS="${REDIS_STARTUP_RETRY_DELAY_SECONDS:-1}"
RUST_BACKEND_BIN="${RUST_BACKEND_BIN:-${APP_HOME}/bin/server-admin-rs}"
GATEWAY_BIN="${APP_HOME}/bin/go-reauth-proxy"
ADMIN_STATIC_PATH="${APP_HOME}/ui/www"
AUTH_STATIC_PATH="${APP_HOME}/server-auth-view/dist"
ACME_BUNDLE_ZIP="${APP_HOME}/server/server-admin/resources/acmesh.zip"
ALTCHA_HMAC_KEY_FILE="${DATA_DIR}/altcha_hmac_key"
HMAC_SECRET_FILE="${DATA_DIR}/hmac_secret"
ADMIN_PROXY_SECRET_FILE="${DATA_DIR}/admin_proxy_secret"

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

wait_for_process_or_fail() {
  local pid="$1"
  local name="$2"

  sleep 1
  if ! kill -0 "${pid}" 2>/dev/null; then
    echo "[fn-knock] ${name} exited early" >&2
    wait "${pid}" || true
    exit 1
  fi
}

wait_for_redis() {
  local wait_seconds="${REDIS_STARTUP_WAIT_SECONDS%.*}"
  local retry_seconds="${REDIS_STARTUP_RETRY_DELAY_SECONDS%.*}"
  local started_at
  local now
  local elapsed
  local logged_waiting=0

  case "${wait_seconds}" in
    ''|*[!0-9]*) wait_seconds=120 ;;
  esac
  case "${retry_seconds}" in
    ''|*[!0-9]*) retry_seconds=1 ;;
  esac
  [ "${wait_seconds}" -gt 0 ] || wait_seconds=120
  [ "${retry_seconds}" -gt 0 ] || retry_seconds=1

  echo "[fn-knock] Waiting for Redis at ${REDIS_HOST}:${REDIS_PORT}"
  started_at="$(date +%s)"
  while true; do
    if (exec 3<>"/dev/tcp/${REDIS_HOST}/${REDIS_PORT}") 2>/dev/null; then
      exec 3<&- 3>&- || true
      echo "[fn-knock] Redis is ready at ${REDIS_HOST}:${REDIS_PORT}"
      return 0
    fi

    now="$(date +%s)"
    elapsed=$((now - started_at))
    if [ "${elapsed}" -ge "${wait_seconds}" ]; then
      echo "[fn-knock] Redis at ${REDIS_HOST}:${REDIS_PORT} was not ready after ${wait_seconds}s" >&2
      exit 1
    fi

    if [ "${logged_waiting}" -eq 0 ]; then
      echo "[fn-knock] Redis is not ready yet; waiting up to ${wait_seconds}s"
      logged_waiting=1
    fi
    sleep "${retry_seconds}"
  done
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

trap cleanup EXIT INT TERM

ensure_runtime_layout
load_or_create_secret ALTCHA_HMAC_KEY "${ALTCHA_HMAC_KEY_FILE}"
load_or_create_secret HMAC_SECRET "${HMAC_SECRET_FILE}"
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
  "${GATEWAY_BIN}" \
    -c "${GATEWAY_CONFIG_DIR}" \
    -admin-port "${GO_BACKEND_PORT}" \
    -proxy-port "${GO_REPROXY_PORT}" &
GATEWAY_PID=$!
wait_for_process_or_fail "${GATEWAY_PID}" "gateway"

wait_for_redis

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
  REDIS_HOST="${REDIS_HOST}" \
  REDIS_PORT="${REDIS_PORT}" \
  REDIS_PASSWORD="${REDIS_PASSWORD}" \
  REDIS_STARTUP_WAIT_SECONDS="${REDIS_STARTUP_WAIT_SECONDS}" \
  ACME_BUNDLE_ZIP="${ACME_BUNDLE_ZIP}" \
  ADMIN_VIEW_PORT="${ADMIN_VIEW_PORT}" \
  BACKEND_PORT="${BACKEND_PORT}" \
  AUTH_PORT="${AUTH_PORT}" \
  GO_BACKEND_PORT="${GO_BACKEND_PORT}" \
  GO_REPROXY_PORT="${GO_REPROXY_PORT}" \
  GO_BACKEND_BASE_URL="${GO_BACKEND_BASE_URL}" \
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

echo "[fn-knock] Services are up"
wait -n "${GATEWAY_PID}" "${BACKEND_PID}"
