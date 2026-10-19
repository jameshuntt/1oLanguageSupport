// engine/schema_completion.rs

use lsp_types::{CompletionItem, Documentation};
use serde_json::Value;

use crate::engine::json_schema_validator::is_repeating;

pub fn completions_for_schema(schema: &Value) -> Vec<CompletionItem> {
    let mut items = vec![];

    if let Some(props) = schema.get("properties").and_then(|v| v.as_object()) {
        for (key, subschema) in props {
            let item = CompletionItem {
                label: key.clone(),
                detail: Some(schema_type_string(subschema)),
                documentation: key_description(subschema).map(|text| Documentation::String(text.to_string())),
                kind: Some(lsp_types::CompletionItemKind::FIELD),
                ..Default::default()
            };
            items.push(item);
        }
    }

    items
}

/// A key's type as completion and hover show it: `string`, or `string (repeats)` for a key
/// written once per value.
pub fn schema_type_string(schema: &Value) -> String {
    let type_of = |schema: &Value| {
        schema
            .get("type")
            .and_then(|t| t.as_str())
            .unwrap_or("unknown")
            .to_string()
    };
    if is_repeating(schema) {
        let items = schema.get("items").map(type_of).unwrap_or_else(|| "unknown".into());
        format!("{items} (repeats)")
    } else {
        type_of(schema)
    }
}

/// A key's `description`; for a repeating key without one, its items' description.
pub fn key_description(schema: &Value) -> Option<&str> {
    schema
        .get("description")
        .or_else(|| schema.get("items").and_then(|items| items.get("description")))
        .and_then(Value::as_str)
}

/// The description of one enum value: `enumDescriptions` beside `enum`, entry for entry (the
/// form VS Code's JSON schemas use), read from the key's schema or, for a repeating key, its items.
pub fn enum_value_description<'a>(schema: &'a Value, value: &Value) -> Option<&'a str> {
    let schema = if is_repeating(schema) { schema.get("items")? } else { schema };
    let index = schema.get("enum")?.as_array()?.iter().position(|allowed| allowed == value)?;
    schema.get("enumDescriptions")?.as_array()?.get(index)?.as_str()
}

/// A key as hover shows it, in Markdown: its type, its description, and its values with theirs.
pub fn describe_key(key: &str, schema: &Value) -> String {
    let mut out = format!("`{key}` · {}", schema_type_string(schema));
    if let Some(description) = key_description(schema) {
        out.push_str("\n\n");
        out.push_str(description);
    }
    let values_schema = if is_repeating(schema) { schema.get("items").unwrap_or(schema) } else { schema };
    if let Some(values) = values_schema.get("enum").and_then(Value::as_array) {
        out.push('\n');
        for value in values {
            let shown = value.as_str().map(str::to_string).unwrap_or_else(|| value.to_string());
            match enum_value_description(values_schema, value) {
                Some(description) => out.push_str(&format!("\n- `{shown}`: {description}")),
                None => out.push_str(&format!("\n- `{shown}`")),
            }
        }
    }
    out
}
