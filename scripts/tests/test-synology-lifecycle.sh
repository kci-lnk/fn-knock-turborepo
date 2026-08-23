#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
LIFECYCLE="${ROOT_DIR}/apps/fn-knock-synology/scripts/start-stop-status"
WORK_DIR="$(mktemp -d "${ROOT_DIR}/dist/synology-lifecycle-test.XXXXXX")"
FAKE_BIN="${WORK_DIR}/bin"
PKGDEST="${WORK_DIR}/target"
PKGVAR="${WORK_DIR}/var"
PKGTMP="${WORK_DIR}/tmp"
PKGHOME="${WORK_DIR}/home"
READY_FILE="${PKGVAR}/runtime.ready"
ENTRYPOINT_PID_FILE="${WORK_DIR}/entrypoint.pid"
WORKER_PID_FILE="${WORK_DIR}/worker.pid"
START_TIMEOUT_FILE="${WORK_DIR}/start-timeout"
SETSID_PID_FILE="${WORK_DIR}/setsid.pid"

cleanup() {
  if [ -r "${PKGVAR}/fn-knock.pid" ]; then
    FN_KNOCK_SYNOLOGY_STOP_TIMEOUT_SECONDS=1 \
    FN_KNOCK_SYNOLOGY_FORCE_KILL_TIMEOUT_SECONDS=2 \
      run_lifecycle stop >/dev/null 2>&1 || true
  fi
  rm -rf "${WORK_DIR}"
}
trap cleanup EXIT

fail() {
  printf '[test-synology-lifecycle] ERROR: %s\n' "$*" >&2
  [ ! -f "${WORK_DIR}/synopkg-message.log" ] || sed -n '1,80p' "${WORK_DIR}/synopkg-message.log" >&2
  [ ! -f "${PKGVAR}/fn-knock.log" ] || sed -n '1,120p' "${PKGVAR}/fn-knock.log" >&2
  exit 1
}

run_lifecycle() {
  PATH="${FAKE_BIN}:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin" \
  SYNOPKG_PKGNAME="fn-knock-synology" \
  SYNOPKG_PKGDEST="${PKGDEST}" \
  SYNOPKG_PKGVAR="${PKGVAR}" \
  SYNOPKG_PKGTMP="${PKGTMP}" \
  SYNOPKG_PKGHOME="${PKGHOME}" \
  SYNOPKG_TEMP_LOGFILE="${WORK_DIR}/synopkg-message.log" \
  TEST_READY_FILE="${READY_FILE}" \
  TEST_ENTRYPOINT_PID_FILE="${ENTRYPOINT_PID_FILE}" \
  TEST_WORKER_PID_FILE="${WORKER_PID_FILE}" \
  TEST_START_TIMEOUT_FILE="${START_TIMEOUT_FILE}" \
  TEST_SETSID_PID_FILE="${SETSID_PID_FILE}" \
  TEST_FAKE_BIN="${FAKE_BIN}" \
    sh "${LIFECYCLE}" "$@"
}

process_is_alive() {
  local pid="$1" proc_stat=""

  if [ -r "/proc/${pid}/stat" ]; then
    proc_stat="$(cat "/proc/${pid}/stat" 2>/dev/null || true)"
    case "${proc_stat}" in
      *") Z "*) return 1 ;;
    esac
  fi

  kill -0 "${pid}" 2>/dev/null
}

wait_until_dead() {
  local pid="$1" attempts=0
  while process_is_alive "${pid}" && [ "${attempts}" -lt 30 ]; do
    attempts=$((attempts + 1))
    sleep 0.1
  done
  ! process_is_alive "${pid}"
}

mkdir -p "${FAKE_BIN}" "${PKGDEST}/bin" "${PKGVAR}" "${PKGTMP}" "${PKGHOME}"

cat > "${FAKE_BIN}/setsid" <<'PY'
#!/usr/bin/env python3
import os
import sys
import time

with open(os.environ["TEST_SETSID_PID_FILE"], "w", encoding="utf-8") as pid_file:
    pid_file.write(f"{os.getpid()}\n")
if os.environ.get("TEST_SETSID_FORK") == "1":
    if os.fork() != 0:
        raise SystemExit(0)
    time.sleep(0.25)
os.setsid()
os.execvp(sys.argv[1], sys.argv[1:])
PY
chmod 755 "${FAKE_BIN}/setsid"

cat > "${PKGDEST}/bin/fn-knock-entrypoint" <<'SH'
#!/bin/sh
printf '%s\n' "$$" > "${TEST_ENTRYPOINT_PID_FILE:?}"
supervisor_pid_tmp="${FN_KNOCK_SUPERVISOR_PID_FILE:?}.tmp-$$"
printf '%s\n' "$$" > "${supervisor_pid_tmp}"
mv -f "${supervisor_pid_tmp}" "${FN_KNOCK_SUPERVISOR_PID_FILE}"
printf '%s\n' "${FN_KNOCK_START_TIMEOUT_SECONDS:-missing}" \
  > "${TEST_START_TIMEOUT_FILE:?}"
mkdir -p "${SYNOPKG_PKGVAR:?}/runtime/pids"
cleanup() {
  trap - TERM INT EXIT
  if [ -n "${ready_pid:-}" ]; then
    kill "${ready_pid}" 2>/dev/null || true
    wait "${ready_pid}" 2>/dev/null || true
  fi
  for child_pid in "${management_pid:-}" "${gateway_pid:-}"; do
    [ -n "${child_pid}" ] || continue
    kill "${child_pid}" 2>/dev/null || true
    wait "${child_pid}" 2>/dev/null || true
  done
  exit 0
}
if [ "${TEST_IGNORE_TERM:-0}" = "1" ]; then
  trap '' TERM INT
else
  trap cleanup TERM INT EXIT
fi
(
  sleep "${TEST_READY_DELAY_SECONDS:-0}"
  : > "${TEST_READY_FILE:?}"
) &
ready_pid=$!
"${TEST_FAKE_BIN:?}/server-admin-rs" 1000 &
management_pid=$!
"${TEST_FAKE_BIN:?}/go-reauth-proxy" 1000 &
gateway_pid=$!
printf '%s\n' "${management_pid}" > "${SYNOPKG_PKGVAR}/runtime/pids/management.pid"
printf '%s\n' "${gateway_pid}" > "${SYNOPKG_PKGVAR}/runtime/pids/gateway.pid"
printf '%s\n' "${gateway_pid}" > "${TEST_WORKER_PID_FILE:?}"
while :; do sleep 1; done
SH

ln -s /bin/sleep "${FAKE_BIN}/server-admin-rs"
ln -s /bin/sleep "${FAKE_BIN}/go-reauth-proxy"
chmod 755 "${PKGDEST}/bin/fn-knock-entrypoint"

rm -f "${READY_FILE}" "${ENTRYPOINT_PID_FILE}"
TEST_READY_DELAY_SECONDS=2 \
FN_KNOCK_SYNOLOGY_STOP_TIMEOUT_SECONDS=3 \
FN_KNOCK_SYNOLOGY_FORCE_KILL_TIMEOUT_SECONDS=2 \
  run_lifecycle start || fail 'ready service failed to start'

[ -s "${PKGVAR}/fn-knock.pid" ] || fail 'successful start did not persist the supervisor PID'
[ "$(cat "${START_TIMEOUT_FILE}")" = "300" ] || \
  fail 'default DSM start timeout was not propagated to the application'
supervisor_pid="$(cat "${PKGVAR}/fn-knock.pid")"
kill -0 "${supervisor_pid}" 2>/dev/null || fail 'supervisor is not running after readiness'
run_lifecycle status || fail 'status did not report the ready service as running'

FN_KNOCK_SYNOLOGY_STOP_TIMEOUT_SECONDS=3 \
FN_KNOCK_SYNOLOGY_FORCE_KILL_TIMEOUT_SECONDS=2 \
  run_lifecycle stop || fail 'normal stop failed'
[ ! -e "${PKGVAR}/fn-knock.pid" ] || fail 'normal stop retained the supervisor PID file'
wait_until_dead "${supervisor_pid}" || fail 'normal stop left the supervisor running'

rm -f "${READY_FILE}" "${ENTRYPOINT_PID_FILE}" "${SETSID_PID_FILE}"
TEST_SETSID_FORK=1 \
TEST_READY_DELAY_SECONDS=1 \
FN_KNOCK_SYNOLOGY_STOP_TIMEOUT_SECONDS=3 \
FN_KNOCK_SYNOLOGY_FORCE_KILL_TIMEOUT_SECONDS=2 \
  run_lifecycle start || fail 'forking setsid launcher failed to start'
forked_supervisor_pid="$(cat "${ENTRYPOINT_PID_FILE}")"
[ "$(cat "${SETSID_PID_FILE}")" != "${forked_supervisor_pid}" ] || \
  fail 'forking setsid fixture did not exercise a launcher/supervisor PID transition'
[ "$(cat "${PKGVAR}/fn-knock.pid")" = "${forked_supervisor_pid}" ] || \
  fail 'forking launcher PID was not replaced by the actual supervisor PID'
run_lifecycle status || fail 'forking launcher service did not report running status'
TEST_SETSID_FORK=1 \
FN_KNOCK_SYNOLOGY_STOP_TIMEOUT_SECONDS=3 \
FN_KNOCK_SYNOLOGY_FORCE_KILL_TIMEOUT_SECONDS=2 \
  run_lifecycle stop || fail 'forking launcher service failed to stop'
wait_until_dead "${forked_supervisor_pid}" || fail 'forking launcher left the supervisor running'

rm -f "${READY_FILE}" "${ENTRYPOINT_PID_FILE}"
if TEST_READY_DELAY_SECONDS=30 \
  FN_KNOCK_SYNOLOGY_START_TIMEOUT_SECONDS=2 \
  FN_KNOCK_SYNOLOGY_STOP_TIMEOUT_SECONDS=2 \
  FN_KNOCK_SYNOLOGY_FORCE_KILL_TIMEOUT_SECONDS=2 \
    run_lifecycle start
then
  fail 'startup succeeded before the complete readiness endpoint became ready'
fi

[ ! -e "${PKGVAR}/fn-knock.pid" ] || fail 'timed-out start retained the supervisor PID file'
timed_out_pid="$(cat "${ENTRYPOINT_PID_FILE}")"
wait_until_dead "${timed_out_pid}" || fail 'timed-out start left the supervisor running'
grep -Fq 'readiness timed out after 2 seconds' "${WORK_DIR}/synopkg-message.log" || \
  fail 'timed-out start did not expose an actionable DSM error'

rm -f "${READY_FILE}" "${ENTRYPOINT_PID_FILE}" "${WORKER_PID_FILE}"
TEST_READY_DELAY_SECONDS=0 \
TEST_IGNORE_TERM=1 \
FN_KNOCK_SYNOLOGY_START_TIMEOUT_SECONDS=4 \
FN_KNOCK_SYNOLOGY_STOP_TIMEOUT_SECONDS=1 \
FN_KNOCK_SYNOLOGY_FORCE_KILL_TIMEOUT_SECONDS=3 \
  run_lifecycle start || fail 'forced-stop fixture failed to start'
forced_supervisor_pid="$(cat "${ENTRYPOINT_PID_FILE}")"
forced_worker_pid="$(cat "${WORKER_PID_FILE}")"

TEST_IGNORE_TERM=1 \
FN_KNOCK_SYNOLOGY_STOP_TIMEOUT_SECONDS=1 \
FN_KNOCK_SYNOLOGY_FORCE_KILL_TIMEOUT_SECONDS=3 \
  run_lifecycle stop || fail 'forced stop failed'
wait_until_dead "${forced_supervisor_pid}" || fail 'forced stop left the supervisor running'
wait_until_dead "${forced_worker_pid}" || fail 'forced stop left a process-group child running'
[ ! -e "${PKGVAR}/fn-knock.pid" ] || fail 'forced stop retained the supervisor PID file'

printf '[test-synology-lifecycle] slow readiness, status, timeout cleanup, and process-group stop passed\n'
