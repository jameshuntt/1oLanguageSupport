# core-1o

The canonical 1o parser, schema registry, workspace validation, and versioned AST projection.

## The language

A 1o document is a list of entries. An entry is a key and a value; a value is a string, an
integer, a float, `true`/`false`, or a block `{ … }` of entries. A key written several times
is kept every time, in order.

```text
// the outer braces are optional since 0.2: a document may start with its record
diff {
  schema "nodeplan.1o.diff.v1"
  record_kind "task"
  tag "custodian"            // a key written once per value
  tag "specs"
  title "Who reads \"NODEPLAN_*\"?\nA second line."
  path r"C:\kit\src"         /* a raw string keeps its backslashes */
}
```

- **Top level.** `{ … }` blocks, whose entries are the document's (the form every 1o file has
  used), and, since 0.2, entries written directly: a named record such as `diff { … }`, or a
  `key value`. Records nest: a block's entries may be blocks.
- **Strings.** `"…"`, over several lines if need be. The escapes are `\"` `\\` `\n` `\t` `\r`
  and `\u{…}` (1 to 6 hex digits), the set task_history's diff writer emits. Any other escape is
  refused where it is written. A raw string, `r"…"` or `r#"…"#` with as many `#` as its text
  needs, holds what is written, backslashes and quotes included.
- **Comments.** `// …` to the end of the line, and `/* … */` (not nested). These are the forms
  the editor grammar and the language configuration have always declared, so the parser now
  agrees with what the editor colours and what its comment key writes. No tool reads them.

Every document 0.1.1 read entries from is read the same, spans included, and validates the
same; a bare record, which 0.1.1 read as no entries, reads as its wrapped form. The golden test
(`goldens/`) proves it against the published 0.1.1 over every document in the repository, and
over any directory listed in `ONEO_GOLDEN_CORPUS`. A string without an escape lexes to the same
token it always did.

## Schemas

A modifier's schema is JSON Schema, of which the validator reads a small subset:

| word | what it does |
|---|---|
| `type` | `object` (a block), `string`, `integer`, `number`, `boolean`, and `array`: a key written once per value, each occurrence checked against `items` |
| `properties`, `required` | a block's keys, and the ones it must have |
| `additionalProperties` | `false` (the default) refuses other keys, `true` admits them, a schema checks each of them against it |
| `minItems`, `maxItems` | how many times an `array` key is written |
| `enum` | the values a key may hold; `enumDescriptions`, entry for entry beside it (the form VS Code's JSON schemas use), says what each means |
| `description` | shown by the language server's completion and hover |
| `x-1o-required-when` | `{ "<key>": { "<value>": ["<key>", …] } }`: the keys a block needs when `<key>` holds `<value>`, as each task_history record kind needs its own fields |
| `x-1o-single-keys` | `true`: in this block a key whose schema is not an `array` is written once. Without it a key may repeat, as every key always could |
| `x-1o-semantic-ref`, `x-1o-workspace-ref` | a value that names something in NodePlan or in the workspace |

`engine::child_schema` is the one rule for which schema a key reads with (`properties`, then
`additionalProperties`, an array's `items`); the validator, completion, hover and the semantic
references all use it.

Pre-release source. Do not publish before explicit approval, license review, and registry name confirmation.

License: MIT OR Apache-2.0 (verify ownership/authorization before publication).
