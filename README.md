# 1o

Canonical source for the `1o.1o` VS Code extension and its Rust implementation.

## Packages

- `core-1o` — reusable Rust parser, AST, schema discovery, completion/validation core.
- `language-server-1o` — LSP/CLI program built on `core-1o`.
- `1o.1o` — the existing VS Code Marketplace extension. It keeps the TextMate grammar, icons and file associations, and bundles the correct `language-server-1o` binary for each platform.

Authored files are `v1.<name>.1o.<type>`. JSON is used only for schema definitions. New `.1o.<type>` files are recognized by the generic 1o association; project schemas live at `schema/<type>/v1.schema.json`. Workspace schemas override the schemas bundled with the extension.

`core-1o` embeds the same 27-schema vocabulary used by the extension. The extension also passes its bundled `schemas/` directory through `ONEO_SCHEMAS`; project-local schemas still override shipped defaults. Applications, the standalone server and VS Code therefore start from the same schema set.

## Using 1o

For editor support, install the `1o` extension once. Projects do not carry their own language-server binary.

For Rust applications that must parse or validate 1o themselves:

```toml
[dependencies]
core-1o = "0.1"
```

For a standalone LSP/CLI outside VS Code:

```bash
cargo install language-server-1o
```

Linux/Windows/macOS server archives are also attached to the GitHub release.

## Release

`PUBLISH_1O.command` is the single release entry point. It stages the canonical source into `jameshuntt/1oLanguageSupport`, runs the all-platform GitHub build, verifies every artifact, asks once for `PUBLISH`, then updates the GitHub repository, publishes the two Rust crates, publishes all platform-specific Marketplace variants, and creates a GitHub Release with the binaries and VSIX files.

Every push also runs `.github/workflows/ci.yml`: the tests on stable and Rust 1.85, and clippy. It never publishes.
