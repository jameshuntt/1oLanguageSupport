use crate::engine::Node;
use crate::schema_definitions::Schema;
use crate::engine::ParseError;

pub fn validate_schema(
    modifier: &str,
    nodes: &[Node],
    schema: &Schema,
) -> Result<(), ParseError> {

    let allowed = match schema.allowed_keys.get(modifier) {
        Some(x) => x,
        None => return Ok(()), // unknown modifier = skip
    };

    for node in nodes {
        if !allowed.contains(&node.key) {
            return Err(ParseError::InvalidStructure {
                message: format!("Key '{}' is not valid for modifier '{}'", node.key, modifier),
                span: node.span,
            });
        }
    }

    Ok(())
}



