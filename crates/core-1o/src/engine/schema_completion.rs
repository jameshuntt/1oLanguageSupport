// engine/schema_completion.rs

use lsp_types::CompletionItem;
use serde_json::Value;

pub fn completions_for_schema(schema: &Value) -> Vec<CompletionItem> {
    let mut items = vec![];

    if let Some(props) = schema.get("properties").and_then(|v| v.as_object()) {
        for (key, subschema) in props {
            let item = CompletionItem {
                label: key.clone(),
                detail: Some(schema_type_string(subschema)),
                kind: Some(lsp_types::CompletionItemKind::FIELD),
                ..Default::default()
            };
            items.push(item);
        }
    }

    items
}

fn schema_type_string(schema: &Value) -> String {
    schema
        .get("type")
        .and_then(|t| t.as_str())
        .unwrap_or("unknown")
        .to_string()
}