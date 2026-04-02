# Tree-sitter Syntax Highlighting Integration - Analysis Report

**Analysis Date**: 2026-04-01
**Analysis Target**: Replace keyword-based syntax highlighter with tree-sitter incremental parsing
**Related Specs**: Architecture doc section 4.2 (termcode-syntax: Tree-sitter Highlighting)

## 1. Related Code Exploration Results

### Current Implementation

The current highlighter is a naive keyword-based system in `crates/termcode-syntax/src/highlighter.rs`:
- `SyntaxHighlighter` holds a `LanguageId` and a static keyword list
- `highlight_line(&self, line: &str) -> Vec<HighlightSpan>` processes one line at a time
- Only supports Rust, Python, JavaScript/TypeScript keywords
- No multi-line construct awareness (multi-line strings, block comments are broken)
- No AST-based understanding (function names, types, fields not distinguished)

### Files in termcode-syntax Crate

| File | Purpose | Lines |
|------|---------|-------|
| `crates/termcode-syntax/src/lib.rs` | Module declarations only | 2 |
| `crates/termcode-syntax/src/highlighter.rs` | Keyword-based highlighter (to be replaced) | 293 |
| `crates/termcode-syntax/src/language.rs` | LanguageId, LanguageConfig, LanguageRegistry | 128 |

### Consumers of SyntaxHighlighter / HighlightSpan

| File | Usage | Change Required |
|------|-------|-----------------|
| `crates/termcode-view/src/document.rs:20` | `pub syntax: Option<SyntaxHighlighter>` field on Document | Yes - type changes |
| `crates/termcode-view/src/document.rs:55` | `SyntaxHighlighter::new(language_id)` in `Document::open()` | Yes - constructor changes |
| `crates/termcode-term/src/ui/editor_view.rs:192` | `.highlight_line(line_text)` during rendering | Yes - API changes |

### Reusable Code

| File | Target | Usage |
|------|--------|-------|
| `crates/termcode-theme/src/theme.rs:218` | `Theme::resolve(&self, scope: &str) -> Style` | Reuse directly - already supports dot-separated scope fallback |
| `crates/termcode-syntax/src/language.rs` | `LanguageRegistry`, `LanguageConfig`, `LanguageId` | Extend - add `grammar` field, load queries |

### Related Type Definitions

| File | Type | Description |
|------|------|-------------|
| `crates/termcode-syntax/src/highlighter.rs:5` | `HighlightSpan { byte_start, byte_end, scope: String }` | Per-line byte ranges with scope names |
| `crates/termcode-syntax/src/language.rs:6` | `LanguageId = Arc<str>` | Language identifier (e.g., "rust") |
| `crates/termcode-syntax/src/language.rs:9` | `LanguageConfig { id, name, file_extensions, highlight_query }` | Language definition - `highlight_query` exists but is always empty |
| `crates/termcode-theme/src/theme.rs:210` | `Theme.scopes: HashMap<String, Style>` | Scope name to style mapping |

### Code Flow (Current Highlighting)

```
[Document::open] → [SyntaxHighlighter::new(language_id)] → keyword list selection
     ↓
[EditorViewWidget::render] → [doc.syntax.highlight_line(line_text)] → Vec<HighlightSpan>
     ↓
[theme.resolve(&span.scope)] → Style → apply to char_styles buffer → ratatui buffer
```

| Step | File | Function | Description |
|------|------|----------|-------------|
| 1. Doc open | `crates/termcode-view/src/document.rs:48-68` | `Document::open()` | Creates SyntaxHighlighter from LanguageId |
| 2. Registry detect | `crates/termcode-view/src/editor.rs:134` | `Editor::open_file()` | Detects language via file extension |
| 3. Registry init | `crates/termcode-term/src/app.rs:93` | `App::new()` | `LanguageRegistry::with_builtins()` |
| 4. Render | `crates/termcode-term/src/ui/editor_view.rs:188-205` | EditorViewWidget render | Calls highlight_line per visible line |
| 5. Style resolve | `crates/termcode-term/src/ui/editor_view.rs:200` | render loop | `theme.resolve(&span.scope)` for each span |

## 2. Impact Scope Analysis

### Directly Affected Files

| File | Change Type | Risk |
|------|-------------|------|
| `crates/termcode-syntax/src/highlighter.rs` | **Rewrite** | High - core replacement |
| `crates/termcode-syntax/src/language.rs` | **Modify** | High - add grammar field, query loading |
| `crates/termcode-syntax/src/lib.rs` | **Modify** | Low - may add modules |
| `crates/termcode-syntax/Cargo.toml` | **Modify** | Low - add tree-sitter grammar crates |
| `Cargo.toml` (workspace) | **Modify** | Low - add grammar deps to workspace |
| `crates/termcode-view/src/document.rs` | **Modify** | Medium - SyntaxHighlighter API changes |
| `crates/termcode-term/src/ui/editor_view.rs` | **Modify** | Medium - rendering call changes |

### Indirectly Affected Files

| File | Impact Reason |
|------|---------------|
| `crates/termcode-term/src/app.rs` | LanguageRegistry initialization may change (loading queries from runtime/) |
| `crates/termcode-view/src/editor.rs` | If SyntaxHighlighter needs mutable access for incremental updates |
| `crates/termcode-term/src/command.rs` | If edit commands need to notify highlighter of changes for incremental re-parse |
| `runtime/queries/rust/highlights.scm` | Must be created - currently empty directory |

### Test Impact

| Test File | Status |
|-----------|--------|
| (none) | No existing tests for termcode-syntax crate |
| `crates/termcode-plugin/tests/integration.rs` | Uses `LanguageRegistry::new()` - may need update |
| `crates/termcode-term/src/mouse.rs:358` | Test uses `LanguageRegistry::new()` - may need update |

## 3. Architecture Analysis

### Current Structure

```
[App::new()]
  → LanguageRegistry::with_builtins()  (hardcoded language list, no grammars)
  → Editor::new(lang_registry, ...)
     → Editor.open_file(path)
        → lang_registry.detect_language(path)  → LanguageId
        → Document::open(id, path, lang_id)
           → SyntaxHighlighter::new(&lang_id)  → keyword lookup

[Render loop]
  → EditorViewWidget.render()
     → doc.syntax.highlight_line(line_text)  → Vec<HighlightSpan> (per line, stateless)
     → theme.resolve(&span.scope)            → Style
```

### Target Structure (from architecture doc)

```
[App::new()]
  → LanguageRegistry::load_from_runtime_dir()  (loads grammars + queries)
  → Editor::new(lang_registry, ...)
     → Editor.open_file(path)
        → lang_registry.detect_language(path) → LanguageId
        → lang_registry.get(&lang_id)         → &LanguageConfig (with grammar)
        → Document::open(id, path, lang_config)
           → SyntaxHighlighter::new(&lang_config)  → parser + tree
           → highlighter.parse(&rope)               → full AST parse

[After each edit]
  → highlighter.update(&rope, &[InputEdit])  → incremental re-parse

[Render loop]
  → highlighter.highlight_lines(&rope, visible_range) → Vec<HighlightSpan>
  → theme.resolve(&span.scope) → Style
```

### Key Architectural Changes

1. **SyntaxHighlighter becomes stateful**: Must hold `tree_sitter::Parser`, `tree_sitter::Tree`, and `HighlightConfiguration`. Currently stateless (just keyword list).

2. **Full-document awareness**: Tree-sitter parses the entire document (not line-by-line). The `highlight_lines` method extracts spans for a viewport range from the parsed tree.

3. **Incremental updates**: After each edit (`Transaction` apply, undo, redo), the tree must be updated via `tree_sitter::Tree::edit()` + `Parser::parse()` with the old tree.

4. **Grammar loading**: Need tree-sitter grammar crates (e.g., `tree-sitter-rust`, `tree-sitter-python`) as dependencies, or load compiled grammars from `runtime/grammars/`.

5. **Query files**: `runtime/queries/{lang}/highlights.scm` must be populated with tree-sitter highlight queries.

### Extension Points

- **LanguageConfig**: Add `grammar: tree_sitter::Language` field (requires tree-sitter grammar crates)
- **SyntaxHighlighter**: Replace entirely with tree-sitter based implementation
- **Document**: Change `syntax` field type, add `update` call after mutations
- **Editor or App**: Wire edit notifications to trigger incremental re-parse

### Constraints

- **termcode-view is frontend-agnostic**: `SyntaxHighlighter` currently lives in `termcode-syntax` which `termcode-view` depends on. Tree-sitter is not a terminal dependency, so this layer boundary is maintained.
- **No closures in CommandHandler**: Edit notifications to highlighter must be wired explicitly, not via callback closures.
- **Performance**: Tree-sitter parse should NOT block the render loop. Initial parse could be synchronous for small files, but large files may need async or background parsing.
- **Graceful degradation**: If a grammar is unavailable, fall back to no highlighting (not keyword-based).

### Cross-cutting Concerns

| Concern | Pattern | Files |
|---------|---------|-------|
| Error Handling | `anyhow::Result` throughout, graceful degradation | All modified files |
| Performance | Viewport-scoped highlighting, incremental parsing | `highlighter.rs`, `editor_view.rs` |
| Resource Loading | Runtime directory scanning | `language.rs`, `app.rs` |

### Component Interfaces

| From | To | Contract |
|------|-----|----------|
| Document | SyntaxHighlighter | `highlight_lines(&self, source: &Rope, range: Range<usize>) -> Vec<HighlightSpan>` |
| Document | SyntaxHighlighter | `update(&mut self, source: &Rope, edits: &[InputEdit])` (new) |
| LanguageRegistry | LanguageConfig | Must now include `grammar: tree_sitter::Language` |
| EditorViewWidget | Document.syntax | Calls highlight method during render |

## 4. Technical Considerations

### Dependencies to Add

Tree-sitter grammar crates (compile grammars into binary):

| Crate | Language | Cargo.toml Key |
|-------|----------|----------------|
| `tree-sitter-rust` | Rust | workspace dep |
| `tree-sitter-python` | Python | workspace dep |
| `tree-sitter-javascript` | JavaScript | workspace dep |
| `tree-sitter-typescript` | TypeScript | workspace dep |
| `tree-sitter-json` | JSON | workspace dep |
| `tree-sitter-toml` | TOML | workspace dep |
| `tree-sitter-c` | C | workspace dep |
| `tree-sitter-cpp` | C++ | workspace dep |
| `tree-sitter-go` | Go | workspace dep |
| `tree-sitter-md` | Markdown | workspace dep |

**Decision point**: Compile grammars into binary (via crate deps) vs. load `.so`/`.dylib` from `runtime/grammars/` at runtime. The architecture doc mentions `runtime/grammars/` but using crate deps is simpler and more reliable for initial implementation. Helix uses crate deps compiled at build time.

### Highlight Query Files

`runtime/queries/{lang}/highlights.scm` files are needed. These can be sourced from:
- Official tree-sitter grammar repos (each includes `queries/highlights.scm`)
- Helix editor's runtime queries (well-curated, MIT licensed)
- nvim-treesitter queries

Currently `runtime/queries/rust/` exists but is empty.

### Performance

- **Initial parse**: `Parser::parse()` on full source text. ~1-5ms for typical files, acceptable synchronously.
- **Incremental parse**: After `Tree::edit()`, re-parse is very fast (sub-millisecond for single edits).
- **Highlight extraction**: `tree-sitter-highlight` crate iterates events. Viewport-scoped extraction is important for large files.
- **Memory**: Each document holds a `tree_sitter::Tree` (compact representation, typically < 1MB even for large files).

### Rope-to-bytes Conversion

Tree-sitter's `Parser::parse()` takes a callback `impl FnMut(usize, Point) -> &[u8]` for reading source text. This can work directly with `ropey::Rope` chunks without converting the entire rope to a `String`. This is a key optimization.

### InputEdit Mapping

`tree_sitter::InputEdit` requires:
- `start_byte`, `old_end_byte`, `new_end_byte`
- `start_position`, `old_end_position`, `new_end_position` (row/col Points)

These can be derived from `Transaction.changes` (which tracks byte-level changes). The mapping from `ChangeSet` to `Vec<InputEdit>` is non-trivial but essential for incremental parsing.

### Security

- No security concerns - tree-sitter grammars are sandboxed parsers
- Compiled grammars (via crate deps) are reviewed at build time

### Migration

- **DB schema change**: No
- **Data migration**: No
- **Config change**: No (scopes in themes already use tree-sitter-compatible names)
- **Breaking change**: No user-facing breaking change; highlighting quality improves transparently

## 5. Recommendations

### Recommended Approach

**Phase 1: Core tree-sitter integration (Rust only)**
1. Add `tree-sitter-rust` crate dependency
2. Rewrite `SyntaxHighlighter` to use `tree_sitter::Parser` + `tree_sitter_highlight::Highlighter`
3. Add `highlights.scm` for Rust to `runtime/queries/rust/`
4. Update `LanguageConfig` to hold `Option<tree_sitter::Language>`
5. Update `Document` to use new API
6. Update `EditorViewWidget` rendering

**Phase 2: Incremental parsing**
1. Wire `Transaction` application to `SyntaxHighlighter::update()`
2. Map `ChangeSet` operations to `tree_sitter::InputEdit`
3. Implement `parse()` with `old_tree` parameter for incremental re-parse

**Phase 3: Additional languages**
1. Add grammar crates for remaining languages
2. Add `highlights.scm` query files for each
3. Extend `LanguageRegistry::with_builtins()` to include grammars

### Patterns to Avoid

- Direct `Rope::to_string()` for tree-sitter input (use chunk-based callback)
- Highlighting entire document on every render (viewport-scoped only)
- Blocking the event loop on initial parse of very large files
- Tight coupling between tree-sitter types and `termcode-view` layer

### Recommended Reusable Code

| Code | Location | Purpose |
|------|----------|---------|
| `Theme::resolve()` | `crates/termcode-theme/src/theme.rs:218` | Scope-to-style resolution with dot-separated fallback |
| `LanguageRegistry` | `crates/termcode-syntax/src/language.rs` | Language detection by extension |
| `HighlightSpan` | `crates/termcode-syntax/src/highlighter.rs:5` | Keep struct (may rename field `scope` to `highlight`) |

## 6. Technical Debt

### Identified Issues

| Area | Issue | Recommendation | Priority |
|------|-------|----------------|----------|
| `termcode-syntax` | No tests at all | Add unit tests for new highlighter | High |
| `highlighter.rs` | 293 lines of keyword matching (dead code after migration) | Delete entirely | High |
| `language.rs:57-117` | `highlight_query: String::new()` on all configs | Populate with actual queries or remove field | Medium |
| `editor_view.rs:185-186` | `rope_line.chars().collect::<String>()` per line on every render | Consider rope chunk-based rendering | Low |

### Improvement Opportunities

- Tree-sitter enables future features: code folding, smart indentation, structural editing, textobjects
- `LanguageConfig.indent_query` field is in architecture doc but not yet implemented - could be added alongside highlight queries
- Consider caching highlight spans per line with invalidation on edit (avoid re-computing unchanged lines)

## 7. Design Recommendation

**design_recommendation**: architecture

**Rationale**: This change modifies a core subsystem (syntax highlighting) that touches the data model (`Document.syntax`), the rendering pipeline (`EditorViewWidget`), and introduces new state management patterns (incremental tree updates after edits). The API contract between `SyntaxHighlighter` and its consumers changes fundamentally (line-based stateless -> document-based stateful). Architecture design is needed to define the exact API surface, the edit notification mechanism, and the grammar loading strategy.

## 8. Essential Files (Must Read)

Files that MUST be read before proceeding to yyy-plan:

| File | Reason | Priority |
|------|--------|----------|
| `crates/termcode-syntax/src/highlighter.rs` | Current implementation to replace | Required |
| `crates/termcode-syntax/src/language.rs` | LanguageConfig/Registry to extend | Required |
| `crates/termcode-view/src/document.rs` | Document struct that holds SyntaxHighlighter | Required |
| `crates/termcode-term/src/ui/editor_view.rs` (lines 180-210) | Rendering code that consumes highlight spans | Required |
| `crates/termcode-theme/src/theme.rs` (lines 205-232) | Theme scope resolution (compatible, no change needed) | Recommended |
| `crates/termcode-term/src/app.rs` (lines 85-100) | App initialization where LanguageRegistry is created | Recommended |
| `docs/architecture/termcode.md` (section 4.2) | Target architecture for tree-sitter integration | Recommended |
| `runtime/themes/one-dark.toml` (scopes section) | Existing scope names already tree-sitter compatible | Recommended |
| `Cargo.toml` (workspace deps) | tree-sitter 0.24 already declared | Recommended |

**IMPORTANT**: yyy-plan agent MUST read all Required files before creating implementation plan.

## 9. Handoff to yyy-plan

### Notes
- tree-sitter 0.24.7 and tree-sitter-highlight 0.24.7 are already in workspace deps (declared but unused)
- Theme scope names (`keyword`, `function`, `string`, `type`, `comment`, etc.) already follow tree-sitter conventions - no theme changes needed
- `LanguageConfig` already has a `highlight_query: String` field, just needs to be populated
- The `runtime/queries/rust/` directory exists but is empty
- No existing tests in `termcode-syntax` - must add tests as part of implementation
- Architecture doc section 4.2 specifies the target API in detail

### Key Decisions Needed in Planning
1. **Grammar loading strategy**: Crate deps (recommended) vs. runtime `.so` loading
2. **Incremental parse wiring**: Where to call `highlighter.update()` after edits - in `Document::apply_transaction()` or in `App` after dispatch?
3. **Rope integration**: Direct chunk callback vs. `Rope::to_string()` for small files
4. **Phasing**: All languages at once vs. Rust-first then expand

### Recommended Phase Structure
1. Core infrastructure: tree-sitter grammar deps, query files, `SyntaxHighlighter` rewrite
2. Document integration: Update `Document` and `LanguageConfig`, wire up initial parse
3. Rendering integration: Update `EditorViewWidget` to use new highlight API
4. Incremental parsing: Wire edit notifications, map ChangeSet to InputEdit
5. Additional languages: Add remaining grammar crates and query files
6. Tests: Unit tests for highlighter, integration tests for full pipeline
