#!/usr/bin/env python3
from pathlib import Path
import argparse, tarfile, zipfile, hashlib, shutil, os
p=argparse.ArgumentParser()
p.add_argument('--binary',required=True);p.add_argument('--platform',required=True);p.add_argument('--outdir',required=True)
a=p.parse_args(); b=Path(a.binary).resolve(); out=Path(a.outdir).resolve(); out.mkdir(parents=True,exist_ok=True)
if not b.is_file(): raise SystemExit('missing binary '+str(b))
import re
manifest=(Path(__file__).resolve().parent.parent/'crates/language-server-1o/Cargo.toml').read_text()
server_version=re.search(r'^version\s*=\s*"([^"]+)"',manifest,flags=re.M).group(1)
base=f'language-server-1o-{server_version}-{a.platform}'
readme=(f'1o language server {server_version} ({a.platform})\n\nRun with --stdio for LSP, --version, check <file> --workspace <dir>, or parse <file>.\n').encode()
if a.platform.startswith('win32-'):
    f=out/(base+'.zip')
    with zipfile.ZipFile(f,'w',zipfile.ZIP_DEFLATED) as z:
        z.write(b,'language-server-1o.exe'); z.writestr('README.txt',readme)
else:
    f=out/(base+'.tar.gz')
    tmp=out/'.archive-readme.txt'; tmp.write_bytes(readme)
    with tarfile.open(f,'w:gz') as t:
        t.add(b,arcname='language-server-1o'); t.add(tmp,arcname='README.txt')
    tmp.unlink()
print(f)
print(hashlib.sha256(f.read_bytes()).hexdigest())
