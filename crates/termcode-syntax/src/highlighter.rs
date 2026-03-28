use crate::language::LanguageId;

/// A highlight span covering a byte range with a named scope.
#[derive(Debug, Clone)]
pub struct HighlightSpan {
    pub byte_start: usize,
    pub byte_end: usize,
    pub scope: String,
}

/// Syntax highlighter for a document.
/// MVP: keyword-based highlighting (no tree-sitter parsing yet).
/// Full tree-sitter integration will be added incrementally.
pub struct SyntaxHighlighter {
    language_id: LanguageId,
    keywords: Vec<(&'static str, &'static str)>,
}

impl SyntaxHighlighter {
    pub fn new(language_id: &LanguageId) -> Self {
        let keywords = match language_id.as_ref() {
            "rust" => rust_keywords(),
            "python" => python_keywords(),
            "javascript" | "typescript" => js_keywords(),
            _ => vec![],
        };
        Self {
            language_id: language_id.clone(),
            keywords,
        }
    }

    pub fn language_id(&self) -> &LanguageId {
        &self.language_id
    }

    /// Highlight a single line of text, returning spans with scope names.
    pub fn highlight_line(&self, line: &str) -> Vec<HighlightSpan> {
        let mut spans = Vec::new();
        if self.keywords.is_empty() {
            return spans;
        }

        let bytes = line.as_bytes();
        let mut i = 0;

        while i < bytes.len() {
            // Skip whitespace
            if bytes[i].is_ascii_whitespace() {
                i += 1;
                continue;
            }

            // Check for line comments
            if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'/' {
                spans.push(HighlightSpan {
                    byte_start: i,
                    byte_end: bytes.len(),
                    scope: "comment".to_string(),
                });
                break;
            }

            // Check for # comments (Python)
            if bytes[i] == b'#' && self.language_id.as_ref() == "python" {
                spans.push(HighlightSpan {
                    byte_start: i,
                    byte_end: bytes.len(),
                    scope: "comment".to_string(),
                });
                break;
            }

            // Check for strings
            if bytes[i] == b'"' || bytes[i] == b'\'' {
                let quote = bytes[i];
                let start = i;
                i += 1;
                while i < bytes.len() && bytes[i] != quote {
                    if bytes[i] == b'\\' {
                        i += 1; // skip escaped char
                    }
                    i += 1;
                }
                if i < bytes.len() {
                    i += 1; // closing quote
                }
                spans.push(HighlightSpan {
                    byte_start: start,
                    byte_end: i,
                    scope: "string".to_string(),
                });
                continue;
            }

            // Check for numbers
            if bytes[i].is_ascii_digit() {
                let start = i;
                while i < bytes.len()
                    && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'.' || bytes[i] == b'_')
                {
                    i += 1;
                }
                spans.push(HighlightSpan {
                    byte_start: start,
                    byte_end: i,
                    scope: "constant.numeric".to_string(),
                });
                continue;
            }

            // Check for identifiers/keywords
            if bytes[i].is_ascii_alphabetic() || bytes[i] == b'_' {
                let start = i;
                while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                    i += 1;
                }
                let word = &line[start..i];

                // Check if it's a keyword
                if let Some((_, scope)) = self.keywords.iter().find(|(kw, _)| *kw == word) {
                    spans.push(HighlightSpan {
                        byte_start: start,
                        byte_end: i,
                        scope: scope.to_string(),
                    });
                }
                // Check for type-like identifiers (starts with uppercase)
                else if word.chars().next().is_some_and(|c| c.is_ascii_uppercase()) {
                    spans.push(HighlightSpan {
                        byte_start: start,
                        byte_end: i,
                        scope: "type".to_string(),
                    });
                }
                continue;
            }

            i += 1;
        }

        spans
    }
}

fn rust_keywords() -> Vec<(&'static str, &'static str)> {
    vec![
        ("fn", "keyword.function"),
        ("let", "keyword"),
        ("mut", "keyword"),
        ("const", "keyword"),
        ("static", "keyword"),
        ("struct", "keyword"),
        ("enum", "keyword"),
        ("impl", "keyword"),
        ("trait", "keyword"),
        ("type", "keyword"),
        ("mod", "keyword"),
        ("use", "keyword.control.import"),
        ("pub", "keyword"),
        ("crate", "keyword"),
        ("self", "variable.builtin"),
        ("Self", "type.builtin"),
        ("super", "keyword"),
        ("if", "keyword.control"),
        ("else", "keyword.control"),
        ("match", "keyword.control"),
        ("for", "keyword.control"),
        ("while", "keyword.control"),
        ("loop", "keyword.control"),
        ("break", "keyword.control"),
        ("continue", "keyword.control"),
        ("return", "keyword.control.return"),
        ("as", "keyword"),
        ("in", "keyword"),
        ("ref", "keyword"),
        ("move", "keyword"),
        ("async", "keyword"),
        ("await", "keyword"),
        ("where", "keyword"),
        ("unsafe", "keyword"),
        ("extern", "keyword"),
        ("dyn", "keyword"),
        ("true", "constant"),
        ("false", "constant"),
        ("None", "constant"),
        ("Some", "constructor"),
        ("Ok", "constructor"),
        ("Err", "constructor"),
        ("Vec", "type.builtin"),
        ("String", "type.builtin"),
        ("Option", "type.builtin"),
        ("Result", "type.builtin"),
        ("Box", "type.builtin"),
        ("Arc", "type.builtin"),
        ("Rc", "type.builtin"),
        ("HashMap", "type.builtin"),
        ("usize", "type.builtin"),
        ("isize", "type.builtin"),
        ("u8", "type.builtin"),
        ("u16", "type.builtin"),
        ("u32", "type.builtin"),
        ("u64", "type.builtin"),
        ("i8", "type.builtin"),
        ("i16", "type.builtin"),
        ("i32", "type.builtin"),
        ("i64", "type.builtin"),
        ("f32", "type.builtin"),
        ("f64", "type.builtin"),
        ("bool", "type.builtin"),
        ("str", "type.builtin"),
        ("char", "type.builtin"),
    ]
}

fn python_keywords() -> Vec<(&'static str, &'static str)> {
    vec![
        ("def", "keyword.function"),
        ("class", "keyword"),
        ("import", "keyword.control.import"),
        ("from", "keyword.control.import"),
        ("if", "keyword.control"),
        ("elif", "keyword.control"),
        ("else", "keyword.control"),
        ("for", "keyword.control"),
        ("while", "keyword.control"),
        ("return", "keyword.control.return"),
        ("yield", "keyword.control.return"),
        ("break", "keyword.control"),
        ("continue", "keyword.control"),
        ("pass", "keyword"),
        ("raise", "keyword"),
        ("try", "keyword.control"),
        ("except", "keyword.control"),
        ("finally", "keyword.control"),
        ("with", "keyword"),
        ("as", "keyword"),
        ("lambda", "keyword.function"),
        ("and", "keyword.operator"),
        ("or", "keyword.operator"),
        ("not", "keyword.operator"),
        ("in", "keyword.operator"),
        ("is", "keyword.operator"),
        ("True", "constant"),
        ("False", "constant"),
        ("None", "constant"),
        ("self", "variable.builtin"),
        ("async", "keyword"),
        ("await", "keyword"),
    ]
}

fn js_keywords() -> Vec<(&'static str, &'static str)> {
    vec![
        ("function", "keyword.function"),
        ("const", "keyword"),
        ("let", "keyword"),
        ("var", "keyword"),
        ("class", "keyword"),
        ("import", "keyword.control.import"),
        ("export", "keyword.control.import"),
        ("from", "keyword.control.import"),
        ("if", "keyword.control"),
        ("else", "keyword.control"),
        ("for", "keyword.control"),
        ("while", "keyword.control"),
        ("return", "keyword.control.return"),
        ("break", "keyword.control"),
        ("continue", "keyword.control"),
        ("switch", "keyword.control"),
        ("case", "keyword.control"),
        ("default", "keyword.control"),
        ("try", "keyword.control"),
        ("catch", "keyword.control"),
        ("finally", "keyword.control"),
        ("throw", "keyword"),
        ("new", "keyword"),
        ("typeof", "keyword.operator"),
        ("instanceof", "keyword.operator"),
        ("this", "variable.builtin"),
        ("super", "variable.builtin"),
        ("true", "constant"),
        ("false", "constant"),
        ("null", "constant"),
        ("undefined", "constant"),
        ("async", "keyword"),
        ("await", "keyword"),
        ("interface", "keyword"),
        ("type", "keyword"),
        ("enum", "keyword"),
    ]
}
