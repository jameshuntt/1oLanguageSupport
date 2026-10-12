/**
 * 1o extension: the Rust LSP supplies syntax-aware analysis and schema-aware
 * completions; domain-specific builds are EXPLICIT tasks from oneo.project.json.
 * No workspace command executes on save, activation or startup.
 */
import { spawn, spawnSync } from 'node:child_process';
import * as path from 'node:path';
import * as fs from 'node:fs';
import * as vscode from 'vscode';
import { LanguageClient, type LanguageClientOptions, type ServerOptions } from 'vscode-languageclient/node';

let client: LanguageClient | undefined;
let activeServer: string | undefined;

function bundledRuntimeKey(): string {
  // VS Code/Node reports 32-bit ARM as `linux` + `arm`, while the
  // Marketplace target is named linux-armhf. Keep the runtime directory
  // explicit so all published native targets resolve deterministically.
  if (process.platform === 'linux' && process.arch === 'arm') return 'linux-arm';
  return `${process.platform}-${process.arch}`;
}
function bundledPath(context: vscode.ExtensionContext): string {
  const runtimeKey = bundledRuntimeKey();
  return context.asAbsolutePath(path.join('server', runtimeKey, process.platform === 'win32' ? 'language-server-1o.exe' : 'language-server-1o'));
}
function resolvedServer(context: vscode.ExtensionContext): string | undefined {
  if (!vscode.workspace.getConfiguration('1o').get<boolean>('server.enabled', true)) return undefined;
  const configured = vscode.workspace.getConfiguration('1o').get<string>('server.path', '').trim();
  const bundled = bundledPath(context);
  const candidates = configured ? [configured] : [bundled, 'language-server-1o'];
  for (const candidate of candidates) {
    // Never shell out; --version is a bounded check before starting the stdio LSP.
    const probe = spawnSync(candidate, ['--version'], { timeout: 3000, encoding: 'utf8' });
    if (!probe.error && probe.status === 0 && probe.stdout.includes('language-server-1o')) return candidate;
  }
  return undefined;
}

interface Builder { id: string; label?: string; command: string[]; }
interface ProjectManifest { formatVersion: number; schemaDirectories?: string[]; builders?: Builder[]; }
function manifestFor(folder: vscode.WorkspaceFolder): ProjectManifest | undefined {
  try {
    const fullPath = path.join(folder.uri.fsPath, 'oneo.project.json');
    const value: unknown = JSON.parse(fs.readFileSync(fullPath, 'utf8'));
    if (!value || typeof value !== 'object' || (value as ProjectManifest).formatVersion !== 1) return undefined;
    return value as ProjectManifest;
  } catch { return undefined; }
}

async function initializeProject(): Promise<void> {
  if (!vscode.workspace.isTrusted) { vscode.window.showErrorMessage('Trust this workspace before initializing a 1o project.'); return; }
  const folder = vscode.workspace.workspaceFolders?.[0];
  if (!folder) { vscode.window.showErrorMessage('Open a project folder first.'); return; }
  const manifest = path.join(folder.uri.fsPath, 'oneo.project.json');
  if (fs.existsSync(manifest)) { vscode.window.showInformationMessage('1o project manifest already exists.'); return; }
  const data = { formatVersion: 1, schemaDirectories: ['schema'], builders: [] };
  await vscode.workspace.fs.writeFile(vscode.Uri.file(manifest), Buffer.from(JSON.stringify(data, null, 2) + '\n'));
  const document = await vscode.workspace.openTextDocument(vscode.Uri.file(manifest));
  await vscode.window.showTextDocument(document);
}

async function runBuilder(output: vscode.OutputChannel): Promise<void> {
  if (!vscode.workspace.isTrusted) { vscode.window.showErrorMessage('Trust this workspace before running a builder.'); return; }
  const options = (vscode.workspace.workspaceFolders ?? []).flatMap(folder => {
    const manifest = manifestFor(folder);
    return (manifest?.builders ?? []).filter(b =>
      b && typeof b.id === 'string' && Array.isArray(b.command) && b.command.length > 0 &&
      b.command.every(part => typeof part === 'string' && part.length > 0)
    ).map(builder => ({ label: `${builder.label || builder.id} (${folder.name})`, folder, builder }));
  });
  if (options.length === 0) { vscode.window.showWarningMessage('No builder declared in oneo.project.json.'); return; }
  const selected = await vscode.window.showQuickPick(options, { placeHolder: 'Choose a 1o builder to run' });
  if (!selected) return;
  const command = selected.builder.command;
  const approval = await vscode.window.showWarningMessage(
    `Run project builder?\n${command.join(' ')}\nFolder: ${selected.folder.uri.fsPath}`,
    { modal: true }, 'Run Builder'
  );
  if (approval !== 'Run Builder') return;
  output.show(true);
  output.appendLine(`1o build: ${selected.builder.id}`);
  // Run the explicit argv of a trusted manifest; never evaluate a shell string.
  const child = spawn(command[0], command.slice(1), { cwd: selected.folder.uri.fsPath, shell: false, env: process.env });
  child.stdout?.on('data', (chunk: Buffer) => output.append(chunk.toString()));
  child.stderr?.on('data', (chunk: Buffer) => output.append(chunk.toString()));
  child.on('error', error => { output.appendLine(`ERROR: ${error.message}`); vscode.window.showErrorMessage(`1o builder failed: ${error.message}`); });
  child.on('close', code => {
    output.appendLine(`1o builder exit: ${code}`);
    if (code === 0) vscode.window.showInformationMessage('1o builder completed.');
    else vscode.window.showErrorMessage(`1o builder failed (exit ${code ?? 'unknown'}).`);
  });
}

export async function activate(context: vscode.ExtensionContext): Promise<void> {
  const output = vscode.window.createOutputChannel('1o');
  context.subscriptions.push(output);
  context.subscriptions.push(vscode.commands.registerCommand('1o.build', () => runBuilder(output)));
  context.subscriptions.push(vscode.commands.registerCommand('1o.initializeProject', initializeProject));
  context.subscriptions.push(vscode.commands.registerCommand('1o.serverStatus', () => {
    vscode.window.showInformationMessage(activeServer ? `1o language server: ${activeServer}` : '1o language server not running. Check the 1o output.');
  }));

  activeServer = resolvedServer(context);
  if (!activeServer) {
    output.appendLine('1o: no compatible server detected; syntax highlighting remains available.');
    output.appendLine('Set 1o.server.path to a built language-server-1o, or package a binary in server/<platform>-<arch>/.');
    return;
  }
  output.appendLine(`Starting 1o LSP: ${activeServer}`);
  const serverOptions: ServerOptions = {
    command: activeServer,
    args: ['--stdio'],
    options: { env: { ...process.env, ONEO_SCHEMAS: context.asAbsolutePath('schemas') } },
  };
  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ language: '1o' }, { language: '1oserver' }, { language: '1oclient' }],
    synchronize: { fileEvents: [vscode.workspace.createFileSystemWatcher('**/schema/**/*.json'), vscode.workspace.createFileSystemWatcher('**/schemas/**/*.json'), vscode.workspace.createFileSystemWatcher('**/oneo.project.json')] },
    outputChannel: output,
  };
  client = new LanguageClient('1o', '1o Language Server', serverOptions, clientOptions);
  await client.start();
  context.subscriptions.push({ dispose: () => { void client?.stop(); } });
}

export async function deactivate(): Promise<void> {
  await client?.stop();
  client = undefined;
}
