#!/usr/bin/env node
import { spawnSync } from 'node:child_process';
import { resolve } from 'node:path';
const root=resolve(import.meta.dirname,'..');
const exe=process.platform==='win32'?'language-server-1o.exe':'language-server-1o';
const binary=resolve(root,'target','release',exe);
const env={...process.env,ONEO_VERIFY_SERVER_PATH:binary,ONEO_SCHEMAS:resolve(root,'schemas')};
let r=spawnSync(process.execPath,[resolve(root,'scripts/verify_1o_installed.mjs'),root],{cwd:root,encoding:'utf8',stdio:'inherit',env,timeout:120000});
if(r.error||r.status!==0) process.exit(r.status||1);
const good=resolve(root,'examples/bundled-schema/v1.good.1o.program');
r=spawnSync(binary,['check',good,'--workspace',resolve(root,'examples/bundled-schema')],{cwd:root,encoding:'utf8',env});
if(r.error||r.status!==0){console.error(r.stderr||r.error);throw new Error('bundled program schema rejected valid 1o');}
const bad=resolve(root,'examples/bundled-schema/v1.bad.1o.program');
r=spawnSync(binary,['check',bad,'--workspace',resolve(root,'examples/bundled-schema')],{cwd:root,encoding:'utf8',env});
if(r.status===0) throw new Error('bundled program schema was not applied: invalid program unexpectedly passed');
console.log('PASS extension-bundled schema vocabulary is actually consumed by language-server-1o');
