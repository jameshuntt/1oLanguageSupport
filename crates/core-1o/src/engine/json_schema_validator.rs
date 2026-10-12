use serde_json::Value;

use crate::engine::{EngineDiagnostic, Node, NodeValue, Span};

/// Validate the parsed top-level 1o nodes against the root object schema.
/// This intentionally implements only the small subset used by 1o v1:
/// object/string/number/integer/boolean, required keys, unknown-key rejection,
/// and simple scalar enum membership. It is not a complete JSON Schema engine.
pub fn validate_ast(
    nodes: &[Node],
    schema: &Value,
    errors: &mut Vec<EngineDiagnostic>,
) {
    let Some("object") = schema.get("type").and_then(Value::as_str) else {
        return;
    };
    let empty = serde_json::Map::new();
    let properties = schema
        .get("properties")
        .and_then(Value::as_object)
        .unwrap_or(&empty);
    let required = schema
        .get("required")
        .and_then(Value::as_array)
        .map(|values| values.iter().filter_map(Value::as_str).collect::<Vec<_>>())
        .unwrap_or_default();
    let root_span = nodes
        .first()
        .zip(nodes.last())
        .map(|(first, last)| Span { start: first.span.start, end: last.span.end })
        .unwrap_or(Span { start: 0, end: 0 });

    for key in required {
        if !nodes.iter().any(|node| node.key == key) {
            errors.push(EngineDiagnostic::error(
                format!("Missing required key '{key}'"),
                root_span,
            ));
        }
    }
    for node in nodes {
        match properties.get(&node.key) {
            Some(node_schema) => validate_node(node, node_schema, errors),
            None => errors.push(EngineDiagnostic::error(
                format!("Unknown top-level key '{}'", node.key),
                node.span,
            )),
        }
    }
}

pub fn validate_node(node: &Node, schema: &Value, errors: &mut Vec<EngineDiagnostic>) {
    match schema.get("type").and_then(Value::as_str) {
        Some("object") => validate_object(node, schema, errors),
        Some("string") => validate_primitive(node, schema, "string", errors),
        Some("number") => validate_primitive(node, schema, "number", errors),
        Some("integer") => validate_primitive(node, schema, "integer", errors),
        Some("boolean") => validate_primitive(node, schema, "boolean", errors),
        _ => {}
    }
}

fn validate_object(node: &Node, schema: &Value, errors: &mut Vec<EngineDiagnostic>) {
    let empty = serde_json::Map::new();
    let properties = schema
        .get("properties")
        .and_then(Value::as_object)
        .unwrap_or(&empty);
    let required = schema
        .get("required")
        .and_then(Value::as_array)
        .map(|values| values.iter().filter_map(Value::as_str).collect::<Vec<_>>())
        .unwrap_or_default();

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

    for key in required {
        if !children.iter().any(|child| child.key == key) {
            errors.push(EngineDiagnostic::error(
                format!("Missing required key '{key}'"),
                node.span,
            ));
        }
    }

    let allow_additional = schema
        .get("additionalProperties")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    for child in children {
        if let Some(child_schema) = properties.get(&child.key) {
            validate_node(child, child_schema, errors);
        } else if !allow_additional {
            errors.push(EngineDiagnostic::error(
                format!("Unknown key '{}' in object '{}'", child.key, node.key),
                child.span,
            ));
        }
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
