#!/usr/bin/env python3
from pathlib import Path
import argparse, shutil
p=argparse.ArgumentParser(); p.add_argument('--source',required=True); p.add_argument('--repo',required=True); a=p.parse_args()
src=Path(a.source).resolve(); dst=Path(a.repo).resolve()
files=['Cargo.toml','package.json','package-lock.json','tsconfig.json','language-configuration.json','.vscodeignore','.gitignore','README.md','CHANGELOG.md','USE_IN_PROJECTS.md','LICENSE-MIT','LICENSE-APACHE','PUBLISH_1O.command','VERIFY_LOCAL.command']
dirs=['crates','examples','adapters','src','syntaxes','icons','schemas','scripts','docs']
for f in files:
    s=src/f
    if not s.is_file(): raise SystemExit('missing source file '+str(s))
    shutil.copy2(s,dst/f)
for d in dirs:
    s=src/d; t=dst/d
    if not s.is_dir(): raise SystemExit('missing source directory '+str(s))
    if t.exists(): shutil.rmtree(t)
    shutil.copytree(s,t,ignore=shutil.ignore_patterns('target','node_modules','release-stage','release-artifacts','.DS_Store'))
wf=dst/'.github/workflows'; wf.mkdir(parents=True,exist_ok=True)
shutil.copy2(src/'.github/workflows/oneo-release.yml',wf/'oneo-release.yml')
print('PASS canonical 1o source staged into existing repository without deleting unrelated tracked files')
