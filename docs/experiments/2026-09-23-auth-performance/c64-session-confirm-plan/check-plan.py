#!/usr/bin/env python3
"""Read-only bundle/preflight/contract checks. Does not start experiments."""
import hashlib
import json
import math
import os
from pathlib import Path
import sys

ROOT=Path(__file__).resolve().parent
HARNESS={'run-auth-performance-isolated.sh','auth-performance.mjs','auth-performance-lib.mjs','auth-performance-worker.mjs','auth-performance-seed.py','auth-performance-profile.mjs','auth-performance-recovery.mjs','auth-performance-soak.mjs'}

def digest(path):
    h=hashlib.sha256()
    with path.open('rb') as f:
        for chunk in iter(lambda:f.read(1024*1024),b''):h.update(chunk)
    return h.hexdigest()

def load(path):return json.loads(path.read_text())

def verify_bundle(root=ROOT):
    m=load(root/'manifest.json')
    assert m['status']=='prepared_not_executed' and m['trial_count']==12
    assert m['gate_flags']==['--require-six-pairs','--require-improvement']
    assert m['options']['pairs']==6 and m['options']['warmup']==20 and m['options']['seconds']==60
    assert m['options']['routes']==['session_hit'] and m['options']['concurrency']==64
    assert m['files'], 'bundle files not finalized'
    for name,expected in m['files'].items():
        p=root/name
        assert p.is_file() and not p.is_symlink() and digest(p)==expected, 'bundle changed: '+name
    assert digest(root/'variants5.json')==m['input_config_sha256']
    return m

def preflight(root=ROOT):
    m=verify_bundle(root)
    assert sys.platform.startswith('linux'), 'target preflight requires Linux'
    assert root==Path(m['remote_bundle']), 'bundle must be installed at its frozen remote path'
    for name,expected in m['tools']['files'].items():
        p=Path(m['remote_tools'])/name
        assert p.is_file() and not p.is_symlink() and digest(p)==expected, 'scripts5 identity changed: '+name
    for name,evidence in m['artifacts'].items():
        p=Path(name)
        assert p.is_file() and not p.is_symlink() and os.access(p,os.X_OK), 'missing executable: '+name
        assert p.stat().st_size==evidence['bytes'] and digest(p)==evidence['sha256'], 'artifact mismatch: '+name
    config=load(root/'variants5.json')
    for key in ['admin_static','auth_static','control_binary']:assert Path(config[key]).exists(), 'missing fixture/control: '+key
    assert config['control_binary']==m['control_binary']['path']
    assert digest(Path(config['control_binary']))==m['control_binary']['sha256'], 'control changed'
    print('Pinned bundle, 12 scripts5 files, control and four product artifacts verified; no load started.')

def contract(m,config,manifest,result):
    errors=[]
    def check(ok,label):
        if not ok:errors.append(label)
    check(result.get('schema_version')==1 and manifest.get('schema_version')==1,'schema_version')
    check(manifest.get('config')==config,'frozen config')
    for field,expected in m['options'].items():
        actual=manifest.get('options',{}).get(field)
        check(type(actual) is type(expected) and actual==expected,'option: '+field)
    identities=manifest.get('identities',[])
    expected_ids={(v['name'],component,v[component],m['artifacts'][v[component]]['sha256']) for v in [config['baseline'],config['candidates'][0]] for component in ['go','rust']}
    check(len(identities)==4 and {(i.get('variant'),i.get('component'),i.get('path'),i.get('sha256')) for i in identities}==expected_ids,'four artifact identities')
    hi=manifest.get('harness_identity',{});files=hi.get('files',[])
    check(len(files)==8 and {i.get('name') for i in files}==HARNESS,'eight harness identities')
    for item in files:check(item.get('sha256')==m['tools']['files'].get(item.get('name')),'harness SHA')
    check(hi.get('control_binary_sha256')==m['control_binary']['sha256'],'control SHA')
    rows=result.get('runs',[])
    expected={('session_hit',pair,role) for pair in range(6) for role in ['baseline','candidate']}
    check(len(rows)==12 and {(r.get('scenario'),r.get('pair'),r.get('role')) for r in rows}==expected,'exact 12 trials')
    check(len({r.get('directory') for r in rows})==12 and all(isinstance(r.get('directory'),str) for r in rows),'unique trial directories')
    expected_order=[(pair,role) for pair in range(6) for role in (['baseline','candidate'] if pair%2==0 else ['candidate','baseline'])]
    check([(r.get('pair'),r.get('role')) for r in rows]==expected_order,'AB/BA alternating order')
    for r in rows:
        role=r.get('role');label=str(r.get('pair'))+'/'+str(role)+': '
        if role not in ['baseline','candidate']:continue
        v=config['baseline'] if role=='baseline' else config['candidates'][0]
        check(r.get('candidate')=='candidate5' and r.get('variant')==v['name'],label+'variant')
        check(r.get('variant_metadata')==v['metadata'] and r.get('env')==v['env'],label+'source/build/env metadata')
        check(r.get('concurrency')==64 and r.get('cache_ttl_seconds')==0,label+'c64/TTL0')
        check(r.get('roles')==['baseline','candidate'] and r.get('profiling') is False and r.get('recovery_probe') is False,label+'roles/diagnostic flags')
        seed=dict(scenario='session_hit',accounts=1000,sessions=1000,ordinary_grants=100,total_grants=100,credential_kind='totp',renewal_tokens_per_phase=0)
        check(all(r.get('seed',{}).get(k)==value for k,value in seed.items()),label+'seed scale')
        env=dict(FN_KNOCK_RUNTIME_TARGET='linux',FN_KNOCK_TOKIO_WORKER_THREADS='2',FN_KNOCK_AUTH_BRIDGE_MAX_IN_FLIGHT=None,FN_KNOCK_SQLITE_AUTH_READERS='1' if role=='candidate' else None,GLIBC_TUNABLES=None,LD_PRELOAD=None,MALLOC_ARENA_MAX=None,GOMEMLIMIT=None,GOGC=None,GOMAXPROCS=None)
        check(r.get('effective_runtime_env')==env,label+'effective env')
        for field in ['cache_control','cache_control_after']:
            observed=r.get(field,{})
            check(observed.get('runtime_verified') is True and observed.get('auth_cache_ttl_seconds')==0 and observed.get('auth_cache_unauthorized_ttl_seconds')==0,label+field)
        check(r.get('validation',{}).get('passed') is True,label+'validation')
        check(all(r.get('quality',{}).get(k) is True for k in ['sampling_ok','health_ok','client_ok','duration_ok']),label+'measurement quality')
        check(r.get('warmup',{}).get('measurement',{}).get('failures')==0 and r.get('measurement',{}).get('failures')==0,label+'warm/load response failures')
        warm_ms=r.get('warmup',{}).get('measurement',{}).get('elapsed_ms')
        load_ms=r.get('measurement',{}).get('elapsed_ms')
        numeric=lambda value:isinstance(value,(int,float)) and not isinstance(value,bool) and math.isfinite(value)
        check(numeric(warm_ms) and warm_ms>=20000,label+'warmup shorter than20s')
        check(numeric(load_ms) and 60000<=load_ms<=63000,label+'load elapsed60s tolerance')
        # Do not add a warmup.duration_ok gate absent from the frozen harness.
        p=r.get('preflight',{})
        check(p.get('status')==200 and p.get('expected')=='origin' and p.get('origin')=='auth-performance-owned-origin-v1' and p.get('renewal') is False,label+'business preflight')
    return dict(schema_version=1,contract_passed=not errors,complete_expected_trials=12,observed_trials=len(rows),concurrency=64,independent_confirmation=True,performance_acceptance_requires_frozen_gate=True,errors=errors)

def main():
    action=sys.argv[1] if len(sys.argv)>1 else 'verify'
    if action=='verify':verify_bundle();print('Frozen local bundle verified; no execution.');return
    if action=='preflight':preflight();return
    if action!='contract' or len(sys.argv)!=3:raise SystemExit('usage: check-plan.py [verify|preflight|contract OUTPUT_DIR]')
    output=Path(sys.argv[2]);m=verify_bundle()
    try:
        answer=contract(m,load(ROOT/'variants5.json'),load(output/'manifest.json'),load(output/'results.json'))
        answer['inputs']={name:dict(path=str(output/name),sha256=digest(output/name)) for name in ['manifest.json','results.json']}
    except (OSError,ValueError,KeyError,TypeError,AttributeError) as error:
        answer=dict(schema_version=1,contract_passed=False,errors=['incomplete/malformed input: '+type(error).__name__],independent_confirmation=True)
    print(json.dumps(answer,indent=2))
    raise SystemExit(0 if answer['contract_passed'] else 1)

if __name__=='__main__':main()
