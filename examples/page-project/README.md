# Example builder (not Gudgi)

Open this folder in VS Code, then run **1o: Build (Configured Builder)**.
The builder calls the Rust binary's `check` and `parse` commands. It reads canonical AST JSON, not 1o source directly. Output: `dist/home.page-ir.json`.

Note: this is an isolated example, **not** a replacement for Gudgi's current compiler. In this kit `../../target/release/language-server-1o` is the expected binary location; set `ONEO_SERVER_PATH` when moved elsewhere.
