#!/usr/bin/env bash
# Prepared only. Explicit `run` is required to start this independent case.
set -euo pipefail
bundle="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
action="${1:-show}"
batch="/tmp/fn-knock-auth-performance-20260923/c64-session-confirm-final"
tools="/tmp/fn-knock-auth-performance-20260923/scripts5"
active_pid=""
stop_active() {
  local code="$1"
  trap - TERM INT
  if [[ -n "$active_pid" ]] && kill -0 "$active_pid" 2>/dev/null; then
    kill -TERM "$active_pid" 2>/dev/null || true
    wait "$active_pid" || true
  fi
  exit "$code"
}
trap 'stop_active 143' TERM
trap 'stop_active 130' INT
python3 -B "$bundle/check-plan.py" verify
case "$action" in
  show)
    printf '%s\n' 'Prepared, not executed: independent c64 session_hit, six AB/BA pairs, warm20/load60.' \
      'N1000 accounts/sessions, grants100, TOTP, TTL0, reader1/z, clients2, Tokio2; normal bridge capacity unset.' \
      "New batch: $batch; original 261 trials remain separate." \
      'Frozen gate: --require-six-pairs --require-improvement; gate failure still preserves report.'
    ;;
  preflight) python3 -B "$bundle/check-plan.py" preflight ;;
  run)
    [[ "$#" == 1 ]] || { echo 'This frozen runner accepts only run; output path cannot be overridden.' >&2; exit 2; }
    exec 9> /tmp/fn-knock-auth-post-experiment.lock
    flock -n 9 || { echo 'Another experiment owns the shared lock; no load started.' >&2; exit 3; }
    if pgrep -af 'auth-performance\.mjs|auth-performance-suite\.mjs run|run-auth-performance-isolated\.sh' >&2; then
      echo 'Another auth harness is active; no load started.' >&2; exit 3
    fi
    python3 -B "$bundle/check-plan.py" preflight
    [[ ! -e "$batch" ]] || { echo 'Frozen batch already exists; no overwrite/retry.' >&2; exit 2; }
    node_binary="${AUTH_PERF_NODE:-$(command -v node)}"
    mkdir -- "$batch"
    cp -- "$bundle/manifest.json" "$batch/prepared-manifest.json"
    cp -- "$bundle/trigger-evidence.json" "$batch/trigger-evidence.json"
    date -u '+%Y-%m-%dT%H:%M:%SZ' > "$batch/started-utc.txt"
    output="$batch/session_hit"
    env -u FN_KNOCK_AUTH_BRIDGE_MAX_IN_FLIGHT -u FN_KNOCK_SQLITE_AUTH_READERS \
      -u FN_KNOCK_TOKIO_WORKER_THREADS -u GLIBC_TUNABLES -u LD_PRELOAD -u MALLOC_ARENA_MAX \
      -u GOMEMLIMIT -u GOGC -u GOMAXPROCS AUTH_PERF_NODE="$node_binary" \
      bash "$tools/run-auth-performance-isolated.sh" \
      --config "$bundle/variants5.json" --out "$output" --candidates candidate5 \
      --routes session_hit --pairs 6 --warmup 20 --seconds 60 \
      --concurrency 64 --clients 2 --accounts 1000 --sessions 1000 --grants 100 \
      --credential-kind totp --cache-ttl 0 --renewals 64 --profile 0 --recovery-probe 0 \
      --roles baseline,candidate > "$output.run.log" 2>&1 &
    active_pid=$!
    harness_exit=0
    wait "$active_pid" || harness_exit=$?
    active_pid=""
    contract_exit=0
    python3 -B "$bundle/check-plan.py" contract "$output" > "$output.contract.json" 2> "$output.contract.stderr.log" || contract_exit=$?
    gate_exit="not_run"
    report_exit="not_run"
    # Run the report even when the exact contract or frozen strict gate fails.
    # Incomplete evidence is retained; no retry, fallback case or relaxed gate.
    if [[ -f "$output/results.json" ]]; then
      gate_exit=0
      "$node_binary" "$tools/check-auth-performance.mjs" "$output/results.json" \
        --require-six-pairs --require-improvement > "$output.gate.json" 2> "$output.gate.stderr.log" || gate_exit=$?
      report_exit=0
      "$node_binary" "$tools/report-auth-performance.mjs" "$output/results.json" \
        > "$output.report.md" 2> "$output.report.stderr.log" || report_exit=$?
    fi
    final_exit=0
    if [[ "$harness_exit" != 0 ]]; then final_exit=20
    elif [[ "$contract_exit" != 0 ]]; then final_exit=21
    elif [[ "$gate_exit" != 0 ]]; then final_exit=22
    elif [[ "$report_exit" != 0 ]]; then final_exit=23
    fi
    python3 -B - "$batch" "$harness_exit" "$contract_exit" "$gate_exit" "$report_exit" "$final_exit" <<'PY'
import datetime,hashlib,json,sys
from pathlib import Path
b=Path(sys.argv[1]);codes=dict(zip(['harness','contract','gate','report'],sys.argv[2:6]))
out={'schema_version':1,'independent_confirmation':True,'mixed_with_prior_trials':False,
     'gate_flags':['--require-six-pairs','--require-improvement'],
     'stages':{k:{'executed':v!='not_run','exit_code':None if v=='not_run' else int(v)} for k,v in codes.items()},
     'exit_code':int(sys.argv[6]),'passed':sys.argv[6]=='0',
     'finished_at_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),
     'evidence':{}}
for name in ['session_hit/results.json','session_hit/manifest.json','session_hit.contract.json','session_hit.gate.json','session_hit.report.md']:
    p=b/name
    if p.is_file():out['evidence'][name]={'sha256':hashlib.sha256(p.read_bytes()).hexdigest(),'bytes':p.stat().st_size}
(b/'outcome.json').write_text(json.dumps(out,indent=2)+'\n')
(b/'finished-utc.txt').write_text(out['finished_at_utc']+'\n')
if out['passed']:(b/'completed-utc.txt').write_text(out['finished_at_utc']+'\n')
print(json.dumps({'passed':out['passed'],'exit_code':out['exit_code'],'stages':out['stages']}))
PY
    exit "$final_exit"
    ;;
  *) echo 'Usage: run-confirm.sh [show|preflight|run]' >&2; exit 2 ;;
esac
