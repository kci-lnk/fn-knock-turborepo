#!/usr/bin/env python3
"""Read-only checks for this fixed 45-trial audit fix experiment."""
import hashlib,json,math,os,sys
from pathlib import Path
ROOT=Path(__file__).resolve().parent
HARNESS={'run-auth-performance-isolated.sh','auth-performance.mjs','auth-performance-lib.mjs','auth-performance-worker.mjs','auth-performance-seed.py','auth-performance-profile.mjs','auth-performance-recovery.mjs','auth-performance-soak.mjs'}
def load(p):return json.loads(Path(p).read_text())
def digest(p):
    h=hashlib.sha256()
    with Path(p).open('rb') as f:
        for block in iter(lambda:f.read(1024*1024),b''):h.update(block)
    return h.hexdigest()
def require(ok,message):
    if not ok:raise ValueError(message)
def verify(frozen=False):
    m=load(ROOT/'manifest.json')
    require(m['trial_count']==45 and sum(j['trial_count'] for j in m['jobs'])==45,'trial plan changed')
    require(m['gate_flags']==['--require-six-pairs'],'gate flags changed')
    require(bool(m['files']),'bundle files not pinned')
    for name,sha in m['files'].items():
        p=ROOT/name;require(p.is_file() and not p.is_symlink() and digest(p)==sha,'bundle file changed: '+name)
    if frozen:
        require(m['status']=='prepared_not_executed' and not m['pending'],'PENDING candidate identity: run prohibited; finalize first')
        require(len(m['artifacts'])==4,'exactly four artifacts required')
        for name in ['primary','recovery']:
            require(digest(ROOT/'configs'/f'{name}.json')==m['config_sha256'][name],'config hash changed')
    return m
def preflight():
    m=verify(True)
    require(sys.platform.startswith('linux'),'Linux target required')
    require(ROOT==Path(m['remote_bundle']),'wrong frozen installation path')
    require(os.sysconf('SC_CLK_TCK')==m['fixture']['clock_ticks_per_second'],'host clock ticks changed')
    for name,sha in m['tools']['files'].items():
        p=Path(m['remote_tools'])/name;require(p.is_file() and not p.is_symlink() and digest(p)==sha,'scripts5 changed: '+name)
    for name,info in m['artifacts'].items():
        p=Path(name);require(p.is_file() and not p.is_symlink() and os.access(p,os.X_OK),'missing artifact: '+name)
        require(p.stat().st_size==info['bytes'] and digest(p)==info['sha256'],'artifact changed: '+name)
    c=load(ROOT/'configs/primary.json')
    for key in ['admin_static','auth_static','control_binary']:require(Path(c[key]).exists(),'missing '+key)
    require(c['control_binary']==m['control_binary']['path'] and digest(c['control_binary'])==m['control_binary']['sha256'],'control changed')
    print('Frozen 45-trial plan, tools, control and four artifacts verified; no load started.')

def contract(m,job,c,manifest,result):
    errors=[]
    def check(ok,label):
        if not ok:errors.append(label)
    o=job['options'];rows=result.get('runs',[])
    check(result.get('schema_version')==manifest.get('schema_version')==1,'schema')
    check(manifest.get('config')==c,'exact frozen config')
    expected_options={**o,'config':m['remote_bundle']+'/configs/'+job['config']+'.json','out':m['remote_batch']+'/'+job['name'],'hz':m['fixture']['clock_ticks_per_second']}
    check(manifest.get('options')==expected_options,'all options including config/output path')
    identities=manifest.get('identities',[])
    expected={(v['name'],component,v[component],m['artifacts'][v[component]]['sha256']) for v in [c['baseline'],c['candidates'][0]] for component in ['go','rust']}
    check(len(identities)==4 and {(x.get('variant'),x.get('component'),x.get('path'),x.get('sha256')) for x in identities}==expected,'four artifact identities')
    hi=manifest.get('harness_identity',{});files=hi.get('files',[])
    check(len(files)==8 and {x.get('name') for x in files}==HARNESS,'eight harness identities')
    for x in files:check(x.get('sha256')==m['tools']['files'].get(x.get('name')),'harness SHA')
    check(hi.get('control_binary_sha256')==m['control_binary']['sha256'],'control SHA')
    order=[(s,p,r) for p in range(o['pairs']) for s in o['routes'] for r in (['baseline','candidate'] if p%2==0 else ['candidate','baseline']) if r in o['roles']]
    check(len(rows)==job['trial_count'] and [(r.get('scenario'),r.get('pair'),r.get('role')) for r in rows]==order,'exact ordered trials / no missing route, pair, role or duplicate')
    check(len({r.get('directory') for r in rows})==job['trial_count'] and all(isinstance(r.get('directory'),str) for r in rows),'unique trial directories')
    seen=set();overload=[]
    numeric=lambda x:isinstance(x,(int,float)) and not isinstance(x,bool) and math.isfinite(x)
    for row in rows:
        role=row.get('role');label=f"{row.get('scenario')}/{row.get('pair')}/{role}: "
        if role not in ['baseline','candidate']:continue
        v=c['baseline'] if role=='baseline' else c['candidates'][0]
        check(row.get('candidate')=='candidate6' and row.get('variant')==v['name'],label+'variant')
        check(row.get('variant_metadata')==v['metadata'] and row.get('env')==v['env'],label+'source/build/env metadata')
        check(row.get('concurrency')==16 and row.get('cache_ttl_seconds')==o['cacheTtl'],label+'concurrency/TTL')
        check(row.get('roles')==o['roles'] and row.get('profiling') is False and row.get('recovery_probe') is o['recoveryProbe'],label+'roles/diagnostic flags')
        seed={'scenario':row.get('scenario'),'accounts':1000,'sessions':1000,'ordinary_grants':100,'total_grants':100,'credential_kind':'totp','renewal_tokens_per_phase':0}
        check(all(row.get('seed',{}).get(k)==x for k,x in seed.items()),label+'fixture scale')
        env=dict(FN_KNOCK_RUNTIME_TARGET='linux',FN_KNOCK_TOKIO_WORKER_THREADS='2',FN_KNOCK_SQLITE_AUTH_READERS='1',FN_KNOCK_AUTH_BRIDGE_MAX_IN_FLIGHT='32' if o['recoveryProbe'] else None,GLIBC_TUNABLES=None,LD_PRELOAD=None,MALLOC_ARENA_MAX=None,GOMEMLIMIT=None,GOGC=None,GOMAXPROCS=None)
        check(row.get('effective_runtime_env')==env,label+'effective runtime env')
        for field in ['cache_control','cache_control_after']:
            cc=row.get(field,{})
            check(cc.get('runtime_verified') is True and cc.get('auth_cache_ttl_seconds')==o['cacheTtl'] and cc.get('auth_cache_unauthorized_ttl_seconds')==o['cacheTtl'],label+field)
        check(row.get('validation',{}).get('passed') is True,label+'validation')
        check(all(row.get('quality',{}).get(k) is True for k in ['sampling_ok','health_ok','client_ok','duration_ok']),label+'measurement quality')
        warm=row.get('warmup',{}).get('measurement',{});loadm=row.get('measurement',{})
        check(warm.get('failures')==0 and loadm.get('failures')==0 and (warm.get('successful_requests') or 0)>0 and (loadm.get('successful_requests') or 0)>0,label+'normal warm/load correctness')
        check(numeric(warm.get('elapsed_ms')) and warm['elapsed_ms']>=o['warmup']*1000,label+'minimum warm duration')
        target=o['seconds']*1000
        check(numeric(loadm.get('elapsed_ms')) and target<=loadm['elapsed_ms']<=target+max(500,target*.05),label+'load duration tolerance')
        spec={'bootstrap':'bootstrap','challenge':'challenge','session_api':'session','session_miss':'redirect','grant_miss':'redirect','auto_ip_miss':'redirect'}.get(row.get('scenario'),'origin')
        pre=row.get('preflight',{})
        check(pre.get('expected')==spec and pre.get('status')==(302 if spec=='redirect' else 200),label+'business preflight')
        # This legacy field means any Set-Cookie, including cookie clearing on
        # session_miss, so do not require false on the entire smoke matrix.
        if row.get('scenario') in ['session_hit','auto_ip_hit']:
            check(pre.get('renewal') is False,label+'unexpected Set-Cookie')
        check(pre.get('origin')==('auth-performance-owned-origin-v1' if spec=='origin' else None),label+'origin marker')
        for component in ['go','rust']:
            res=row.get('resources',{}).get(component,{});before=res.get('before',{});after=res.get('after',{})
            identity=(component,before.get('pid'),before.get('start_ticks'))
            check(isinstance(identity[1],int) and identity[1]>0 and isinstance(identity[2],int) and identity[2]>0,label+component+' process identity')
            check(before.get('pid')==after.get('pid') and before.get('start_ticks')==after.get('start_ticks'),label+component+' stable process during load')
            check(identity not in seen,label+component+' fresh process across trials');seen.add(identity)
        if o['recoveryProbe']:
            p=row.get('recovery',{})
            for k,x in dict(burst_concurrency=64,burst_duration_ms=2000,recovery_concurrency=1,recovery_deadline_ms=10000,success_window_ms=2000,performance_claim_allowed=False,recovered=True,passed=True).items():check(p.get(k)==x,label+'recovery '+k)
            check(numeric(p.get('recovery_elapsed_ms')) and 2000<=p['recovery_elapsed_ms']<=10000,label+'recovery completed window within deadline')
            check(not p.get('burst_error'),label+'burst execution failure')
            successes=[x for x in p.get('checks',[]) if x.get('continuous_success') is True]
            check(bool(successes),label+'recovery successful check')
            for x in successes:
                q=x.get('result',{}).get('measurement',{})
                check(q.get('failures')==0 and (q.get('successful_requests') or 0)>0 and (q.get('elapsed_ms') or 0)>=2000 and (x.get('requests_finished_after_burst_ms') or math.inf)<=10000,label+'recovery check evidence')
            count=p.get('burst_503_responses');burst=p.get('burst',{})
            check(isinstance(burst.get('measurement'),dict) and isinstance(burst['measurement'].get('statuses'),dict),label+'burst measurement evidence')
            check(isinstance(count,int) and count>=0 and count==burst.get('measurement',{}).get('statuses',{}).get('503',0),label+'recorded burst 503 count')
            overload.append({'role':role,'burst_503_responses':count,'observed':isinstance(count,int) and count>0,'burst_quality':burst.get('quality'),'check_quality':[x.get('result',{}).get('quality') for x in p.get('checks',[])],'quality_note':'Reported separately; does not add thresholds beyond the frozen recovery passed protocol.'})
    return dict(schema_version=1,case=job['name'],contract_passed=not errors,expected_trials=job['trial_count'],observed_trials=len(rows),errors=errors,overload_observations=overload,notes=['Recovery time includes a complete 2-second success window and starts after burst workers drain. No new 503-count gate is added.'] if overload else [])

def main():
    action=sys.argv[1] if len(sys.argv)>1 else 'verify'
    if action=='verify':m=verify();print(m['status']);return
    if action=='preflight':preflight();return
    if action!='contract' or len(sys.argv)!=4:raise SystemExit('usage: check-plan.py verify|preflight|contract CASE OUTPUT_DIR')
    m=verify(True);name=sys.argv[2];out=Path(sys.argv[3])
    job=next(j for j in m['jobs'] if j['name']==name)
    try:
        answer=contract(m,job,load(ROOT/'configs'/f"{job['config']}.json"),load(out/'manifest.json'),load(out/'results.json'))
        answer['inputs']={n:{'path':str(out/n),'sha256':digest(out/n)} for n in ['manifest.json','results.json']}
    except (OSError,ValueError,KeyError,TypeError,AttributeError) as e:answer={'contract_passed':False,'case':name,'errors':['missing/malformed input: '+str(e)]}
    print(json.dumps(answer,indent=2));raise SystemExit(0 if answer['contract_passed'] else 1)
if __name__=='__main__':main()
