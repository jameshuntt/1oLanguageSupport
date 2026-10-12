# Using 1o after publication

## You only want VS Code to understand `.1o.*`
Install the `1o` extension once. Do not copy `language-server-1o` into each project.

A project can simply contain:

    my-project/
      v1.home.1o.page
      v1.billing.1o.invoice
      schema/
        invoice/
          v1.schema.json

The extension starts its bundled language server. The server sees `.1o.invoice`, loads `schema/invoice/v1.schema.json`, and provides completion/validation.

## Your application itself parses/validates 1o
Depend on the reusable Rust library:

    core-1o = "0.1"

That is application functionality, not editor functionality.

## You need an LSP/CLI on a Linux server or another editor
Install `language-server-1o` with Cargo or use the matching standalone binary archive from the GitHub Release.
