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
READY_FILE="${WORK_DIR}/ready"
ENTRYPOINT_PID_FILE="${WORK_DIR}/entrypoint.pid"
WORKER_PID_FILE="${WORK_DIR}/worker.pid"

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

SYSTEM_SETSID="$(PATH="/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin" command -v setsid 2>/dev/null || true)"
if [ -z "${SYSTEM_SETSID}" ]; then
  cat > "${FAKE_BIN}/setsid" <<'PY'
#!/usr/bin/env python3
import os
import sys

os.setsid()
os.execvp(sys.argv[1], sys.argv[1:])
PY
  chmod 755 "${FAKE_BIN}/setsid"
fi

cat > "${FAKE_BIN}/curl" <<'SH'
#!/bin/sh
last_argument=""
for argument in "$@"; do
  last_argument="${argument}"
done
[ "${last_argument}" = "http://127.0.0.1:7998/__fn-knock/readyz" ] || exit 64
[ -f "${TEST_READY_FILE:?}" ]
SH

cat > "${PKGDEST}/bin/fn-knock-entrypoint" <<'SH'
#!/bin/sh
printf '%s\n' "$$" > "${TEST_ENTRYPOINT_PID_FILE:?}"
cleanup() {
  trap - TERM INT EXIT
  if [ -n "${ready_pid:-}" ]; then
    kill "${ready_pid}" 2>/dev/null || true
    wait "${ready_pid}" 2>/dev/null || true
  fi
  if [ -n "${worker_pid:-}" ]; then
    kill "${worker_pid}" 2>/dev/null || true
    wait "${worker_pid}" 2>/dev/null || true
  fi
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
(
  if [ "${TEST_IGNORE_TERM:-0}" = "1" ]; then
    trap '' TERM INT
  fi
  while :; do sleep 1; done
) &
worker_pid=$!
printf '%s\n' "${worker_pid}" > "${TEST_WORKER_PID_FILE:?}"
while :; do sleep 1; done
SH

chmod 755 "${FAKE_BIN}/curl" "${PKGDEST}/bin/fn-knock-entrypoint"

rm -f "${READY_FILE}" "${ENTRYPOINT_PID_FILE}"
TEST_READY_DELAY_SECONDS=2 \
FN_KNOCK_SYNOLOGY_START_TIMEOUT_SECONDS=6 \
FN_KNOCK_SYNOLOGY_STOP_TIMEOUT_SECONDS=3 \
FN_KNOCK_SYNOLOGY_FORCE_KILL_TIMEOUT_SECONDS=2 \
  run_lifecycle start || fail 'ready service failed to start'

[ -s "${PKGVAR}/fn-knock.pid" ] || fail 'successful start did not persist the supervisor PID'
supervisor_pid="$(cat "${PKGVAR}/fn-knock.pid")"
kill -0 "${supervisor_pid}" 2>/dev/null || fail 'supervisor is not running after readiness'
run_lifecycle status || fail 'status did not report the ready service as running'

FN_KNOCK_SYNOLOGY_STOP_TIMEOUT_SECONDS=3 \
FN_KNOCK_SYNOLOGY_FORCE_KILL_TIMEOUT_SECONDS=2 \
  run_lifecycle stop || fail 'normal stop failed'
[ ! -e "${PKGVAR}/fn-knock.pid" ] || fail 'normal stop retained the supervisor PID file'
wait_until_dead "${supervisor_pid}" || fail 'normal stop left the supervisor running'

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
