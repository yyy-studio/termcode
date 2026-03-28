use std::path::{Path, PathBuf};

use termcode_core::{
    diagnostic::{Diagnostic, DiagnosticSeverity},
    position::Position,
};

pub fn position_to_lsp(pos: &Position) -> lsp_types::Position {
    lsp_types::Position {
        line: pos.line as u32,
        character: pos.column as u32,
    }
}

pub fn lsp_to_position(pos: &lsp_types::Position) -> Position {
    Position {
        line: pos.line as usize,
        column: pos.character as usize,
    }
}

pub fn diagnostic_from_lsp(diag: &lsp_types::Diagnostic) -> Diagnostic {
    let severity = match diag.severity {
        Some(lsp_types::DiagnosticSeverity::ERROR) => DiagnosticSeverity::Error,
        Some(lsp_types::DiagnosticSeverity::WARNING) => DiagnosticSeverity::Warning,
        Some(lsp_types::DiagnosticSeverity::INFORMATION) => DiagnosticSeverity::Info,
        Some(lsp_types::DiagnosticSeverity::HINT) => DiagnosticSeverity::Hint,
        _ => DiagnosticSeverity::Warning,
    };

    Diagnostic {
        range: (
            lsp_to_position(&diag.range.start),
            lsp_to_position(&diag.range.end),
        ),
        severity,
        message: diag.message.clone(),
        source: diag.source.clone(),
    }
}

pub fn path_to_uri(path: &Path) -> lsp_types::Uri {
    path_to_uri_string(path)
        .parse()
        .unwrap_or_else(|_| "file:///".parse().unwrap())
}

pub fn uri_to_path(uri: &lsp_types::Uri) -> Option<PathBuf> {
    uri_str_to_path(uri.as_str())
}

/// Convert a file URI string to a PathBuf, handling both 2-slash and 3-slash forms
/// with percent-decoding.
pub fn uri_str_to_path(uri: &str) -> Option<PathBuf> {
    let path_str = uri
        .strip_prefix("file:///")
        .or_else(|| uri.strip_prefix("file://"))?;
    let decoded = percent_decode(path_str.trim_start_matches('/'));
    Some(PathBuf::from(format!("/{decoded}")))
}

/// Convert a path to a file:// URI string with percent-encoding.
pub fn path_to_uri_string(path: &Path) -> String {
    let raw = path.display().to_string();
    let trimmed = raw.trim_start_matches('/');
    let encoded = percent_encode_path(trimmed);
    format!("file:///{encoded}")
}

/// Percent-encode path components for URI (encodes spaces and special chars,
/// preserves `/`, alphanumerics, `-`, `_`, `.`, `~`).
fn percent_encode_path(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' | b':' => {
                result.push(byte as char)
            }
            _ => {
                result.push('%');
                result.push_str(&format!("{byte:02X}"));
            }
        }
    }
    result
}

/// Basic percent-decoding for file URIs (decodes %XX sequences).
/// Collects decoded bytes into a Vec<u8> first to correctly handle multi-byte UTF-8 sequences.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                decoded.push(byte);
                i += 3;
                continue;
            }
        }
        decoded.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

/// Helper to parse a string into an lsp_types::Uri, with fallback.
pub fn parse_uri(s: &str) -> lsp_types::Uri {
    s.parse().unwrap_or_else(|_| "file:///".parse().unwrap())
}

/// Async event delivered from the LSP layer to the main event loop.
#[derive(Debug)]
pub enum LspResponse {
    Diagnostics {
        uri: String,
        diagnostics: Vec<Diagnostic>,
    },
    Completion {
        items: Vec<CompletionItem>,
    },
    Hover {
        contents: String,
    },
    Definition {
        uri: String,
        position: Position,
    },
    ServerStarted {
        language: String,
        trigger_characters: Vec<String>,
    },
    ServerError {
        language: String,
        error: String,
    },
}

/// A simplified completion item for display.
#[derive(Debug, Clone)]
pub struct CompletionItem {
    pub label: String,
    pub detail: Option<String>,
    pub insert_text: String,
}

impl From<&lsp_types::CompletionItem> for CompletionItem {
    fn from(item: &lsp_types::CompletionItem) -> Self {
        Self {
            label: item.label.clone(),
            detail: item.detail.clone(),
            insert_text: item
                .insert_text
                .clone()
                .unwrap_or_else(|| item.label.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_position_roundtrip() {
        let pos = Position::new(10, 5);
        let lsp_pos = position_to_lsp(&pos);
        assert_eq!(lsp_pos.line, 10);
        assert_eq!(lsp_pos.character, 5);
        let back = lsp_to_position(&lsp_pos);
        assert_eq!(back, pos);
    }

    #[test]
    fn test_diagnostic_from_lsp() {
        let lsp_diag = lsp_types::Diagnostic {
            range: lsp_types::Range {
                start: lsp_types::Position {
                    line: 5,
                    character: 0,
                },
                end: lsp_types::Position {
                    line: 5,
                    character: 10,
                },
            },
            severity: Some(lsp_types::DiagnosticSeverity::ERROR),
            message: "expected `;`".to_string(),
            source: Some("rustc".to_string()),
            ..Default::default()
        };
        let diag = diagnostic_from_lsp(&lsp_diag);
        assert_eq!(diag.range.0, Position::new(5, 0));
        assert_eq!(diag.range.1, Position::new(5, 10));
        assert_eq!(diag.severity, DiagnosticSeverity::Error);
        assert_eq!(diag.message, "expected `;`");
        assert_eq!(diag.source, Some("rustc".to_string()));
    }

    #[test]
    fn test_path_to_uri() {
        let path = Path::new("/tmp/test.rs");
        let uri = path_to_uri(path);
        assert!(uri.as_str().starts_with("file:///"));
        assert!(uri.as_str().ends_with("/tmp/test.rs"));
        assert_eq!(uri.as_str(), "file:///tmp/test.rs");
    }

    #[test]
    fn test_uri_to_path() {
        let uri: lsp_types::Uri = "file:///tmp/test.rs".parse().unwrap();
        let path = uri_to_path(&uri).unwrap();
        assert_eq!(path, PathBuf::from("/tmp/test.rs"));
    }

    #[test]
    fn test_completion_item_from_lsp() {
        let lsp_item = lsp_types::CompletionItem {
            label: "println!".to_string(),
            detail: Some("macro".to_string()),
            insert_text: Some("println!($0)".to_string()),
            ..Default::default()
        };
        let item = CompletionItem::from(&lsp_item);
        assert_eq!(item.label, "println!");
        assert_eq!(item.detail, Some("macro".to_string()));
        assert_eq!(item.insert_text, "println!($0)");
    }

    #[test]
    fn test_completion_item_fallback_to_label() {
        let lsp_item = lsp_types::CompletionItem {
            label: "foo".to_string(),
            ..Default::default()
        };
        let item = CompletionItem::from(&lsp_item);
        assert_eq!(item.insert_text, "foo");
    }

    #[test]
    fn test_parse_uri() {
        let uri = parse_uri("file:///tmp/test.rs");
        assert_eq!(uri.as_str(), "file:///tmp/test.rs");
    }

    #[test]
    fn test_percent_decode_multibyte_utf8() {
        // "hello 世界" percent-encoded: 世 = E4 B8 96, 界 = E7 95 8C
        let encoded = "hello%20%E4%B8%96%E7%95%8C";
        let decoded = percent_decode(encoded);
        assert_eq!(decoded, "hello 世界");
    }

    #[test]
    fn test_uri_roundtrip_with_spaces() {
        let path = Path::new("/tmp/my project/test.rs");
        let uri_str = path_to_uri_string(path);
        let back = uri_str_to_path(&uri_str).unwrap();
        assert_eq!(back, path);
    }
}
