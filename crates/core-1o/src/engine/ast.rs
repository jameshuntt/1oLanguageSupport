// src/engine/ast.rs

#[derive(Debug, Clone)]
pub struct Node {
    pub key: String,
    pub value: NodeValue,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum NodeValue {
    Block(Vec<Node>),
    String(String),
    Number(i64),
    Float(f64),
    Bool(bool),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}
