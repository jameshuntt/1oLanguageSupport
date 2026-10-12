#!/usr/bin/env python3
from pathlib import Path
from zipfile import ZipFile
import argparse, json, re, subprocess, sys, time
EXPECTED={'darwin-arm64','darwin-x64','win32-x64','win32-arm64','linux-x64','linux-arm64','linux-armhf','alpine-x64','alpine-arm64'}

def show():
    r=subprocess.run(['npx','--yes','@vscode/vsce','show','1o.1o','--json'],capture_output=True,text=True,timeout=120)
    if r.returncode!=0: raise RuntimeError(r.stderr.strip() or 'Marketplace metadata lookup failed')
    return json.loads(r.stdout)

def targets_for(data,version):
    out=set(); universal=False
    for v in data.get('versions') or []:
        if str(v.get('version'))!=version: continue
        t=v.get('targetPlatform')
        if t: out.add(str(t))
        else: universal=True
    return out,universal

def target_of(vsix):
    with ZipFile(vsix) as z:
        s=z.read('extension.vsixmanifest').decode('utf8','replace')
    m=re.search(r'TargetPlatform="([^"]+)"',s)
    if not m: raise RuntimeError('VSIX has no TargetPlatform: '+str(vsix))
    return m.group(1)

def main():
    ap=argparse.ArgumentParser(); ap.add_argument('--version',required=True); ap.add_argument('--dir',required=True); ap.add_argument('--verify-only',action='store_true'); a=ap.parse_args()
    root=Path(a.dir).resolve(); files=list(root.rglob(f'1o-{a.version}-*.vsix'))
    by={target_of(f):f for f in files}
    if set(by)!=EXPECTED: raise RuntimeError(f'Expected targets {sorted(EXPECTED)}, found {sorted(by)}')
    existing,universal=targets_for(show(),a.version)
    if universal: raise RuntimeError(f'Marketplace already has a universal 1o.1o {a.version}; platform-specific release would conflict')
    unknown=existing-EXPECTED
    if unknown: raise RuntimeError('Marketplace has unexpected targets for this version: '+', '.join(sorted(unknown)))
    print('Marketplace existing targets for this version:', ', '.join(sorted(existing)) or 'none')
    if a.verify_only: return 0
    for target in sorted(EXPECTED):
        if target in existing:
            print('PASS already published:',target); continue
        f=by[target]
        ok=False
        for attempt in range(1,4):
            print(f'Publishing {target} (attempt {attempt}/3): {f.name}',flush=True)
            r=subprocess.run(['npx','--yes','@vscode/vsce','publish','--packagePath',str(f)],timeout=300)
            time.sleep(4)
            try: now,universal=targets_for(show(),a.version)
            except Exception as e:
                if attempt==3: raise
                print('INFO Marketplace verification retry:',e); time.sleep(5); continue
            if target in now:
                print('PASS Marketplace confirmed:',target); existing=now; ok=True; break
            if r.returncode==0:
                print('INFO publish command succeeded but target not visible yet; retrying verification')
            else:
                print('INFO publish command failed and target is not visible yet; retrying')
            time.sleep(6)
        if not ok: raise RuntimeError('Marketplace did not confirm '+target+' after retries')
    final,universal=targets_for(show(),a.version)
    if final != EXPECTED: raise RuntimeError('Marketplace final target set incomplete: '+', '.join(sorted(final)))
    print('PASS Marketplace 1o.1o '+a.version+' published for all 9 targets')
    return 0
if __name__=='__main__':
    try: sys.exit(main())
    except Exception as e:
        print('STOP '+str(e),file=sys.stderr); sys.exit(1)
