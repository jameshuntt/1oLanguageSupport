//! The six things custodian could not say in 1o, each as a document that says it.
//! Every test here uses only the API 0.1.1 already had (the lexer, the parser,
//! `ir::parse_document` and `validate_ast`), so the same file ran against 0.1.1
//! and failed there, gap by gap.

use core_1o::engine::{validate_ast, EngineDiagnostic, Lexer, Node, NodeValue, Parser};
use core_1o::ir::parse_document;
use serde_json::{json, Value};

fn parse(text: &str) -> Vec<Node> {
    Parser::new(Lexer::new(text).tokenize())
        .parse()
        .unwrap_or_else(|e| panic!("{text}\ndid not parse: {e:?}"))
}

fn string_at<'a>(nodes: &'a [Node], path: &[&str]) -> &'a str {
    let (first, rest) = path.split_first().unwrap();
    let node = nodes.iter().find(|n| n.key == *first).unwrap_or_else(|| panic!("no `{first}`"));
    match (&node.value, rest.is_empty()) {
        (NodeValue::String(s), true) => s,
        (NodeValue::Block(children), false) => string_at(children, rest),
        (other, _) => panic!("`{first}` is {other:?}"),
    }
}

fn messages(nodes: &[Node], schema: &Value) -> Vec<String> {
    let mut diagnostics: Vec<EngineDiagnostic> = Vec::new();
    validate_ast(nodes, schema, &mut diagnostics);
    diagnostics.into_iter().map(|d| d.message).collect()
}

// 1. String escapes, and raw strings.

#[test]
fn a_string_holds_a_quote_a_backslash_a_newline_a_tab_and_a_code_point_through_escapes() {
    let nodes = parse(r#"{ reading { question "Who reads \"NODEPLAN_*\"?\nC:\\kit\tdone \u{e9}\u{1F600}" } }"#);
    assert_eq!(
        string_at(&nodes, &["reading", "question"]),
        "Who reads \"NODEPLAN_*\"?\nC:\\kit\tdone \u{e9}\u{1F600}"
    );
}

#[test]
fn a_raw_string_holds_backslashes_and_quotes_as_written() {
    let nodes = parse(r###"{ a r"C:\kit\n" b r#"say "hi" \t"# c r##"a "# inside"## }"###);
    assert_eq!(string_at(&nodes, &["a"]), r"C:\kit\n");
    assert_eq!(string_at(&nodes, &["b"]), r#"say "hi" \t"#);
    assert_eq!(string_at(&nodes, &["c"]), r##"a "# inside"##);
}

#[test]
fn an_escaped_string_ends_where_its_closing_quote_is() {
    // the node's span is the source text it covers, not the length of its value
    let text = r#"{ a "x\"y\\z" b 1 }"#;
    let nodes = parse(text);
    let a = &nodes[0];
    assert_eq!(&text[a.span.start..a.span.end], r#"a "x\"y\\z""#);
    assert_eq!(&text[nodes[1].span.start..nodes[1].span.end], "b 1");
}

#[test]
fn an_unknown_escape_is_refused_where_it_is_written() {
    let text = r#"{ a "x\qy" }"#;
    let error = Parser::new(Lexer::new(text).tokenize()).parse().expect_err("\\q is not an escape");
    let diagnostic = EngineDiagnostic::from(&error);
    assert!(diagnostic.message.contains("escape"), "{}", diagnostic.message);
    assert_eq!(&text[diagnostic.span.start..diagnostic.span.end], r"\q");
}

#[test]
fn a_key_named_r_is_still_a_key() {
    let nodes = parse(r#"{ r "x" rr 1 }"#);
    assert_eq!(string_at(&nodes, &["r"]), "x");
}

#[test]
fn a_string_or_comment_never_closed_is_refused_where_it_opens() {
    for (text, opens) in [(r#"{ a "never closed }"#, 4), ("{ a 1 /* never closed }", 6), (r##"{ a r#"never closed" }"##, 4)] {
        let error = Parser::new(Lexer::new(text).tokenize()).parse().expect_err(text);
        let diagnostic = EngineDiagnostic::from(&error);
        assert!(diagnostic.message.contains("never closed"), "{text}: {}", diagnostic.message);
        assert_eq!(diagnostic.span.start, opens, "{text}");
    }
}

// 2. Comments.

#[test]
fn line_and_block_comments_are_read_by_no_one() {
    let with = "// the reading\n{ /* one */ reading { name \"m\" // trailing\n question \"a // not a comment /* nor this */\" } }\n";
    let without = "{ reading { name \"m\" question \"a // not a comment /* nor this */\" } }";
    let a = serde_json::to_value(parse_document(with).unwrap().nodes.iter().map(|n| strip(&serde_json::to_value(n).unwrap())).collect::<Vec<_>>()).unwrap();
    let b = serde_json::to_value(parse_document(without).unwrap().nodes.iter().map(|n| strip(&serde_json::to_value(n).unwrap())).collect::<Vec<_>>()).unwrap();
    assert_eq!(a, b);
    assert_eq!(string_at(&parse(with), &["reading", "question"]), "a // not a comment /* nor this */");
}

/// The IR with its spans taken out: what a document says, not where.
fn strip(value: &Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(map.iter().filter(|(k, _)| *k != "span").map(|(k, v)| (k.clone(), strip(v))).collect()),
        Value::Array(items) => Value::Array(items.iter().map(strip).collect()),
        other => other.clone(),
    }
}

// 3. A named record at the top level.

fn diff_schema() -> Value {
    json!({
        "type": "object",
        "required": ["diff"],
        "properties": {
            "diff": {
                "type": "object",
                "additionalProperties": true,
                "required": ["schema", "record_kind", "task_id", "revision", "sequence"],
                "properties": {
                    "schema": { "type": "string", "enum": ["nodeplan.1o.diff.v1"] },
                    "record_kind": { "type": "string" },
                    "task_id": { "type": "string" },
                    "revision": { "type": "string" },
                    "sequence": { "type": "integer" }
                }
            }
        }
    })
}

const BARE_DIFF: &str = "diff {\n  schema \"nodeplan.1o.diff.v1\"\n  record_kind \"intent\"\n  task_id \"t\"\n  revision \"base\"\n  sequence 2\n  intent \"read the specs\"\n}\n";

#[test]
fn a_record_written_without_the_outer_braces_is_the_same_record() {
    let bare = parse(BARE_DIFF);
    assert_eq!(bare.len(), 1, "{bare:?}");
    assert_eq!(string_at(&bare, &["diff", "intent"]), "read the specs");
    assert!(messages(&bare, &diff_schema()).is_empty(), "{:?}", messages(&bare, &diff_schema()));
    let wrapped = parse_document(&format!("{{{BARE_DIFF}}}")).unwrap();
    let bare_ir = parse_document(BARE_DIFF).unwrap();
    assert_eq!(strip(&serde_json::to_value(&bare_ir.nodes).unwrap()), strip(&serde_json::to_value(&wrapped.nodes).unwrap()));
}

#[test]
fn several_named_records_sit_at_the_top_level() {
    let nodes = parse("task { title \"t\" }\nchange { path \"src/a.rs\" }\nnote \"top-level key\"\n");
    assert_eq!(nodes.iter().map(|n| n.key.as_str()).collect::<Vec<_>>(), vec!["task", "change", "note"]);
}

#[test]
fn named_records_nest_and_repeat_inside_a_record() {
    // 0.1.1 already read this; it is here so the premise stays proven
    let nodes = parse("{ reading { declared_in { files \"decl:marketplace\" key \"binary\" } declared_in { files \"decl:slices\" key \"env.*\" } } }");
    let NodeValue::Block(children) = &nodes[0].value else { panic!() };
    assert_eq!(children.iter().filter(|n| n.key == "declared_in").count(), 2);
}

// 4. A key that repeats: `type: array`.

fn tags_schema(extra: Value) -> Value {
    let mut task = json!({
        "type": "object",
        "properties": {
            "title": { "type": "string" },
            "tag": { "type": "array", "items": { "type": "string" } }
        }
    });
    task.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    json!({ "type": "object", "properties": { "task": task } })
}

#[test]
fn every_occurrence_of_a_repeating_key_is_checked_against_its_items() {
    let nodes = parse("{ task { tag \"custodian\" tag 7 tag \"specs\" } }");
    let found = messages(&nodes, &tags_schema(json!({})));
    assert_eq!(found, vec!["Expected string for 'tag'".to_string()], "{found:?}");
}

#[test]
fn a_repeating_key_is_counted_against_min_items_and_max_items() {
    let schema = |min: u64, max: u64| {
        json!({ "type": "object", "properties": { "task": { "type": "object", "properties": {
            "tag": { "type": "array", "items": { "type": "string" }, "minItems": min, "maxItems": max }
        } } } })
    };
    let three = parse("{ task { tag \"a\" tag \"b\" tag \"c\" } }");
    assert!(messages(&three, &schema(1, 2)).iter().any(|m| m.contains("'tag'") && m.contains("at most 2")), "{:?}", messages(&three, &schema(1, 2)));
    assert!(messages(&three, &schema(4, 9)).iter().any(|m| m.contains("'tag'") && m.contains("at least 4")), "{:?}", messages(&three, &schema(4, 9)));
    assert!(messages(&three, &schema(1, 3)).is_empty());
}

#[test]
fn a_block_can_say_its_other_keys_take_one_value() {
    let nodes = parse("{ task { title \"a\" title \"b\" tag \"x\" tag \"y\" } }");
    let found = messages(&nodes, &tags_schema(json!({ "x-1o-single-keys": true })));
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].contains("'title'") && found[0].contains("2 times"), "{found:?}");
    // without the word, a key may repeat as it always could
    assert!(messages(&nodes, &tags_schema(json!({}))).is_empty());
}

// 5. A schema for additional keys, and keys a kind requires.

#[test]
fn additional_keys_are_checked_against_the_additional_properties_schema() {
    let schema = json!({ "type": "object", "properties": { "paths": { "type": "object", "additionalProperties": false, "properties": {
        "decl": { "type": "object", "additionalProperties": { "type": "array", "items": { "type": "string" } } }
    } } } });
    let good = parse("{ paths { decl { marketplace \"marketplace/*.toml\" slices \"a/*.toml\" slices \"b/*.toml\" } } }");
    assert!(messages(&good, &schema).is_empty(), "{:?}", messages(&good, &schema));
    let bad = parse("{ paths { decl { marketplace 7 } } }");
    assert_eq!(messages(&bad, &schema), vec!["Expected string for 'marketplace'".to_string()]);
}

#[test]
fn a_record_kind_names_the_keys_it_requires() {
    let mut schema = diff_schema();
    schema["properties"]["diff"]["x-1o-required-when"] = json!({
        "record_kind": {
            "task": ["title", "parent_milestone", "status"],
            "intent": ["intent"]
        }
    });
    let intent = parse(BARE_DIFF);
    assert!(messages(&intent, &schema).is_empty(), "{:?}", messages(&intent, &schema));
    let task = parse("diff { schema \"nodeplan.1o.diff.v1\" record_kind \"task\" task_id \"t\" revision \"base\" sequence 1 title \"t\" }");
    let found = messages(&task, &schema);
    assert_eq!(
        found,
        vec![
            "Missing required key 'parent_milestone' for record_kind 'task'".to_string(),
            "Missing required key 'status' for record_kind 'task'".to_string(),
        ]
    );
}

// 6. Enum values with descriptions: the validator still reads `enum` alone.

#[test]
fn enum_descriptions_change_nothing_the_validator_decides() {
    let schema = json!({ "type": "object", "properties": { "reading": { "type": "object", "properties": {
        "kind": { "type": "string", "enum": ["use_sites", "reach"], "enumDescriptions": ["where a name is used", "what a change reaches"] }
    } } } });
    assert!(messages(&parse("{ reading { kind \"reach\" } }"), &schema).is_empty());
    assert_eq!(
        messages(&parse("{ reading { kind \"nope\" } }"), &schema),
        vec!["Value for 'kind' is not one of the allowed enum values".to_string()]
    );
}
