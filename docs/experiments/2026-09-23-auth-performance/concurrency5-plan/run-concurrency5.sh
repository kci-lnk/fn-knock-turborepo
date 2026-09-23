#!/usr/bin/env bash
# Prepared supplement only: no load without the explicit run action.
set -euo pipefail
bundle="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
action="${1:-show}"
batch="${2:-}"
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

verify_bundle() {
  python3 - "$bundle" <<'PY'
import hashlib, json, sys
from pathlib import Path
root = Path(sys.argv[1])
m = json.loads((root / 'manifest.json').read_text())
for name, expected in m['files'].items():
    assert hashlib.sha256((root / name).read_bytes()).hexdigest() == expected, name
assert m['status'] == 'prepared_not_executed'
assert m['trial_count'] == 24
PY
}

preflight() {
  [[ "$(uname -s)" == Linux ]] || { echo 'Execution preflight requires the target Linux host.' >&2; return 1; }
  python3 - "$bundle" <<'PY'
import hashlib, json, os, sys
from pathlib import Path
root = Path(sys.argv[1])
m = json.loads((root / 'manifest.json').read_text())
assert root == Path(m['remote_bundle']), 'install this unchanged bundle at its recorded path'
def digest(path):
    h = hashlib.sha256()
    with path.open('rb') as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b''):
            h.update(chunk)
    return h.hexdigest()
for name, expected in m['tools']['files'].items():
    assert digest(Path(m['remote_tools']) / name) == expected, 'scripts5 changed: ' + name
for name, evidence in m['artifacts'].items():
    p = Path(name)
    assert p.is_file() and os.access(p, os.X_OK), 'missing executable: ' + name
    assert digest(p) == evidence['sha256'], 'binary identity mismatch: ' + name
config = json.loads((root / 'variants5.json').read_text())
for key in ('admin_static', 'auth_static', 'control_binary'):
    assert Path(config[key]).exists(), 'missing fixture/control path: ' + config[key]
assert config['control_binary'] == m['control_binary']['path']
assert digest(Path(config['control_binary'])) == m['control_binary']['sha256'], 'control binary changed'
print('Pinned bundle, scripts5 and four product binaries verified; no load started.')
PY
}

check_contract() {
  python3 - "$bundle" "$1" "$2" <<'PY'
import json, sys
from pathlib import Path
root, output, concurrency = Path(sys.argv[1]), Path(sys.argv[2]), int(sys.argv[3])
pin = json.loads((root / 'manifest.json').read_text())
config = json.loads((root / 'variants5.json').read_text())
m = json.loads((output / 'manifest.json').read_text())
result = json.loads((output / 'results.json').read_text())
assert result['schema_version'] == 1
assert m['config'] == config, 'config changed'
expected_options = dict(pin['options'], concurrency=concurrency)
for key, value in expected_options.items():
    assert m['options'].get(key) == value, 'wrong option: ' + key
identities = {i['path']: i['sha256'] for i in m['identities']}
assert len(m['identities']) == 4 and identities == {k:v['sha256'] for k,v in pin['artifacts'].items()}
for item in m['harness_identity']['files']:
    assert item['sha256'] == pin['tools']['files'][item['name']], 'harness file changed'
assert m['harness_identity']['control_binary_sha256'] == pin['control_binary']['sha256'], 'measured control binary changed'
routes = pin['options']['routes']
rows = result['runs']
expected = {(route, 0, role) for route in routes for role in ['baseline', 'candidate']}
assert len(rows) == 8 and {(r['scenario'], r['pair'], r['role']) for r in rows} == expected, 'incomplete 8-trial matrix'
assert len({r['directory'] for r in rows}) == 8, 'reused trial directory'
for r in rows:
    v = config['baseline'] if r['role'] == 'baseline' else config['candidates'][0]
    assert r['candidate'] == 'candidate5' and r['variant'] == v['name']
    assert r['variant_metadata'] == v['metadata'] and r['env'] == v['env']
    assert r['concurrency'] == concurrency and r['cache_ttl_seconds'] == 0
    assert r['roles'] == ['baseline', 'candidate'] and r['profiling'] is False and r['recovery_probe'] is False
    assert r['validation']['passed'] is True
    assert all(r['quality'][k] is True for k in ['sampling_ok', 'health_ok', 'client_ok', 'duration_ok'])
    seed = dict(scenario=r['scenario'], accounts=1000, sessions=1000, ordinary_grants=100,
                total_grants=100, credential_kind='totp', renewal_tokens_per_phase=0)
    assert all(r['seed'].get(k) == v for k,v in seed.items()), 'wrong seed scale'
    env = r['effective_runtime_env']
    assert env['FN_KNOCK_RUNTIME_TARGET'] == 'linux' and env['FN_KNOCK_TOKIO_WORKER_THREADS'] == '2'
    assert env['FN_KNOCK_AUTH_BRIDGE_MAX_IN_FLIGHT'] is None
    assert env['FN_KNOCK_SQLITE_AUTH_READERS'] == ('1' if r['role'] == 'candidate' else None)
    for name in ['cache_control', 'cache_control_after']:
        assert r[name]['runtime_verified'] is True
        assert r[name]['auth_cache_ttl_seconds'] == 0 and r[name]['auth_cache_unauthorized_ttl_seconds'] == 0
print(json.dumps({'complete_expected_trials': 8, 'concurrency': concurrency,
                  'contract_passed': True, 'performance_acceptance': False}, indent=2))
PY
}

verify_bundle
case "$action" in
  show)
    printf '%s\n' 'Prepared, not executed: c1/c16/c64 × 4 routes × baseline/candidate = 24 trials.' \
      'Each group: 1 pair, warm5s/load15s, accounts/sessions1000, grants100, TOTP, TTL0, clients2 maximum.' \
      'No six-pair/improvement gate; no default change. Run only after post5 is idle.'
    ;;
  preflight) preflight ;;
  run)
    [[ "$batch" = /* ]] || { echo 'Supply an absolute NEW batch directory.' >&2; exit 2; }
    exec 9> /tmp/fn-knock-auth-post-experiment.lock
    flock -n 9 || { echo 'Another post experiment owns the lock; no load started.' >&2; exit 1; }
    if pgrep -af 'auth-performance\.mjs|auth-performance-suite\.mjs run|run-auth-performance-isolated\.sh' >&2; then
      echo 'An auth experiment is running; no load started.' >&2; exit 1
    fi
    preflight
    node_binary="${AUTH_PERF_NODE:-$(command -v node)}"
    mkdir -- "$batch"
    cp -- "$bundle/manifest.json" "$batch/prepared-manifest.json"
    date -u '+%Y-%m-%dT%H:%M:%SZ' > "$batch/started-utc.txt"
    for concurrency in 1 16 64; do
      output="$batch/c$concurrency"
      printf 'Starting c%s (8 trials)\n' "$concurrency"
      env -u FN_KNOCK_AUTH_BRIDGE_MAX_IN_FLIGHT -u FN_KNOCK_SQLITE_AUTH_READERS \
        -u FN_KNOCK_TOKIO_WORKER_THREADS -u GLIBC_TUNABLES -u LD_PRELOAD -u MALLOC_ARENA_MAX \
        -u GOMEMLIMIT -u GOGC -u GOMAXPROCS AUTH_PERF_NODE="$node_binary" \
        bash "$tools/run-auth-performance-isolated.sh" \
        --config "$bundle/variants5.json" --out "$output" --candidates candidate5 \
        --routes bootstrap,session_hit,grant_hit,auto_ip_hit --pairs 1 --warmup 5 --seconds 15 \
        --concurrency "$concurrency" --clients 2 --accounts 1000 --sessions 1000 --grants 100 \
        --credential-kind totp --cache-ttl 0 --renewals 64 --profile 0 --recovery-probe 0 \
        --roles baseline,candidate > "$output.run.log" 2>&1 &
      active_pid=$!
      harness_exit=0
      wait "$active_pid" || harness_exit=$?
      active_pid=""
      if [[ "$harness_exit" != 0 ]]; then
        printf 'c%s failed (exit %s); retained %s. No retry or later group.\n' "$concurrency" "$harness_exit" "$output" >&2
        exit "$harness_exit"
      fi
      check_contract "$output" "$concurrency" > "$output.contract.json"
      "$node_binary" "$tools/check-auth-performance.mjs" "$output/results.json" > "$output.validity.json"
      "$node_binary" "$tools/report-auth-performance.mjs" "$output/results.json" > "$output.report.md"
      printf 'c%s\n' "$concurrency" >> "$batch/completed-groups.txt"
    done
    date -u '+%Y-%m-%dT%H:%M:%SZ' > "$batch/completed-utc.txt"
    ;;
  *) echo 'Usage: run-concurrency5.sh [show|preflight|run /absolute/NEW_BATCH]' >&2; exit 2 ;;
esac
