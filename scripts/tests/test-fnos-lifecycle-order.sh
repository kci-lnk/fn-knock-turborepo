#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
MAIN_ENTRYPOINT="${ROOT_DIR}/apps/fn-knock/cmd/main"
LITE_ENTRYPOINT="${ROOT_DIR}/apps/fn-knock-lite/cmd/main"
LINUX_ENTRYPOINT="${ROOT_DIR}/deploy/linux/fn-knock-entrypoint"
MACOS_ENTRYPOINT="${ROOT_DIR}/deploy/macos/fn-knock-entrypoint"
DOCKER_ENTRYPOINT="${ROOT_DIR}/deploy/docker/entrypoint.sh"
SYNOLOGY_ENTRYPOINT="${ROOT_DIR}/apps/fn-knock-synology/package/bin/fn-knock-entrypoint"
SYNOLOGY_LIFECYCLE="${ROOT_DIR}/apps/fn-knock-synology/scripts/start-stop-status"
WINDOWS_SERVICE="${ROOT_DIR}/apps/server-admin-rs/src/windows_service.rs"

fail() {
  printf '[test-fnos-lifecycle-order] ERROR: %s\n' "$*" >&2
  exit 1
}

line_for() {
  local source="$1" pattern="$2"
  local line=""
  line="$(printf '%s\n' "${source}" | grep -nF "${pattern}" | head -n 1 | cut -d: -f1)"
  [ -n "${line}" ] || fail "missing lifecycle step: ${pattern}"
  printf '%s\n' "${line}"
}

assert_before() {
  local source="$1" first="$2" second="$3"
  local first_line second_line
  first_line="$(line_for "${source}" "${first}")"
  second_line="$(line_for "${source}" "${second}")"
  [ "${first_line}" -lt "${second_line}" ] || \
    fail "expected '${first}' before '${second}'"
}

extract_stop_body() {
  awk '
    /^stop\(\) \{/ { capture = 1 }
    capture { print }
    capture && /^}/ { exit }
  ' "$1"
}

for entrypoint in \
  "${MAIN_ENTRYPOINT}" \
  "${LITE_ENTRYPOINT}" \
  "${LINUX_ENTRYPOINT}" \
  "${MACOS_ENTRYPOINT}" \
  "${DOCKER_ENTRYPOINT}" \
  "${SYNOLOGY_ENTRYPOINT}"
do
  bash -n "${entrypoint}"
done
sh -n "${SYNOLOGY_LIFECYCLE}"

main_stop_body="$(extract_stop_body "${MAIN_ENTRYPOINT}")"
lite_stop_body="$(extract_stop_body "${LITE_ENTRYPOINT}")"

assert_before "${main_stop_body}" \
  'if ! cleanup_fn_connect_waf_rules; then' \
  'if ! stop_service "${GATEWAY_PID_FILE}" "Gateway"; then'
assert_before "${main_stop_body}" \
  'if ! stop_service "${GATEWAY_PID_FILE}" "Gateway"; then' \
  'if ! stop_matching_processes "${GATEWAY_BIN}" "Gateway"; then'
assert_before "${main_stop_body}" \
  'if ! stop_matching_processes "${GATEWAY_BIN}" "Gateway"; then' \
  'if ! stop_service "${BACKEND_PID_FILE}" "Backend"; then'
assert_before "${main_stop_body}" \
  'if ! stop_service "${BACKEND_PID_FILE}" "Backend"; then' \
  'if ! stop_matching_processes "${BACKEND_ENTRY}" "Backend"; then'
assert_before "${main_stop_body}" \
  'if ! stop_matching_processes "${BACKEND_ENTRY}" "Backend"; then' \
  'rm -f "${READINESS_MARKER}"'

printf '%s\n' "${main_stop_body}" | grep -Fq \
  'backend kept running to preserve the auth upstream' || \
  fail 'gateway failure no longer preserves the auth upstream'

assert_before "${lite_stop_body}" \
  'if ! stop_service "${GATEWAY_PID_FILE}" "Gateway"; then' \
  'if ! stop_matching_processes "${GATEWAY_BIN}" "Gateway"; then'
assert_before "${lite_stop_body}" \
  'if ! stop_matching_processes "${GATEWAY_BIN}" "Gateway"; then' \
  'if ! stop_service "${BACKEND_PID_FILE}" "Backend"; then'
assert_before "${lite_stop_body}" \
  'if ! stop_service "${BACKEND_PID_FILE}" "Backend"; then' \
  'if ! stop_matching_processes "${BACKEND_ENTRY}" "Backend"; then'

for entrypoint in \
  "${LINUX_ENTRYPOINT}" \
  "${MACOS_ENTRYPOINT}" \
  "${DOCKER_ENTRYPOINT}" \
  "${SYNOLOGY_ENTRYPOINT}"
do
  entrypoint_source="$(cat "${entrypoint}")"
  assert_before "${entrypoint_source}" \
    'terminate_child "${GATEWAY_PID:-}"' \
    'terminate_child "${BACKEND_PID:-}"'
done

synology_lifecycle_source="$(cat "${SYNOLOGY_LIFECYCLE}")"
assert_before "${synology_lifecycle_source}" \
  'stop_recorded_child "${signal}" "${GATEWAY_PID_FILE}"' \
  'stop_recorded_child "${signal}" "${MANAGEMENT_PID_FILE}"'

windows_graceful_shutdown="$(sed -n \
  '/^async fn graceful_shutdown/,/^async fn wait_for_gateway_control_plane/p' \
  "${WINDOWS_SERVICE}")"
assert_before "${windows_graceful_shutdown}" \
  'shutdown_gateway_only(go_client, gateway).await;' \
  'app_shutdown.cancel();'
grep -Fq \
  'app::run_with_settings(settings, app_shutdown.clone(), Some(ready_tx))' \
  "${WINDOWS_SERVICE}" || fail 'Windows service shares its supervisor shutdown token with the auth backend'

printf '[test-fnos-lifecycle-order] gateway-first shutdown contract passed on fnOS, Linux, macOS, Docker, Synology, and Windows\n'
