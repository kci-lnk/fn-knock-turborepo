import hashlib,json,mmap,shutil,sys
from pathlib import Path
root=Path(sys.argv[1]).resolve();role=sys.argv[2]
m=json.loads((root/'manifest.json').read_text());v=next(x for x in m['variants'] if x['label']==role)
assert v['status']=='complete'
before=json.loads((root/'native-target-before.json').read_text())['deps_files']
messages=[json.loads(l) for l in (root/role/'build.jsonl').read_text().splitlines()]
msg=next(x for x in messages if x.get('reason')=='compiler-artifact' and x.get('executable') and x.get('target',{}).get('name')=='server_admin_rs' and x.get('profile',{}).get('test'))
assert not msg['fresh'] and v['source'] in msg['package_id']
binary=Path(msg['executable']);copy=root/role/'server-admin-rs-sql-diagnostic.test'
def digest(p):
 h=hashlib.sha256()
 with p.open('rb') as f:
  for b in iter(lambda:f.read(1048576),b''):h.update(b)
 return h.hexdigest()
assert digest(copy)==v['binary_sha256']
proof=[]
for p in [binary,*binary.parent.glob(binary.name+'.*.o'),binary.with_suffix('.d')]:
 if not p.is_file():continue
 assert str(p) not in before, f'preexisting: {p}'
 with p.open('rb') as f,mmap.mmap(f.fileno(),0,access=mmap.ACCESS_READ) as b:at=b.find(v['source'].encode())
 assert at>=0,f'no source evidence: {p}'
 item={'path':str(p),'bytes':p.stat().st_size,'sha256':digest(p),'source_archive_path_offset':at,'absent_in_before_inventory':True,'cargo_package_id':msg['package_id']}
 if p==binary:assert item['sha256']==v['binary_sha256']
 proof.append(item)
record={'role':role,'free_bytes_before':shutil.disk_usage(binary.parent).free,'retained_binary':str(copy),'retained_binary_sha256':v['binary_sha256'],'files':proof}
log=root/f'{role}-cleanup.json';log.write_text(json.dumps(record,indent=2)+'\n')
for item in proof:
 p=Path(item['path']);assert digest(p)==item['sha256'];p.unlink()
record.update(bytes_removed=sum(x['bytes'] for x in proof),free_bytes_after=shutil.disk_usage(binary.parent).free,all_removed=all(not Path(x['path']).exists() for x in proof))
log.write_text(json.dumps(record,indent=2)+'\n')
print({k:record[k] for k in ['role','bytes_removed','free_bytes_before','free_bytes_after','all_removed']})
