#!/bin/bash
set -euo pipefail
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/fn-knock-planned-stop.XXXXXX")"
trap 'rm -rf "${WORK_DIR}"' EXIT
fail() { echo "[planned-stop] $*" >&2; exit 1; }
ENTRY="${ROOT_DIR}/apps/fn-knock/cmd/main"
bash -n "${ENTRY}"
# Exercise the production shell handshake using isolated files and mocked process identities.
sed -n '/^planned_stop_write() {/,/^stop_pid() {/p' "${ENTRY}" | sed '$d' > "${WORK_DIR}/functions.sh"
. "${WORK_DIR}/functions.sh"
PKG_VAR_DIR="${WORK_DIR}/data"
BACKEND_PID_FILE="${PKG_VAR_DIR}/backend.pid"
GATEWAY_PID_FILE="${PKG_VAR_DIR}/gateway.pid"
STOP_TIMEOUT_SECONDS=75
FORCE_KILL_TIMEOUT_SECONDS=10
mkdir -p "${PKG_VAR_DIR}/runtime/planned-stop"
printf '11\n' > "${BACKEND_PID_FILE}"
printf '22\n' > "${GATEWAY_PID_FILE}"
printf '1 11 manager-one 111 22 gateway-one 222 boot-one\n' > "${PKG_VAR_DIR}/runtime/planned-stop/instance"
supervisor_log() { :; }
log_msg() { printf '%s\n' "$*" >> "${WORK_DIR}/trace"; }
planned_stop_process_matches() { [ "$1:$2" = 11:111 ] || [ "$1:$2" = 22:222 ]; }
# Keep the handshake portable to macOS; no live PID is signaled by this test.
cat() {
    if [ "$1" = /proc/sys/kernel/random/uuid ]; then
        printf 'operation-%s\n' "${OP_NUMBER:-1}"
    elif [ "$1" = /proc/sys/kernel/random/boot_id ]; then
        printf "boot-one\n"
    else
        command cat "$@"
    fi
}
start_ack_worker() {
    python3 - "${PKG_VAR_DIR}/runtime/planned-stop" <<'PY' &
import json,sys,pathlib,time
p=pathlib.Path(sys.argv[1]);deadline=time.monotonic()+5
while time.monotonic()<deadline:
    try:
        d=json.loads((p/'request.json').read_text())
        if d['action']=='prepare':
            time.sleep(.15)
            (p/'ack.tmp').write_text(f"{d['operation_id']} {d['management_instance']} {d['gateway_instance']}\n")
            (p/'ack.tmp').replace(p/'ack')
            break
    except (FileNotFoundError,ValueError):pass
    time.sleep(.02)
else:raise SystemExit('no prepare request')
PY
    ACK_PID=$!
}
start_ack_worker
prepare_planned_stop || fail 'matching ACK was rejected'
wait "${ACK_PID}"
[ "${PLANNED_STOP_OPERATION}" = operation-1 ] || fail 'operation identity lost'
planned_stop_write gateway_stopped
python3 - "${PKG_VAR_DIR}/runtime/planned-stop/request.json" <<'PY'
import json,sys
r=json.load(open(sys.argv[1]));assert r['action']=='gateway_stopped';assert r['timeout_seconds']==180
PY
cancel_planned_stop
python3 - "${PKG_VAR_DIR}/runtime/planned-stop/request.json" <<'PY'
import json,sys
assert json.load(open(sys.argv[1]))['action']=='cancel'
PY
# An ACK from the prior operation must not authorize another stop.
OP_NUMBER=2
if prepare_planned_stop; then fail 'stale ACK authorized a new operation'; fi
python3 - "${PKG_VAR_DIR}/runtime/planned-stop/request.json" <<'PY'
import json,sys
r=json.load(open(sys.argv[1]));assert r['action']=='cancel';assert r['operation_id']=='operation-2'
PY
# A capable management process with no gateway descriptor must fail, not silently downgrade.
printf '11 111 manager-one boot-one\n' > "${PKG_VAR_DIR}/runtime/planned-stop/manager"
rm "${PKG_VAR_DIR}/runtime/planned-stop/instance"
if prepare_planned_stop; then fail 'capable but unready process downgraded to legacy'; fi
rm "${PKG_VAR_DIR}/runtime/planned-stop/manager"
prepare_planned_stop || fail 'old-version upgrade compatibility broken'
[ -z "${PLANNED_STOP_OPERATION}" ] || fail 'legacy fallback inherited a stop operation'
# Check the actual stop function cannot reach TERM without prepare completing.
python3 - "${ENTRY}" <<'PY'
import sys
s=open(sys.argv[1]).read().split('\nstop() {',1)[1].split('\nstatus() {',1)[0]
assert s.index('cleanup_fn_connect_waf_for_stop 1') < s.index('if ! prepare_planned_stop; then') < s.index('stop_service "${GATEWAY_PID_FILE}"')
assert s.index('planned_stop_write gateway_stopped') < s.index('stop_service "${BACKEND_PID_FILE}"')
assert 'flock -o -w 5' in open(sys.argv[1]).read()
PY
printf '[planned-stop] handshake, stale ACK, cancellation, upgrade compatibility and stop ordering passed\n'
