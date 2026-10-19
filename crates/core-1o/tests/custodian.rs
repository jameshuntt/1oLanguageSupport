//! custodian's documents in 1o alone: the reading spec, the job file, the paths table and
//! task_history's diff records (`tests/custodian/`), with custodian_kit's and task_history's
//! schemas saying in the schema what their Rust forms had to check by hand: which keys repeat,
//! which take one value, which keys each record kind requires, and what each enum value means.

use core_1o::engine::{diagnostics_for_text_workspace, Lexer, NodeValue, Parser, Workspace};
use core_1o::utils::parse_filename;
use std::path::{Path, PathBuf};

fn corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/custodian")
}

fn workspace() -> Workspace {
    Workspace::new(corpus().join("config"))
}

fn errors(modifier: &str, text: &str) -> Vec<String> {
    diagnostics_for_text_workspace(&workspace(), modifier, "v1", text)
        .into_iter()
        .map(|d| d.message)
        .collect()
}

#[test]
fn every_custodian_document_validates_against_its_schema() {
    let mut seen = Vec::new();
    for entry in walkdir::WalkDir::new(corpus()).into_iter().flatten() {
        let path = entry.path();
        let Some(record) = path.file_name().and_then(|n| n.to_str()).and_then(parse_filename) else { continue };
        let text = std::fs::read_to_string(path).unwrap();
        let found = errors(&record.modifier, &text);
        assert!(found.is_empty(), "{}: {found:?}", path.display());
        seen.push(record.modifier);
    }
    seen.sort();
    seen.dedup();
    assert_eq!(seen, vec!["diff", "job", "paths", "reading"]);
}

#[test]
fn a_reading_question_keeps_its_quotes_and_its_second_line() {
    let text = std::fs::read_to_string(corpus().join("state/jobs/done/first-reading/v1.serve-metrics-use-sites.1o.reading")).unwrap();
    let nodes = Parser::new(Lexer::new(&text).tokenize()).parse().unwrap();
    let NodeValue::Block(reading) = &nodes[0].value else { panic!() };
    let question = reading.iter().find(|n| n.key == "question").unwrap();
    assert!(matches!(&question.value, NodeValue::String(q) if q == "Where is \"serve_metrics\" used now?\nIt should be one place."));
}

/// task_history's quoting (`DiffDocument::render`): `\\ \" \n \t \r` escaped.
fn quote(value: &str) -> String {
    let mut out = String::from("\"");
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[test]
fn any_string_written_with_the_five_escapes_reads_back_as_itself() {
    for value in ["", "plain", "Who reads \"NODEPLAN_*\"?", "C:\\kit\\new", "two\nlines\r\n", "\ttab \u{e9} \u{1F600}", "\\\"", "// not a comment", "/* nor this */", "}{"] {
        let text = format!("reading {{ question {} }}", quote(value));
        let nodes = Parser::new(Lexer::new(&text).tokenize()).parse().unwrap_or_else(|e| panic!("{text}: {e:?}"));
        let NodeValue::Block(reading) = &nodes[0].value else { panic!("{text}") };
        assert!(matches!(&reading[0].value, NodeValue::String(read) if read == value), "{text}: {:?}", reading[0].value);
    }
}

const TASK: &str = "schema \"nodeplan.1o.diff.v1\" task_id \"t\" revision \"base\" sequence 1";

#[test]
fn a_record_kind_refuses_a_record_missing_what_it_requires() {
    let found = errors("diff", &format!("diff {{ {TASK} record_kind \"change\" path \"src/a.rs\" operation \"add\" before_digest \"none\" }}"));
    assert_eq!(found, vec!["Missing required key 'after_digest' for record_kind 'change'".to_string()]);
    let found = errors("diff", &format!("diff {{ {TASK} record_kind \"index\" task_count 1 }}"));
    assert_eq!(found, vec!["Missing required key 'revision_count' for record_kind 'index'".to_string()]);
}

#[test]
fn a_key_that_takes_one_value_is_refused_the_second_time() {
    let found = errors("reading", "reading { name \"m\" question \"q\" kind \"reach\" kind \"use_sites\" target \"x\" }");
    assert_eq!(found, vec!["Key 'kind' is written 2 times in 'reading'; it takes one value".to_string()]);
    let found = errors("diff", &format!("diff {{ {TASK} record_kind \"intent\" intent \"a\" intent \"b\" }}"));
    assert_eq!(found, vec!["Key 'intent' is written 2 times in 'diff'; it takes one value".to_string()]);
}

#[test]
fn a_repeating_key_checks_each_value() {
    let found = errors("reading", "reading { name \"m\" question \"q\" kind \"reach\" target \"x\" scope { exclude \"a/**\" exclude 7 } }");
    assert_eq!(found, vec!["Expected string for 'exclude'".to_string()]);
    let found = errors("paths", "paths { decl { marketplace \"a/*.toml\" marketplace 7 } }");
    assert_eq!(found, vec!["Expected string for 'marketplace'".to_string()]);
    let found = errors("reading", "reading { name \"m\" question \"q\" kind \"reach\" target \"x\" declared_in { key \"binary\" } }");
    assert_eq!(found, vec!["Missing required key 'files'".to_string()]);
}

#[test]
fn the_reading_kinds_carry_custodian_kit_s_descriptions() {
    let schema = workspace().schema_for_file("reading", "v1").unwrap();
    let kind = &schema["properties"]["reading"]["properties"]["kind"];
    let described = core_1o::engine::enum_value_description(kind, &serde_json::json!("reach")).unwrap();
    assert!(described.starts_with("what a change at a crate, a file or an item touches"), "{described}");
    let shown = core_1o::engine::describe_key("kind", kind);
    assert!(shown.contains("- `use_sites`: every place a path"), "{shown}");
}
