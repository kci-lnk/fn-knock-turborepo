import os,subprocess,time,pathlib,urllib.request,json,signal
root=pathlib.Path('/tmp/fn-knock-ab-20260918'); d=root/'pilot'; d.mkdir(exist_ok=True)
env=dict(os.environ,FN_KNOCK_RUNTIME_TARGET='linux',FN_KNOCK_DISABLE_REDIS_MIGRATION='1',FN_KNOCK_INTERNAL_RPC_TOKEN='isolated-ab-token-20260918',HMAC_SECRET='isolated-ab-hmac-20260918',FN_KNOCK_DATA_DIR=str(d),FN_KNOCK_GATEWAY_CONFIG_DIR=str(d),FN_KNOCK_SQLITE_PATH=str(d/'store.sqlite3'),GO_BACKEND_GRPC_ADDR='127.0.0.1:27996',BACKEND_PORT='27998',AUTH_PORT='27997',ADMIN_VIEW_PORT='27991',ADMIN_VIEW_HOST='127.0.0.1',ADMIN_STATIC_PATH=str(root/'v2415/ui/www'),AUTH_STATIC_PATH=str(root/'v2415/server-auth-view/dist'))
(d/'config.json').write_text(json.dumps({'gateway_listener':{'scope':'loopback'},'auth_config':{'auth_port':27997},'rules':[]}))
procs=[]
try:
 for name,args in [('go',[str(root/'v2415/server/go-reauth-proxy-linux-amd64'),'-c',str(d/'config.json'),'-admin-port','27996','-proxy-port','27999']),('rust',[str(root/'v2415/server/server-admin-rs')])]:
  procs.append(subprocess.Popen(args,env=env,cwd=d,stdout=open(d/(name+'.log'),'w'),stderr=subprocess.STDOUT,start_new_session=True));time.sleep(2)
 for n in range(40):
  if (d/'store.sqlite3').exists(): pass
  try:
   with urllib.request.urlopen('http://127.0.0.1:27997/api/auth/bootstrap',timeout=1) as r: print('ready',r.status,len(r.read()),flush=True)
   break
  except Exception: time.sleep(.5)
 for port,path in [(27997,'/api/auth/bootstrap'),(27999,'/__auth__/api/auth/bootstrap'),(27999,'/auth/api/auth/bootstrap'),(27997,'/api/auth/session')]:
  try:
   with urllib.request.urlopen(f'http://127.0.0.1:{port}{path}',timeout=3) as r: print(port,path,r.status,r.read()[:120])
  except Exception as e: print(port,path,str(e))
 print('pids',[(p.pid,p.poll()) for p in procs],flush=True)
finally:
 for p in procs[::-1]:
  if p.poll() is None: os.killpg(p.pid,signal.SIGTERM)
 for p in procs:
  try:p.wait(timeout=5)
  except subprocess.TimeoutExpired:os.killpg(p.pid,signal.SIGKILL);p.wait()
