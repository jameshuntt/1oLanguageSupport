#!/usr/bin/env python3
from pathlib import Path
from zipfile import ZipFile
import json, hashlib, sys
root=Path(sys.argv[1]).resolve()
source=Path(__file__).resolve().parent.parent
version=json.loads((source/'package.json').read_text())['version']
import re
server_version=re.search(r'^version\s*=\s*"([^"]+)"',(source/'crates/language-server-1o/Cargo.toml').read_text(),flags=re.M).group(1)
expect={
 'darwin-arm64':('darwin-arm64','language-server-1o'),
 'darwin-x64':('darwin-x64','language-server-1o'),
 'win32-x64':('win32-x64','language-server-1o.exe'),
 'win32-arm64':('win32-arm64','language-server-1o.exe'),
 'linux-x64':('linux-x64','language-server-1o'),
 'linux-arm64':('linux-arm64','language-server-1o'),
 'alpine-x64':('linux-x64','language-server-1o'),
 'alpine-arm64':('linux-arm64','language-server-1o'),
 'linux-armhf':('linux-arm','language-server-1o'),
}
lines=[]
for target,(runtime,exe) in expect.items():
    hits=list(root.rglob(f'1o-{version}-{target}.vsix'))
    if len(hits)!=1: raise SystemExit(f'Expected one {target} VSIX, found {len(hits)}')
    f=hits[0]
    with ZipFile(f) as z:
        if z.testzip() is not None: raise SystemExit(f'corrupt VSIX: {f}')
        names=set(z.namelist())
        req={'extension/package.json','extension/dist/extension.js','extension/syntaxes/1o.tmLanguage.json',f'extension/server/{runtime}/{exe}'}
        req |= {f'extension/schemas/{p.relative_to(source/"schemas").as_posix()}' for p in (source/'schemas').rglob('*.json')}
        miss=req-names
        if miss: raise SystemExit(f'{target} missing {sorted(miss)[:10]}')
        manifest=z.read('extension.vsixmanifest').decode('utf8','replace')
        if target not in manifest: raise SystemExit(f'{target} target metadata absent')
        pkg=json.loads(z.read('extension/package.json'))
        if (pkg.get('publisher'),pkg.get('name'),pkg.get('version')) != ('1o','1o',version): raise SystemExit(f'identity mismatch {f}')
    lines.append(f'{hashlib.sha256(f.read_bytes()).hexdigest()}  {f.name}')
# Each native target must also have a standalone server archive.
for target in expect:
    globs=[f'language-server-1o-{server_version}-{target}.zip',f'language-server-1o-{server_version}-{target}.tar.gz']
    hits=[]
    for g in globs: hits += list(root.rglob(g))
    if len(hits)!=1: raise SystemExit(f'Expected one standalone server archive for {target}, found {len(hits)}')
    lines.append(f'{hashlib.sha256(hits[0].read_bytes()).hexdigest()}  {hits[0].name}')
(root/'SHA256SUMS.txt').write_text('\n'.join(sorted(lines))+'\n')
print(f'PASS {len(expect)} Marketplace VSIXs + {len(expect)} standalone server archives verified')
