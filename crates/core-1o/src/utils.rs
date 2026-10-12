pub mod parse_filename;
pub use parse_filename::*;

use lsp_types::{Diagnostic, DiagnosticSeverity, Position, Range, Uri};

use crate::engine::{EngineDiagnostic, EngineSeverity, LineMap, ParseError, Span};

pub fn convert_parse_error(err: &ParseError) -> Diagnostic {
    match err {
        ParseError::UnexpectedToken { expected, found } => Diagnostic {
            message: format!("Expected {expected}, found something else"),
            range: span_to_range_legacy(*found),
            severity: Some(DiagnosticSeverity::ERROR),
            source: Some("1o-lsp".into()),
            ..Default::default()
        },
        ParseError::UnexpectedEOF { span } => Diagnostic {
            message: "Unexpected EOF".into(),
            range: span_to_range_legacy(*span),
            severity: Some(DiagnosticSeverity::ERROR),
            source: Some("1o-lsp".into()),
            ..Default::default()
        },
        ParseError::InvalidStructure { message, span } => Diagnostic {
            message: message.clone(),
            range: span_to_range_legacy(*span),
            severity: Some(DiagnosticSeverity::ERROR),
            source: Some("1o-lsp".into()),
            ..Default::default()
        },
        ParseError::MismatchedBraces { open, close } => Diagnostic {
            message: "Mismatched braces".into(),
            range: span_to_range_legacy(Span { start: open.start, end: close.end }),
            severity: Some(DiagnosticSeverity::ERROR),
            source: Some("1o-lsp".into()),
            ..Default::default()
        },
    }
}

pub fn to_lsp(text: &str, diagnostic: &EngineDiagnostic) -> Diagnostic {
    let map = LineMap::new(text);
    Diagnostic {
        message: diagnostic.message.clone(),
        range: map.span_to_range(diagnostic.span),
        severity: Some(match diagnostic.severity {
            EngineSeverity::Error => DiagnosticSeverity::ERROR,
            EngineSeverity::Warning => DiagnosticSeverity::WARNING,
        }),
        source: Some("1o-lsp".into()),
        ..Default::default()
    }
}

fn span_to_range_legacy(span: Span) -> Range {
    Range {
        start: Position { line: 0, character: span.start as u32 },
        end: Position { line: 0, character: span.end as u32 },
    }
}

pub fn uri_to_path(uri: &Uri) -> std::path::PathBuf {
    let decoded = percent_encoding::percent_decode_str(uri.as_str()).decode_utf8_lossy();
    let stripped = decoded.strip_prefix("file://").unwrap_or(&decoded);
    std::path::PathBuf::from(stripped)
}
