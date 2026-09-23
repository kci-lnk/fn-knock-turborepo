#!/usr/bin/env bash
# Local preparation only until identities are finalized. No implicit execution.
set -euo pipefail
bundle="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
action="${1:-show}"
active_pid=""
batch=""
current_case=""
write_outcome() {
  python3 -B - "$batch" "$current_case" "$@" <<'PY'
import datetime,hashlib,json,sys
from pathlib import Path
b=Path(sys.argv[1]);case=sys.argv[2]
if not b.is_dir():raise SystemExit(0)
codes=dict(zip(['harness','contract','gate','report'],sys.argv[3:7]))
code=int(sys.argv[7]);timestamp=datetime.datetime.now(datetime.timezone.utc).isoformat()
value={'schema_version':1,'case':case,'passed':code==0,'exit_code':code,'finished_at_utc':timestamp,
 'gate_flags':['--require-six-pairs'] if case.startswith('session-ttl') else [],
 'stages':{k:{'executed':v!='not_run','exit_code':None if v=='not_run' else int(v)} for k,v in codes.items()},'evidence':{}}
for name in [f'{case}/results.json',f'{case}/manifest.json',f'{case}.contract.json',f'{case}.gate.json',f'{case}.soak-summary.json',f'{case}.report.md',f'{case}.run.log']:
 p=b/name
 if p.is_file():value['evidence'][name]={'sha256':hashlib.sha256(p.read_bytes()).hexdigest(),'bytes':p.stat().st_size}
(b/f'{case}.outcome.json').write_text(json.dumps(value,indent=2)+'\n')
if code:
 (b/'outcome.json').write_text(json.dumps({'schema_version':1,'passed':False,'failed_case':case,'exit_code':code,'finished_at_utc':timestamp},indent=2)+'\n')
 (b/'finished-utc.txt').write_text(timestamp+'\n')
PY
}
stop_active() {
  local code="$1"
  trap - TERM INT
  if [[ -n "$active_pid" ]] && kill -0 "$active_pid" 2>/dev/null; then
    kill -TERM "$active_pid" 2>/dev/null || true
    wait "$active_pid" || true
  fi
  if [[ -n "$batch" && -n "$current_case" ]]; then
    write_outcome "$code" not_run not_run not_run "$code" || true
  fi
  exit "$code"
}
trap 'stop_active 143' TERM
trap 'stop_active 130' INT
python3 -B "$bundle/check-plan.py" verify
case "$action" in
  show)
    python3 -B - "$bundle/manifest.json" <<'PY'
import json,sys
m=json.load(open(sys.argv[1]));print(m['status'])
print('45 trials; optimized candidate5 baseline -> audit fix candidate6. No execution.')
for j in m['jobs']:
 o=j['options'];print(j['name'],j['trial_count'],'trials',','.join(o['routes']),f"warm/load={o['warmup']}/{o['seconds']}",f"c={o['concurrency']}",f"TTL={o['cacheTtl']}",j['gate'])
print('Frozen target:',m['remote_batch'])
PY
    ;;
  preflight) python3 -B "$bundle/check-plan.py" preflight ;;
  run)
    [[ "$#" == 1 ]] || { echo 'Usage: run-audit6.sh run; no path/group override or resume.' >&2; exit 2; }
    # Reject placeholders before opening a lock or creating output.
    python3 -B "$bundle/check-plan.py" preflight
    exec 9> /tmp/fn-knock-auth-post-experiment.lock
    flock -n 9 || { echo 'Shared experiment lock busy; no load started.' >&2; exit 3; }
    if pgrep -af 'auth-performance\.mjs|auth-performance-suite\.mjs run|run-auth-performance-isolated\.sh' >&2; then
      echo 'Another auth harness is active; no load started.' >&2; exit 3
    fi
    python3 -B "$bundle/check-plan.py" preflight
    tools="$(python3 -B -c 'import json,sys;print(json.load(open(sys.argv[1]))["remote_tools"])' "$bundle/manifest.json")"
    batch="$(python3 -B -c 'import json,sys;print(json.load(open(sys.argv[1]))["remote_batch"])' "$bundle/manifest.json")"
    node_binary="${AUTH_PERF_NODE:-$(command -v node)}"
    [[ ! -e "$batch" ]] || { echo 'Output exists; refusing overwrite/retry.' >&2; exit 2; }
    mkdir -- "$batch"
    cp -- "$bundle/manifest.json" "$batch/prepared-manifest.json"
    date -u '+%Y-%m-%dT%H:%M:%SZ' > "$batch/started-utc.txt"
    run_case() {
      local name="$1" config="$2" gate="$3"
      shift 3
      current_case="$name"
      local output="$batch/$name"
      [[ ! -e "$output" && ! -e "$output.run.log" ]] || return 2
      env -u FN_KNOCK_AUTH_BRIDGE_MAX_IN_FLIGHT -u FN_KNOCK_SQLITE_AUTH_READERS \
        -u FN_KNOCK_TOKIO_WORKER_THREADS -u GLIBC_TUNABLES -u LD_PRELOAD -u MALLOC_ARENA_MAX \
        -u GOMEMLIMIT -u GOGC -u GOMAXPROCS AUTH_PERF_NODE="$node_binary" \
        bash "$tools/run-auth-performance-isolated.sh" \
        --config "$bundle/configs/$config.json" --out "$output" "$@" \
        > "$output.run.log" 2>&1 &
      active_pid=$!
      local harness_exit=0 contract_exit=0 gate_exit=not_run report_exit=not_run final_exit=0
      wait "$active_pid" || harness_exit=$?
      active_pid=""
      python3 -B "$bundle/check-plan.py" contract "$name" "$output" \
        > "$output.contract.json" 2> "$output.contract.stderr.log" || contract_exit=$?
      # Retain interpretable partial evidence on every failure; never retry.
      if [[ -f "$output/results.json" ]]; then
        gate_exit=0
        if [[ "$gate" == soak ]]; then
          local raw=""
          raw="$(python3 -B - "$output/results.json" <<'PY'
import json,sys
r=json.load(open(sys.argv[1]))['runs'];assert len(r)==1 and r[0]['role']=='candidate';print(r[0]['raw_result_file'])
PY
          )" || gate_exit=$?
          if [[ "$gate_exit" == 0 ]]; then
            "$node_binary" "$tools/auth-performance-soak.mjs" "$raw" > "$output.soak-summary.json" 2> "$output.gate.stderr.log" || gate_exit=$?
          fi
          if [[ "$gate_exit" == 0 ]]; then
            python3 -B - "$output/results.json" "$output.soak-summary.json" > "$output.gate.json" 2>> "$output.gate.stderr.log" <<'PY' || gate_exit=$?
import json,sys
r=json.load(open(sys.argv[1]))['runs'][0];s=json.load(open(sys.argv[2]))
assert s['completed_30_minutes_without_request_or_quality_failure'] is True
assert (s['candidate'],s['role'],s['scenario'])==(r['candidate'],r['role'],r['scenario'])
assert s['elapsed_ms']==r['measurement']['elapsed_ms'] and s['successful_requests']==r['measurement']['successful_requests'] and s['failures']==0 and s['quality']==r['quality']
print(json.dumps({'passed':True,'completion_only':True,'resource_growth_gate':False}))
PY
          fi
        else
          local flags=()
          [[ "$gate" != six_pair_nonregression ]] || flags=(--require-six-pairs)
          "$node_binary" "$tools/check-auth-performance.mjs" "$output/results.json" "${flags[@]}" \
            > "$output.gate.json" 2> "$output.gate.stderr.log" || gate_exit=$?
        fi
        report_exit=0
        "$node_binary" "$tools/report-auth-performance.mjs" "$output/results.json" \
          > "$output.report.md" 2> "$output.report.stderr.log" || report_exit=$?
      fi
      if [[ "$harness_exit" != 0 ]]; then final_exit=20
      elif [[ "$contract_exit" != 0 ]]; then final_exit=21
      elif [[ "$gate_exit" != 0 ]]; then final_exit=22
      elif [[ "$report_exit" != 0 ]]; then final_exit=23
      fi
      write_outcome "$harness_exit" "$contract_exit" "$gate_exit" "$report_exit" "$final_exit"
      [[ "$final_exit" == 0 ]] || return "$final_exit"
      printf '%s\n' "$name" >> "$batch/completed-cases.txt"
    }
    source "$bundle/jobs.sh"
    run_all
    python3 -B - "$batch" <<'PY'
import datetime,json,sys
from pathlib import Path
b=Path(sys.argv[1]);names=['smoke','session-ttl0','session-ttl1','soak-ttl1','recovery']
outcomes=[json.loads((b/f'{n}.outcome.json').read_text()) for n in names]
rows=[r for n in names for r in json.loads((b/n/'results.json').read_text())['runs']]
processes=[(component,r['resources'][component]['before']['pid'],r['resources'][component]['before']['start_ticks']) for r in rows for component in ['go','rust']]
errors=[]
if not all(x['passed'] for x in outcomes):errors.append('case failed')
if len(rows)!=45 or len({r['directory'] for r in rows})!=45:errors.append('batch trial directories/count')
if len(set(processes))!=90:errors.append('product process reused across cases')
code=24 if errors else 0
now=datetime.datetime.now(datetime.timezone.utc).isoformat()
(b/'outcome.json').write_text(json.dumps({'schema_version':1,'passed':not errors,'exit_code':code,'completed_cases':names,'expected_trials':45,'finished_at_utc':now,'no_prior_trial_pooling':True,'errors':errors},indent=2)+'\n')
if not errors:(b/'completed-utc.txt').write_text(now+'\n')
(b/'finished-utc.txt').write_text(now+'\n')
raise SystemExit(code)
PY
    ;;
  *) echo 'Usage: run-audit6.sh [show|preflight|run]' >&2; exit 2 ;;
esac
