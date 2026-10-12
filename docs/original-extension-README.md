# 1o Language Support

VS Code support for the 1o language and its schema'd document vocabulary.

- **Highlighting** for the `.1o` source families (`.1o.page`, `.1o.slice`,
  `.1o.controller`, …) across the app/server/client language ids.
- **Schema validation** for `*.1o.json` documents (page, section, component,
  application state/slice/action/effect/service, daemon_harness, walkthrough,
  runtime_connection) against the vendored 1o schema vocabulary in
  `schemas/` — the kernel's `crates/schemas` is the source of truth
  (`scripts/sync-schemas.sh` refreshes the copy).
- **Language server**: a thin LSP client over `language-server-1o` (stdio).
  Build it from the kernel (`cargo build --release -p language-server-1o`)
  and point `1o.server.path` at the binary — or put it on PATH. Without the
  binary the extension degrades gracefully to highlighting + validation.

## Develop

```bash
npm install
npm run compile      # bundle src/extension.ts -> dist/extension.js
npm run package      # vsce package -> .vsix
```

Launch the extension host with F5 (see `.vscode/launch.json`).
