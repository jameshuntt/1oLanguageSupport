use crate::engine::Span;

#[derive(Debug)]
pub enum ParseError {
    UnexpectedEOF { span: Span },
    UnexpectedToken { expected: &'static str, found: Span },
    InvalidStructure { message: String, span: Span },
    MismatchedBraces { open: Span, close: Span },
}
