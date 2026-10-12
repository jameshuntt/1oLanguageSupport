#!/usr/bin/env python3
"""Fail immediately if any platform artifact omits the expected executable or schema assets."""
from pathlib import Path
from zipfile import ZipFile
import json, sys
if len(sys.argv)!=3: sys.exit('usage: verify_single_vsix.py FILE TARGET')
p=Path(sys.argv[1]); target=sys.argv[2]
root=Path(__file__).resolve().parent.parent
runtime={'alpine-x64':'linux-x64','alpine-arm64':'linux-arm64','linux-armhf':'linux-arm'}.get(target,target)
exe='language-server-1o.exe' if target.startswith('win32-') else 'language-server-1o'
with ZipFile(p) as z:
    assert z.testzip() is None, 'corrupted VSIX'
    members=set(z.namelist())
    required={'extension/package.json','extension/dist/extension.js',
              'extension/syntaxes/1o.tmLanguage.json','extension.vsixmanifest',
              f'extension/server/{runtime}/{exe}'}
    for schema in (root/'schemas').rglob('*.json'):
        required.add('extension/schemas/'+schema.relative_to(root/'schemas').as_posix())
    missing=required-members
    assert not missing, f'{target} missing: {sorted(missing)[:10]}'
    meta=json.loads(z.read('extension/package.json'))
    assert (meta['publisher'],meta['name'],meta['version']) == ('1o','1o','0.1.3')
    assert target in z.read('extension.vsixmanifest').decode('utf8','replace'), f'wrong target platform: {target}'
    assert len(z.read(f'extension/server/{runtime}/{exe}'))>100_000, 'empty native server'
print('PASS native VSIX '+target+' (bundled Rust server, JS client, grammar, schemas, identity, platform marker)')
