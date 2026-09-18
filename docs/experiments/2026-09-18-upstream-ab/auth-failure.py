import sqlite3,os,subprocess,time,pathlib,json,signal,http.client,shutil
ROOT=pathlib.Path('/tmp/fn-knock-ab-20260918'); TOKEN='isolated-ab-token-20260918'
def probe(host,path):
 t=time.monotonic();c=http.client.HTTPConnection('127.0.0.1',27999,timeout=7,source_address=('198.18.0.1',0))
 try:
  c.request('GET',path,headers={'Host':host,'Accept':'text/html'});r=c.getresponse();b=r.read().decode(errors='replace')
  return {'host':host,'path':path,'status':r.status,'class':r.getheader('X-Fn-Knock-Upstream-Error-Class'),'has_20005':'20005' in b,'auth_unavailable':'Authentication Service Unavailable' in b,'seconds':round(time.monotonic()-t,3),'body_prefix':b[:100]}
 except Exception as e:return {'host':host,'path':path,'client_error':type(e).__name__+': '+str(e),'seconds':round(time.monotonic()-t,3)}
 finally:c.close()
def run(label,gv,rv):
 d=ROOT/'runs'/label;d.mkdir(parents=True,exist_ok=False)
 env=dict(os.environ,FN_KNOCK_RUNTIME_TARGET='linux',FN_KNOCK_DISABLE_REDIS_MIGRATION='1',FN_KNOCK_INTERNAL_RPC_TOKEN=TOKEN,HMAC_SECRET='isolated-ab-hmac-20260918',FN_KNOCK_DATA_DIR=str(d),FN_KNOCK_GATEWAY_CONFIG_DIR=str(d),FN_KNOCK_SQLITE_PATH=str(d/'store.sqlite3'),GO_BACKEND_GRPC_ADDR='127.0.0.1:27996',BACKEND_PORT='27998',AUTH_PORT='27997',ADMIN_VIEW_PORT='27991',ADMIN_VIEW_HOST='127.0.0.1',FN_KNOCK_AUTH_BRIDGE_MAX_IN_FLIGHT='32',ADMIN_STATIC_PATH=str(ROOT/f'v{rv}/ui/www'),AUTH_STATIC_PATH=str(ROOT/f'v{rv}/server-auth-view/dist'),FN_KNOCK_TOKIO_WORKER_THREADS='2')
 (d/'config.json').write_text(json.dumps({'gateway_listener':{'scope':'loopback'},'auth_config':{'auth_port':27997},'rules':[]}))
 source=sqlite3.connect(str(ROOT/'pilot/store.sqlite3'));db=sqlite3.connect(str(d/'store.sqlite3'));source.backup(db);source.close()
 config=json.loads(db.execute('select document_json from config_documents').fetchone()[0]);config['host_mappings']=[{'host':'rust.local','target':'http://127.0.0.1:27997','target_type':'proxy','use_auth':False,'suppress_toolbar':True},{'host':'protected.local','target':'http://127.0.0.1:28082','target_type':'proxy','use_auth':True,'suppress_toolbar':True}];config['run_type']=3;config['auto_manage_firewall']=False
 db.execute('update config_documents set document_json=?,revision=revision+1',(json.dumps(config),));db.execute('update kv_strings set value=? where key=?',(json.dumps(config),'fn_knock:config'));db.commit();db.close()
 ps=[];logs=[];results=[]
 try:
  for name,args in [('go',[str(ROOT/f'v{gv}/server/go-reauth-proxy-linux-amd64'),'-c',str(d/'config.json'),'-admin-port','27996','-proxy-port','27999']),('rust',[str(ROOT/f'v{rv}/server/server-admin-rs')])]:
   f=open(d/(name+'.log'),'w');logs.append(f);ps.append(subprocess.Popen(args,env=env,cwd=d,stdout=f,stderr=subprocess.STDOUT,start_new_session=True));time.sleep(1)
  for _ in range(60):
   if probe('ab.local','/__auth__/api/auth/bootstrap').get('status')==200:break
   time.sleep(.5)
  else:raise RuntimeError('not ready')
  def phase(name):
   rows=[probe('ab.local','/__auth__/api/auth/bootstrap'),probe('rust.local','/api/auth/bootstrap'),probe('protected.local','/payload')]
   results.append({'phase':name,'probes':rows});print(json.dumps(results[-1]),flush=True)
  phase('healthy')
  os.kill(ps[1].pid,signal.SIGSTOP);time.sleep(2);phase('rust_stopped')
  os.kill(ps[1].pid,signal.SIGCONT);time.sleep(2);phase('rust_resumed')
  os.kill(ps[1].pid,signal.SIGKILL);ps[1].wait();time.sleep(2);phase('rust_exited')
  (d/'summary.json').write_text(json.dumps({'label':label,'results':results},indent=2))
 finally:
  for p in ps[::-1]:
   if p.poll() is None:os.killpg(p.pid,signal.SIGCONT);os.killpg(p.pid,signal.SIGTERM)
  for p in ps:
   try:p.wait(timeout=5)
   except subprocess.TimeoutExpired:os.killpg(p.pid,signal.SIGKILL);p.wait()
  for f in logs:f.close()
for v in [2412,2415]:run('auth-failure-'+str(v),v,v)
