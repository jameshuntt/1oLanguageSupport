# Change Log

## 0.1.3

- Bundle the Rust `language-server-1o` with the existing `1o.1o` extension.
- Split reusable Rust functionality into `core-1o` and the LSP/CLI into `language-server-1o`.
- Preserve the existing TextMate grammar, icons, client/server associations and Marketplace identity.
- Recognize new `v1.<name>.1o.<type>` files without adding each type to the extension.
- Discover workspace schemas and the extension's bundled schema vocabulary.
- Fix schema completion between sibling properties.
- Add schema-aware completion, diagnostics, AST output and builder support.
- Package native extension/server builds for macOS ARM64/x64, Windows ARM64/x64, Linux ARM64/x64/ARMHF and Alpine ARM64/x64.
