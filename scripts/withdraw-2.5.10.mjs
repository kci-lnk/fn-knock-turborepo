import { readFile, writeFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import assert from 'node:assert/strict';
import { createTencentAdapters } from './fn-knock-cos-publish.mjs';
const {store, cdn} = createTencentAdapters({secretId:process.env.COS_SECRETID,secretKey:process.env.COS_SECRETKEY,bucket:process.env.COS_BUCKET,region:process.env.COS_REGION,accelerateDomain:process.env.COS_ACC});
const current = JSON.parse((await store.get('latest.json')).body);
assert(['2.5.10','2.5.9'].includes(current.version),'Only withdraw the affected version');
const manifest = JSON.parse(await readFile('rollback/release-manifest.json','utf8'));
assert.equal(manifest.version,'2.5.9');
const latest = JSON.parse(JSON.stringify(current).replaceAll('2.5.10','2.5.9'));
const notes = await readFile('rollback/notes.md','utf8');
latest.release_notes = notes;
latest.force_update = false;
for (const group of Object.values(latest.packages)) for (const entry of Object.values(group)) {
  const artifact = manifest.artifacts.find(a=>a.name === entry.file_name);
  assert(artifact,`Missing previous artifact ${entry.file_name}`);
  entry.sha256=artifact.sha256; entry.size=artifact.size;
  const head = await store.head(entry.object_key);
  assert(head && head.size===artifact.size,`Previous object missing: ${entry.object_key}`);
  if(head.sha256) assert.equal(head.sha256,artifact.sha256);
}
latest.sha256=latest.packages.fpk.amd64.sha256;
latest.sha256_arm64=latest.packages.fpk.arm64.sha256;
const objects=[];
for(const type of ['linux','macos']) for(const [arch,entry] of Object.entries(latest.packages[type])) objects.push({key:`${type}/latest/${arch}.env`,body:Buffer.from(`VERSION=2.5.9\nURL=${entry.download_url}\nSHA256=${entry.sha256}\nSIZE=${entry.size}\n`)});
const updater=await store.get('files/2.5.9/windows/x86_64/updater.json');
assert(updater); assert.equal(JSON.parse(updater.body).version,'2.5.9');
objects.push({key:'windows/stable/latest.json',body:updater.body});
for(const key of ['install.sh','macos/install.sh']) objects.push({key,body:await readFile(`rollback/${key}`)});
objects.push({key:'latest.json',body:Buffer.from(JSON.stringify(latest,null,2)+'\n')});
await writeFile('rollback/withdrawn-latest.json',JSON.stringify(current,null,2));
for(const object of objects){
  await store.put({...object,size:object.body.length,sha256:createHash('sha256').update(object.body).digest('hex'),contentType:object.key.endsWith('.json')?'application/json':'text/plain',cacheControl:'no-cache'});
  assert((await store.get(object.key)).body.equals(object.body));
  console.log(`Restored ${object.key}`);
}
const badKeys=Object.values(current.packages).flatMap(group=>Object.values(group).map(e=>e.object_key.replaceAll('2.5.9','2.5.10')));
const windows=current.packages.windows.x86_64;
badKeys.push('files/2.5.10/windows/x86_64/'+windows.file_name.replaceAll('2.5.9','2.5.10')+'.sha256','files/2.5.10/windows/x86_64/release.json','files/2.5.10/windows/x86_64/updater.json');
for(const field of ['sha256_url','release_url','updater_url']) if(windows[field]) badKeys.push(new URL(windows[field]).pathname.slice(1));
for(const key of [...new Set(badKeys)]){assert(key.startsWith('files/2.5.10/'));await store.delete(key);assert.equal(await store.head(key),null);console.log(`Removed ${key}`);}
const bases=[...new Set([process.env.COS_PUBLICBASICURL.replace(/\/$/,''),'https://cor.fnknock.cn','https://cdn.fnknock.cn'])];
const urls=bases.flatMap(base=>[...objects.map(o=>o.key),...badKeys].map(key=>`${base}/${key}`));
await cdn.purgeAndWait([...new Set(urls)]);
for(const base of bases){const r=await fetch(`${base}/latest.json`);assert(r.ok);assert.equal((await r.json()).version,'2.5.9');console.log(`Verified ${base}/latest.json`);}
