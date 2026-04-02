# Tree-sitter Syntax Highlighting Integration

Replace the keyword-based syntax highlighter with tree-sitter incremental parsing to provide accurate, AST-aware syntax highlighting and enable easy addition of new languages via query files.

## Code Reference Checklist

| Item                    | Result                                                                                                                                                                                                              |
| ----------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Similar feature exists? | Yes. `SyntaxHighlighter` in `crates/termcode-syntax/src/highlighter.rs` provides keyword-based highlighting for Rust, Python, JS/TS. This spec replaces it entirely with tree-sitter.                               |
| Reference pattern       | `LanguageRegistry` in `language.rs` already defines `LanguageConfig` with `highlight_query: String` field (always empty). `Theme::resolve(&scope)` in `termcode-theme` supports dot-separated scope fallback.       |
| Technical constraints   | `termcode-view` is frontend-agnostic (no terminal deps). Tree-sitter lives in `termcode-syntax` (Layer 1), which `termcode-view` depends on. Layer boundary is maintained. No closures allowed in `CommandHandler`. |

### Key Files

| File                                         | Role                                                  |
| -------------------------------------------- | ----------------------------------------------------- |
| `crates/termcode-syntax/src/highlighter.rs`  | Current keyword-based highlighter (to be rewritten)   |
| `crates/termcode-syntax/src/language.rs`     | `LanguageConfig`, `LanguageRegistry` (to be extended) |
| `crates/termcode-syntax/src/lib.rs`          | Module declarations                                   |
| `crates/termcode-syntax/Cargo.toml`          | Dependencies (tree-sitter already listed)             |
| `crates/termcode-view/src/document.rs`       | `Document.syntax` field, `Document::open()`           |
| `crates/termcode-term/src/ui/editor_view.rs` | Rendering code that calls `highlight_line()`          |
| `crates/termcode-term/src/app.rs`            | `LanguageRegistry` initialization in `App::new()`     |
| `crates/termcode-theme/src/theme.rs`         | `Theme::resolve()` scope-to-style resolution          |
| `Cargo.toml` (workspace)                     | Workspace dependency declarations                     |
| `runtime/queries/{lang}/highlights.scm`      | Tree-sitter highlight query files                     |

---

## Functional Requirements

### FR-SYNTAX-001: Tree-sitter Parser Integration

- **Description**: Replace the keyword-based `SyntaxHighlighter` with a tree-sitter-based implementation that holds a `tree_sitter::Parser`, a parsed `tree_sitter::Tree`, and a `tree_sitter_highlight::HighlightConfiguration`.
- **Priority**: High
- **Status**: Draft
- **Code Reference**: `crates/termcode-syntax/src/highlighter.rs` (entire file rewrite)
- **Details**:
  - `SyntaxHighlighter` becomes stateful, holding:
    - `parser: tree_sitter::Parser` -- configured with the language grammar
    - `tree: Option<tree_sitter::Tree>` -- the most recent parse tree
    - `highlight_config: Arc<HighlightConfiguration>` -- loaded from `highlights.scm`
    - `highlight_names: Arc<[String]>` -- recognized scope names for the theme
  - Constructor: `SyntaxHighlighter::new(config: &LanguageConfig) -> Option<Self>`
    - Returns `None` if `config.grammar` is `None` (no grammar available for this language)
    - Creates a `Parser`, sets the language grammar, and builds the `HighlightConfiguration` from `config.highlight_query`
  - The `HighlightSpan` struct is preserved with the same fields: `byte_start`, `byte_end`, `scope: String`

### FR-SYNTAX-002: Full-Document Parsing

- **Description**: Parse the entire document source on initial open, producing a `tree_sitter::Tree` for subsequent highlighting and incremental updates.
- **Priority**: High
- **Status**: Draft
- **Code Reference**: `Document::open()` in `crates/termcode-view/src/document.rs:48`
- **Details**:
  - New method: `SyntaxHighlighter::parse(&mut self, source: &Rope)`
    - Uses `Parser::parse_with()` with a callback that reads from `Rope` chunks directly (no full `Rope::to_string()` conversion)
    - Stores the resulting `Tree` in `self.tree`
  - Rope-to-bytes callback: `|byte_offset, _point| -> &[u8]` reads from the appropriate `Rope` chunk at the given byte offset using `Rope::byte_slice()` or chunk iteration
  - Called once in `Document::open()` after creating the `SyntaxHighlighter`
  - Performance: synchronous parse. Typical files parse in 1-5ms. No async required for initial implementation.

### FR-SYNTAX-003: Viewport-Scoped Highlighting

- **Description**: Extract highlight spans only for the visible viewport range, avoiding processing the entire document on every render.
- **Priority**: High
- **Status**: Draft
- **Code Reference**: `EditorViewWidget::render()` in `crates/termcode-term/src/ui/editor_view.rs:188-205`
- **Details**:
  - New method: `SyntaxHighlighter::highlight_lines(&self, source: &Rope, line_range: std::ops::Range<usize>) -> Vec<Vec<HighlightSpan>>`
    - Uses `tree_sitter_highlight::Highlighter::highlight()` to iterate `HighlightEvent`s
    - Filters events to only produce spans within the given byte range (derived from `line_range`)
    - Returns a `Vec` of `Vec<HighlightSpan>`, one inner vec per line in the range
    - Each span's `byte_start` and `byte_end` are relative to the start of that line (matching current `highlight_line` contract)
  - Scope names: Resolved from `HighlightEvent::HighlightStart(Highlight)` index into `highlight_names`
  - The current per-line `highlight_line()` method is removed. The rendering code calls `highlight_lines()` once for the visible range and indexes into the result per line.
  - If `self.tree` is `None` (parse not yet done), return empty spans (graceful degradation).

### FR-SYNTAX-004: Incremental Re-parsing After Edits

- **Description**: After each document mutation (insert, delete, undo, redo, search-replace), update the parse tree incrementally rather than re-parsing from scratch.
- **Priority**: High
- **Status**: Draft
- **Code Reference**: `Document` mutation methods; `Transaction` application in `crates/termcode-view/src/document.rs`
- **Details**:
  - New method: `SyntaxHighlighter::update(&mut self, source: &Rope, edits: &[tree_sitter::InputEdit])`
    - Calls `tree.edit(&input_edit)` for each edit on the existing tree
    - Calls `parser.parse_with(callback, Some(&old_tree))` for incremental re-parse
    - Stores the new tree, replacing the old one
  - `InputEdit` mapping from `Transaction.changes`:
    - Each change in the `ChangeSet` provides `start_byte`, `old_end_byte`, `new_end_byte`
    - Row/column `Point` values are derived from the rope via `Rope::byte_to_line()` and `Rope::line_to_byte()`
  - New helper: `fn changeset_to_input_edits(changes: &ChangeSet, rope: &Rope) -> Vec<tree_sitter::InputEdit>`
    - Lives in `crates/termcode-syntax/src/highlighter.rs` (or a new `edit.rs` module)
  - Call site: `SyntaxHighlighter::update()` must be called after every `Buffer::apply()` in `Document`. This includes:
    - Direct text edits (insert, delete)
    - Undo (`History::undo()` returns `Transaction`, applied to buffer)
    - Redo (`History::redo()` returns `Transaction`, applied to buffer)
    - Search-replace operations
  - Incremental re-parse performance: sub-millisecond for single edits.

### FR-SYNTAX-005: LanguageConfig Grammar Extension

- **Description**: Extend `LanguageConfig` to hold an optional tree-sitter `Language` grammar, and load highlight queries from `runtime/queries/{lang}/highlights.scm`.
- **Priority**: High
- **Status**: Draft
- **Code Reference**: `crates/termcode-syntax/src/language.rs:9`
- **Details**:
  - Add field to `LanguageConfig`:
    ```rust
    pub grammar: Option<tree_sitter::Language>,
    ```
  - The `highlight_query: String` field already exists. It will now be populated with the contents of `runtime/queries/{lang}/highlights.scm` at registration time.
  - `LanguageRegistry::with_builtins()` is updated to:
    - Set `grammar: Some(tree_sitter_rust::LANGUAGE.into())` (and equivalent for each language)
    - Load `highlight_query` from `runtime/queries/{lang}/highlights.scm` using a runtime directory resolver
  - New method: `LanguageRegistry::load_queries(&mut self, runtime_dir: &Path)`
    - Scans `runtime_dir/queries/` for subdirectories
    - For each known language ID, reads `highlights.scm` and sets `highlight_query`
    - Called from `App::new()` after `with_builtins()`
  - Grammar crate dependencies (added to workspace `Cargo.toml` and `termcode-syntax/Cargo.toml`):

    | Crate                    | Language   |
    | ------------------------ | ---------- |
    | `tree-sitter-rust`       | Rust       |
    | `tree-sitter-python`     | Python     |
    | `tree-sitter-javascript` | JavaScript |
    | `tree-sitter-typescript` | TypeScript |
    | `tree-sitter-json`       | JSON       |
    | `tree-sitter-toml`       | TOML       |
    | `tree-sitter-c`          | C          |
    | `tree-sitter-cpp`        | C++        |
    | `tree-sitter-go`         | Go         |
    | `tree-sitter-md`         | Markdown   |

### FR-SYNTAX-006: Highlight Query Files

- **Description**: Provide `highlights.scm` tree-sitter query files for all supported languages in `runtime/queries/{lang}/`.
- **Priority**: High
- **Status**: Draft
- **Code Reference**: `runtime/queries/rust/` (exists but empty)
- **Details**:
  - Directory structure:
    ```
    runtime/queries/
    ├── rust/highlights.scm
    ├── python/highlights.scm
    ├── javascript/highlights.scm
    ├── typescript/highlights.scm
    ├── json/highlights.scm
    ├── toml/highlights.scm
    ├── c/highlights.scm
    ├── cpp/highlights.scm
    ├── go/highlights.scm
    └── markdown/highlights.scm
    ```
  - Query files should be sourced from official tree-sitter grammar repositories or the Helix editor's query collection (MIT licensed).
  - Scope names used in queries must match the theme scope names already defined in `runtime/themes/one-dark.toml` `[scopes]` section: `keyword`, `keyword.control`, `keyword.function`, `keyword.control.import`, `keyword.control.return`, `keyword.operator`, `function`, `function.builtin`, `type`, `type.builtin`, `variable`, `variable.builtin`, `string`, `comment`, `constant`, `constant.numeric`, `constructor`, `operator`, `punctuation`, `punctuation.bracket`, `punctuation.delimiter`, `attribute`, `label`, `namespace`.
  - TypeScript queries may inherit from JavaScript queries via `; inherits: javascript` directive (tree-sitter convention).

### FR-SYNTAX-007: Document Integration

- **Description**: Update `Document` to use the new tree-sitter-based `SyntaxHighlighter` API.
- **Priority**: High
- **Status**: Draft
- **Code Reference**: `crates/termcode-view/src/document.rs:20,55`
- **Details**:
  - `Document::open()` changes:
    - Receive `&LanguageConfig` instead of `Option<LanguageId>` (or receive the whole config to access grammar + query)
    - Create `SyntaxHighlighter::new(&config)` (returns `Option`)
    - Call `highlighter.parse(&buffer.rope())` immediately after creation
  - `Document.syntax` field type remains `Option<SyntaxHighlighter>` (unchanged)
  - `Document.language_id` field is preserved for display/LSP purposes
  - After every transaction application (`buffer.apply()`), call:
    ```rust
    if let Some(ref mut syntax) = self.syntax {
        let edits = changeset_to_input_edits(&transaction.changes, &self.buffer.rope());
        syntax.update(&self.buffer.rope(), &edits);
    }
    ```
  - This wiring happens in `Document` methods that apply transactions, not in `App` (keeps the syntax update close to the buffer mutation).

### FR-SYNTAX-008: Rendering Integration

- **Description**: Update `EditorViewWidget` rendering to use `highlight_lines()` instead of per-line `highlight_line()`.
- **Priority**: High
- **Status**: Draft
- **Code Reference**: `crates/termcode-term/src/ui/editor_view.rs:188-205`
- **Details**:
  - Before the per-line rendering loop, call:
    ```rust
    let all_spans = doc.syntax.as_ref()
        .map(|s| s.highlight_lines(&doc.buffer.rope(), first_line..last_line))
        .unwrap_or_default();
    ```
  - In the per-line loop, index into `all_spans[line_idx - first_line]` instead of calling `highlight_line()`.
  - The rest of the rendering logic (applying spans to `char_styles`, `theme.resolve()`) remains the same.
  - If `all_spans` is empty (no highlighter or no tree), rendering falls back to default foreground color (current behavior for unsupported languages).

### FR-SYNTAX-009: Graceful Degradation

- **Description**: If a tree-sitter grammar is unavailable for a language, the editor must not crash and must fall back to no highlighting.
- **Priority**: High
- **Status**: Draft
- **Code Reference**: N/A
- **Details**:
  - `SyntaxHighlighter::new()` returns `None` when no grammar is available. `Document.syntax` is set to `None`.
  - `highlight_lines()` returns empty spans if `self.tree` is `None`.
  - Query file not found: `LanguageRegistry::load_queries()` logs a warning and leaves `highlight_query` empty. `SyntaxHighlighter::new()` will still return `None` if the query is empty.
  - Parse error (malformed source): tree-sitter still produces a partial tree with `ERROR` nodes. Highlighting proceeds with available nodes. No crash.
  - The keyword-based highlighter is deleted entirely. There is no fallback to keyword highlighting.

### FR-SYNTAX-010: Supported Languages

- **Description**: Define the initial set of supported languages with tree-sitter grammars.
- **Priority**: High
- **Status**: Draft
- **Code Reference**: `LanguageRegistry::with_builtins()` in `crates/termcode-syntax/src/language.rs:50`
- **Details**:

  | Language   | File Extensions               | Grammar Crate            | Query File                                  |
  | ---------- | ----------------------------- | ------------------------ | ------------------------------------------- |
  | Rust       | `.rs`                         | `tree-sitter-rust`       | `runtime/queries/rust/highlights.scm`       |
  | Python     | `.py`, `.pyi`                 | `tree-sitter-python`     | `runtime/queries/python/highlights.scm`     |
  | JavaScript | `.js`, `.mjs`, `.cjs`         | `tree-sitter-javascript` | `runtime/queries/javascript/highlights.scm` |
  | TypeScript | `.ts`, `.tsx`                 | `tree-sitter-typescript` | `runtime/queries/typescript/highlights.scm` |
  | JSON       | `.json`, `.jsonp`, `.jsonl`   | `tree-sitter-json`       | `runtime/queries/json/highlights.scm`       |
  | Markdown   | `.md`, `.markdown`            | `tree-sitter-md`         | `runtime/queries/markdown/highlights.scm`   |
  | Bash       | `.sh`, `.bash`                | `tree-sitter-bash`       | `runtime/queries/bash/highlights.scm`       |
  | TOML       | `.toml`                       | `tree-sitter-toml`       | `runtime/queries/toml/highlights.scm`       |
  | C          | `.c`, `.h`                    | `tree-sitter-c`          | `runtime/queries/c/highlights.scm`          |
  | C++        | `.cpp`, `.cc`, `.cxx`, `.hpp` | `tree-sitter-cpp`        | `runtime/queries/cpp/highlights.scm`        |
  | Go         | `.go`                         | `tree-sitter-go`         | `runtime/queries/go/highlights.scm`         |
  - Note: Bash (`.sh`, `.bash`) is a new language not currently in `LanguageRegistry`. It must be added to `with_builtins()`.
  - Note: JSON extensions `.jsonp` and `.jsonl` are new and must be added to the existing JSON config.

### FR-SYNTAX-011: Unit Tests

- **Description**: Add unit tests for the new tree-sitter-based highlighter.
- **Priority**: High
- **Status**: Draft
- **Code Reference**: No existing tests in `crates/termcode-syntax/`
- **Details**:
  - Test cases in `crates/termcode-syntax/src/highlighter.rs` (or `tests/` directory):
    1. **Parse produces tree**: Parse a Rust source string, verify `self.tree` is `Some`.
    2. **Highlight spans for keywords**: Parse `fn main() {}`, verify spans include `keyword.function` for `fn` and `function` for `main`.
    3. **Highlight spans for strings**: Parse `let x = "hello";`, verify a `string` span covers `"hello"`.
    4. **Highlight spans for comments**: Parse `// comment`, verify a `comment` span.
    5. **Incremental update**: Parse a source, apply an edit (insert character), call `update()`, verify tree is updated and spans reflect the change.
    6. **No grammar returns None**: Attempt to create a highlighter with an empty `LanguageConfig` (no grammar), verify `None`.
    7. **Empty source**: Parse an empty string, verify no crash and empty spans.
    8. **Multi-language**: Verify highlighting works for at least Rust, Python, and JavaScript.
  - Test for `changeset_to_input_edits`: Given a known `ChangeSet` and `Rope`, verify the produced `InputEdit` values match expected byte offsets and positions.

---

## Implementation Notes

### API Changes Summary

| Current API                                      | New API                                                                       |
| ------------------------------------------------ | ----------------------------------------------------------------------------- |
| `SyntaxHighlighter::new(&LanguageId)`            | `SyntaxHighlighter::new(&LanguageConfig) -> Option<Self>`                     |
| `highlighter.highlight_line(&str) -> Vec<Span>`  | `highlighter.highlight_lines(&Rope, Range<usize>) -> Vec<Vec<HighlightSpan>>` |
| (none)                                           | `highlighter.parse(&mut self, &Rope)`                                         |
| (none)                                           | `highlighter.update(&mut self, &Rope, &[InputEdit])`                          |
| `LanguageConfig.grammar: (none)`                 | `LanguageConfig.grammar: Option<tree_sitter::Language>`                       |
| `LanguageConfig.highlight_query: String` (empty) | `LanguageConfig.highlight_query: String` (populated from .scm files)          |

### Data Flow (After)

```
[App::new()]
  -> LanguageRegistry::with_builtins()   (sets grammar for each language)
  -> lang_registry.load_queries(runtime_dir)  (reads highlights.scm files)
  -> Editor::new(lang_registry, ...)

[Editor::open_file(path)]
  -> lang_registry.detect_language(path) -> LanguageId
  -> lang_registry.get(&lang_id)         -> &LanguageConfig (with grammar + query)
  -> Document::open(id, path, &lang_config)
     -> SyntaxHighlighter::new(&lang_config)  -> Option<SyntaxHighlighter>
     -> highlighter.parse(&rope)               -> full AST parse

[After each edit (insert/delete/undo/redo)]
  -> changeset_to_input_edits(&changes, &rope) -> Vec<InputEdit>
  -> highlighter.update(&rope, &edits)          -> incremental re-parse

[Render loop]
  -> highlighter.highlight_lines(&rope, visible_range) -> Vec<Vec<HighlightSpan>>
  -> theme.resolve(&span.scope) -> Style
  -> apply styles to ratatui buffer
```

### Dependency Changes

Workspace `Cargo.toml` additions (tree-sitter and tree-sitter-highlight already declared):

```toml
tree-sitter-rust = "0.24"
tree-sitter-python = "0.24"
tree-sitter-javascript = "0.24"
tree-sitter-typescript = "0.24"
tree-sitter-json = "0.24"
tree-sitter-toml = "0.24"
tree-sitter-c = "0.24"
tree-sitter-cpp = "0.24"
tree-sitter-go = "0.24"
tree-sitter-md = "0.24"
tree-sitter-bash = "0.24"
```

These are added as dependencies of `termcode-syntax` only.

### Performance Considerations

- Initial parse: synchronous, 1-5ms for typical files (acceptable)
- Incremental re-parse: sub-millisecond for single edits
- Viewport-scoped highlighting: only process visible lines during render
- Rope chunk callback: avoid `Rope::to_string()` for tree-sitter input; use chunk-based reading
- `HighlightConfiguration` can be shared (`Arc`) across documents with the same language

### Migration

- No user-facing breaking changes (highlighting quality improves transparently)
- No configuration changes needed (theme scopes already compatible)
- No database/schema changes
- The keyword-based code (293 lines in `highlighter.rs`, keyword functions) is deleted entirely

---

## Out of Scope

- Runtime loading of compiled grammars from `.so`/`.dylib` files (compile-time crate deps only)
- Code folding, smart indentation, or structural editing via tree-sitter (future features)
- Async/background parsing for very large files
- Injected languages (e.g., JavaScript inside HTML)
- Custom user-provided grammar or query files
- `indent_query` support (architecture doc mentions this but it is a separate feature)
- Syntax-aware text objects (future feature)
