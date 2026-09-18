import os,subprocess,time,pathlib,json,signal,http.client,concurrent.futures,collections,threading,shutil,sys
ROOT=pathlib.Path('/tmp/fn-knock-ab-20260918'); TOKEN='isolated-ab-token-20260918'
PATHS=['/__auth__/api/auth/bootstrap','/__auth__/api/auth/session','/__auth__/api/auth/captcha/config','/__auth__/api/auth/challenge','/__auth__/api/auth/oidc/providers','/__auth__/']
def request(path,conn=None):
 c=conn or http.client.HTTPConnection('127.0.0.1',27999,timeout=5)
 c.request('GET',path,headers={'Host':'ab.local','Accept-Encoding':'identity'});r=c.getresponse();b=r.read();return r.status,r.getheader('X-Fn-Knock-Upstream-Error-Class'),len(b)
def stats(pid):
 try:
  raw=pathlib.Path(f'/proc/{pid}/stat').read_text().split(') ',1)[1].split(); status=pathlib.Path(f'/proc/{pid}/status').read_text(); rss=int(next(x for x in status.splitlines() if x.startswith('VmRSS:')).split()[1]);return {'cpu_ticks':int(raw[11])+int(raw[12]),'rss_kib':rss,'fds':len(list(pathlib.Path(f'/proc/{pid}/fd').iterdir()))}
 except Exception:return None
def load(seconds,workers):
 end=time.monotonic()+seconds
 def worker(i):
  conn=http.client.HTTPConnection('127.0.0.1',27999,timeout=5); times=[];counts=collections.Counter(); errors=collections.Counter();sizes=0
  while time.monotonic()<end:
   t=time.monotonic()
   try:
    s,e,n=request(PATHS[i%len(PATHS)],conn);counts[str(s)]+=1;sizes+=n
    if e:errors[e]+=1
   except Exception as e:
    errors[type(e).__name__+':'+str(e)]+=1;conn.close();conn=http.client.HTTPConnection('127.0.0.1',27999,timeout=5)
   times.append((time.monotonic()-t)*1000);i+=1
  conn.close();return times,counts,errors,sizes
 with concurrent.futures.ThreadPoolExecutor(max_workers=workers) as pool: results=list(pool.map(worker,range(workers)))
 ts=sorted(x for r in results for x in r[0]); counts=sum((r[1] for r in results),collections.Counter()); errors=sum((r[2] for r in results),collections.Counter())
 return {'workers':workers,'requests':len(ts),'statuses':dict(counts),'errors':dict(errors),'p50_ms':round(ts[int(len(ts)*.5)],2),'p99_ms':round(ts[int(len(ts)*.99)],2),'max_ms':round(max(ts),2),'bytes':sum(r[3] for r in results)}
def run(label,gv,rv,tcache,threshold):
 d=ROOT/'runs'/label;d.mkdir(parents=True,exist_ok=False)
 env=dict(os.environ,FN_KNOCK_RUNTIME_TARGET='linux',FN_KNOCK_DISABLE_REDIS_MIGRATION='1',FN_KNOCK_INTERNAL_RPC_TOKEN=TOKEN,HMAC_SECRET='isolated-ab-hmac-20260918',FN_KNOCK_DATA_DIR=str(d),FN_KNOCK_GATEWAY_CONFIG_DIR=str(d),FN_KNOCK_SQLITE_PATH=str(d/'store.sqlite3'),GO_BACKEND_GRPC_ADDR='127.0.0.1:27996',BACKEND_PORT='27998',AUTH_PORT='27997',ADMIN_VIEW_PORT='27991',ADMIN_VIEW_HOST='127.0.0.1',FN_KNOCK_AUTH_BRIDGE_MAX_IN_FLIGHT='32',ADMIN_STATIC_PATH=str(ROOT/f'v{rv}/ui/www'),AUTH_STATIC_PATH=str(ROOT/f'v{rv}/server-auth-view/dist'),FN_KNOCK_TOKIO_WORKER_THREADS='2')
 (d/'config.json').write_text(json.dumps({'gateway_listener':{'scope':'loopback'},'auth_config':{'auth_port':27997},'rules':[]}))
 ps=[];logs=[];stop=threading.Event();samples=[];sampler=None
 try:
  for name,args in [('go',[str(ROOT/(('old-as-new' if gv==2412 else 'new-as-old') if gv!=rv else f'v{gv}/server/go-reauth-proxy-linux-amd64')),'-c',str(d/'config.json'),'-admin-port','27996','-proxy-port','27999']),('rust',[str(ROOT/f'v{rv}/server/server-admin-rs')])]:
   e=env.copy()
   if name=='rust': e.update(GLIBC_TUNABLES=f'glibc.malloc.tcache_count={tcache}',LD_PRELOAD=str(ROOT/'allocator.so'),AB_MMAP_THRESHOLD=str(threshold))
   f=open(d/(name+'.log'),'w');logs.append(f);ps.append(subprocess.Popen(args,env=e,cwd=d,stdout=f,stderr=subprocess.STDOUT,start_new_session=True));time.sleep(1)
  ready=False
  for n in range(90):
   try:
    if request(PATHS[0])[0]==200:ready=True;break
   except Exception:pass
   if any(p.poll() is not None for p in ps):break
   time.sleep(.5)
  if not ready:raise RuntimeError('startup failed')
  preflight=[(p,request(p)) for p in PATHS]
  if any(r[0]!=(401 if p.endswith('/session') else 200) for p,r in preflight):raise RuntimeError(str(preflight))
  def monitor():
   while not stop.wait(.2):samples.append([stats(p.pid) for p in ps])
  sampler=threading.Thread(target=monitor);sampler.start();start=[stats(p.pid) for p in ps]
  phases=[load(20,16),load(20,64)]
  time.sleep(3); recovery=load(3,8);finish=[stats(p.pid) for p in ps]
  summary={'label':label,'go':gv,'rust':rv,'tcache':tcache,'mmap_threshold':threshold,'phases':phases,'after_idle':recovery,'process_exit':[p.poll() for p in ps],'resources':{}}
  for i,name in enumerate(['go','rust']):
   vals=[s[i] for s in samples if s[i]];summary['resources'][name]={'cpu_seconds':round((finish[i]['cpu_ticks']-start[i]['cpu_ticks'])/os.sysconf('SC_CLK_TCK'),2),'peak_rss_kib':max(x['rss_kib'] for x in vals),'peak_fds':max(x['fds'] for x in vals),'end':finish[i]}
  (d/'summary.json').write_text(json.dumps(summary,indent=2));print(json.dumps(summary),flush=True)
 except Exception as e:
  print(json.dumps({'label':label,'failed':str(e),'exit':[p.poll() for p in ps]}),flush=True);raise
 finally:
  stop.set()
  if sampler:sampler.join()
  for p in ps[::-1]:
   if p.poll() is None:os.killpg(p.pid,signal.SIGTERM)
  for p in ps:
   try:p.wait(timeout=5)
   except subprocess.TimeoutExpired:os.killpg(p.pid,signal.SIGKILL);p.wait()
  for f in logs:f.close()
  time.sleep(1)
cells=[('mixed_old_new',2412,2415,7,131072),('mixed_new_old',2415,2412,7,131072)]
for cell in cells:run(*cell)
