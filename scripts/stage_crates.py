#!/usr/bin/env python3
"""Stage a registry-shaped source tree. NO publishing, credentials, or network.
Do not edit the verified development sources; staged manifests are independently reviewable.
"""
from pathlib import Path
import shutil
import sys
import re

ROOT = Path(__file__).resolve().parent.parent
STAGE = ROOT/'release-stage'
assert (ROOT/'Cargo.toml').is_file()
if STAGE.exists(): shutil.rmtree(STAGE)
STAGE.mkdir(parents=True)
shutil.copy2(ROOT/'Cargo.toml', STAGE/'Cargo.toml')
for license_name in ['LICENSE-MIT', 'LICENSE-APACHE']:
    shutil.copy2(ROOT/license_name, STAGE/license_name)
for name in ['core-1o','language-server-1o']:
    source=ROOT/'crates'/name
    dest=STAGE/'crates'/name
    shutil.copytree(source,dest,ignore=shutil.ignore_patterns('target','*.vsix','.DS_Store'))
    manifest=dest/'Cargo.toml'
    text=manifest.read_text()
    if not re.search(r'^publish = false\b',text,flags=re.M):
        raise SystemExit('Expected explicit publish=false safety lock: '+str(manifest))
    text=re.sub(r'^publish = false.*$', '# publish=false removed ONLY IN STAGING; review before any publish',text,flags=re.M)
    if name=='language-server-1o' and not re.search(r'core-1o\s*=\s*\{[^}]*version\s*=\s*"0\.1\.0"[^}]*path',text):
        raise SystemExit('Server needs path+version core-1o dependency')
    manifest.write_text(text)
    for license_name in ['LICENSE-MIT', 'LICENSE-APACHE']:
        if not (dest/license_name).is_file(): raise SystemExit(f'Missing {license_name} in {name}')
print('Release-source staging: PASS')
print('Review staged manifests under:',STAGE)
print('No crates have been published. Original source manifests remain publish=false.')
