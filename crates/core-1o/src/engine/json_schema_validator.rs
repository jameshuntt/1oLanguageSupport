use serde_json::Value;

use crate::engine::{EngineDiagnostic, Node, NodeValue, Span};

/// Validate the parsed top-level 1o nodes against the root object schema.
/// This intentionally implements only the small subset used by 1o:
/// object/string/number/integer/boolean, required keys, unknown-key rejection,
/// and simple scalar enum membership; since 0.2 also `type: array` (a key written
/// once per value), `minItems`/`maxItems`, a schema for `additionalProperties`, and
/// two 1o words, `x-1o-required-when` and `x-1o-single-keys` (see [`validate_entries`]).
/// It is not a complete JSON Schema engine.
pub fn validate_ast(
    nodes: &[Node],
    schema: &Value,
    errors: &mut Vec<EngineDiagnostic>,
) {
    let Some("object") = schema.get("type").and_then(Value::as_str) else {
        return;
    };
    let root_span = nodes
        .first()
        .zip(nodes.last())
        .map(|(first, last)| Span { start: first.span.start, end: last.span.end })
        .unwrap_or(Span { start: 0, end: 0 });
    validate_entries(nodes, schema, None, root_span, errors);
}

pub fn validate_node(node: &Node, schema: &Value, errors: &mut Vec<EngineDiagnostic>) {
    match schema.get("type").and_then(Value::as_str) {
        Some("object") => validate_object(node, schema, errors),
        Some("string") => validate_primitive(node, schema, "string", errors),
        Some("number") => validate_primitive(node, schema, "number", errors),
        Some("integer") => validate_primitive(node, schema, "integer", errors),
        Some("boolean") => validate_primitive(node, schema, "boolean", errors),
        // one occurrence of a repeating key: checked against the items
        Some("array") => {
            if let Some(items) = schema.get("items") {
                validate_node(node, items, errors);
            }
        }
        _ => {}
    }
}

/// What a block's schema says about one of its keys.
#[derive(Debug, Clone, Copy)]
pub enum ChildSchema<'a> {
    /// Declared in `properties`, or covered by an `additionalProperties` schema: the schema each
    /// occurrence of the key is checked against (an array's `items`, when the key repeats).
    Declared(&'a Value),
    /// Allowed without a schema: `additionalProperties: true`, or an array without `items`.
    Any,
    /// Not allowed: neither declared nor admitted by `additionalProperties`.
    Unknown,
}

/// The schema a key of a block reads with. One rule for the validator, completion, hover and
/// semantic references: `properties` first, then `additionalProperties` (a schema, or `true`);
/// a key whose schema is `type: array` is written once per value, each occurrence read with `items`.
pub fn child_schema<'a>(object_schema: &'a Value, key: &str) -> ChildSchema<'a> {
    match declared_schema(object_schema, key) {
        Some(declared) => match occurrence_schema(declared) {
            Some(schema) => ChildSchema::Declared(schema),
            None => ChildSchema::Any,
        },
        None if object_schema.get("additionalProperties").and_then(Value::as_bool) == Some(true) => ChildSchema::Any,
        None => ChildSchema::Unknown,
    }
}

/// The schema written for `key` in a block: its `properties` entry, else an `additionalProperties`
/// schema. Before an array is unwrapped to its items.
pub fn declared_schema<'a>(object_schema: &'a Value, key: &str) -> Option<&'a Value> {
    object_schema
        .get("properties")
        .and_then(Value::as_object)
        .and_then(|properties| properties.get(key))
        .or_else(|| object_schema.get("additionalProperties").filter(|value| value.is_object()))
}

/// Whether a schema says its key repeats.
pub fn is_repeating(schema: &Value) -> bool {
    schema.get("type").and_then(Value::as_str) == Some("array")
}

fn occurrence_schema(mut schema: &Value) -> Option<&Value> {
    while is_repeating(schema) {
        schema = schema.get("items")?;
    }
    Some(schema)
}

fn validate_object(node: &Node, schema: &Value, errors: &mut Vec<EngineDiagnostic>) {
    let children = match &node.value {
        NodeValue::Block(nodes) => nodes,
        _ => {
            errors.push(EngineDiagnostic::error(
                format!("Expected block for '{}'", node.key),
                node.span,
            ));
            return;
        }
    };
    validate_entries(children, schema, Some(&node.key), node.span, errors);
}

/// One block's entries against its object schema; the top level is the block with no name.
///
/// - `required`: each key written at least once.
/// - `x-1o-required-when`: `{ "<key>": { "<value>": ["<required key>", …] } }`, the keys a block
///   needs when `<key>` holds `<value>`: task_history's record kinds, each with its own fields.
/// - `type: array` with `minItems`/`maxItems`: how many times a repeating key is written.
/// - `x-1o-single-keys: true`: a key whose schema is not an array is written once. Without it a
///   key may repeat, as every key could before 0.2.
/// - every entry: checked against [`child_schema`], or refused as unknown.
fn validate_entries(
    children: &[Node],
    schema: &Value,
    owner: Option<&str>,
    span: Span,
    errors: &mut Vec<EngineDiagnostic>,
) {
    let required = schema
        .get("required")
        .and_then(Value::as_array)
        .map(|values| values.iter().filter_map(Value::as_str).collect::<Vec<_>>())
        .unwrap_or_default();
    for key in required {
        if !children.iter().any(|child| child.key == key) {
            errors.push(EngineDiagnostic::error(
                format!("Missing required key '{key}'"),
                span,
            ));
        }
    }

    if let Some(by_key) = schema.get("x-1o-required-when").and_then(Value::as_object) {
        for (discriminator, by_value) in by_key {
            for child in children.iter().filter(|child| &child.key == discriminator) {
                let Some(value) = scalar_text(&child.value) else { continue };
                let Some(keys) = by_value.get(&value).and_then(Value::as_array) else { continue };
                for key in keys.iter().filter_map(Value::as_str) {
                    if !children.iter().any(|other| other.key == key) {
                        errors.push(EngineDiagnostic::error(
                            format!("Missing required key '{key}' for {discriminator} '{value}'"),
                            span,
                        ));
                    }
                }
            }
        }
    }

    let single_keys = schema.get("x-1o-single-keys").and_then(Value::as_bool) == Some(true);
    let place = owner.map(|key| format!("'{key}'")).unwrap_or_else(|| "the document".into());
    let mut counted: Vec<&str> = Vec::new();
    for child in children {
        if counted.contains(&child.key.as_str()) {
            continue;
        }
        counted.push(&child.key);
        let occurrences = children.iter().filter(|other| other.key == child.key).collect::<Vec<_>>();
        let times = occurrences.len() as u64;
        let Some(declared) = declared_schema(schema, &child.key) else { continue };
        if is_repeating(declared) {
            let bound = |word: &str| declared.get(word).and_then(Value::as_u64);
            if let Some(min) = bound("minItems").filter(|min| times < *min) {
                errors.push(EngineDiagnostic::error(
                    format!("Key '{}' is written {times} times in {place}; it needs at least {min}", child.key),
                    child.span,
                ));
            }
            if let Some(max) = bound("maxItems").filter(|max| times > *max) {
                errors.push(EngineDiagnostic::error(
                    format!("Key '{}' is written {times} times in {place}; it takes at most {max}", child.key),
                    occurrences[max as usize].span,
                ));
            }
        } else if single_keys && times > 1 {
            errors.push(EngineDiagnostic::error(
                format!("Key '{}' is written {times} times in {place}; it takes one value", child.key),
                occurrences[1].span,
            ));
        }
    }
    // minItems on a key never written: the count is 0
    if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
        for (key, declared) in properties {
            let min = declared.get("minItems").and_then(Value::as_u64).unwrap_or(0);
            if is_repeating(declared) && min > 0 && !children.iter().any(|child| &child.key == key) {
                errors.push(EngineDiagnostic::error(
                    format!("Key '{key}' is written 0 times in {place}; it needs at least {min}"),
                    span,
                ));
            }
        }
    }

    for child in children {
        match child_schema(schema, &child.key) {
            ChildSchema::Declared(child_schema) => validate_node(child, child_schema, errors),
            ChildSchema::Any => {}
            ChildSchema::Unknown => errors.push(EngineDiagnostic::error(
                match owner {
                    Some(owner) => format!("Unknown key '{}' in object '{owner}'", child.key),
                    None => format!("Unknown top-level key '{}'", child.key),
                },
                child.span,
            )),
        }
    }
}

/// A scalar value as the text a schema's map keys it by: `"task"` → `task`, `3` → `3`.
fn scalar_text(value: &NodeValue) -> Option<String> {
    match value {
        NodeValue::String(value) => Some(value.clone()),
        NodeValue::Number(value) => Some(value.to_string()),
        NodeValue::Float(value) => Some(value.to_string()),
        NodeValue::Bool(value) => Some(value.to_string()),
        NodeValue::Block(_) => None,
    }
}

fn validate_primitive(
    node: &Node,
    schema: &Value,
    expected: &str,
    errors: &mut Vec<EngineDiagnostic>,
) {
    let type_matches = match expected {
        "string" => matches!(&node.value, NodeValue::String(_)),
        "number" => matches!(&node.value, NodeValue::Float(_) | NodeValue::Number(_)),
        "integer" => matches!(&node.value, NodeValue::Number(_)),
        "boolean" => matches!(&node.value, NodeValue::Bool(_)),
        _ => true,
    };
    if !type_matches {
        errors.push(EngineDiagnostic::error(
            format!("Expected {expected} for '{}'", node.key),
            node.span,
        ));
        return;
    }

    if let Some(allowed) = schema.get("enum").and_then(Value::as_array) {
        let actual = match &node.value {
            NodeValue::String(value) => Value::String(value.clone()),
            NodeValue::Number(value) => Value::from(*value),
            NodeValue::Float(value) => Value::from(*value),
            NodeValue::Bool(value) => Value::from(*value),
            NodeValue::Block(_) => return,
        };
        if !allowed.contains(&actual) {
            errors.push(EngineDiagnostic::error(
                format!("Value for '{}' is not one of the allowed enum values", node.key),
                node.span,
            ));
        }
    }
}
