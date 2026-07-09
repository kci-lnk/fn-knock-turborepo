#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LINUX_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
KNOCK="${LINUX_DIR}/knock"
ENTRYPOINT="${LINUX_DIR}/fn-knock-entrypoint"
TEST_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/fn-knock-linux-tests.XXXXXX")"

cleanup() {
  rm -rf "${TEST_ROOT}"
}
trap cleanup EXIT

fail_test() {
  printf 'not ok - %s\n' "$*" >&2
  exit 1
}

assert_equal() {
  local expected="$1" actual="$2" message="$3"
  [ "${expected}" = "${actual}" ] || fail_test "${message}: expected '${expected}', got '${actual}'"
}

make_fake_commands() {
  local root="$1"
  mkdir -p "${root}/fake-bin" "${root}/state"
  printf '%s\n' 1 > "${root}/state/enabled"
  printf '%s\n' 1 > "${root}/state/active"

  cat > "${root}/fake-bin/id" <<'EOF'
#!/usr/bin/env bash
if [ "${1:-}" = "-u" ]; then
  printf '%s\n' 0
else
  exec /usr/bin/id "$@"
fi
EOF

  cat > "${root}/fake-bin/systemctl" <<'EOF'
#!/usr/bin/env bash
set -u
command_name="${1:-}"
shift || true
fail_once() {
  [ "${FN_MOCK_FAIL:-}" = "$1" ] || return 1
  [ ! -e "${FN_MOCK_STATE}/failed-${1}" ] || return 1
  : > "${FN_MOCK_STATE}/failed-${1}"
  return 0
}
case "${command_name}" in
  is-enabled) [ "$(< "${FN_MOCK_STATE}/enabled")" = "1" ] ;;
  is-active) [ "$(< "${FN_MOCK_STATE}/active")" = "1" ] ;;
  daemon-reload)
    if fail_once daemon-reload; then exit 1; fi
    ;;
  enable)
    printf '%s\n' 1 > "${FN_MOCK_STATE}/enabled"
    if fail_once enable; then exit 1; fi
    ;;
  disable) printf '%s\n' 0 > "${FN_MOCK_STATE}/enabled" ;;
  restart)
    printf '%s\n' 1 > "${FN_MOCK_STATE}/active"
    if fail_once restart; then exit 1; fi
    ;;
  start) printf '%s\n' 1 > "${FN_MOCK_STATE}/active" ;;
  stop) printf '%s\n' 0 > "${FN_MOCK_STATE}/active" ;;
  reset-failed|status) ;;
  *) printf 'unexpected systemctl command: %s\n' "${command_name}" >&2; exit 2 ;;
esac
EOF

  cat > "${root}/fake-bin/curl" <<'EOF'
#!/usr/bin/env bash
if [ "${FN_MOCK_FAIL:-}" = "health" ] && [ ! -e "${FN_MOCK_STATE}/failed-health" ]; then
  : > "${FN_MOCK_STATE}/failed-health"
  exit 1
fi
exit 0
EOF

  cat > "${root}/fake-bin/journalctl" <<'EOF'
#!/usr/bin/env bash
exit 0
EOF

  cat > "${root}/fake-bin/sleep" <<'EOF'
#!/usr/bin/env bash
exit 0
EOF

  # BSD mv lacks GNU's -T option; this wrapper keeps the test runnable on macOS.
  cat > "${root}/fake-bin/mv" <<'EOF'
#!/usr/bin/env bash
if [ "${1:-}" = "-Tf" ]; then
  /bin/rm -f "$3"
  exec /bin/mv "$2" "$3"
fi
exec /bin/mv "$@"
EOF

  chmod 0755 "${root}/fake-bin/"*
}

make_release() {
  local destination="$1" version="$2" marker="$3"
  mkdir -p "${destination}/bin" "${destination}/systemd" "${destination}/config"
  printf '{\n  "version": "%s"\n}\n' "${version}" > "${destination}/release.json"
  printf '#!/usr/bin/env bash\n# %s\n' "${marker}" > "${destination}/bin/knock"
  printf '#!/usr/bin/env bash\nexit 0\n' > "${destination}/bin/fn-knock-entrypoint"
  printf '#!/usr/bin/env bash\nexit 0\n' > "${destination}/bin/go-reauth-proxy"
  printf '#!/usr/bin/env bash\nexit 0\n' > "${destination}/bin/server-admin-rs"
  printf '%s\n' "unit-${marker}" > "${destination}/systemd/fn-knock.service"
  printf '%s\n' 'ADMIN_VIEW_PORT=7991' > "${destination}/config/fn-knock.env"
  chmod 0755 "${destination}/bin/"*
}

prepare_install_fixture() {
  local root="$1"
  make_fake_commands "${root}"
  mkdir -p "${root}/app/releases" "${root}/config" "${root}/data" "${root}/systemd" "${root}/commands" "${root}/tmp"
  make_release "${root}/app/releases/0.9.0" 0.9.0 old-previous
  make_release "${root}/app/releases/1.0.0" 1.0.0 old-current
  make_release "${root}/source-2.0.0" 2.0.0 candidate
  ln -s "${root}/app/releases/1.0.0" "${root}/app/current"
  ln -s "${root}/app/releases/0.9.0" "${root}/app/previous"
  cp -p "${root}/app/releases/1.0.0/bin/knock" "${root}/commands/knock"
  cp -p "${root}/app/releases/1.0.0/systemd/fn-knock.service" "${root}/systemd/fn-knock.service"
  printf '%s\n' 'ADMIN_VIEW_PORT=7991' > "${root}/config/fn-knock.env"
  cp -p "${root}/commands/knock" "${root}/expected-command"
  cp -p "${root}/systemd/fn-knock.service" "${root}/expected-unit"
}

run_knock() {
  local root="$1" failure="$2"
  shift 2
  env \
    PATH="${root}/fake-bin:${PATH}" \
    TMPDIR="${root}/tmp" \
    FN_MOCK_FAIL="${failure}" \
    FN_MOCK_STATE="${root}/state" \
    FN_KNOCK_APP_ROOT="${root}/app" \
    FN_KNOCK_CONFIG_DIR="${root}/config" \
    FN_KNOCK_DATA_DIR="${root}/data" \
    FN_KNOCK_UNIT_FILE="${root}/systemd/fn-knock.service" \
    FN_KNOCK_COMMAND_FILE="${root}/commands/knock" \
    FN_KNOCK_HEALTH_ATTEMPTS=1 \
    bash "${KNOCK}" "$@"
}

assert_install_restored() {
  local root="$1"
  assert_equal "${root}/app/releases/1.0.0" "$(readlink "${root}/app/current")" "current link was not restored"
  assert_equal "${root}/app/releases/0.9.0" "$(readlink "${root}/app/previous")" "previous link was not restored"
  cmp -s "${root}/expected-command" "${root}/commands/knock" || fail_test "management command was not restored"
  cmp -s "${root}/expected-unit" "${root}/systemd/fn-knock.service" || fail_test "systemd unit was not restored"
  assert_equal 1 "$(< "${root}/state/enabled")" "enabled state was not restored"
  assert_equal 1 "$(< "${root}/state/active")" "active state was not restored"
}

test_install_failures_restore_everything() {
  local failure root
  for failure in daemon-reload enable restart health; do
    root="${TEST_ROOT}/install-${failure}"
    prepare_install_fixture "${root}"
    if run_knock "${root}" "${failure}" _install-extracted "${root}/source-2.0.0" 2.0.0; then
      fail_test "installation unexpectedly succeeded when ${failure} failed"
    fi
    assert_install_restored "${root}"
  done
  printf '%s\n' 'ok - failed installation restores links, files, unit, enabled state, and active state'
}

test_failed_first_install_restores_absent_state() {
  local root="${TEST_ROOT}/first-install-restart"
  make_fake_commands "${root}"
  printf '%s\n' 0 > "${root}/state/enabled"
  printf '%s\n' 0 > "${root}/state/active"
  mkdir -p "${root}/app/releases" "${root}/config" "${root}/data" "${root}/systemd" "${root}/commands" "${root}/tmp"
  make_release "${root}/source-2.0.0" 2.0.0 candidate

  if run_knock "${root}" restart _install-extracted "${root}/source-2.0.0" 2.0.0; then
    fail_test "first installation unexpectedly succeeded after restart failed"
  fi
  [ ! -e "${root}/app/current" ] && [ ! -L "${root}/app/current" ] || fail_test "failed first install left a current link"
  [ ! -e "${root}/app/previous" ] && [ ! -L "${root}/app/previous" ] || fail_test "failed first install left a previous link"
  [ ! -e "${root}/commands/knock" ] || fail_test "failed first install left a global management command"
  [ ! -e "${root}/systemd/fn-knock.service" ] || fail_test "failed first install left a systemd unit"
  assert_equal 0 "$(< "${root}/state/enabled")" "failed first install left the service enabled"
  assert_equal 0 "$(< "${root}/state/active")" "failed first install left the service active"
  printf '%s\n' 'ok - failed first install restores the original absent/disabled/inactive state'
}

test_failed_manual_rollback_restores_everything() {
  local failure root
  for failure in daemon-reload restart health; do
    root="${TEST_ROOT}/rollback-${failure}"
    prepare_install_fixture "${root}"
    rm -f "${root}/app/current" "${root}/app/previous"
    ln -s "${root}/source-2.0.0" "${root}/app/current"
    ln -s "${root}/app/releases/1.0.0" "${root}/app/previous"
    cp -p "${root}/source-2.0.0/bin/knock" "${root}/commands/knock"
    cp -p "${root}/source-2.0.0/systemd/fn-knock.service" "${root}/systemd/fn-knock.service"
    cp -p "${root}/commands/knock" "${root}/expected-command"
    cp -p "${root}/systemd/fn-knock.service" "${root}/expected-unit"

    if run_knock "${root}" "${failure}" rollback; then
      fail_test "manual rollback unexpectedly succeeded when ${failure} failed"
    fi
    assert_equal "${root}/source-2.0.0" "$(readlink "${root}/app/current")" "rollback current link was not restored"
    assert_equal "${root}/app/releases/1.0.0" "$(readlink "${root}/app/previous")" "rollback previous link was not restored"
    cmp -s "${root}/expected-command" "${root}/commands/knock" || fail_test "rollback command was not restored"
    cmp -s "${root}/expected-unit" "${root}/systemd/fn-knock.service" || fail_test "rollback unit was not restored"
    assert_equal 1 "$(< "${root}/state/enabled")" "rollback enabled state was not restored"
    assert_equal 1 "$(< "${root}/state/active")" "rollback active state was not restored"
  done
  printf '%s\n' 'ok - failed manual rollback restores the pre-rollback installation'
}

test_noninteractive_purge_is_rejected() {
  local root="${TEST_ROOT}/purge"
  prepare_install_fixture "${root}"
  if run_knock "${root}" '' uninstall --purge --yes </dev/null; then
    fail_test "non-interactive --purge --yes unexpectedly succeeded"
  fi
  [ -e "${root}/config/fn-knock.env" ] || fail_test "purge removed configuration without DELETE confirmation"
  [ -d "${root}/data" ] || fail_test "purge removed data without DELETE confirmation"
  printf '%s\n' 'ok - --purge --yes still requires an interactive DELETE confirmation'
}

make_entrypoint_fixture() {
  local root="$1" backend_mode="$2"
  mkdir -p \
    "${root}/app/bin" \
    "${root}/app/ui/www" \
    "${root}/app/server-auth-view/dist" \
    "${root}/app/server/server-admin/resources" \
    "${root}/data" \
    "${root}/gateway"
  : > "${root}/app/server/server-admin/resources/acmesh.zip"

  cat > "${root}/app/bin/go-reauth-proxy" <<EOF
#!/usr/bin/env bash
printf '%s\n' "\$\$" > "${root}/gateway.pid"
/bin/sleep 30 &
sleeper=\$!
trap 'kill "\${sleeper}" 2>/dev/null || true; wait "\${sleeper}" 2>/dev/null || true; exit 0' INT TERM
wait "\${sleeper}"
EOF
  if [ "${backend_mode}" = "exit" ]; then
    cat > "${root}/app/bin/server-admin-rs" <<'EOF'
#!/usr/bin/env bash
/bin/sleep 2
exit 0
EOF
  elif [ "${backend_mode}" = "fail" ]; then
    cat > "${root}/app/bin/server-admin-rs" <<'EOF'
#!/usr/bin/env bash
/bin/sleep 2
exit 7
EOF
  else
    cat > "${root}/app/bin/server-admin-rs" <<EOF
#!/usr/bin/env bash
printf '%s\n' "\$\$" > "${root}/backend.pid"
/bin/sleep 30 &
sleeper=\$!
trap 'kill "\${sleeper}" 2>/dev/null || true; wait "\${sleeper}" 2>/dev/null || true; exit 0' INT TERM
wait "\${sleeper}"
EOF
  fi
  chmod 0755 "${root}/app/bin/"*
}

run_entrypoint() {
  local root="$1"
  env \
    FN_KNOCK_APP_HOME="${root}/app" \
    FN_KNOCK_DATA_DIR="${root}/data" \
    FN_KNOCK_GATEWAY_CONFIG_DIR="${root}/gateway" \
    bash "${ENTRYPOINT}"
}

test_child_exit_forces_failure() {
  local mode root status
  for mode in exit fail; do
    root="${TEST_ROOT}/entrypoint-child-${mode}"
    make_entrypoint_fixture "${root}" "${mode}"
    set +e
    run_entrypoint "${root}" >"${root}/output.log" 2>&1
    status=$?
    set -e
    [ "${status}" -ne 0 ] || fail_test "${mode} child exit produced a successful supervisor status"
  done
  printf '%s\n' 'ok - clean and failing child exits both produce a failure for Restart=on-failure'
}

test_explicit_stop_is_graceful() {
  local root="${TEST_ROOT}/entrypoint-stop" wrapper_pid status attempts=0
  make_entrypoint_fixture "${root}" loop
  env \
    FN_KNOCK_APP_HOME="${root}/app" \
    FN_KNOCK_DATA_DIR="${root}/data" \
    FN_KNOCK_GATEWAY_CONFIG_DIR="${root}/gateway" \
    bash "${ENTRYPOINT}" >"${root}/output.log" 2>&1 &
  wrapper_pid=$!
  while ! grep -q 'services are ready' "${root}/output.log" 2>/dev/null; do
    attempts=$((attempts + 1))
    if [ "${attempts}" -gt 50 ]; then
      kill -TERM "${wrapper_pid}" 2>/dev/null || true
      fail_test "entrypoint did not become ready"
    fi
    /bin/sleep 0.1
  done
  kill -TERM "${wrapper_pid}"
  set +e
  wait "${wrapper_pid}"
  status=$?
  set -e
  assert_equal 0 "${status}" "explicit supervisor stop was not graceful"
  printf '%s\n' 'ok - an explicit TERM stops both children and exits successfully'
}

test_install_failures_restore_everything
test_failed_first_install_restores_absent_state
test_failed_manual_rollback_restores_everything
test_noninteractive_purge_is_rejected
test_child_exit_forces_failure
test_explicit_stop_is_graceful
