#!/usr/bin/env node
// Read-only checks of the installed extension and actual stdio 1o LSP.
// The example builder writes only inside ONEO_STANDALONE_KIT/examples/page-project/dist.
import { spawn, spawnSync } from 'node:child_process';
import { existsSync, readFileSync, readdirSync, statSync, writeFileSync, mkdirSync } from 'node:fs';
import { homedir } from 'node:os';
import { join, resolve, dirname } from 'node:path';
import { pathToFileURL } from 'node:url';

const kit = resolve(process.argv[2] || join(import.meta.dirname, '..'));
const expectedId = '1o.1o';
const expectedVersion = JSON.parse(readFileSync(join(kit, 'package.json'), 'utf8')).version;
const platform = `${process.platform}-${process.arch}`;
const binName = process.platform === 'win32' ? 'language-server-1o.exe' : 'language-server-1o';
const lines = [];
function pass(msg) { const l = `PASS ${msg}`; lines.push(l); console.log(l); }
function info(msg) { const l = `INFO ${msg}`; lines.push(l); console.log(l); }
function fail(msg) { throw new Error(msg); }
function expect(ok, msg) { if (!ok) fail(msg); }
function run(binary, args, opts = {}) {
  const result = spawnSync(binary, args, {encoding: 'utf8', timeout: 15_000, maxBuffer: 1024 * 1024, ...opts });
  expect(!result.error && result.status === 0, `Command failed: ${binary} ${args.join(' ')}: ${result.error?.message || result.stderr?.slice(0,500) || result.status}`);
  return result.stdout;
}
const candidateCode = [process.env.ONEO_VSCODE_CLI, 'code', '/Applications/Visual Studio Code.app/Contents/Resources/app/bin/code'].filter(Boolean);
let vscodeCli;
for (const cmd of candidateCode) {
  const r = spawnSync(cmd, ['--version'], {encoding:'utf8',timeout:12000});
  if (!r.error && r.status === 0) { vscodeCli=cmd; break; }
}
function installedExtension() {
  const paths = [];
  if (vscodeCli) {
    const r = spawnSync(vscodeCli, ['--locate-extension', expectedId], {encoding:'utf8',timeout:12000});
    if (r.status === 0) {
      const location=r.stdout.trim().split(/\r?\n/).find(x=>x.startsWith('/'));
      if (location && existsSync(join(location,'package.json'))) paths.push(location);
    }
  }
  const bases = [
    join(homedir(), '.vscode/extensions'),
    join(homedir(), '.vscode-insiders/extensions'),
    join(homedir(), '.vscode-oss/extensions'),
  ];
  for (const base of bases) {
    if (!existsSync(base)) continue;
    for (const name of readdirSync(base)) {
      if (name.startsWith(`${expectedId}-`)) paths.push(join(base,name));
    }
  }
  const candidates=[];
  for (const folder of new Set(paths)) {
    try {
      const pkg=JSON.parse(readFileSync(join(folder,'package.json'),'utf8'));
      if (`${pkg.publisher}.${pkg.name}` === expectedId) candidates.push({folder,pkg});
    } catch { /* other extension/profile */ }
  }
  const exact = candidates.filter(x=>x.pkg.version===expectedVersion).sort((a,b) => {
    try {return statSync(b.folder).mtimeMs-statSync(a.folder).mtimeMs;} catch {return 0;}
  });
  if (!exact.length) fail(`Installed ${expectedId}@${expectedVersion} not found. Installed copies: ${candidates.map(x=>x.pkg.version).join(', ') || 'none'}. Install the matching 1o VSIX first.`);
  return exact[0];
}

// Minimal, dependency-free JSON-RPC/LSP client with bounded waits, no sockets or network.
async function smokeLsp(binary, exampleDir) {
  const server = spawn(binary, ['--stdio'], {stdio: ['pipe','pipe','pipe'], cwd:exampleDir, env:{...process.env, ONEO_SCHEMAS:join(kit,'schemas')}});
  let stderr = '', buffer=Buffer.alloc(0), nextId=1;
  const responses=new Map(), waiters=[];
  server.stderr.on('data', d=>{ if(stderr.length<10000) stderr+=d.toString(); });
  const onMessage=(msg)=>{
    for(let i=0;i<waiters.length;i++) if(waiters[i].match(msg)) {
      const waiter=waiters.splice(i,1)[0]; clearTimeout(waiter.timer); waiter.resolve(msg); return;
    }
    responses.set(msg.id===undefined ? `${msg.method}:${msg.params?.uri || ''}` : `id:${msg.id}`, msg);
  };
  server.stdout.on('data', data=>{
    buffer=Buffer.concat([buffer,data]);
    while (true) {
      const separator=buffer.indexOf('\r\n\r\n');
      if(separator<0) break;
      const header=buffer.subarray(0,separator).toString('ascii');
      const match=header.match(/(?:^|\r\n)Content-Length:\s*(\d+)/i);
      if(!match) throw new Error(`Malformed LSP header: ${header}`);
      const size=Number(match[1]); if(size>8_000_000) throw new Error('Oversized LSP message');
      if(buffer.length<separator+4+size) break;
      const body=buffer.subarray(separator+4,separator+4+size);
      buffer=buffer.subarray(separator+4+size);
      onMessage(JSON.parse(body.toString('utf8')));
    }
  });
  server.on('error', error=>{stderr+=`\n${error.message}`;});
  const send=obj=>{
    const data=Buffer.from(JSON.stringify(obj));
    server.stdin.write(`Content-Length: ${data.length}\r\n\r\n`);
    server.stdin.write(data);
  };
  const wait=(match,timeout=8000)=>new Promise((resolve,reject)=>{
    for(const [key,msg] of responses.entries()) if(match(msg)) {responses.delete(key);return resolve(msg);}
    const waiter={match,resolve,reject,timer:undefined};
    waiter.timer=setTimeout(()=>{const index=waiters.indexOf(waiter);if(index>=0)waiters.splice(index,1); reject(new Error(`Timed out waiting for LSP result. Server output: ${stderr.slice(-1600)}`));},timeout);
    waiters.push(waiter);
  });
  const request=(method,params)=>{const id=nextId++;send({jsonrpc:'2.0',id,method,params});return wait(x=>x.id===id);};
  const notify=(method,params)=>send({jsonrpc:'2.0',method,params});
  const document=join(exampleDir,'v1.home.1o.page');
  const uri=pathToFileURL(document).href;
  const source=readFileSync(document,'utf8');
  const rootUri=pathToFileURL(exampleDir).href;
  try {
    const init=await request('initialize',{processId:process.pid,rootUri,workspaceFolders:[{uri:rootUri,name:'1o-example'}],capabilities:{}});
    expect(!init.error && init.result?.capabilities?.completionProvider,'LSP initialization/completion capability missing');
    pass('LSP initialized and advertised completion');
    notify('initialized',{});
    notify('textDocument/didOpen',{textDocument:{uri,languageId:'1oclient',version:1,text:source}});
    const good=await wait(x=>x.method==='textDocument/publishDiagnostics' && x.params?.uri===uri);
    expect(!good.params.diagnostics.some(d=>d.severity===1),`Valid document produced errors: ${JSON.stringify(good.params.diagnostics)}`);
    pass('Valid document receives no error diagnostics');
    const position={line:3,character:3};
    const completion=await request('textDocument/completion',{textDocument:{uri},position,context:{triggerKind:1}});
    expect(!completion.error,`Completion failed: ${JSON.stringify(completion.error)}`);
    const items=Array.isArray(completion.result)?completion.result:completion.result?.items||[];
    const labels=items.map(x=>x.label);
    expect(labels.includes('route') && labels.includes('title'),`Expected page property completions route/title, got: ${labels.join(', ')}`);
    pass('LSP returns schema-based route/title completions');
    const bad='{ page { id "bad" } }\n';
    notify('textDocument/didChange',{textDocument:{uri,version:2},contentChanges:[{text:bad}]});
    const invalid=await wait(x=>x.method==='textDocument/publishDiagnostics' && x.params?.uri===uri);
    const errors=invalid.params.diagnostics.filter(d=>d.severity===1);
    expect(errors.some(e=>/route|title/i.test(e.message)),`Expected missing required field diagnostics, got: ${JSON.stringify(errors)}`);
    pass('Unsaved invalid document reports missing required fields');
    notify('textDocument/didChange',{textDocument:{uri,version:3},contentChanges:[{text:source}]});
    const recovered=await wait(x=>x.method==='textDocument/publishDiagnostics' && x.params?.uri===uri);
    expect(!recovered.params.diagnostics.some(d=>d.severity===1),'Repaired document still reports errors');
    pass('Restoring document clears errors');
    // A completely new modifier .1o.invoice must work from a workspace schema.
    // No compiled-in invoice parser type, hardcoded extension association, or LSP change.
    const customFile=join(exampleDir,'v1.demo.1o.invoice');
    const customUri=pathToFileURL(customFile).href;
    const customText=readFileSync(customFile,'utf8');
    notify('textDocument/didOpen',{textDocument:{uri:customUri,languageId:'1o',version:1,text:customText}});
    const customGood=await wait(x=>x.method==='textDocument/publishDiagnostics' && x.params?.uri===customUri);
    expect(!customGood.params.diagnostics.some(d=>d.severity===1),`Valid new modifier failed: ${JSON.stringify(customGood.params.diagnostics)}`);
    pass('New .1o.invoice modifier loads workspace schema without server changes');
    const newComplete=await request('textDocument/completion',{textDocument:{uri:customUri},position:{line:1,character:2},context:{triggerKind:1}});
    const customItems=Array.isArray(newComplete.result)?newComplete.result:newComplete.result?.items||[];
    expect(customItems.some(x=>x.label==='recipient'),`New invoice schema completion missing recipient; got: ${customItems.map(x=>x.label).join(', ')}`);
    pass('New schema properties appear in LSP completion');
    notify('textDocument/didChange',{textDocument:{uri:customUri,version:2},contentChanges:[{text:'{ invoice { id "INV-2" recipient 42 } }\n'}]});
    const customInvalid=await wait(x=>x.method==='textDocument/publishDiagnostics' && x.params?.uri===customUri);
    expect(customInvalid.params.diagnostics.some(d=>d.severity===1 && /string/i.test(d.message)),`New schema failed to reject invalid value: ${JSON.stringify(customInvalid.params.diagnostics)}`);
    pass('New schema reports invalid typed values');
    notify('textDocument/didClose',{textDocument:{uri:customUri}});
    notify('textDocument/didClose',{textDocument:{uri}});
    const shut=await request('shutdown',null);
    expect(!shut.error,'LSP shutdown failed');
    notify('exit');
    server.stdin.end();
    return;
  } finally {
    try {server.stdin.end();}catch{}
    setTimeout(()=>{try{server.kill();}catch{}},1000).unref();
  }
}

(async()=>{
  info(`Target: ${expectedId}@${expectedVersion} on ${platform}`);
  // The exact same LSP assertions run before and after installation.
  // Preflight deliberately bypasses installed-extension version discovery.
  const preflightPath = process.env.ONEO_VERIFY_SERVER_PATH;
  let binary;
  if (preflightPath) {
    binary = resolve(preflightPath);
    expect(existsSync(binary),`Preflight language-server executable missing: ${binary}`);
    info(`Preflight compiled binary: ${binary}`);
    expect(run(binary,['--version']).includes('language-server-1o'),'Preflight server identity mismatch');
    pass('Freshly compiled server executable exists and responds');
  } else {
    const {folder}=installedExtension();
    info(`Installed at: ${folder}`);
    expect(existsSync(join(folder,'dist/extension.js')),'Installed extension missing compiled client');
    binary=join(folder,'server',platform,binName);
    expect(existsSync(binary),`Installed extension missing native ${platform} server`);
    expect(run(binary,['--version']).includes('language-server-1o'),'Native executable identity mismatch');
    pass('Installed VS Code extension and bundled Rust executable');
    if(vscodeCli) {
      const installed=run(vscodeCli,['--list-extensions','--show-versions']);
      expect(installed.split(/\r?\n/).some(x=>x.toLowerCase()===`${expectedId}@${expectedVersion}`),'VS Code CLI does not report expected extension/version');
      pass('VS Code CLI confirms installed extension version');
    } else info('VS Code CLI unavailable; extension folder verified instead');
  }
  const exampleDir=join(kit,'examples/page-project');
  expect(existsSync(join(exampleDir,'v1.home.1o.page')),'Missing example project; extract the full kit');
  await smokeLsp(binary,exampleDir);
  const checked=run(binary,['check',join(exampleDir,'v1.home.1o.page'),'--workspace',exampleDir]);
  expect(/validation passed/i.test(checked),'Schema validation did not report success');
  pass('Server CLI validates the example schema');
  const parsed=JSON.parse(run(binary,['parse',join(exampleDir,'v1.home.1o.page')]));
  expect(parsed.format==='1o.ast' && parsed.version===1,'Installed server AST contract mismatch');
  pass('Server emits canonical 1o.ast v1');
  run('node',['scripts/build.mjs'],{cwd:exampleDir,env:{...process.env,ONEO_SERVER_PATH:binary}});
  const built=JSON.parse(readFileSync(join(exampleDir,'dist/home.page-ir.json'),'utf8'));
  expect(built.sections?.length===2 && built.title==='Hello from 1o','Builder emitted unexpected PageIR');
  pass('Example builder produced PageIR with two sections');
  info('Smoke test complete.');
  console.log(`\n1O ${preflightPath ? 'PREINSTALL' : 'POSTINSTALL'} PIPELINE: PASS`);
})().catch(err=>{
  console.error(`\n1O ${process.env.ONEO_VERIFY_SERVER_PATH ? 'PREINSTALL' : 'POSTINSTALL'} PIPELINE: FAIL\n${err?.message||err}`);
  process.exitCode=1;
});
