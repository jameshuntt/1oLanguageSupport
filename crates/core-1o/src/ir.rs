//! Versioned, lossless-for-structure AST projection for external builders.
//! Repeated keys and declaration order are preserved as ordered arrays.
use serde::Serialize;
use crate::engine::{Lexer, Node, NodeValue, ParseError, Parser};

#[derive(Debug, Clone, Serialize)]
pub struct DocumentIR {
    pub format: &'static str,
    pub version: u32,
    pub nodes: Vec<EntryIR>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EntryIR {
    pub key: String,
    pub span: SourceSpan,
    pub value: ValueIR,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct SourceSpan { pub start: usize, pub end: usize }

#[derive(Debug, Clone, Serialize)]
#[serde(tag="kind", content="value", rename_all="snake_case")]
pub enum ValueIR {
    Block(Vec<EntryIR>),
    String(String),
    Number(i64),
    Float(f64),
    Bool(bool),
}

impl From<&Node> for EntryIR {
    fn from(node: &Node) -> Self {
        let value = match &node.value {
            NodeValue::Block(entries) => ValueIR::Block(entries.iter().map(Self::from).collect()),
            NodeValue::String(s) => ValueIR::String(s.clone()),
            NodeValue::Number(n) => ValueIR::Number(*n),
            NodeValue::Float(f) => ValueIR::Float(*f),
            NodeValue::Bool(b) => ValueIR::Bool(*b),
        };
        Self { key: node.key.clone(), span: SourceSpan { start: node.span.start, end: node.span.end }, value }
    }
}

pub fn parse_document(text: &str) -> Result<DocumentIR, ParseError> {
    let nodes = Parser::new(Lexer::new(text).tokenize()).parse()?;
    Ok(DocumentIR { format: "1o.ast", version: 1, nodes: nodes.iter().map(EntryIR::from).collect() })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retains_repeated_declarations_and_order() {
        let ast = parse_document("{ section { id \"a\" } section { id \"b\" } }").unwrap();
        assert_eq!(ast.nodes.len(), 2);
        assert_eq!(ast.nodes[0].key, "section");
        assert_eq!(ast.nodes[1].key, "section");
        let json = serde_json::to_value(ast).unwrap();
        assert_eq!(json["version"], 1);
        assert_eq!(json["nodes"][1]["value"]["value"][0]["value"]["value"], "b");
    }
}
