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
    uri_to_path_for_platform(uri.as_str(), cfg!(windows))
}

fn uri_to_path_for_platform(uri: &str, is_windows: bool) -> std::path::PathBuf {
    let decoded = percent_encoding::percent_decode_str(uri).decode_utf8_lossy();
    let stripped = decoded.strip_prefix("file://").unwrap_or(&decoded);
    if is_windows && stripped.len() >= 3 {
        let bytes = stripped.as_bytes();
        // file:///C:/Work/.. -> C:/Work/..; PathBuf on Windows accepts / separators.
        if bytes[0] == b'/' && bytes[1].is_ascii_alphabetic() && bytes[2] == b':' {
            return std::path::PathBuf::from(&stripped[1..]);
        }
        // file://nas/share/.. -> UNC (\\nas\share..).
        if !stripped.starts_with('/') && !stripped.starts_with("localhost/") {
            return std::path::PathBuf::from(format!("\\\\{}", stripped.replace('/', "\\")));
        }
    }
    std::path::PathBuf::from(stripped)
}

#[cfg(test)]
mod uri_path_tests {
    use super::uri_to_path_for_platform;
    #[test]
    fn decodes_macos_and_linux_path() {
        assert_eq!(uri_to_path_for_platform("file:///tmp/My%20Project/page.1o", false).to_string_lossy(), "/tmp/My Project/page.1o");
    }
    #[test]
    fn decodes_windows_drive_from_file_uri() {
        assert_eq!(uri_to_path_for_platform("file:///C:/Users/Test%20Name/project/page.1o", true).to_string_lossy(), "C:/Users/Test Name/project/page.1o");
    }
    #[test]
    fn decodes_windows_network_share() {
        assert_eq!(uri_to_path_for_platform("file://nas/share/project/page.1o", true).to_string_lossy(), r"\\nas\share\project\page.1o");
    }
}
