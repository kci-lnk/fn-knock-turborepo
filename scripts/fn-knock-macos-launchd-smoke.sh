#!/usr/bin/env bash
set -euo pipefail

ARCHIVE="${1:-}"
APP_ROOT="/Library/Application Support/FnKnock"
LOG_DIR="/Library/Logs/FnKnock"
UNIT_FILE="/Library/LaunchDaemons/cn.fnknock.service.plist"
COMMAND_FILE="/usr/local/bin/knock"
SERVICE_NAME="cn.fnknock.service"
WORK_DIR=""
OWNER_TOKEN="fn-knock-ci-${GITHUB_RUN_ID:-local}-$$"
MARKER_FILE="${APP_ROOT}/.ci-smoke-owner"

fail() { printf '[fn-knock-macos-launchd-smoke] ERROR: %s\n' "$*" >&2; exit 1; }

owned_installation() {
  [ -f "${MARKER_FILE}" ] && [ "$(cat "${MARKER_FILE}")" = "${OWNER_TOKEN}" ]
}

cleanup() {
  set +e
  if owned_installation; then
    launchctl bootout "system/${SERVICE_NAME}" >/dev/null 2>&1 || true
    rm -f "${UNIT_FILE}" "${COMMAND_FILE}"
    rm -rf "${APP_ROOT}" "${LOG_DIR}"
  fi
  [ -z "${WORK_DIR}" ] || rm -rf "${WORK_DIR}"
}
trap cleanup EXIT INT TERM

wait_ready() {
  local attempt
  for ((attempt = 1; attempt <= 30; attempt++)); do
    if curl --fail --silent --max-time 2 \
      "http://127.0.0.1:7991/__fn-knock/readyz" >/dev/null 2>&1; then
      return 0
    fi
    [ "${attempt}" -lt 30 ] || return 1
    sleep 2
  done
}

[ "${CI:-}" = "true" ] || fail "this destructive launchd smoke test is restricted to disposable CI runners"
[ "$(id -u)" -eq 0 ] || fail "run this test through sudo"
[ "$(uname -s)" = Darwin ] || fail "macOS is required"
[ -f "${ARCHIVE}" ] || fail "archive is missing: ${ARCHIVE}"
for path in "${APP_ROOT}" "${LOG_DIR}" "${UNIT_FILE}" "${COMMAND_FILE}"; do
  [ ! -e "${path}" ] && [ ! -L "${path}" ] || fail "refusing to replace pre-existing path: ${path}"
done

WORK_DIR="$(mktemp -d /private/tmp/fn-knock-launchd-smoke.XXXXXX)"
tar -xzf "${ARCHIVE}" -C "${WORK_DIR}"
RELEASE_DIR="${WORK_DIR}/fn-knock"
[ -x "${RELEASE_DIR}/bin/knock" ] || fail "archive does not contain the management command"
VERSION="$(sed -nE 's/^[[:space:]]*"version"[[:space:]]*:[[:space:]]*"([^"]+)".*/\1/p' "${RELEASE_DIR}/release.json" | head -n1)"
[ -n "${VERSION}" ] || fail "archive version is missing"

mkdir -p "${APP_ROOT}"
printf '%s\n' "${OWNER_TOKEN}" > "${MARKER_FILE}"
FN_KNOCK_ASSUME_YES=1 "${RELEASE_DIR}/bin/knock" _prepare-install
"${RELEASE_DIR}/bin/knock" _install-extracted "${RELEASE_DIR}" "${VERSION}"

for installed_binary in \
  "${APP_ROOT}/current/bin/go-reauth-proxy" \
  "${APP_ROOT}/current/bin/server-admin-rs" \
  "${APP_ROOT}/current/bin/knock" \
  "${APP_ROOT}/current/bin/fn-knock-entrypoint"
do
  [ "$(stat -f '%Su:%Sg' "${installed_binary}")" = "root:wheel" ] || \
    fail "installed binary is not owned by root:wheel: ${installed_binary}"
done

"${COMMAND_FILE}" status >/dev/null
"${COMMAND_FILE}" logs >/dev/null
"${COMMAND_FILE}" stop
if launchctl print "system/${SERVICE_NAME}" >/dev/null 2>&1; then
  fail "stop left the LaunchDaemon loaded"
fi
"${COMMAND_FILE}" start
wait_ready || fail "LaunchDaemon did not become ready after start"
"${COMMAND_FILE}" restart
wait_ready || fail "LaunchDaemon did not become ready after restart"
"${COMMAND_FILE}" uninstall --yes
[ -f "${APP_ROOT}/data/internal_rpc_token" ] || fail "normal uninstall did not preserve application data"
[ ! -e "${UNIT_FILE}" ] || fail "uninstall left the LaunchDaemon plist"
[ ! -e "${COMMAND_FILE}" ] || fail "uninstall left the management command"

printf '[fn-knock-macos-launchd-smoke] real launchd lifecycle passed\n'
