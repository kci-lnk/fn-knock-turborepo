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
UNINSTALL_CALLBACK="${ROOT_DIR}/apps/fn-knock/cmd/uninstall_callback"

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

extract_start_body() {
  awk '
    /^start\(\) \{/ { capture = 1 }
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
bash -n "${UNINSTALL_CALLBACK}"

main_stop_body="$(extract_stop_body "${MAIN_ENTRYPOINT}")"
main_start_body="$(extract_start_body "${MAIN_ENTRYPOINT}")"
lite_stop_body="$(extract_stop_body "${LITE_ENTRYPOINT}")"

printf '%s\n' "${main_start_body}" | grep -Fq \
  'if ! prepare_fn_connect_waf_for_start; then' || \
  fail 'fnOS startup no longer applies the WAF cleanup startup policy'

assert_before "${main_stop_body}" \
  'if ! cleanup_fn_connect_waf_for_stop 1; then' \
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
[ "$(printf '%s\n' "${main_stop_body}" | grep -Fc 'if ! cleanup_fn_connect_waf_for_stop')" -eq 2 ] || \
  fail 'fnOS shutdown must verify WAF cleanup before and after stopping the runtime'
printf '%s\n' "${main_stop_body}" | grep -Fq \
  'if ! cleanup_fn_connect_waf_for_stop 3; then' || \
  fail 'fnOS shutdown no longer retries firewall cleanup after processes exit'
grep -Fq 'fn_connect_waf_rules_absent' "${MAIN_ENTRYPOINT}" || \
  fail 'fnOS WAF cleanup no longer verifies converged firewall state'
grep -Fq 'local max_jump_deletions=64' "${MAIN_ENTRYPOINT}" || \
  fail 'fnOS WAF cleanup no longer drains accumulated duplicate jumps'
for runtime_file in backend.pid gateway.pid runtime.ready runtime-ports.env; do
  grep -Fq '"${PKG_VAR_DIR}/'"${runtime_file}"'"' "${UNINSTALL_CALLBACK}" || \
    fail "fnOS uninstall leaves stale runtime identity: ${runtime_file}"
done
uninstall_var_dir="${WORK_DIR}/uninstall-var"
mkdir -p "${uninstall_var_dir}"
touch \
  "${uninstall_var_dir}/backend.pid" \
  "${uninstall_var_dir}/gateway.pid" \
  "${uninstall_var_dir}/runtime.ready" \
  "${uninstall_var_dir}/runtime-ports.env" \
  "${uninstall_var_dir}/settings.sqlite"
TRIM_PKGVAR="${uninstall_var_dir}" bash "${UNINSTALL_CALLBACK}"
for runtime_file in backend.pid gateway.pid runtime.ready runtime-ports.env; do
  [ ! -e "${uninstall_var_dir}/${runtime_file}" ] || \
    fail "fnOS uninstall did not remove runtime identity: ${runtime_file}"
done
[ -e "${uninstall_var_dir}/settings.sqlite" ] || \
  fail 'fnOS uninstall callback removed persistent user data'

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
  mixed)
    if [ "$(basename "$0")" = "ip6tables" ]; then
      echo 'Address family not supported by protocol' >&2
      exit 2
    fi
    if [ "${last}" = "-S" ]; then
      echo '-P INPUT ACCEPT'
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
  supervisor_log() { :; }
  write_temp_log() { START_ERROR="$1"; }
  sleep() { :; }
  FIREWALL_WAIT_SECONDS=1
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
  [ "${cleanup_status}" -eq 2 ] || fail 'unverifiable firewall cleanup did not preserve its distinct status'

  export MOCK_FIREWALL_MODE=mixed
  fn_connect_waf_rules_absent || fail 'unsupported IPv6 firewall family was not skipped'
  cleanup_fn_connect_waf_rules || fail 'unsupported IPv6 firewall family made cleanup fail'

  export MOCK_FIREWALL_MODE=error
  prepare_fn_connect_waf_for_start || fail 'unverifiable firewall state blocked startup'
  cleanup_fn_connect_waf_for_stop || fail 'unverifiable firewall state blocked a bounded stop'

  export MOCK_FIREWALL_MODE=present
  START_ERROR=""
  set +e
  prepare_fn_connect_waf_for_start
  startup_status=$?
  set -e
  [ "${startup_status}" -eq 1 ] || fail 'confirmed residual WAF rules did not block startup'
  [ "${START_ERROR}" = 'fn-knock refused to start because stale FN Connect WAF rules remain and could not be removed' ] || \
    fail 'confirmed residual WAF rules did not provide the expected startup diagnostic'
  START_ERROR=""
  set +e
  cleanup_fn_connect_waf_for_stop
  stop_policy_status=$?
  set -e
  [ "${stop_policy_status}" -eq 1 ] || fail 'confirmed residual WAF rules did not block shutdown'
  [ "${START_ERROR}" = 'fn-knock refused to stop because confirmed FN Connect WAF rules remain and could not be removed' ] || \
    fail 'confirmed residual WAF rules did not provide the expected stop diagnostic'
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
