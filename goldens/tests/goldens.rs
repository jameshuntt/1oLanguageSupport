//! Every 1o document that exists today reads the same in this version as in the published
//! 0.1.1, which is the oracle here (a dev-dependency, `core-1o-v0-1`), not a snapshot.
//!
//! The corpus is every `v1.<name>.1o.<modifier>` file in this repository's fixtures and
//! examples, and every one under the directories `ONEO_GOLDEN_CORPUS` lists (a path list, as
//! `PATH` is written), so a project's own documents are checked by pointing it at them:
//!
//!     ONEO_GOLDEN_CORPUS=/path/to/project/.custodian cargo test --manifest-path goldens/Cargo.toml -- --nocapture
//!
//! For each document:
//! - 0.1.1 read entries: this version reads the same entries, spans included, and its schema
//!   gives the same diagnostics.
//! - 0.1.1 read nothing because the document is a bare named record (`diff { … }`, 0.1.1 read
//!   only a top-level `{ … }`): this version reads what 0.1.1 read with the braces added.
//! - 0.1.1 refused it: counted; a refusal is not a reading to keep.

use core_1o::utils::parse_filename;
use serde_json::Value;
use std::path::{Path, PathBuf};

use core_1o_v0_1 as old;

fn corpus_roots() -> Vec<PathBuf> {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut roots = vec![
        repo.join("crates/core-1o/fixtures"),
        repo.join("crates/language-server-1o/fixtures"),
        repo.join("examples"),
    ];
    if let Some(extra) = std::env::var_os("ONEO_GOLDEN_CORPUS") {
        roots.extend(std::env::split_paths(&extra));
    }
    roots
}

fn documents(root: &Path) -> Vec<PathBuf> {
    let mut out = walkdir::WalkDir::new(root)
        .into_iter()
        .flatten()
        .map(|entry| entry.into_path())
        .filter(|path| path.is_file())
        .filter(|path| path.file_name().and_then(|n| n.to_str()).and_then(parse_filename).is_some())
        .collect::<Vec<_>>();
    out.sort();
    out
}

/// The directory a document's schemas are read from: the nearest ancestor with a `schema/`
/// folder, or with `config/schema/` (custodian's layout); else the document's own folder.
fn workspace_root(document: &Path) -> PathBuf {
    for dir in document.ancestors().skip(1) {
        if dir.join("schema").is_dir() {
            return dir.to_path_buf();
        }
        if dir.join("config/schema").is_dir() {
            return dir.join("config");
        }
    }
    document.parent().unwrap().to_path_buf()
}

fn strip_spans(value: &Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(map.iter().filter(|(k, _)| *k != "span").map(|(k, v)| (k.clone(), strip_spans(v))).collect()),
        Value::Array(items) => Value::Array(items.iter().map(strip_spans).collect()),
        other => other.clone(),
    }
}

fn new_diagnostics(root: &Path, modifier: &str, version: &str, text: &str) -> Vec<(String, usize, usize)> {
    let workspace = core_1o::engine::Workspace::new(root);
    core_1o::engine::diagnostics_for_text_workspace(&workspace, modifier, version, text)
        .into_iter()
        .map(|d| (d.message, d.span.start, d.span.end))
        .collect()
}

fn old_diagnostics(root: &Path, modifier: &str, version: &str, text: &str) -> Vec<(String, usize, usize)> {
    let workspace = old::engine::Workspace::new(root);
    old::engine::diagnostics_for_text_workspace(&workspace, modifier, version, text)
        .into_iter()
        .map(|d| (d.message, d.span.start, d.span.end))
        .collect()
}

fn messages(diagnostics: &[(String, usize, usize)]) -> Vec<&str> {
    diagnostics.iter().map(|(m, _, _)| m.as_str()).collect()
}

#[test]
fn every_existing_document_reads_the_same_as_in_0_1_1() {
    let (mut same, mut bare, mut refused) = (0, 0, 0);
    for root in corpus_roots() {
        for path in documents(&root) {
            let text = std::fs::read_to_string(&path).unwrap();
            let name = path.file_name().unwrap().to_str().unwrap();
            let record = parse_filename(name).unwrap();
            let root = workspace_root(&path);
            let shown = path.display();
            match old::ir::parse_document(&text) {
                Err(_) => refused += 1,
                Ok(was) if !was.nodes.is_empty() => {
                    let now = core_1o::ir::parse_document(&text).unwrap_or_else(|e| panic!("{shown}: 0.1.1 read it, this refuses it: {e:?}"));
                    assert_eq!(
                        serde_json::to_value(&now).unwrap(),
                        serde_json::to_value(&was).unwrap(),
                        "{shown}: the reading changed"
                    );
                    assert_eq!(
                        new_diagnostics(&root, &record.modifier, &record.version, &text),
                        old_diagnostics(&root, &record.modifier, &record.version, &text),
                        "{shown}: the schema's diagnostics changed"
                    );
                    same += 1;
                }
                Ok(_) => {
                    // 0.1.1 read nothing: a bare record, or an empty document
                    let wrapped = format!("{{\n{text}\n}}");
                    let was = old::ir::parse_document(&wrapped).unwrap_or_else(|e| panic!("{shown}: 0.1.1 refuses it wrapped too: {e:?}"));
                    let now = core_1o::ir::parse_document(&text).unwrap_or_else(|e| panic!("{shown}: {e:?}"));
                    assert_eq!(
                        strip_spans(&serde_json::to_value(&now.nodes).unwrap()),
                        strip_spans(&serde_json::to_value(&was.nodes).unwrap()),
                        "{shown}: the bare record does not read as its wrapped form"
                    );
                    assert_eq!(
                        messages(&new_diagnostics(&root, &record.modifier, &record.version, &text)),
                        messages(&old_diagnostics(&root, &record.modifier, &record.version, &wrapped)),
                        "{shown}: the bare record does not validate as its wrapped form"
                    );
                    bare += 1;
                }
            }
        }
    }
    eprintln!("goldens: {same} read the same as 0.1.1, {bare} bare records read as their wrapped form, {refused} refused by 0.1.1");
    assert!(same > 0);
}
