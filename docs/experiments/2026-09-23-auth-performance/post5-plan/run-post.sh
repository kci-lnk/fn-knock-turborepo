#!/usr/bin/env bash
# This experiment-specific runner does nothing unless an explicit action is supplied.
set -euo pipefail
bundle="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
node_binary="${AUTH_PERF_NODE:-$(command -v node)}"
action="${1:-show}"
group="${2:-}"
batch="${3:-}"
active_pid=""
stop_active() {
  local code="$1"
  trap - TERM INT
  if [[ -n "$active_pid" ]] && kill -0 "$active_pid" 2>/dev/null; then
    # The isolated wrapper execs Node. Signal only our current harness; it owns
    # cleanup of its synthetic Go/Rust processes and namespace.
    kill -TERM "$active_pid" 2>/dev/null || true
    wait "$active_pid" || true
  fi
  exit "$code"
}
trap 'stop_active 143' TERM
trap 'stop_active 130' INT
tools="$(python3 - "$bundle/bundle-manifest.json" <<'PY'
import json, sys
print(json.load(open(sys.argv[1]))['remote_tools'])
PY
)"

verify_bundle() {
  python3 - "$bundle" <<'PY'
import hashlib, json, sys
from pathlib import Path
root = Path(sys.argv[1])
m = json.loads((root / 'bundle-manifest.json').read_text())
for name, expected in m['files'].items():
    actual = hashlib.sha256((root / name).read_bytes()).hexdigest()
    if actual != expected:
        raise SystemExit(f'bundle file changed: {name}')
PY
}

verify_tools() {
  python3 - "$bundle/bundle-manifest.json" <<'PY'
import hashlib, json, sys
from pathlib import Path
m = json.load(open(sys.argv[1]))
for name, expected in m['tools'].items():
    actual = hashlib.sha256((Path(m['remote_tools']) / name).read_bytes()).hexdigest()
    if actual != expected:
        raise SystemExit(f'frozen scripts5 tool changed: {name}')
PY
}

show() {
  python3 - "$bundle" <<'PY'
import json, sys
from pathlib import Path
root = Path(sys.argv[1])
m = json.loads((root / 'bundle-manifest.json').read_text())
print('Prepared only; explicit run is required. Candidate:', m['candidate_name'])
for j in json.loads((root / 'jobs.json').read_text()):
    o = j['options']
    print(j['group'], j['name'], 'routes='+o['routes'], 'pairs='+str(o['pairs']),
          'warm/load='+str(o['warmup'])+'/'+str(o['seconds']),
          'c='+str(o['concurrency']), 'TTL='+str(o['cache-ttl']), 'gate='+j['gate'])
PY
}

preflight() {
  [[ "$(uname -s)" == Linux ]] || { echo 'Execution preflight requires the target Linux host.' >&2; return 1; }
  verify_tools
  python3 - "$bundle" <<'PY'
import hashlib, json, os, sys
from pathlib import Path
root = Path(sys.argv[1])
m = json.loads((root / 'bundle-manifest.json').read_text())
if root != Path(m['remote_bundle']):
    raise SystemExit('bundle installed at a different path; prepare a new bundle')
for name, evidence in m['artifacts'].items():
    p = Path(name)
    if not p.is_file() or not os.access(p, os.X_OK):
        raise SystemExit(f'missing executable: {name}')
    h = hashlib.sha256()
    with p.open('rb') as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b''):
            h.update(chunk)
    if h.hexdigest() != evidence['sha256']:
        raise SystemExit(f'binary identity mismatch: {name}')
for key in ('admin_static', 'auth_static', 'control_binary'):
    p = Path(json.loads((root / 'configs/primary.json').read_text())[key])
    if not p.exists():
        raise SystemExit(f'missing fixture/control path: {p}')
print('Bundle and all product binary hashes match; no load has started.')
PY
}

check_main() {
  verify_tools
  local results
  results="$(python3 - "$bundle/bundle-manifest.json" <<'PY'
import json, sys
print(json.load(open(sys.argv[1]))['main_results'])
PY
)"
  # Verify identity hashes too: plan metadata alone is not a content-to-source proof.
  python3 - "$bundle/bundle-manifest.json" "$results" <<'PY'
import json, sys
from pathlib import Path
b = json.load(open(sys.argv[1]))
results = Path(sys.argv[2])
rows = json.loads(results.read_text())
path = results.parent / rows.get('manifest_file', 'manifest.json')
m = json.loads(path.read_text())
for identity in m['identities']:
    known = b['artifacts'].get(identity['path'])
    if known is None or identity['sha256'] != known['sha256']:
        raise SystemExit('main recorded binary hash differs from this frozen bundle')
PY
  "$node_binary" "$tools/check-auth-performance-plan.mjs" \
    --plan "$bundle/EXPERIMENT_PLAN.json" --results "$results" --phase main
}

run_case() {
  local name="$1" config="$2" gate="$3"
  shift 3
  local output="$batch/$name"
  [[ ! -e "$output" && ! -e "$output.run.log" ]] || { echo "Refusing existing case: $output" >&2; return 1; }
  printf 'Running %s (config=%s; gate=%s)\n' "$name" "$config" "$gate"
  env -u FN_KNOCK_AUTH_BRIDGE_MAX_IN_FLIGHT -u FN_KNOCK_SQLITE_AUTH_READERS \
    -u GLIBC_TUNABLES -u LD_PRELOAD -u MALLOC_ARENA_MAX -u GOMEMLIMIT -u GOGC -u GOMAXPROCS \
    AUTH_PERF_NODE="$node_binary" \
    bash "$tools/run-auth-performance-isolated.sh" \
    --config "$bundle/configs/$config.json" --out "$output" "$@" \
    > "$output.run.log" 2>&1 &
  active_pid=$!
  local harness_exit=0
  wait "$active_pid" || harness_exit=$?
  active_pid=""
  if [[ "$harness_exit" != 0 ]]; then
    printf 'Case %s failed (exit %s); evidence retained at %s\n' "$name" "$harness_exit" "$output" >&2
    return "$harness_exit"
  fi
  case "$gate" in
    cache|renewal)
      "$node_binary" "$tools/check-auth-performance-plan.mjs" \
        --plan "$bundle/EXPERIMENT_PLAN.json" --results "$output/results.json" --phase "$gate" \
        > "$output.acceptance.json"
      ;;
    smoke)
      "$node_binary" "$tools/check-auth-performance.mjs" "$output/results.json" > "$output.validity.json"
      ;;
    soak)
      local raw
      raw="$(python3 - "$output/results.json" <<'PY'
import json, sys
rows = json.load(open(sys.argv[1]))['runs']
assert len(rows) == 1 and rows[0]['role'] == 'candidate'
print(rows[0]['raw_result_file'])
PY
)"
      "$node_binary" "$tools/auth-performance-soak.mjs" "$raw" > "$output.soak-summary.json"
      python3 - "$output.soak-summary.json" <<'PY'
import json, sys
assert json.load(open(sys.argv[1]))['completed_30_minutes_without_request_or_quality_failure']
PY
      ;;
  esac
  "$node_binary" "$tools/report-auth-performance.mjs" "$output/results.json" > "$output.report.md"
  printf '%s\n' "$name" >> "$batch/completed-cases.txt"
}

verify_bundle
case "$action" in
  show) show ;;
  preflight) preflight ;;
  check-main) check_main ;;
  run)
    [[ "$group" =~ ^(cache|renewal|grants|params|profile|soak|recovery|all)$ ]] || { echo 'Specify a known group.' >&2; exit 2; }
    [[ "$batch" = /* ]] || { echo 'Supply an absolute NEW result batch directory as third argument.' >&2; exit 2; }
    # Every bundle copy uses the same root-host lock; scripts4 does not, so also
    # reject any live harness. Never stop an existing experiment from here.
    exec 9> /tmp/fn-knock-auth-post-experiment.lock
    flock -n 9 || { echo 'Another post experiment owns the lock.' >&2; exit 1; }
    if pgrep -af 'auth-performance\.mjs|auth-performance-suite\.mjs run|run-auth-performance-isolated\.sh' >&2; then
      echo 'An auth experiment is already running; no new load started.' >&2; exit 1
    fi
    preflight
    mkdir -- "$batch"
    cp -- "$bundle/bundle-manifest.json" "$batch/prepared-bundle-manifest.json"
    source "$bundle/jobs.sh"
    if [[ "$group" == all ]]; then
      check_main > "$batch/main.acceptance.json"
      run_cache
      run_renewal
      run_grants
      run_params
      run_profile
      run_soak
      run_recovery
    else
      "run_$group"
    fi
    ;;
  *) echo 'Usage: run-post.sh [show|preflight|check-main|run GROUP /absolute/NEW_RESULTS]' >&2; exit 2 ;;
esac
