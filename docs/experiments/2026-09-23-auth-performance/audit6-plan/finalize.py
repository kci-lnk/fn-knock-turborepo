#!/usr/bin/env python3
"""Local-only identity finalization. Never builds, uploads or starts a service."""
import argparse, copy, datetime, hashlib, json, re, shutil
from pathlib import Path

ROOT = Path(__file__).resolve().parent
def load(p): return json.loads(Path(p).read_text())
def sha(p):
    h=hashlib.sha256()
    with Path(p).open('rb') as f:
        for block in iter(lambda:f.read(1024*1024),b''): h.update(block)
    return h.hexdigest()
def write(p,v): p.write_text(json.dumps(v,indent=2)+'\n')
def require(ok,message):
    if not ok: raise ValueError(message)
def linux_amd64(binary):
    with Path(binary).open('rb') as stream: header=stream.read(20)
    require(header[:6]==b'\x7fELF\x02\x01' and len(header)==20 and int.from_bytes(header[18:20],'little')==62,'artifact must be ELF64 little-endian x86-64')

def go_check(build,binary,commit,expected_sha):
    linux_amd64(binary)
    require(build['source_commit']==commit,'Go source mismatch')
    require(sha(binary)==expected_sha==build['sha256'],'Go artifact hash mismatch')
    require(Path(binary).stat().st_size==build['size_bytes'],'Go artifact size mismatch')
    require(Path(build['binary']).resolve()==Path(binary).resolve(),'Go manifest binary path mismatch')
    argv=build['command_argv']
    flags='-s -w -X go-reauth-proxy/pkg/version.Version=2.4.15 -X go-reauth-proxy/pkg/version.Commit='+commit
    require(argv[:2]==['go','build'] and '-trimpath' in argv and '-buildvcs=false' in argv,'Go build flags mismatch')
    require(argv[argv.index('-ldflags')+1]==flags,'Go link flags mismatch')
    require(argv[-1]=='./cmd/server','Go target mismatch')
    if 'environment' in build:
        require(all(build['environment'].get(k)==v for k,v in {'CGO_ENABLED':'0','GOOS':'linux','GOARCH':'amd64'}.items()),'Go target environment mismatch')
    return flags

def rust_check(build,binary,source,gateway,expected_sha):
    linux_amd64(binary)
    require(build['status']=='complete' and build['source_commit']==source,'Rust source/status mismatch')
    require(build['gateway_commit']==gateway,'Rust embedded gateway commit mismatch')
    variants=[v for v in build['variants'] if v['opt_level']=='z']
    require(len(variants)==1,'exactly one Rust z artifact required')
    variant=variants[0]
    require(variant['status']=='complete' and variant['exit_code']==0,'Rust z build incomplete')
    require(sha(binary)==expected_sha==variant['sha256'],'Rust artifact hash mismatch')
    require(Path(binary).stat().st_size==variant['size_bytes'],'Rust artifact size mismatch')
    require(Path(variant['artifact']).resolve()==Path(binary).resolve(),'Rust manifest artifact path mismatch')
    env=variant['environment']; argv=build['command_argv']
    for key,value in {'FN_KNOCK_GATEWAY_COMMIT':gateway,'CARGO_PROFILE_RELEASE_OPT_LEVEL':'z','CARGO_PROFILE_RELEASE_LTO':'fat','CARGO_PROFILE_RELEASE_CODEGEN_UNITS':'1'}.items():
        require(env.get(key)==value,'Rust environment mismatch: '+key)
    require(argv[:2]==['cargo','zigbuild'] and '--locked' in argv and '--release' in argv,'Rust build flags mismatch')
    require(argv[argv.index('--target')+1]=='x86_64-unknown-linux-gnu','Rust target mismatch')
    require(argv[argv.index('--bin')+1]=='server-admin-rs','Rust binary target mismatch')

def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--identity',required=True,type=Path)
    p.add_argument('--out',required=True,type=Path,help='new local ready bundle directory; never overwritten')
    args=p.parse_args()
    require(not args.out.exists(),'output already exists; refusing overwrite')
    m=load(ROOT/'manifest.json'); data=load(args.identity)
    require(m['status']=='template_pending_identities_do_not_execute','input is not the pending template')
    for name,expected in m['files'].items(): require(sha(ROOT/name)==expected,'template changed: '+name)
    commit=data['go_commit']; require(re.fullmatch('[0-9a-f]{40}',commit) is not None,'candidate Go commit not finalized')
    require(commit!=m['product_sources']['baseline']['go'],'candidate Go source must contain the fix')
    for key in ['go_sha256','rust_sha256']: require(re.fullmatch('[0-9a-f]{64}',data[key]) is not None,key+' not finalized')
    for key in ['go_build_manifest','rust_build_manifest','go_binary','rust_binary']:
        require(Path(data[key]).is_absolute() and Path(data[key]).is_file(),key+' must name an existing local file')
    gb=load(data['go_build_manifest']); rb=load(data['rust_build_manifest'])
    bg=load(m['baseline_build_inputs']['go']); br=load(m['baseline_build_inputs']['rust'])
    source=m['product_sources']['baseline']['rust']; oldgo=m['product_sources']['baseline']['go']
    baseline=load(ROOT/'configs/primary.json')['baseline']
    for component,build in [('go',bg),('rust',br)]:
        a=m['artifacts'][baseline[component]]
        if component=='go': go_check(build,a['local_source'],oldgo,a['sha256'])
        else: rust_check(build,a['local_source'],source,oldgo,a['sha256'])
    flags=go_check(gb,data['go_binary'],commit,data['go_sha256'])
    rust_check(rb,data['rust_binary'],source,commit,data['rust_sha256'])
    require(gb['toolchain']==bg['toolchain'],'Go toolchain changed')
    require(rb['toolchain']==br['toolchain'],'Rust toolchain changed')
    require(rb['cargo_lock_sha256']==br['cargo_lock_sha256'],'Rust Cargo.lock changed')
    # Real content hashes bind copied build evidence; source-to-binary attribution
    # still rests on the recorded reproducible build commands/manifests.
    shutil.copytree(ROOT,args.out,ignore=shutil.ignore_patterns('__pycache__','*.pyc'))
    evidence=args.out/'build-evidence'; evidence.mkdir()
    for name,path in [('baseline-go.json',m['baseline_build_inputs']['go']),('baseline-rust.json',m['baseline_build_inputs']['rust']),('candidate-go.json',data['go_build_manifest']),('candidate-rust.json',data['rust_build_manifest'])]:
        shutil.copyfile(path,evidence/name)
    write(evidence/'candidate-input.json',data)
    for name in ['primary','recovery']:
        c=load(args.out/'configs'/f'{name}.json'); v=c['candidates'][0]
        v['metadata']['go_commit']=commit;v['metadata']['embedded_gateway_commit']=commit;v['metadata']['go_ldflags']=flags
        for variant,g,r,gn,rn in [(c['baseline'],bg,br,'baseline-go.json','baseline-rust.json'),(v,gb,rb,'candidate-go.json','candidate-rust.json')]:
            variant['metadata'].update(go_build_manifest_sha256=sha(evidence/gn),rust_build_manifest_sha256=sha(evidence/rn),go_archive_sha256=g['archive_sha256'],cargo_lock_sha256=r['cargo_lock_sha256'],go_build_environment_recorded=g.get('environment'),artifact_format_verified='ELF64 little-endian x86-64')
        write(args.out/'configs'/f'{name}.json',c)
        if name=='primary':
            for component in ['go','rust']:
                binary=Path(data[component+'_binary'])
                m['artifacts'][v[component]]={'sha256':data[component+'_sha256'],'bytes':binary.stat().st_size,'local_source':str(binary)}
    m.update(status='prepared_not_executed',pending=[],frozen_at_utc=datetime.datetime.now(datetime.timezone.utc).isoformat())
    m['product_sources']['candidate']['go']=commit
    m['build_evidence']={name:sha(path) for name,path in [(str(p.relative_to(args.out)),p) for p in sorted(evidence.iterdir())]}
    m['config_sha256']={name:sha(args.out/'configs'/f'{name}.json') for name in ['primary','recovery']}
    m['files']={str(p.relative_to(args.out)):sha(p) for p in sorted(args.out.rglob('*')) if p.is_file() and p.name!='manifest.json'}
    write(args.out/'manifest.json',m)
    print(json.dumps({'status':m['status'],'bundle':str(args.out),'manifest_sha256':sha(args.out/'manifest.json'),'trials':45,'executed':False},indent=2))

if __name__=='__main__': main()
