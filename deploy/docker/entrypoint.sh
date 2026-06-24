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
DOCKER_ADMIN_TRUSTED_PROXY_CIDRS="${DOCKER_ADMIN_TRUSTED_PROXY_CIDRS:-}"
DOCKER_DISCOVER_LAN_IP="${DOCKER_DISCOVER_LAN_IP:-}"
BACKEND_HOST="${BACKEND_HOST:-127.0.0.1}"
AUTH_HOST="${AUTH_HOST:-127.0.0.1}"
ADMIN_VIEW_HOST="${ADMIN_VIEW_HOST:-${BACKEND_HOST}}"
GO_BACKEND_BASE_URL="${GO_BACKEND_BASE_URL:-http://127.0.0.1:${GO_BACKEND_PORT}}"
REDIS_HOST="${REDIS_HOST:-redis}"
REDIS_PORT="${REDIS_PORT:-6379}"
REDIS_STARTUP_WAIT_SECONDS="${REDIS_STARTUP_WAIT_SECONDS:-120}"
REDIS_STARTUP_RETRY_DELAY_SECONDS="${REDIS_STARTUP_RETRY_DELAY_SECONDS:-1}"
NODE_BIN="${NODE_BIN:-node}"
BACKEND_ENTRY="${APP_HOME}/server/server-admin/index.js"
GATEWAY_BIN="${APP_HOME}/bin/go-reauth-proxy"
ADMIN_STATIC_PATH="${APP_HOME}/ui/www"
AUTH_STATIC_PATH="${APP_HOME}/server-auth-view/dist"
ACME_BUNDLE_ZIP="${APP_HOME}/server/server-admin/resources/acmesh.zip"
ALTCHA_HMAC_KEY_FILE="${DATA_DIR}/altcha_hmac_key"
HMAC_SECRET_FILE="${DATA_DIR}/hmac_secret"
ADMIN_PROXY_SECRET_FILE="${DATA_DIR}/admin_proxy_secret"

generate_random_hex() {
  "${NODE_BIN}" -e "console.log(require('node:crypto').randomBytes(32).toString('hex'))"
}

load_or_create_secret() {
  local __var_name="$1"
  local file_path="$2"
  local current_value="${!__var_name:-}"

  if [ -n "${current_value}" ]; then
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
  echo "[fn-knock] Waiting for Redis at ${REDIS_HOST}:${REDIS_PORT}"
  REDIS_HOST="${REDIS_HOST}" \
    REDIS_PORT="${REDIS_PORT}" \
    REDIS_STARTUP_WAIT_SECONDS="${REDIS_STARTUP_WAIT_SECONDS}" \
    REDIS_STARTUP_RETRY_DELAY_SECONDS="${REDIS_STARTUP_RETRY_DELAY_SECONDS}" \
    "${NODE_BIN}" <<'NODE'
const net = require("node:net");

const host = process.env.REDIS_HOST || "redis";
const port = Number.parseInt(process.env.REDIS_PORT || "6379", 10);
const waitSeconds = Number.parseFloat(
  process.env.REDIS_STARTUP_WAIT_SECONDS || "120",
);
const retryDelaySeconds = Number.parseFloat(
  process.env.REDIS_STARTUP_RETRY_DELAY_SECONDS || "1",
);
const waitMs =
  Number.isFinite(waitSeconds) && waitSeconds > 0
    ? Math.floor(waitSeconds * 1000)
    : 120000;
const retryDelayMs =
  Number.isFinite(retryDelaySeconds) && retryDelaySeconds > 0
    ? Math.floor(retryDelaySeconds * 1000)
    : 1000;

if (!Number.isInteger(port) || port <= 0 || port > 65535) {
  console.error(`[fn-knock] Invalid REDIS_PORT: ${process.env.REDIS_PORT}`);
  process.exit(1);
}

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

const probeRedis = () =>
  new Promise((resolve, reject) => {
    const socket = net.createConnection({ host, port, timeout: 1000 });
    let settled = false;
    const finish = (error) => {
      if (settled) return;
      settled = true;
      socket.destroy();
      if (error) {
        reject(error);
      } else {
        resolve();
      }
    };

    socket.once("connect", () => finish());
    socket.once("timeout", () => finish(new Error("timeout")));
    socket.once("error", finish);
  });

const main = async () => {
  const startedAt = Date.now();
  let attempts = 0;
  let loggedWaiting = false;
  let lastError = null;

  while (true) {
    attempts += 1;
    try {
      await probeRedis();
      if (attempts > 1) {
        console.log(`[fn-knock] Redis is ready at ${host}:${port}`);
      }
      return;
    } catch (error) {
      lastError = error;
      const elapsedMs = Date.now() - startedAt;
      if (elapsedMs >= waitMs) {
        const message =
          lastError instanceof Error ? lastError.message : String(lastError);
        console.error(
          `[fn-knock] Redis at ${host}:${port} was not ready after ${Math.ceil(
            waitMs / 1000,
          )}s: ${message}`,
        );
        process.exit(1);
      }

      if (!loggedWaiting) {
        console.log(
          `[fn-knock] Redis is not ready yet; waiting up to ${Math.ceil(
            waitMs / 1000,
          )}s`,
        );
        loggedWaiting = true;
      }

      await sleep(Math.min(retryDelayMs, waitMs - elapsedMs));
    }
  }
};

main().catch((error) => {
  const message = error instanceof Error ? error.message : String(error);
  console.error(`[fn-knock] Failed while waiting for Redis: ${message}`);
  process.exit(1);
});
NODE
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
  echo "[fn-knock] Starting backend on ${BACKEND_HOST}:${BACKEND_PORT} (admin view ${ADMIN_VIEW_HOST}:${ADMIN_VIEW_PORT})"
else
  echo "[fn-knock] Starting backend on ${BACKEND_HOST}:${BACKEND_PORT}"
fi
(
  cd "${APP_HOME}" && \
  ADMIN_STATIC_PATH="${ADMIN_STATIC_PATH}" \
  AUTH_STATIC_PATH="${AUTH_STATIC_PATH}" \
  FN_KNOCK_DATA_DIR="${DATA_DIR}" \
  FN_KNOCK_GATEWAY_CONFIG_DIR="${GATEWAY_CONFIG_DIR}" \
  FN_KNOCK_RUNTIME_TARGET="${FN_KNOCK_RUNTIME_TARGET:-docker}" \
  REDIS_HOST="${REDIS_HOST}" \
  REDIS_PORT="${REDIS_PORT}" \
  REDIS_STARTUP_WAIT_SECONDS="${REDIS_STARTUP_WAIT_SECONDS}" \
  ACME_BUNDLE_ZIP="${ACME_BUNDLE_ZIP}" \
  ADMIN_VIEW_PORT="${ADMIN_VIEW_PORT}" \
  BACKEND_PORT="${BACKEND_PORT}" \
  AUTH_PORT="${AUTH_PORT}" \
  GO_BACKEND_PORT="${GO_BACKEND_PORT}" \
  GO_REPROXY_PORT="${GO_REPROXY_PORT}" \
  DOCKER_ADMIN_TRUSTED_PROXY_CIDRS="${DOCKER_ADMIN_TRUSTED_PROXY_CIDRS}" \
  DOCKER_DISCOVER_LAN_IP="${DOCKER_DISCOVER_LAN_IP}" \
  GO_BACKEND_BASE_URL="${GO_BACKEND_BASE_URL}" \
  ADMIN_VIEW_HOST="${ADMIN_VIEW_HOST}" \
  BACKEND_HOST="${BACKEND_HOST}" \
  AUTH_HOST="${AUTH_HOST}" \
  ALTCHA_HMAC_KEY="${ALTCHA_HMAC_KEY}" \
  HMAC_SECRET="${HMAC_SECRET}" \
  ADMIN_PROXY_SECRET="${ADMIN_PROXY_SECRET}" \
  "${NODE_BIN}" "${BACKEND_ENTRY}"
) &
BACKEND_PID=$!
wait_for_process_or_fail "${BACKEND_PID}" "backend"

echo "[fn-knock] Services are up"
wait -n "${GATEWAY_PID}" "${BACKEND_PID}"
