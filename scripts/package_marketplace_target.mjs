#!/usr/bin/env node
import { cpSync, existsSync, mkdirSync, readFileSync, rmSync, chmodSync, writeFileSync, statSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';

const raw=process.argv.slice(2); const args={};
for(let i=0;i<raw.length;i++) if(raw[i].startsWith('--')) args[raw[i].slice(2)]=raw[++i];
const root=resolve(import.meta.dirname,'..');
const {target,runtime}=args; const binary=args.binary?resolve(args.binary):''; const output=args.out?resolve(args.out):'';
if(!target||!runtime||!binary||!output) throw new Error('usage: --target --runtime --binary --out');
if(!existsSync(binary) || !statSync(binary).isFile() || statSync(binary).size < 100_000) throw new Error(`missing or empty server binary: ${binary}`);
if(!existsSync(resolve(root, 'dist/extension.js'))) throw new Error('Compiled extension bundle missing; build first');
const pkg=JSON.parse(readFileSync(resolve(root,'package.json'),'utf8'));
if(`${pkg.publisher}.${pkg.name}`!=='1o.1o') throw new Error('wrong Marketplace identity');
const stage=resolve(root,'.release-package-stage',target);
rmSync(stage,{recursive:true,force:true}); mkdirSync(stage,{recursive:true});
const topFiles=['package.json','README.md','CHANGELOG.md','LICENSE-MIT','LICENSE-APACHE','language-configuration.json','.vscodeignore'];
for(const f of topFiles) cpSync(resolve(root,f),resolve(stage,f));
for(const d of ['dist','icons','syntaxes','schemas']) cpSync(resolve(root,d),resolve(stage,d),{recursive:true});
// @vscode/vsce executes vscode:prepublish by default. This minimal staging copy
// deliberately excludes TypeScript sources and node_modules because the client
// was already compiled and tested. Remove only that build hook from the staged
// manifest to avoid rebuilding (and failing) on every native CI runner.
const stagedManifest = resolve(stage,'package.json');
const staged = JSON.parse(readFileSync(stagedManifest,'utf8'));
if(staged.scripts) delete staged.scripts['vscode:prepublish'];
writeFileSync(stagedManifest, JSON.stringify(staged, null, 2) + '\n');

const exe=target.startsWith('win32-')?'language-server-1o.exe':'language-server-1o';
const serverDest=resolve(stage,'server',runtime,exe); mkdirSync(dirname(serverDest),{recursive:true}); cpSync(binary,serverDest);
if(!target.startsWith('win32-')) chmodSync(serverDest,0o755);
mkdirSync(dirname(output),{recursive:true});
const npx=process.platform==='win32'?'npx.cmd':'npx';
const r=spawnSync(npx,['--yes','@vscode/vsce','package','--target',target,'--no-dependencies','--out',output],{
  cwd:stage,stdio:'inherit',timeout:180000,
  // Node 22+ on Windows cannot execute .cmd shims directly without a shell.
  shell: process.platform === 'win32',
});
if(r.error||r.status!==0) throw new Error(`vsce package failed for ${target}: ${r.error?.message||r.status}`);
console.log(`PACKAGED ${target}: ${output}`);
