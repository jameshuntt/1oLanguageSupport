#!/usr/bin/env python3
from pathlib import Path
import argparse, json, os, shutil, subprocess, sys, time, urllib.request, urllib.error
ROOT=Path(__file__).resolve().parent.parent
STAGE=ROOT/'release-stage'; REPORT=ROOT/'CRATES_RELEASE_RESULT.txt'
def manifest_version(name):
    import re
    text=(ROOT/'crates'/name/'Cargo.toml').read_text(encoding='utf8')
    return re.search(r'^version\s*=\s*"([^"]+)"',text,flags=re.M).group(1)
# the versions released are the manifests' own
ITEMS=[(n,manifest_version(n)) for n in ('core-1o','language-server-1o')]
VERSION=dict(ITEMS)
EXPECTED_REPO='github.com/jameshuntt/1oLanguageSupport'
class Stop(Exception): pass
def log(s):
    print(s,flush=True)
    with REPORT.open('a',encoding='utf8') as f:f.write(s+'\n')
def run(argv,cwd=ROOT):
    log('$ '+' '.join(argv)); subprocess.run(argv,cwd=cwd,check=True)
def registry(name,version):
    req=urllib.request.Request('https://crates.io/api/v1/crates/'+name,headers={'User-Agent':'1o-release/0.1'})
    try:
        with urllib.request.urlopen(req,timeout=20) as r: body=json.load(r)
    except urllib.error.HTTPError as e:
        if e.code==404:return 'available'
        raise Stop(f'crates.io lookup failed for {name}: HTTP {e.code}')
    except Exception as e: raise Stop(f'crates.io lookup failed for {name}: {e}')
    crate=body.get('crate') or {}; versions=body.get('versions') or []
    exact=any(str(v.get('num'))==version and not v.get('yanked',False) for v in versions)
    repo=str(crate.get('repository') or '')
    ours=EXPECTED_REPO.lower() in repo.lower()
    if exact:
        if ours: return 'already-ours'
        raise Stop(f'{name} {version} already exists but is not identifiable as this repository; stopping.')
    # a new version of a crate this repository already publishes
    if ours: return 'available'
    raise Stop(f'Crate name {name} already exists and {version} is not ours; choose/review the name before publishing.')
def wait(name,version):
    for _ in range(30):
        try:
            if registry(name,version)=='already-ours': return
        except Stop as e:
            if 'already exists and' in str(e): pass
        time.sleep(4)
    raise Stop(f'{name} {version} was submitted but did not become visible; inspect crates.io before retrying.')
def preflight():
    for x in ['cargo','python3']:
        if not shutil.which(x): raise Stop(x+' is missing')
    run(['cargo','test','--workspace','--lib'])
    run(['cargo','build','--release','-p','language-server-1o'])
    run(['python3','scripts/stage_crates.py'])
    run(['cargo','package','--manifest-path',str(STAGE/'Cargo.toml'),'-p','core-1o','--allow-dirty'])
    states={}
    for n,v in ITEMS:
        states[n]=registry(n,v); log(f'INFO {n} {v}: {states[n]}')
    return states
def main():
    ap=argparse.ArgumentParser(); ap.add_argument('--verify-only',action='store_true'); ap.add_argument('--publish-confirmed',action='store_true'); a=ap.parse_args()
    REPORT.write_text('1o crates release\n',encoding='utf8')
    try:
        states=preflight()
        if a.verify_only:
            log('PASS crates.io preflight; nothing published')
            return 0
        if not a.publish_confirmed or os.environ.get('ONEO_MASTER_RELEASE_CONFIRMED')!='YES': raise Stop('publication requires the master release confirmation')
        core_state=states['core-1o']
        if core_state!='already-ours':
            run(['cargo','publish','--manifest-path',str(STAGE/'Cargo.toml'),'-p','core-1o','--allow-dirty'])
            wait('core-1o',VERSION['core-1o']); log(f"PASS core-1o {VERSION['core-1o']} published")
        else: log(f"PASS core-1o {VERSION['core-1o']} already published from this repository; resume accepted")
        # Re-stage after registry visibility so Cargo verifies the path+version dependency against crates.io.
        run(['python3','scripts/stage_crates.py'])
        if states['language-server-1o']!='already-ours':
            run(['cargo','publish','--manifest-path',str(STAGE/'Cargo.toml'),'-p','language-server-1o','--allow-dirty'])
            wait('language-server-1o',VERSION['language-server-1o']); log(f"PASS language-server-1o {VERSION['language-server-1o']} published")
        else: log(f"PASS language-server-1o {VERSION['language-server-1o']} already published from this repository; resume accepted")
        return 0
    except (Stop,subprocess.CalledProcessError) as e:
        log('STOP '+str(e)); return 1
if __name__=='__main__':sys.exit(main())
