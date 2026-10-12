// Demonstration framework builder: consumes canonical Rust 1o AST JSON.
// It does NOT reimplement or replace the 1o lexer/parser.
import { spawnSync } from 'node:child_process';
import { readFileSync, mkdirSync, writeFileSync } from 'node:fs';
import { resolve, dirname, join, relative, isAbsolute, sep } from 'node:path';
const root = resolve(import.meta.dirname, '..');
const kit = resolve(root, '../..');
const binary = process.env.ONEO_SERVER_PATH || join(kit, 'target', 'release', process.platform === 'win32' ? 'language-server-1o.exe' : 'language-server-1o');
const sourceFile = join(root, 'v1.home.1o.page');
for (const args of [['check', sourceFile, '--workspace', root], ['parse', sourceFile]]) {
  const result = spawnSync(binary, args, { encoding: 'utf8', timeout: 15_000 });
  if (result.error || result.status !== 0) throw new Error(result.stderr || result.error?.message || '1o server failed');
  if (args[0] !== 'parse') continue;
  const doc = JSON.parse(result.stdout);
  if (doc.format !== '1o.ast' || doc.version !== 1) throw new Error('Unsupported 1o AST format');
  const block = (entry) => {
    if (entry?.value?.kind !== 'block') throw new Error('Expected block');
    return entry.value.value;
  };
  const prop = (nodes, name) => nodes.find(e => e.key === name)?.value?.value;
  const page = block(doc.nodes.find(e => e.key === 'page'));
  const resultPage = {
    id: prop(page, 'id'), route: prop(page, 'route'), title: prop(page, 'title'),
    sections: page.filter(e => e.key === 'section').map(e => {
      const nodes = block(e); const markdownFile = prop(nodes, 'markdown');
      const abs = resolve(root, markdownFile);
      const relativePath=relative(root,abs);
      if (!relativePath || relativePath === '..' || relativePath.startsWith('..' + sep) || isAbsolute(relativePath)) throw new Error('Markdown reference leaves project');
      return { id: prop(nodes, 'id'), markdown: readFileSync(abs, 'utf8') };
    })
  };
  const out = join(root, 'dist', 'home.page-ir.json');
  mkdirSync(dirname(out), { recursive: true });
  writeFileSync(out, JSON.stringify(resultPage, null, 2) + '\n');
  console.log(`Wrote ${out} (${resultPage.sections.length} sections)`);
}
