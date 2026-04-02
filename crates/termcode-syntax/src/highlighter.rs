use std::ops::Range;
use std::sync::Arc;

use ropey::Rope;
use tree_sitter::{InputEdit, Parser, Point, Tree};
use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter};

use crate::language::LanguageConfig;

/// Known highlight scope names matching theme scopes.
/// The index of each name is the `Highlight.0` value returned by tree-sitter-highlight.
const HIGHLIGHT_NAMES: &[&str] = &[
    "attribute",
    "comment",
    "constant",
    "constant.character",
    "constant.character.escape",
    "constant.numeric",
    "constructor",
    "function",
    "function.builtin",
    "function.macro",
    "keyword",
    "keyword.control",
    "keyword.control.import",
    "keyword.control.return",
    "keyword.function",
    "keyword.operator",
    "label",
    "namespace",
    "operator",
    "punctuation",
    "punctuation.bracket",
    "punctuation.delimiter",
    "string",
    "string.special",
    "tag",
    "type",
    "type.builtin",
    "variable",
    "variable.builtin",
    "variable.parameter",
    "special",
];

/// A highlight span covering a byte range with a named scope.
#[derive(Debug, Clone)]
pub struct HighlightSpan {
    pub byte_start: usize,
    pub byte_end: usize,
    pub scope: String,
}

/// Tree-sitter based syntax highlighter for a document.
pub struct SyntaxHighlighter {
    parser: Parser,
    tree: Option<Tree>,
    highlight_config: Arc<HighlightConfiguration>,
    /// Cached per-line highlight spans, invalidated on parse/update.
    cached_spans: Vec<Vec<HighlightSpan>>,
}

impl SyntaxHighlighter {
    /// Create a new highlighter for a language configuration.
    /// Returns `None` if the language has no grammar or no highlight query.
    pub fn new(config: &LanguageConfig) -> Option<Self> {
        let grammar = config.grammar.clone()?;
        if config.highlight_query.is_empty() {
            return None;
        }

        let mut hl_config = match HighlightConfiguration::new(
            grammar.clone(),
            &config.name,
            &config.highlight_query,
            "",
            "",
        ) {
            Ok(c) => c,
            Err(e) => {
                log::warn!("Failed to create highlight config for {}: {}", config.id, e);
                return None;
            }
        };
        hl_config.configure(HIGHLIGHT_NAMES);

        let mut parser = Parser::new();
        if parser.set_language(&grammar).is_err() {
            log::warn!("Failed to set parser language for {}", config.id);
            return None;
        }

        Some(Self {
            parser,
            tree: None,
            highlight_config: Arc::new(hl_config),
            cached_spans: Vec::new(),
        })
    }

    /// Full parse of the document source (Rope).
    /// Uses chunk-based callback to avoid Rope::to_string().
    /// Also rebuilds the highlight cache for the entire document.
    pub fn parse(&mut self, source: &Rope) {
        let tree = self.parser.parse_with_options(
            &mut |byte_offset, _position| -> &[u8] {
                if byte_offset >= source.len_bytes() {
                    return &[];
                }
                let (chunk, chunk_byte_start, _, _) = source.chunk_at_byte(byte_offset);
                let chunk_bytes: &[u8] = chunk.as_bytes();
                &chunk_bytes[byte_offset - chunk_byte_start..]
            },
            self.tree.as_ref(),
            None,
        );
        self.tree = tree;
        self.rebuild_cache(source);
    }

    /// Incremental update after edits.
    /// Apply InputEdits to the existing tree, then re-parse.
    pub fn update(&mut self, source: &Rope, edits: &[InputEdit]) {
        if let Some(tree) = &mut self.tree {
            for edit in edits {
                tree.edit(edit);
            }
        }
        self.parse(source);
    }

    /// Rebuild the full-document highlight cache.
    fn rebuild_cache(&mut self, source: &Rope) {
        let line_count = source.len_lines();
        let mut result: Vec<Vec<HighlightSpan>> = vec![vec![]; line_count];

        let source_str = String::from(source);
        let source_bytes = source_str.as_bytes();

        let mut highlighter = Highlighter::new();
        let config = &self.highlight_config;

        let events = match highlighter.highlight(config, source_bytes, None, |_| None) {
            Ok(events) => events,
            Err(_) => {
                self.cached_spans = result;
                return;
            }
        };

        let mut scope_stack: Vec<&str> = Vec::new();

        for event in events {
            match event {
                Ok(HighlightEvent::HighlightStart(highlight)) => {
                    let idx = highlight.0;
                    if idx < HIGHLIGHT_NAMES.len() {
                        scope_stack.push(HIGHLIGHT_NAMES[idx]);
                    }
                }
                Ok(HighlightEvent::HighlightEnd) => {
                    scope_stack.pop();
                }
                Ok(HighlightEvent::Source { start, end }) => {
                    if let Some(&scope) = scope_stack.last() {
                        if start >= end {
                            continue;
                        }

                        let start_line_idx = source.byte_to_line(start);
                        let end_line_idx = source.byte_to_line(end.saturating_sub(1));

                        for line_idx in start_line_idx..=end_line_idx {
                            if line_idx >= line_count {
                                break;
                            }

                            let line_byte_start = source.line_to_byte(line_idx);
                            let line_byte_end = if line_idx + 1 < line_count {
                                source.line_to_byte(line_idx + 1)
                            } else {
                                source.len_bytes()
                            };

                            let span_start = start.max(line_byte_start) - line_byte_start;
                            let span_end = end.min(line_byte_end) - line_byte_start;

                            if span_start < span_end {
                                result[line_idx].push(HighlightSpan {
                                    byte_start: span_start,
                                    byte_end: span_end,
                                    scope: scope.to_string(),
                                });
                            }
                        }
                    }
                }
                Err(_) => break,
            }
        }

        self.cached_spans = result;
    }

    /// Highlight a range of lines, returning per-line spans from the cache.
    /// Line indices are 0-based. The returned Vec has one entry per line in the range.
    /// Each entry contains HighlightSpans with byte offsets relative to the line start.
    pub fn highlight_lines(
        &self,
        _source: &Rope,
        line_range: Range<usize>,
    ) -> Vec<Vec<HighlightSpan>> {
        let cache_len = self.cached_spans.len();
        let start_line = line_range.start.min(cache_len);
        let end_line = line_range.end.min(cache_len);

        if start_line >= end_line {
            return vec![];
        }

        self.cached_spans[start_line..end_line].to_vec()
    }
}

/// Convert byte offset in a Rope to a tree-sitter Point (row, column).
fn byte_to_point(rope: &Rope, byte_offset: usize) -> Point {
    let line = rope.byte_to_line(byte_offset);
    let line_byte = rope.line_to_byte(line);
    let col = byte_offset - line_byte;
    Point::new(line, col)
}

/// Convert ChangeSet operations to tree-sitter InputEdits.
/// `old_rope` is the Rope state BEFORE the changeset was applied.
pub fn changeset_to_input_edits(
    old_rope: &Rope,
    changes: &termcode_core::transaction::ChangeSet,
) -> Vec<InputEdit> {
    use termcode_core::transaction::Operation;

    let mut edits = Vec::new();
    let mut old_byte: usize = 0;

    for op in changes.ops() {
        match op {
            Operation::Retain(n) => {
                old_byte += n;
            }
            Operation::Insert(text) => {
                let start_byte = old_byte;
                let start_position = byte_to_point(old_rope, start_byte.min(old_rope.len_bytes()));
                let new_end_byte = start_byte + text.len();
                let inserted_lines: usize = text.bytes().filter(|&b| b == b'\n').count();
                let last_newline_pos = text.rfind('\n');
                let new_end_col = match last_newline_pos {
                    Some(pos) => text.len() - pos - 1,
                    None => start_position.column + text.len(),
                };

                edits.push(InputEdit {
                    start_byte,
                    old_end_byte: start_byte,
                    new_end_byte,
                    start_position,
                    old_end_position: start_position,
                    new_end_position: Point::new(start_position.row + inserted_lines, new_end_col),
                });
            }
            Operation::Delete(n) => {
                let start_byte = old_byte;
                let end_byte = old_byte + n;
                let start_position = byte_to_point(old_rope, start_byte.min(old_rope.len_bytes()));
                let old_end_position = byte_to_point(old_rope, end_byte.min(old_rope.len_bytes()));

                edits.push(InputEdit {
                    start_byte,
                    old_end_byte: end_byte,
                    new_end_byte: start_byte,
                    start_position,
                    old_end_position,
                    new_end_position: start_position,
                });
                old_byte += n;
            }
        }
    }

    edits
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::language::LanguageConfig;
    use std::sync::Arc;

    fn rust_config() -> LanguageConfig {
        LanguageConfig {
            id: Arc::from("rust"),
            name: "Rust".to_string(),
            file_extensions: vec!["rs".to_string()],
            highlight_query: include_str!("../../../runtime/queries/rust/highlights.scm")
                .to_string(),
            grammar: Some(tree_sitter_rust::LANGUAGE.into()),
        }
    }

    #[test]
    fn test_parse_produces_tree() {
        let config = rust_config();
        let mut hl = SyntaxHighlighter::new(&config).unwrap();
        let source = Rope::from_str("fn main() {}");
        hl.parse(&source);
        assert!(hl.tree.is_some());
    }

    #[test]
    fn test_highlight_keyword() {
        let config = rust_config();
        let mut hl = SyntaxHighlighter::new(&config).unwrap();
        let source = Rope::from_str("fn main() {}\n");
        hl.parse(&source);
        let lines = hl.highlight_lines(&source, 0..1);
        assert_eq!(lines.len(), 1);
        let fn_span = lines[0].iter().find(|s| s.scope == "keyword.function");
        assert!(fn_span.is_some(), "Expected keyword.function span for 'fn'");
        let span = fn_span.unwrap();
        assert_eq!(span.byte_start, 0);
        assert_eq!(span.byte_end, 2);
    }

    #[test]
    fn test_highlight_string() {
        let config = rust_config();
        let mut hl = SyntaxHighlighter::new(&config).unwrap();
        let source = Rope::from_str("let s = \"hello\";\n");
        hl.parse(&source);
        let lines = hl.highlight_lines(&source, 0..1);
        let string_spans: Vec<_> = lines[0].iter().filter(|s| s.scope == "string").collect();
        assert!(!string_spans.is_empty(), "Expected string span");
    }

    #[test]
    fn test_highlight_comment() {
        let config = rust_config();
        let mut hl = SyntaxHighlighter::new(&config).unwrap();
        let source = Rope::from_str("// hello\n");
        hl.parse(&source);
        let lines = hl.highlight_lines(&source, 0..1);
        let comment_spans: Vec<_> = lines[0].iter().filter(|s| s.scope == "comment").collect();
        assert!(
            !comment_spans.is_empty(),
            "Expected comment span for '// hello'"
        );
    }

    #[test]
    fn test_highlight_number() {
        let config = rust_config();
        let mut hl = SyntaxHighlighter::new(&config).unwrap();
        let source = Rope::from_str("let x = 42;\n");
        hl.parse(&source);
        let lines = hl.highlight_lines(&source, 0..1);
        let num_spans: Vec<_> = lines[0]
            .iter()
            .filter(|s| s.scope == "constant.numeric")
            .collect();
        assert!(
            !num_spans.is_empty(),
            "Expected constant.numeric span for 42"
        );
    }

    #[test]
    fn test_no_grammar_returns_none() {
        let config = LanguageConfig {
            id: Arc::from("unknown"),
            name: "Unknown".to_string(),
            file_extensions: vec![],
            highlight_query: String::new(),
            grammar: None,
        };
        assert!(SyntaxHighlighter::new(&config).is_none());
    }

    #[test]
    fn test_empty_source_no_crash() {
        let config = rust_config();
        let mut hl = SyntaxHighlighter::new(&config).unwrap();
        let source = Rope::from_str("");
        hl.parse(&source);
        let lines = hl.highlight_lines(&source, 0..0);
        assert!(lines.is_empty());
    }

    #[test]
    fn test_incremental_update() {
        let config = rust_config();
        let mut hl = SyntaxHighlighter::new(&config).unwrap();
        let source = Rope::from_str("fn foo() {}\n");
        hl.parse(&source);
        assert!(hl.tree.is_some());

        let edit = InputEdit {
            start_byte: 3,
            old_end_byte: 6,
            new_end_byte: 6,
            start_position: Point::new(0, 3),
            old_end_position: Point::new(0, 6),
            new_end_position: Point::new(0, 6),
        };
        let new_source = Rope::from_str("fn bar() {}\n");
        hl.update(&new_source, &[edit]);
        assert!(hl.tree.is_some());

        let lines = hl.highlight_lines(&new_source, 0..1);
        let fn_span = lines[0].iter().find(|s| s.scope == "keyword.function");
        assert!(fn_span.is_some());
    }

    #[test]
    fn test_changeset_to_input_edits_insert() {
        let rope = Rope::from_str("hello world");
        let mut cs = termcode_core::transaction::ChangeSet::new(rope.len_bytes());
        cs.retain(5);
        cs.insert(" beautiful".to_string());
        cs.retain(6);

        let edits = changeset_to_input_edits(&rope, &cs);
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].start_byte, 5);
        assert_eq!(edits[0].old_end_byte, 5);
        assert_eq!(edits[0].new_end_byte, 15);
    }

    #[test]
    fn test_changeset_to_input_edits_delete() {
        let rope = Rope::from_str("hello beautiful world");
        let mut cs = termcode_core::transaction::ChangeSet::new(rope.len_bytes());
        cs.retain(5);
        cs.delete(10);
        cs.retain(6);

        let edits = changeset_to_input_edits(&rope, &cs);
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].start_byte, 5);
        assert_eq!(edits[0].old_end_byte, 15);
        assert_eq!(edits[0].new_end_byte, 5);
    }

    fn python_config() -> LanguageConfig {
        LanguageConfig {
            id: Arc::from("python"),
            name: "Python".to_string(),
            file_extensions: vec!["py".to_string()],
            highlight_query: include_str!("../../../runtime/queries/python/highlights.scm")
                .to_string(),
            grammar: Some(tree_sitter_python::LANGUAGE.into()),
        }
    }

    fn js_config() -> LanguageConfig {
        LanguageConfig {
            id: Arc::from("javascript"),
            name: "JavaScript".to_string(),
            file_extensions: vec!["js".to_string()],
            highlight_query: include_str!("../../../runtime/queries/javascript/highlights.scm")
                .to_string(),
            grammar: Some(tree_sitter_javascript::LANGUAGE.into()),
        }
    }

    #[test]
    fn test_python_highlighting() {
        let config = python_config();
        let mut hl = SyntaxHighlighter::new(&config).unwrap();
        let source = Rope::from_str("def hello():\n    return 42\n");
        hl.parse(&source);
        let lines = hl.highlight_lines(&source, 0..2);
        assert_eq!(lines.len(), 2);
        let def_span = lines[0].iter().find(|s| s.scope == "keyword.function");
        assert!(def_span.is_some(), "Expected keyword.function for 'def'");
    }

    #[test]
    fn test_javascript_highlighting() {
        let config = js_config();
        let hl = SyntaxHighlighter::new(&config);
        assert!(hl.is_some(), "JS highlighter creation should succeed");
        let mut hl = hl.unwrap();
        let source = Rope::from_str("function hello() {\n  return 42;\n}\n");
        hl.parse(&source);
        let lines = hl.highlight_lines(&source, 0..3);
        assert_eq!(lines.len(), 3);
        let fn_span = lines[0].iter().find(|s| s.scope == "keyword.function");
        assert!(
            fn_span.is_some(),
            "Expected keyword.function for 'function'"
        );
    }

    macro_rules! test_grammar {
        ($name:ident, $lang:expr, $query:expr) => {
            #[test]
            fn $name() {
                let config = LanguageConfig {
                    id: Arc::from(stringify!($name)),
                    name: stringify!($name).to_string(),
                    file_extensions: vec![],
                    highlight_query: $query.to_string(),
                    grammar: Some($lang),
                };
                let hl = SyntaxHighlighter::new(&config);
                assert!(
                    hl.is_some(),
                    "Failed to create highlighter for {}",
                    stringify!($name)
                );
            }
        };
    }

    test_grammar!(
        grammar_rust,
        tree_sitter_rust::LANGUAGE.into(),
        include_str!("../../../runtime/queries/rust/highlights.scm")
    );
    test_grammar!(
        grammar_python,
        tree_sitter_python::LANGUAGE.into(),
        include_str!("../../../runtime/queries/python/highlights.scm")
    );
    test_grammar!(
        grammar_javascript,
        tree_sitter_javascript::LANGUAGE.into(),
        include_str!("../../../runtime/queries/javascript/highlights.scm")
    );
    test_grammar!(
        grammar_typescript,
        tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        include_str!("../../../runtime/queries/typescript/highlights.scm")
    );
    test_grammar!(
        grammar_json,
        tree_sitter_json::LANGUAGE.into(),
        include_str!("../../../runtime/queries/json/highlights.scm")
    );
    test_grammar!(
        grammar_c,
        tree_sitter_c::LANGUAGE.into(),
        include_str!("../../../runtime/queries/c/highlights.scm")
    );
    test_grammar!(
        grammar_cpp,
        tree_sitter_cpp::LANGUAGE.into(),
        include_str!("../../../runtime/queries/cpp/highlights.scm")
    );
    test_grammar!(
        grammar_go,
        tree_sitter_go::LANGUAGE.into(),
        include_str!("../../../runtime/queries/go/highlights.scm")
    );
    test_grammar!(
        grammar_bash,
        tree_sitter_bash::LANGUAGE.into(),
        include_str!("../../../runtime/queries/bash/highlights.scm")
    );
    test_grammar!(
        grammar_markdown,
        tree_sitter_md::LANGUAGE.into(),
        include_str!("../../../runtime/queries/markdown/highlights.scm")
    );

    #[test]
    fn test_markdown_actual_highlighting() {
        let config = LanguageConfig {
            id: Arc::from("markdown"),
            name: "Markdown".to_string(),
            file_extensions: vec!["md".to_string()],
            highlight_query: include_str!("../../../runtime/queries/markdown/highlights.scm")
                .to_string(),
            grammar: Some(tree_sitter_md::LANGUAGE.into()),
        };
        let mut hl =
            SyntaxHighlighter::new(&config).expect("markdown highlighter should be created");
        let source = Rope::from_str("# Hello World\n\nSome text.\n");
        hl.parse(&source);
        let lines = hl.highlight_lines(&source, 0..3);
        eprintln!("MD highlight results:");
        for (i, spans) in lines.iter().enumerate() {
            eprintln!(
                "  Line {}: {:?}",
                i,
                spans
                    .iter()
                    .map(|s| (&s.scope, s.byte_start, s.byte_end))
                    .collect::<Vec<_>>()
            );
        }
        let heading_spans: Vec<_> = lines[0]
            .iter()
            .filter(|s| s.scope == "keyword" || s.scope.starts_with("keyword"))
            .collect();
        assert!(
            !heading_spans.is_empty(),
            "Expected heading marker to be highlighted, got: {:?}",
            lines[0]
        );
    }
    test_grammar!(
        grammar_toml,
        crate::language::toml_language(),
        include_str!("../../../runtime/queries/toml/highlights.scm")
    );
}
