# language-server-1o

The stdio LSP and CLI for 1o; uses core-1o for parsing and validation.

It publishes diagnostics as a document changes, completes keys and values from the document's
schema, and answers hover: on a key, its type, its description and its values with theirs; on an
enum value, that value's description (`enumDescriptions`). The language and the schema words are
described in core-1o's README.

Pre-release source. Do not publish before explicit approval, license review, and registry name confirmation.

License: MIT OR Apache-2.0 (verify ownership/authorization before publication).
