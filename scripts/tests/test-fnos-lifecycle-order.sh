#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/fn-knock-lifecycle-test.XXXXXX")"
MAIN_ENTRYPOINT="${ROOT_DIR}/apps/fn-knock/cmd/main"
LITE_ENTRYPOINT="${ROOT_DIR}/apps/fn-knock-lite/cmd/main"
LINUX_ENTRYPOINT="${ROOT_DIR}/deploy/linux/fn-knock-entrypoint"
MACOS_ENTRYPOINT="${ROOT_DIR}/deploy/macos/fn-knock-entrypoint"
DOCKER_ENTRYPOINT="${ROOT_DIR}/deploy/docker/entrypoint.sh"
SYNOLOGY_ENTRYPOINT="${ROOT_DIR}/apps/fn-knock-synology/package/bin/fn-knock-entrypoint"
SYNOLOGY_LIFECYCLE="${ROOT_DIR}/apps/fn-knock-synology/scripts/start-stop-status"
WINDOWS_SERVICE="${ROOT_DIR}/apps/server-admin-rs/src/windows_service.rs"
DEPLOY_SCRIPT="${ROOT_DIR}/scripts/fn-knock-deploy.sh"

cleanup() {
  rm -rf "${WORK_DIR}"
}
trap cleanup EXIT

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
bash -n "${DEPLOY_SCRIPT}"

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
[ "$(printf '%s\n' "${main_stop_body}" | grep -Fc 'if ! cleanup_fn_connect_waf_rules; then')" -eq 2 ] || \
  fail 'fnOS shutdown must verify WAF cleanup before and after stopping the runtime'
grep -Fq 'fn_connect_waf_rules_absent' "${MAIN_ENTRYPOINT}" || \
  fail 'fnOS WAF cleanup no longer verifies converged firewall state'

deploy_source="$(cat "${DEPLOY_SCRIPT}")"
printf '%s\n' "${deploy_source}" | grep -Fq \
  'REMOTE_LIFECYCLE_MAIN="/var/apps/${APP_NAME}/cmd/main"' || \
  fail 'remote deploy must stage lifecycle scripts in the fnOS App Center metadata directory'
printf '%s\n' "${deploy_source}" | grep -Fq \
  'Stage the packaged lifecycle entrypoint for upgrade compatibility' || \
  fail 'remote deploy no longer stages the fixed lifecycle before stopping an old app'
if printf '%s\n' "${deploy_source}" | grep -Eq \
  "appcenter-cli (stop|uninstall).*\\|\\| true"; then
  fail 'remote deploy must not ignore stop or uninstall failures'
fi

firewall_bin_dir="${WORK_DIR}/bin"
mkdir -p "${firewall_bin_dir}"
cat > "${firewall_bin_dir}/iptables" <<'EOF'
#!/bin/bash
last="${!#}"
case "${MOCK_FIREWALL_MODE:-absent}" in
  error)
    echo 'xtables lock unavailable' >&2
    exit 2
    ;;
  present)
    if [ "${last}" = "-S" ]; then
      echo '-N FNK_FNC_PRE'
      echo '-A PREROUTING -j FNK_FNC_PRE'
      exit 0
    fi
    exit 1
    ;;
  absent)
    if [ "${last}" = "-S" ]; then
      echo '-P INPUT ACCEPT'
      exit 0
    fi
    exit 1
    ;;
esac
EOF
chmod 755 "${firewall_bin_dir}/iptables"
cp "${firewall_bin_dir}/iptables" "${firewall_bin_dir}/ip6tables"

cleanup_functions="$(sed -n \
  '/^cleanup_fn_connect_waf_rules_once() {/,/^generate_random_hex() {/p' \
  "${MAIN_ENTRYPOINT}" | sed '$d')"
(
  PATH="${firewall_bin_dir}:${PATH}"
  log_msg() { :; }
  sleep() { :; }
  eval "${cleanup_functions}"

  export MOCK_FIREWALL_MODE=absent
  fn_connect_waf_rules_absent || fail 'absent firewall state was not accepted'

  export MOCK_FIREWALL_MODE=present
  set +e
  fn_connect_waf_rules_absent
  present_status=$?
  set -e
  [ "${present_status}" -eq 1 ] || fail 'managed firewall rules were not detected'

  export MOCK_FIREWALL_MODE=error
  set +e
  fn_connect_waf_rules_absent
  error_status=$?
  cleanup_fn_connect_waf_rules
  cleanup_status=$?
  set -e
  [ "${error_status}" -eq 2 ] || fail 'firewall inspection errors were treated as absence'
  [ "${cleanup_status}" -ne 0 ] || fail 'unverifiable firewall cleanup was treated as success'
)

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
