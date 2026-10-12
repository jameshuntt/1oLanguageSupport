use crate::engine::Span;

#[derive(Debug, Clone)]
pub struct EngineDiagnostic {
    pub message: String,
    pub span: Span,
    pub severity: EngineSeverity,
}

#[derive(Debug, Clone)]
pub enum EngineSeverity {
    Error,
    Warning,
}


impl EngineDiagnostic {
    pub fn error(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            span,
            severity: EngineSeverity::Error,
        }
    }

    pub fn warning(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            span,
            severity: EngineSeverity::Warning,
        }
    }
}

use crate::engine::ParseError;

impl From<&ParseError> for EngineDiagnostic {
    fn from(err: &ParseError) -> Self {
        match err {
            ParseError::UnexpectedToken { expected, found } => {
                EngineDiagnostic::error(
                    format!("Expected {}", expected),
                    *found,
                )
            }
            ParseError::UnexpectedEOF { span } => {
                EngineDiagnostic::error("Unexpected end of file", *span)
            }
            ParseError::InvalidStructure { message, span } => {
                EngineDiagnostic::error(message.clone(), *span)
            }
            ParseError::MismatchedBraces { open, close } => {
                EngineDiagnostic::error(
                    "Mismatched braces",
                    Span {
                        start: open.start,
                        end: close.end,
                    },
                )
            }
        }
    }
}

impl From<ParseError> for EngineDiagnostic {
    fn from(err: ParseError) -> Self {
        match err {
            ParseError::UnexpectedToken { expected, found } => {
                EngineDiagnostic::error(
                    format!("Expected {}", expected),
                    found,
                )
            }
            ParseError::UnexpectedEOF { span } => {
                EngineDiagnostic::error("Unexpected end of file", span)
            }
            ParseError::InvalidStructure { message, span } => {
                EngineDiagnostic::error(message.clone(), span)
            }
            ParseError::MismatchedBraces { open, close } => {
                EngineDiagnostic::error(
                    "Mismatched braces",
                    Span {
                        start: open.start,
                        end: close.end,
                    },
                )
            }
        }
    }
}

pub type EngineDiagnostics = Vec<EngineDiagnostic>;
