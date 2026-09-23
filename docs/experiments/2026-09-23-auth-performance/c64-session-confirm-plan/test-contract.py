#!/usr/bin/env python3
"""Synthetic contract checks only; no load, remote access, binaries or actual results."""
import copy
import importlib.util
import json
from pathlib import Path
root=Path(__file__).resolve().parent
sp=importlib.util.spec_from_file_location('check_plan',root/'check-plan.py')
c=importlib.util.module_from_spec(sp);sp.loader.exec_module(c)
m=json.loads((root/'manifest.json').read_text());config=json.loads((root/'variants5.json').read_text())
manifest=dict(schema_version=1,config=config,options=m['options'],identities=[dict(variant=v['name'],component=k,path=v[k],sha256=m['artifacts'][v[k]]['sha256']) for v in [config['baseline'],config['candidates'][0]] for k in ['go','rust']],harness_identity=dict(files=[dict(name=n,sha256=m['tools']['files'][n]) for n in sorted(c.HARNESS)],control_binary_sha256=m['control_binary']['sha256']))
result=dict(schema_version=1,runs=[])
for pair in range(6):
    for role in (['baseline','candidate'] if pair%2==0 else ['candidate','baseline']):
        v=config['baseline'] if role=='baseline' else config['candidates'][0]
        row=dict(scenario='session_hit',pair=pair,role=role,directory=f'/synthetic/{pair}/{role}',candidate='candidate5',variant=v['name'],variant_metadata=v['metadata'],env=v['env'],concurrency=64,cache_ttl_seconds=0,roles=['baseline','candidate'],profiling=False,recovery_probe=False,seed=dict(scenario='session_hit',accounts=1000,sessions=1000,ordinary_grants=100,total_grants=100,credential_kind='totp',renewal_tokens_per_phase=0),effective_runtime_env=dict(FN_KNOCK_RUNTIME_TARGET='linux',FN_KNOCK_TOKIO_WORKER_THREADS='2',FN_KNOCK_AUTH_BRIDGE_MAX_IN_FLIGHT=None,FN_KNOCK_SQLITE_AUTH_READERS='1' if role=='candidate' else None,GLIBC_TUNABLES=None,LD_PRELOAD=None,MALLOC_ARENA_MAX=None,GOMEMLIMIT=None,GOGC=None,GOMAXPROCS=None),validation=dict(passed=True),quality=dict(sampling_ok=True,health_ok=True,client_ok=True,duration_ok=True),warmup=dict(measurement=dict(failures=0,elapsed_ms=20100)),measurement=dict(failures=0,elapsed_ms=60050),preflight=dict(status=200,expected='origin',origin='auth-performance-owned-origin-v1',renewal=False))
        for key in ['cache_control','cache_control_after']:row[key]=dict(runtime_verified=True,auth_cache_ttl_seconds=0,auth_cache_unauthorized_ttl_seconds=0)
        result['runs'].append(row)
assert c.contract(m,config,manifest,result)['contract_passed']
mutations=[
 ('missing pair',lambda a,b:b['runs'].pop()),
 ('duplicate role',lambda a,b:b['runs'][0].__setitem__('role','candidate')),
 ('AB only',lambda a,b:b['runs'].__setitem__(slice(2,4),list(reversed(b['runs'][2:4])))),
 ('short load protocol',lambda a,b:a['options'].__setitem__('seconds',15)),
 ('wrong artifact',lambda a,b:a['identities'][0].__setitem__('sha256','0'*64)),
 ('missing tool',lambda a,b:a['harness_identity']['files'].pop()),
 ('wrong metadata',lambda a,b:b['runs'][0].__setitem__('variant_metadata',{})),
 ('wrong capacity',lambda a,b:b['runs'][0]['effective_runtime_env'].__setitem__('FN_KNOCK_AUTH_BRIDGE_MAX_IN_FLIGHT','32')),
 ('wrong scale',lambda a,b:b['runs'][0]['seed'].__setitem__('accounts',1)),
 ('TTL enabled',lambda a,b:b['runs'][0]['cache_control_after'].__setitem__('auth_cache_ttl_seconds',1)),
 ('load failure',lambda a,b:b['runs'][0]['measurement'].__setitem__('failures',1)),
 ('short measurement',lambda a,b:b['runs'][0]['measurement'].__setitem__('elapsed_ms',15000)),
 ('warm too short',lambda a,b:b['runs'][0]['warmup']['measurement'].__setitem__('elapsed_ms',5000)),
 ('business marker missing',lambda a,b:b['runs'][0]['preflight'].__setitem__('origin',None)),
]
for label,modify in mutations:
    a,b=copy.deepcopy(manifest),copy.deepcopy(result);modify(a,b)
    assert not c.contract(m,config,a,b)['contract_passed'],label
# Preserve frozen warmup semantics: response correctness remains required,
# but an extended drain/false duration diagnostic is not a new performance gate.
b=copy.deepcopy(result);b['runs'][0]['warmup']['measurement']['elapsed_ms']=22000;b['runs'][0]['warmup']['quality']={'duration_ok':False}
assert c.contract(m,config,manifest,b)['contract_passed']
b['runs'][0]['warmup']['measurement']['failures']=1
assert not c.contract(m,config,manifest,b)['contract_passed']
print(json.dumps(dict(passed=True,valid_matrix_trials=12,invalid_contract_mutations=len(mutations),warmup_gate_compatibility=True,no_experiment_executed=True),indent=2))
