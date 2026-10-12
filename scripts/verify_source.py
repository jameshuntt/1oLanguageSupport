#!/usr/bin/env python3
from pathlib import Path
import json, re, sys
ROOT=Path(__file__).resolve().parent.parent
pkg=json.loads((ROOT/'package.json').read_text())
assert (pkg.get('publisher'),pkg.get('name')) == ('1o','1o')
assert pkg.get('version') == '0.1.3'
assert pkg.get('main') == './dist/extension.js'
assert pkg.get('extensionKind') == ['workspace']
langs={x['id']:x for x in pkg['contributes']['languages']}
assert {'1o','1oserver','1oclient'} <= set(langs)
assert '*.1o.*' in langs['1o'].get('filenamePatterns',[])
assert 'json' not in langs
assert 'jsonValidation' not in pkg['contributes']
for pat in ['*.1o.server','*.1o.model']:
    assert pat in langs['1oserver'].get('filenamePatterns',[])
for pat in ['*.1o.page','*.1o.component','*.1o.section']:
    assert pat in langs['1oclient'].get('filenamePatterns',[])
assert (ROOT/'syntaxes/1o.tmLanguage.json').is_file()
for icon in ['icons/app.png','icons/client.png','icons/server.png']:
    assert (ROOT/icon).is_file(), icon
schemas=list((ROOT/'schemas').rglob('*.json'))
assert len(schemas)==27, len(schemas)
for f in schemas:
    json.loads(f.read_text())
core_vocab=ROOT/'crates/core-1o/schema_vocabulary'
core_schemas=list(core_vocab.rglob('*.json'))
assert len(core_schemas)==27, len(core_schemas)
for f in schemas:
    rel=f.relative_to(ROOT/'schemas')
    assert (core_vocab/rel).read_bytes()==f.read_bytes(), 'schema drift: '+str(rel)
core=(ROOT/'crates/core-1o/Cargo.toml').read_text()
server=(ROOT/'crates/language-server-1o/Cargo.toml').read_text()
assert re.search(r'^name\s*=\s*"core-1o"',core,re.M)
assert re.search(r'^name\s*=\s*"language-server-1o"',server,re.M)
assert 'core-1o = { version = "0.1.0", path = "../core-1o" }' in server
assert 'publish = false' in core and 'publish = false' in server
lib=(ROOT/'crates/language-server-1o/src/lib.rs').read_text()
assert 'pub use core_1o' in lib
ext=(ROOT/'src/extension.ts').read_text()
assert "ONEO_SCHEMAS: context.asAbsolutePath('schemas')" in ext
assert "language-server-1o" in ext
assert "process.platform === 'linux' && process.arch === 'arm'" in ext
ws=(ROOT/'crates/core-1o/src/engine/workspace.rs').read_text()
assert 'merge_vocabulary_dir' in ws and 'ONEO_SCHEMAS' in ws and 'merge_embedded_vocabulary' in ws
assert 'EMBEDDED_VOCABULARY' in (ROOT/'crates/core-1o/src/embedded_vocabulary.rs').read_text()
assert (ROOT/'examples/page-project/schema/invoice/v1.schema.json').is_file()
workflow=(ROOT/'.github/workflows/oneo-release.yml').read_text()
for target in ['darwin-arm64','darwin-x64','win32-x64','win32-arm64','linux-x64','linux-arm64','linux-armhf','alpine-x64','alpine-arm64']:
    assert target in workflow, target
assert "branches:\n      - '1o-release-*'" in workflow
ignore=(ROOT/'.vscodeignore').read_text()
for item in ['crates/**','examples/**','scripts/**']:
    assert item in ignore
print('PASS source audit: names, grammar, icons, schemas, dynamic 1o types, bundled schema wiring, all 9 platforms')
