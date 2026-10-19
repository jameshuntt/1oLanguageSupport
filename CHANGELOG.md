# Change Log

## 0.2.0

core-1o 0.2.0, language-server-1o 0.2.0 and this extension. Six things a document could not say,
found by custodian, which kept a TOML twin of each 1o document for them. Every document that
0.1.1 read entries from reads the same, spans and diagnostics included: the golden test compares
this parser with the published 0.1.1 itself over every document in the repository, and over a
project's own documents when `ONEO_GOLDEN_CORPUS` lists them. A string without an escape lexes to
the token it always did. Test: [goldens/tests/goldens.rs#L91](https://github.com/jameshuntt/1oLanguageSupport/blob/ad9e7b334ea20685f556d6947ef616bc2d0a1a2b/goldens/tests/goldens.rs#L91).

- Strings take escapes. A value could not hold a quote, and a backslash was an ordinary
  character, so a path or a message with either could not be written. Was: [crates/core-1o/src/engine/lexer.rs#L119-L139](https://github.com/jameshuntt/1oLanguageSupport/blob/9193e2ab5cbc641ea91a1f2615775884aa1354f6/crates/core-1o/src/engine/lexer.rs#L119-L139).
  Now `\"` `\\` `\n` `\t` `\r` `\u{…}`, the set task_history's diff writer emits; any other
  escape is refused where it is written, not guessed at; raw strings `r"…"` and `r#"…"#` hold
  what is written. A string with an escape is a new token that carries its end, so spans stay
  the source text: [crates/core-1o/src/engine/lexer.rs#L24-L32](https://github.com/jameshuntt/1oLanguageSupport/blob/ad9e7b334ea20685f556d6947ef616bc2d0a1a2b/crates/core-1o/src/engine/lexer.rs#L24-L32), [crates/core-1o/src/engine/lexer.rs#L177-L320](https://github.com/jameshuntt/1oLanguageSupport/blob/ad9e7b334ea20685f556d6947ef616bc2d0a1a2b/crates/core-1o/src/engine/lexer.rs#L177-L320). Tests: [crates/core-1o/tests/gaps.rs#L35](https://github.com/jameshuntt/1oLanguageSupport/blob/ad9e7b334ea20685f556d6947ef616bc2d0a1a2b/crates/core-1o/tests/gaps.rs#L35), [crates/core-1o/tests/gaps.rs#L44](https://github.com/jameshuntt/1oLanguageSupport/blob/ad9e7b334ea20685f556d6947ef616bc2d0a1a2b/crates/core-1o/tests/gaps.rs#L44),
  [crates/core-1o/tests/gaps.rs#L52](https://github.com/jameshuntt/1oLanguageSupport/blob/ad9e7b334ea20685f556d6947ef616bc2d0a1a2b/crates/core-1o/tests/gaps.rs#L52), [crates/core-1o/tests/gaps.rs#L62](https://github.com/jameshuntt/1oLanguageSupport/blob/ad9e7b334ea20685f556d6947ef616bc2d0a1a2b/crates/core-1o/tests/gaps.rs#L62), [crates/core-1o/tests/gaps.rs#L77](https://github.com/jameshuntt/1oLanguageSupport/blob/ad9e7b334ea20685f556d6947ef616bc2d0a1a2b/crates/core-1o/tests/gaps.rs#L77).
- Comments. The grammar coloured `//` and `/* */` and the comment key wrote `//`
  ([syntaxes/1o.tmLanguage.json#L16-L17](https://github.com/jameshuntt/1oLanguageSupport/blob/9193e2ab5cbc641ea91a1f2615775884aa1354f6/syntaxes/1o.tmLanguage.json#L16-L17), [language-configuration.json#L3-L4](https://github.com/jameshuntt/1oLanguageSupport/blob/9193e2ab5cbc641ea91a1f2615775884aa1354f6/language-configuration.json#L3-L4)), but the lexer skipped the slashes
  and read the words after them ([crates/core-1o/src/engine/lexer.rs#L86](https://github.com/jameshuntt/1oLanguageSupport/blob/9193e2ab5cbc641ea91a1f2615775884aa1354f6/crates/core-1o/src/engine/lexer.rs#L86), [crates/core-1o/src/engine/lexer.rs#L110-L113](https://github.com/jameshuntt/1oLanguageSupport/blob/9193e2ab5cbc641ea91a1f2615775884aa1354f6/crates/core-1o/src/engine/lexer.rs#L110-L113)). Now the lexer skips both
  forms as the editor does: [crates/core-1o/src/engine/lexer.rs#L92-L130](https://github.com/jameshuntt/1oLanguageSupport/blob/ad9e7b334ea20685f556d6947ef616bc2d0a1a2b/crates/core-1o/src/engine/lexer.rs#L92-L130). Test: [crates/core-1o/tests/gaps.rs#L89](https://github.com/jameshuntt/1oLanguageSupport/blob/ad9e7b334ea20685f556d6947ef616bc2d0a1a2b/crates/core-1o/tests/gaps.rs#L89).
- A record at the top level. The parser read only a top-level `{ … }` and stopped at anything
  else ([crates/core-1o/src/engine/parser.rs#L200-L218](https://github.com/jameshuntt/1oLanguageSupport/blob/9193e2ab5cbc641ea91a1f2615775884aa1354f6/crates/core-1o/src/engine/parser.rs#L200-L218)), so task_history's `diff { … }` records read as nothing. Now a
  document may start with its record, hold several, and mix both forms: [crates/core-1o/src/engine/parser.rs#L210-L235](https://github.com/jameshuntt/1oLanguageSupport/blob/ad9e7b334ea20685f556d6947ef616bc2d0a1a2b/crates/core-1o/src/engine/parser.rs#L210-L235).
  Tests: [crates/core-1o/tests/gaps.rs#L133](https://github.com/jameshuntt/1oLanguageSupport/blob/ad9e7b334ea20685f556d6947ef616bc2d0a1a2b/crates/core-1o/tests/gaps.rs#L133), [crates/core-1o/tests/gaps.rs#L144](https://github.com/jameshuntt/1oLanguageSupport/blob/ad9e7b334ea20685f556d6947ef616bc2d0a1a2b/crates/core-1o/tests/gaps.rs#L144); records already nested ([crates/core-1o/tests/gaps.rs#L150](https://github.com/jameshuntt/1oLanguageSupport/blob/ad9e7b334ea20685f556d6947ef616bc2d0a1a2b/crates/core-1o/tests/gaps.rs#L150)).
- Keys that repeat. `type: array` was ignored ([crates/core-1o/src/engine/json_schema_validator.rs#L59](https://github.com/jameshuntt/1oLanguageSupport/blob/9193e2ab5cbc641ea91a1f2615775884aa1354f6/crates/core-1o/src/engine/json_schema_validator.rs#L59)). Now it is a key written once per
  value, each occurrence checked against `items`, counted against `minItems`/`maxItems`; and
  `x-1o-single-keys: true` makes a block's other keys take one value, which custodian's Rust form
  had to check itself. Tests: [crates/core-1o/tests/gaps.rs#L172](https://github.com/jameshuntt/1oLanguageSupport/blob/ad9e7b334ea20685f556d6947ef616bc2d0a1a2b/crates/core-1o/tests/gaps.rs#L172), [crates/core-1o/tests/gaps.rs#L179](https://github.com/jameshuntt/1oLanguageSupport/blob/ad9e7b334ea20685f556d6947ef616bc2d0a1a2b/crates/core-1o/tests/gaps.rs#L179), [crates/core-1o/tests/gaps.rs#L192](https://github.com/jameshuntt/1oLanguageSupport/blob/ad9e7b334ea20685f556d6947ef616bc2d0a1a2b/crates/core-1o/tests/gaps.rs#L192).
- Other keys and record kinds. An `additionalProperties` schema read as `false`
  ([crates/core-1o/src/engine/json_schema_validator.rs#L95-L98](https://github.com/jameshuntt/1oLanguageSupport/blob/9193e2ab5cbc641ea91a1f2615775884aa1354f6/crates/core-1o/src/engine/json_schema_validator.rs#L95-L98)), so container `environment` and custodian's `decl` refused every key. Now
  other keys are checked against it, and `x-1o-required-when` names the keys a block needs when a
  key holds a value, task_history's record kinds: [crates/core-1o/src/engine/json_schema_validator.rs#L57-L69](https://github.com/jameshuntt/1oLanguageSupport/blob/ad9e7b334ea20685f556d6947ef616bc2d0a1a2b/crates/core-1o/src/engine/json_schema_validator.rs#L57-L69), [crates/core-1o/src/engine/json_schema_validator.rs#L107-L212](https://github.com/jameshuntt/1oLanguageSupport/blob/ad9e7b334ea20685f556d6947ef616bc2d0a1a2b/crates/core-1o/src/engine/json_schema_validator.rs#L107-L212). Tests:
  [crates/core-1o/tests/gaps.rs#L204](https://github.com/jameshuntt/1oLanguageSupport/blob/ad9e7b334ea20685f556d6947ef616bc2d0a1a2b/crates/core-1o/tests/gaps.rs#L204), [crates/core-1o/tests/gaps.rs#L215](https://github.com/jameshuntt/1oLanguageSupport/blob/ad9e7b334ea20685f556d6947ef616bc2d0a1a2b/crates/core-1o/tests/gaps.rs#L215).
- Enum values with descriptions. Completion offered a value with no word on it
  ([crates/language-server-1o/src/lsp/backend.rs#L405](https://github.com/jameshuntt/1oLanguageSupport/blob/9193e2ab5cbc641ea91a1f2615775884aa1354f6/crates/language-server-1o/src/lsp/backend.rs#L405)). Now `enumDescriptions` beside `enum` (VS Code's form) and each key's
  `description` show in completion and in hover, which the server now answers:
  [crates/core-1o/src/engine/schema_completion.rs#L27-L80](https://github.com/jameshuntt/1oLanguageSupport/blob/ad9e7b334ea20685f556d6947ef616bc2d0a1a2b/crates/core-1o/src/engine/schema_completion.rs#L27-L80), [crates/language-server-1o/src/lsp/backend.rs#L369-L410](https://github.com/jameshuntt/1oLanguageSupport/blob/ad9e7b334ea20685f556d6947ef616bc2d0a1a2b/crates/language-server-1o/src/lsp/backend.rs#L369-L410), [crates/language-server-1o/src/lsp/backend.rs#L450-L470](https://github.com/jameshuntt/1oLanguageSupport/blob/ad9e7b334ea20685f556d6947ef616bc2d0a1a2b/crates/language-server-1o/src/lsp/backend.rs#L450-L470). Tests: [crates/language-server-1o/src/lsp/backend.rs#L911](https://github.com/jameshuntt/1oLanguageSupport/blob/ad9e7b334ea20685f556d6947ef616bc2d0a1a2b/crates/language-server-1o/src/lsp/backend.rs#L911), [crates/language-server-1o/src/lsp/backend.rs#L921](https://github.com/jameshuntt/1oLanguageSupport/blob/ad9e7b334ea20685f556d6947ef616bc2d0a1a2b/crates/language-server-1o/src/lsp/backend.rs#L921),
  [crates/language-server-1o/src/lsp/backend.rs#L942](https://github.com/jameshuntt/1oLanguageSupport/blob/ad9e7b334ea20685f556d6947ef616bc2d0a1a2b/crates/language-server-1o/src/lsp/backend.rs#L942).

One rule for which schema a key reads with, `engine::child_schema`, serves the validator,
completion, hover and the semantic references. The grammar colours raw strings, the escapes and
an unknown escape: [syntaxes/1o.tmLanguage.json#L36-L52](https://github.com/jameshuntt/1oLanguageSupport/blob/ad9e7b334ea20685f556d6947ef616bc2d0a1a2b/syntaxes/1o.tmLanguage.json#L36-L52). custodian's documents written in 1o alone, with its schemas
saying all of the above: [crates/core-1o/tests/custodian.rs#L26](https://github.com/jameshuntt/1oLanguageSupport/blob/ad9e7b334ea20685f556d6947ef616bc2d0a1a2b/crates/core-1o/tests/custodian.rs#L26) and the tests after it.

The release reads its versions from the manifests: the crates from their Cargo.toml, the
extension and its VSIX names from package.json, the standalone server archives from the server's
Cargo.toml (they were written as 0.1.3, 0.1.0 and 0.1.0 in the scripts and the release workflow).
The crates.io preflight accepts a new version of a crate this repository already publishes;
before, it refused anything but a first publication or a resume. The release commit is made even
when the source is already on the default branch. `goldens/` stays out of the VSIX.

## 0.1.3

- Bundle the Rust `language-server-1o` with the existing `1o.1o` extension.
- Split reusable Rust functionality into `core-1o` and the LSP/CLI into `language-server-1o`.
- Preserve the existing TextMate grammar, icons, client/server associations and Marketplace identity.
- Recognize new `v1.<name>.1o.<type>` files without adding each type to the extension.
- Discover workspace schemas and the extension's bundled schema vocabulary.
- Fix schema completion between sibling properties.
- Add schema-aware completion, diagnostics, AST output and builder support.
- Package native extension/server builds for macOS ARM64/x64, Windows ARM64/x64, Linux ARM64/x64/ARMHF and Alpine ARM64/x64.
