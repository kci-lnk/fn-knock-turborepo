#!/usr/bin/env bash
set -euo pipefail

ARCHIVE="${1:-}"
[ -f "${ARCHIVE}" ] || { printf '[fn-knock-macos-smoke] archive is missing: %s\n' "${ARCHIVE}" >&2; exit 1; }
[ "$(uname -s)" = Darwin ] || { printf '[fn-knock-macos-smoke] macOS is required\n' >&2; exit 1; }

WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/fn-knock-macos-smoke.XXXXXX")"
ENTRYPOINT_PID=""
RUST_PID=""
GO_PID=""
cleanup() {
  if [ -n "${ENTRYPOINT_PID}" ] && kill -0 "${ENTRYPOINT_PID}" 2>/dev/null; then
    kill "${ENTRYPOINT_PID}" 2>/dev/null || true
    wait "${ENTRYPOINT_PID}" 2>/dev/null || true
  fi
  for runtime_pid in "${RUST_PID}" "${GO_PID}"; do
    if [ -n "${runtime_pid}" ] && kill -0 "${runtime_pid}" 2>/dev/null; then
      kill "${runtime_pid}" 2>/dev/null || true
    fi
  done
  rm -rf "${WORK_DIR}"
}
trap cleanup EXIT INT TERM

tar -xzf "${ARCHIVE}" -C "${WORK_DIR}"
APP_HOME="${WORK_DIR}/fn-knock"
[ -x "${APP_HOME}/bin/fn-knock-entrypoint" ] || { printf '[fn-knock-macos-smoke] entrypoint is missing\n' >&2; exit 1; }

BASE_PORT=$((24000 + ($$ % 10000)))
for offset in 0 1 2 3 4; do
  if /usr/sbin/lsof -nP -iTCP:"$((BASE_PORT + offset))" -sTCP:LISTEN >/dev/null 2>&1; then
    printf '[fn-knock-macos-smoke] selected port is busy: %s\n' "$((BASE_PORT + offset))" >&2
    exit 1
  fi
done

FN_KNOCK_APP_HOME="${APP_HOME}" \
FN_KNOCK_TEST_MODE=1 \
FN_KNOCK_APP_ROOT="${WORK_DIR}/Application Support/FnKnock" \
FN_KNOCK_DATA_DIR="${WORK_DIR}/Application Support/FnKnock/data" \
FN_KNOCK_GATEWAY_CONFIG_DIR="${WORK_DIR}/Application Support/FnKnock/config/gateway" \
BACKEND_PORT="$((BASE_PORT + 1))" \
AUTH_PORT="$((BASE_PORT + 2))" \
ADMIN_VIEW_PORT="${BASE_PORT}" \
GO_BACKEND_PORT="$((BASE_PORT + 3))" \
GO_REPROXY_PORT="$((BASE_PORT + 4))" \
ADMIN_VIEW_HOST=127.0.0.1 \
  "${APP_HOME}/bin/fn-knock-entrypoint" > "${WORK_DIR}/stdout.log" 2> "${WORK_DIR}/stderr.log" &
ENTRYPOINT_PID=$!

HEALTH_URL="http://127.0.0.1:${BASE_PORT}/api/admin/healthz"
READY_URL="http://127.0.0.1:${BASE_PORT}/__fn-knock/readyz"
RELEASE_VERSION="$(sed -nE 's/^[[:space:]]*"version"[[:space:]]*:[[:space:]]*"([^"]+)".*/\1/p' "${APP_HOME}/release.json" | head -n1)"
CONTROL_API_VERSION="$(sed -nE 's/^[[:space:]]*"control_api_version"[[:space:]]*:[[:space:]]*([0-9]+).*/\1/p' "${APP_HOME}/release.json" | head -n1)"
[ -n "${RELEASE_VERSION}" ] && [ -n "${CONTROL_API_VERSION}" ] || {
  printf '[fn-knock-macos-smoke] release metadata is incomplete\n' >&2
  exit 1
}
healthy=0
for ((attempt = 1; attempt <= 30; attempt++)); do
  if curl --silent --max-time 2 "${HEALTH_URL}" > "${WORK_DIR}/health.json" && \
     curl --silent --max-time 2 "${READY_URL}" > "${WORK_DIR}/ready.json" && \
     grep -Fq '"success":true' "${WORK_DIR}/health.json" && \
     grep -Fq '"ready":true' "${WORK_DIR}/ready.json"; then
    healthy=1
    break
  fi
  if ! kill -0 "${ENTRYPOINT_PID}" 2>/dev/null; then
    break
  fi
  sleep 2
done

if [ "${healthy}" != "1" ]; then
  cat "${WORK_DIR}/stdout.log" >&2 || true
  cat "${WORK_DIR}/stderr.log" >&2 || true
  [ ! -f "${WORK_DIR}/health.json" ] || cat "${WORK_DIR}/health.json" >&2
  [ ! -f "${WORK_DIR}/ready.json" ] || cat "${WORK_DIR}/ready.json" >&2
  printf '[fn-knock-macos-smoke] health check failed\n' >&2
  exit 1
fi
grep -Fq '"deployment_target":"macos"' "${WORK_DIR}/health.json"
grep -Fq '"is_macos":true' "${WORK_DIR}/health.json"
grep -Fq '"reachable":true' "${WORK_DIR}/health.json"
grep -Fq '"ready":true' "${WORK_DIR}/ready.json"
grep -Fq "\"version\":\"${RELEASE_VERSION}\"" "${WORK_DIR}/ready.json"
grep -Fq "\"control_api_version\":${CONTROL_API_VERSION}" "${WORK_DIR}/ready.json"

RUST_PID="$(/usr/bin/pgrep -P "${ENTRYPOINT_PID}" -x server-admin-rs 2>/dev/null || true)"
GO_PID="$(/usr/bin/pgrep -P "${ENTRYPOINT_PID}" -x go-reauth-proxy 2>/dev/null || true)"
[ -n "${RUST_PID}" ] || { printf '[fn-knock-macos-smoke] Rust backend is not a direct supervisor child\n' >&2; exit 1; }
[ -n "${GO_PID}" ] || { printf '[fn-knock-macos-smoke] Go gateway is not a direct supervisor child\n' >&2; exit 1; }

kill "${ENTRYPOINT_PID}"
wait "${ENTRYPOINT_PID}"
ENTRYPOINT_PID=""
for runtime_pid in "${RUST_PID}" "${GO_PID}"; do
  if kill -0 "${runtime_pid}" 2>/dev/null; then
    printf '[fn-knock-macos-smoke] child process leaked after supervisor exit: %s\n' "${runtime_pid}" >&2
    exit 1
  fi
done
RUST_PID=""
GO_PID=""
printf '[fn-knock-macos-smoke] native runtime health check passed\n'
