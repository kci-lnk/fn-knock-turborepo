#!/usr/bin/env python3
"""Synthetic contract tests only: no network, builds, services or performance runs."""
import copy,importlib.util,json
from pathlib import Path
ROOT=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('check_plan',ROOT/'check-plan.py')
checker=importlib.util.module_from_spec(spec);spec.loader.exec_module(checker)
m=checker.load(ROOT/'manifest.json')

def fixture(job):
    c=checker.load(ROOT/'configs'/f"{job['config']}.json");o=job['options']
    manifest={'schema_version':1,'config':copy.deepcopy(c),'options':{**copy.deepcopy(o),'config':m['remote_bundle']+'/configs/'+job['config']+'.json','out':m['remote_batch']+'/'+job['name'],'hz':100},'identities':[{'variant':v['name'],'component':k,'path':v[k],'sha256':m['artifacts'][v[k]]['sha256']} for v in [c['baseline'],c['candidates'][0]] for k in ['go','rust']],'harness_identity':{'files':[{'name':name,'sha256':m['tools']['files'][name]} for name in sorted(checker.HARNESS)],'control_binary_sha256':m['control_binary']['sha256']}}
    rows=[]
    for p in range(o['pairs']):
        for s in o['routes']:
            for role in (['baseline','candidate'] if p%2==0 else ['candidate','baseline']):
                if role not in o['roles']:continue
                v=c['baseline'] if role=='baseline' else c['candidates'][0]
                expected={'bootstrap':'bootstrap','challenge':'challenge','session_api':'session','session_miss':'redirect','grant_miss':'redirect','auto_ip_miss':'redirect'}.get(s,'origin')
                measure=lambda seconds:dict(failures=0,successful_requests=100,elapsed_ms=seconds*1000+1)
                quality={k:True for k in ['sampling_ok','health_ok','client_ok','duration_ok']}
                row=dict(candidate='candidate6',variant=v['name'],variant_metadata=copy.deepcopy(v['metadata']),env=copy.deepcopy(v['env']),scenario=s,pair=p,role=role,directory=f'synthetic-contract-only/{len(rows)}',concurrency=16,cache_ttl_seconds=o['cacheTtl'],roles=o['roles'],profiling=False,recovery_probe=o['recoveryProbe'],seed=dict(scenario=s,accounts=1000,sessions=1000,ordinary_grants=100,total_grants=100,credential_kind='totp',renewal_tokens_per_phase=0),effective_runtime_env=dict(FN_KNOCK_RUNTIME_TARGET='linux',FN_KNOCK_TOKIO_WORKER_THREADS='2',FN_KNOCK_SQLITE_AUTH_READERS='1',FN_KNOCK_AUTH_BRIDGE_MAX_IN_FLIGHT='32' if o['recoveryProbe'] else None,GLIBC_TUNABLES=None,LD_PRELOAD=None,MALLOC_ARENA_MAX=None,GOMEMLIMIT=None,GOGC=None,GOMAXPROCS=None),validation={'passed':True},quality=quality,warmup={'measurement':measure(o['warmup'])},measurement=measure(o['seconds']),preflight=dict(status=302 if expected=='redirect' else 200,expected=expected,origin='auth-performance-owned-origin-v1' if expected=='origin' else None,renewal=s=='session_miss'),resources={k:{x:{'pid':1000+len(rows)*2+i,'start_ticks':1000+len(rows)*2+i} for x in ['before','after']} for i,k in enumerate(['go','rust'])})
                for k in ['cache_control','cache_control_after']:row[k]=dict(runtime_verified=True,auth_cache_ttl_seconds=o['cacheTtl'],auth_cache_unauthorized_ttl_seconds=o['cacheTtl'])
                if o['recoveryProbe']:row['recovery']=dict(burst_concurrency=64,burst_duration_ms=2000,recovery_concurrency=1,recovery_deadline_ms=10000,success_window_ms=2000,performance_claim_allowed=False,recovered=True,passed=True,recovery_elapsed_ms=2100,burst_503_responses=10,burst={'measurement':{'statuses':{'503':10}},'quality':quality},checks=[dict(continuous_success=True,requests_finished_after_burst_ms=2100,result={'measurement':measure(2),'quality':quality})])
                rows.append(row)
    return c,manifest,{'schema_version':1,'runs':rows}

positive=0;negative=0
for job in m['jobs']:
    c,mf,r=fixture(job)
    result=checker.contract(m,job,c,mf,r);assert result['contract_passed'],result
    positive+=1
    mutations=[('drop row',lambda x:x['runs'].pop()),('duplicate row',lambda x:x['runs'].append(copy.deepcopy(x['runs'][0]))),('TTL after',lambda x:x['runs'][0]['cache_control_after'].update(auth_cache_ttl_seconds=99)),('source',lambda x:x['runs'][0]['variant_metadata'].update(go_commit='wrong')),('capacity',lambda x:x['runs'][0]['effective_runtime_env'].update(FN_KNOCK_AUTH_BRIDGE_MAX_IN_FLIGHT='999')),('bad normal response',lambda x:x['runs'][0]['measurement'].update(failures=1)),('short duration',lambda x:x['runs'][0]['measurement'].update(elapsed_ms=1)),('restarted process',lambda x:x['runs'][0]['resources']['rust']['after'].update(start_ticks=1))]
    for label,mutation in mutations:
        changed=copy.deepcopy(r);mutation(changed)
        assert not checker.contract(m,job,c,mf,changed)['contract_passed'],(job['name'],label)
        negative+=1
    bad=copy.deepcopy(mf);bad['identities'][0]['sha256']='wrong'
    assert not checker.contract(m,job,c,bad,r)['contract_passed'];negative+=1
    bad=copy.deepcopy(mf);bad['harness_identity']['control_binary_sha256']='wrong'
    assert not checker.contract(m,job,c,bad,r)['contract_passed'];negative+=1
    if job['name']=='smoke':
        changed=copy.deepcopy(r);changed['runs']=[x for x in changed['runs'] if x['scenario']!='challenge']
        assert not checker.contract(m,job,c,mf,changed)['contract_passed'];negative+=1
    if job['name']=='recovery':
        changed=copy.deepcopy(r);changed['runs'][0]['recovery']['recovery_elapsed_ms']=10001
        assert not checker.contract(m,job,c,mf,changed)['contract_passed'];negative+=1
print(json.dumps({'synthetic_contract_only':True,'positive_cases':positive,'negative_cases':negative,'passed':True,'experiment_executed':False},indent=2))
