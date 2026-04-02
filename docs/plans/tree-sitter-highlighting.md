# Tree-sitter Syntax Highlighting Implementation Plan

**Created**: 2026-04-01
**Analysis Report**: docs/analysis/tree-sitter-highlighting.md
**Specification**: docs/specs/tree-sitter-highlighting.md
**Status**: Pending

## 1. Requirements Summary

### Functional Requirements

- [FR-SYNTAX-001] Replace keyword-based SyntaxHighlighter with tree-sitter Parser + HighlightConfiguration
- [FR-SYNTAX-002] Full-document parsing on open via Rope chunk callback (no Rope::to_string)
- [FR-SYNTAX-003] Viewport-scoped highlighting via highlight_lines() replacing per-line highlight_line()
- [FR-SYNTAX-004] Incremental re-parsing after every edit (insert, delete, undo, redo, search-replace)
- [FR-SYNTAX-005] Extend LanguageConfig with grammar field, load highlight queries from runtime/queries/
- [FR-SYNTAX-006] Provide highlights.scm query files for 11 languages
- [FR-SYNTAX-007] Update Document to use new SyntaxHighlighter API and wire incremental updates
- [FR-SYNTAX-008] Update EditorViewWidget rendering to use highlight_lines()
- [FR-SYNTAX-009] Graceful degradation when grammar unavailable (no crash, fall back to no highlighting)
- [FR-SYNTAX-010] Support 11 languages: Rust, Python, JS, TS, JSON, Markdown, Bash, TOML, C, C++, Go
- [FR-SYNTAX-011] Unit tests for the new highlighter

### Database

- None

### API

- None

### UI

- No UI changes (highlighting quality improves transparently)

## 2. Analysis Report Reference

### Reference Documents

- Analysis Report: `docs/analysis/tree-sitter-highlighting.md`
- Specification: `docs/specs/tree-sitter-highlighting.md`

### Applied Recommendations

- Use compile-time grammar crates (not runtime .so loading) for simplicity and reliability
- Rewrite SyntaxHighlighter entirely (not incremental refactor of keyword-based code)
- Use Rope chunk-based callback for tree-sitter parsing (avoid Rope::to_string)
- Viewport-scoped highlighting to avoid processing entire document on every render
- Share HighlightConfiguration via Arc across documents with the same language
- Phase incrementally: Rust first, then remaining languages

### Reusable Code

| Code                     | Location                                      | Purpose                                                                               |
| ------------------------ | --------------------------------------------- | ------------------------------------------------------------------------------------- |
| `Theme::resolve(&scope)` | `crates/termcode-theme/src/theme.rs:218`      | Scope-to-style resolution with dot-separated fallback; already tree-sitter compatible |
| `LanguageRegistry`       | `crates/termcode-syntax/src/language.rs`      | Language detection by extension; extend with grammar field                            |
| `HighlightSpan` struct   | `crates/termcode-syntax/src/highlighter.rs:5` | Preserve struct signature (byte_start, byte_end, scope)                               |
| `ChangeSet` ops          | `crates/termcode-core/src/transaction.rs`     | Map to tree_sitter::InputEdit for incremental parsing                                 |

### Constraints

- termcode-syntax is Layer 1: depends only on termcode-core and termcode-theme. Tree-sitter grammar crates are pure parsers with no terminal deps, so this is safe.
- termcode-view is frontend-agnostic: SyntaxHighlighter lives in termcode-syntax, not termcode-view. Layer boundary maintained.
- No closures in CommandHandler: Edit notifications to highlighter must be wired explicitly in Document methods.
- ChangeSet.ops is private: Must add a public accessor method (e.g., `ops()` or `iter()`) on ChangeSet to enable InputEdit mapping.
- Performance: Initial parse is synchronous (acceptable for typical files). No async parsing in this iteration.

## 3. Impact Analysis (Dry-Run)

### Overall Risk: Medium

### Files to Create

| File                                        | Risk | Description                                  |
| ------------------------------------------- | ---- | -------------------------------------------- |
| `runtime/queries/rust/highlights.scm`       | Low  | Tree-sitter highlight queries for Rust       |
| `runtime/queries/python/highlights.scm`     | Low  | Tree-sitter highlight queries for Python     |
| `runtime/queries/javascript/highlights.scm` | Low  | Tree-sitter highlight queries for JavaScript |
| `runtime/queries/typescript/highlights.scm` | Low  | Tree-sitter highlight queries for TypeScript |
| `runtime/queries/json/highlights.scm`       | Low  | Tree-sitter highlight queries for JSON       |
| `runtime/queries/markdown/highlights.scm`   | Low  | Tree-sitter highlight queries for Markdown   |
| `runtime/queries/bash/highlights.scm`       | Low  | Tree-sitter highlight queries for Bash       |
| `runtime/queries/toml/highlights.scm`       | Low  | Tree-sitter highlight queries for TOML       |
| `runtime/queries/c/highlights.scm`          | Low  | Tree-sitter highlight queries for C          |
| `runtime/queries/cpp/highlights.scm`        | Low  | Tree-sitter highlight queries for C++        |
| `runtime/queries/go/highlights.scm`         | Low  | Tree-sitter highlight queries for Go         |

### Files to Modify

| File                                         | Risk   | Description                                                                                                                         |
| -------------------------------------------- | ------ | ----------------------------------------------------------------------------------------------------------------------------------- |
| `Cargo.toml` (workspace)                     | Low    | Add 11 tree-sitter grammar crate deps to workspace dependencies                                                                     |
| `crates/termcode-syntax/Cargo.toml`          | Low    | Add grammar crate deps                                                                                                              |
| `crates/termcode-syntax/src/highlighter.rs`  | High   | Complete rewrite: tree-sitter Parser + HighlightConfiguration replacing keyword-based system (293 lines)                            |
| `crates/termcode-syntax/src/language.rs`     | High   | Add `grammar: Option<tree_sitter::Language>` to LanguageConfig, add Bash language, add JSON extensions, add `load_queries()` method |
| `crates/termcode-syntax/src/lib.rs`          | Low    | Potentially add new module declarations                                                                                             |
| `crates/termcode-core/src/transaction.rs`    | Medium | Add public accessor for ChangeSet.ops (needed for InputEdit mapping)                                                                |
| `crates/termcode-view/src/document.rs`       | Medium | Update Document::open() to use new SyntaxHighlighter API, wire incremental updates in apply_transaction/undo/redo                   |
| `crates/termcode-view/src/editor.rs`         | Medium | Pass LanguageConfig (not just LanguageId) to Document::open()                                                                       |
| `crates/termcode-term/src/ui/editor_view.rs` | Medium | Replace highlight_line() calls with highlight_lines() for viewport range                                                            |
| `crates/termcode-term/src/app.rs`            | Low    | Call lang_registry.load_queries() after with_builtins()                                                                             |

### Files to Delete

| File | Risk | Description                                                                                                                                                                            |
| ---- | ---- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| None | -    | No files deleted. The keyword functions (rust_keywords, python_keywords, js_keywords) are removed as part of the highlighter.rs rewrite, but the file itself is preserved (rewritten). |

### Destructive Operations

- None. The keyword-based highlighter code is replaced in-place (not a separate file deletion). All changes are reversible via git.

### Rollback Plan

- Full rollback via branch deletion: `git checkout main && git branch -D feature/tree-sitter-highlighting`
- No DB migrations or config changes to revert
- All grammar crate additions can be reverted by restoring Cargo.toml files
- Query files in `runtime/queries/` are new additions only (safe to delete)

## 4. Implementation Order

### Phase 1: Dependencies and Query Files

**Goal**: Add all tree-sitter grammar crate dependencies and create highlight query files
**Risk**: Low
**Status**: Complete

- [x] Task 1.1: Add grammar crate dependencies to workspace `Cargo.toml`
  - `tree-sitter-rust`, `tree-sitter-python`, `tree-sitter-javascript`, `tree-sitter-typescript`, `tree-sitter-json`, `tree-sitter-toml`, `tree-sitter-c`, `tree-sitter-cpp`, `tree-sitter-go`, `tree-sitter-md`, `tree-sitter-bash`
  - Pin to 0.24.x versions compatible with tree-sitter 0.24 already in workspace
- [x] Task 1.2: Add grammar crate dependencies to `crates/termcode-syntax/Cargo.toml`
- [x] Task 1.3: Create `runtime/queries/{lang}/highlights.scm` for all 11 languages
  - Source from official tree-sitter grammar repos (MIT licensed)
  - Ensure scope names match existing theme scopes: `keyword`, `keyword.control`, `keyword.function`, `function`, `type`, `type.builtin`, `variable`, `variable.builtin`, `string`, `comment`, `constant`, `constant.numeric`, `constructor`, `operator`, `punctuation`, `punctuation.bracket`, `punctuation.delimiter`, `attribute`, `label`, `namespace`
- [x] Task 1.4: Verify `cargo build` succeeds with new dependencies

### Phase 2: LanguageConfig Extension

**Goal**: Extend LanguageConfig with grammar field and query loading, add Bash language, update JSON extensions
**Risk**: High
**Status**: Complete

- [x] Task 2.1: Add `grammar: Option<tree_sitter::Language>` field to `LanguageConfig` in `crates/termcode-syntax/src/language.rs`
- [x] Task 2.2: Update `LanguageRegistry::with_builtins()` to set grammar for each language
  - e.g., `grammar: Some(tree_sitter_rust::LANGUAGE.into())`
  - Add new Bash language entry: `id: "bash"`, extensions: `["sh", "bash"]`
  - Add `.jsonp` and `.jsonl` extensions to JSON config
- [x] Task 2.3: Add `LanguageRegistry::load_queries(&mut self, runtime_dir: &Path)` method
  - Scan `runtime_dir/queries/{lang_id}/highlights.scm` for each registered language
  - Populate `highlight_query` field with file contents
  - Log warning if query file not found (graceful degradation)
- [x] Task 2.4: Add public accessor `ChangeSet::ops(&self) -> &[Operation]` in `crates/termcode-core/src/transaction.rs`
  - Required for Phase 4 (InputEdit mapping)
  - Also ensure `Operation` is publicly accessible
- [x] Task 2.5: Update `App::new()` in `crates/termcode-term/src/app.rs` to call `lang_registry.load_queries(runtime_dir)` after `with_builtins()`
  - Runtime directory resolution: check `runtime/` relative to binary, then `~/.config/termcode/`
- [x] Task 2.6: Verify build and existing tests pass

### Phase 3: SyntaxHighlighter Rewrite

**Goal**: Replace keyword-based highlighter with tree-sitter implementation
**Risk**: High
**Status**: Complete

- [x] Task 3.1: Rewrite `SyntaxHighlighter` struct in `crates/termcode-syntax/src/highlighter.rs`
  - Fields: `parser: tree_sitter::Parser`, `tree: Option<tree_sitter::Tree>`, `highlight_config: Arc<HighlightConfiguration>`, `highlight_names: Arc<[String]>`
  - Constructor: `SyntaxHighlighter::new(config: &LanguageConfig) -> Option<Self>`
  - Returns `None` if `config.grammar` is `None` or `config.highlight_query` is empty
- [x] Task 3.2: Implement `parse(&mut self, source: &Rope)` method
  - Use `Parser::parse_with()` with Rope chunk-based callback
  - Callback reads from appropriate Rope chunk at given byte offset
  - Store resulting Tree in `self.tree`
- [x] Task 3.3: Implement `highlight_lines(&self, source: &Rope, line_range: Range<usize>) -> Vec<Vec<HighlightSpan>>`
  - Use `tree_sitter_highlight::Highlighter::highlight()` to iterate HighlightEvents
  - Filter events to viewport byte range
  - Return per-line spans with byte offsets relative to line start
  - Return empty if `self.tree` is `None`
- [x] Task 3.4: Preserve `HighlightSpan` struct with same fields (byte_start, byte_end, scope: String)
- [x] Task 3.5: Delete keyword functions (rust_keywords, python_keywords, js_keywords)
- [x] Task 3.6: Unit tests for SyntaxHighlighter
  - Parse produces tree (Rust source)
  - Highlight spans for keywords (fn main)
  - Highlight spans for strings, comments, numbers
  - No grammar returns None
  - Empty source produces no crash

### Phase 4: Document and Editor Integration

**Goal**: Wire new SyntaxHighlighter into Document lifecycle and incremental updates
**Risk**: Medium
**Status**: Complete

- [x] Task 4.1: Update `Document::open()` in `crates/termcode-view/src/document.rs`
  - Accept `Option<&LanguageConfig>` instead of `Option<LanguageId>` (or accept both)
  - Create `SyntaxHighlighter::new(&config)` (returns Option)
  - Call `highlighter.parse(&buffer.rope())` immediately after creation
  - Preserve `language_id` field for display/LSP purposes
- [x] Task 4.2: Update `Editor::open_file()` in `crates/termcode-view/src/editor.rs`
  - After `detect_language()`, get `&LanguageConfig` via `lang_registry.get()`
  - Pass config to `Document::open()`
- [x] Task 4.3: Implement `changeset_to_input_edits()` helper
  - Location: `crates/termcode-syntax/src/highlighter.rs` (or new `edit.rs` module)
  - Map ChangeSet operations (Retain/Insert/Delete) to `Vec<tree_sitter::InputEdit>`
  - Derive row/col Points from Rope via `byte_to_line()` and `line_to_byte()`
- [x] Task 4.4: Wire incremental updates in Document
  - In `apply_transaction()`: after `buffer.apply()`, compute InputEdits from the transaction changes and call `syntax.update(&rope, &edits)`
  - In `undo()`: after `buffer.apply(&inverse)`, compute InputEdits and call `syntax.update()`
  - In `redo()`: after `buffer.apply(&txn)`, compute InputEdits and call `syntax.update()`
- [x] Task 4.5: Implement `SyntaxHighlighter::update(&mut self, source: &Rope, edits: &[InputEdit])`
  - Call `tree.edit(&input_edit)` for each edit
  - Call `parser.parse_with(callback, Some(&old_tree))` for incremental re-parse
  - Store new tree

### Phase 5: Rendering Integration

**Goal**: Update EditorViewWidget to use viewport-scoped highlighting
**Risk**: Medium
**Status**: In Progress (manual verification pending)

- [x] Task 5.1: Update `EditorViewWidget::render()` in `crates/termcode-term/src/ui/editor_view.rs`
  - Before per-line loop, call `doc.syntax.highlight_lines(&rope, first_line..last_line)`
  - In per-line loop, index into result by `line_idx - first_line`
  - Remove old `highlight_line()` call
  - If no spans available, fall back to default foreground color (unchanged behavior)
- [ ] Task 5.2: Verify visual highlighting works for Rust files (manual)
- [ ] Task 5.3: Verify visual highlighting for Python and JavaScript files (manual)
- [ ] Task 5.4: Verify graceful degradation for unsupported languages (manual)

### Phase 6: Tests and Polish

**Goal**: Comprehensive tests and final verification
**Risk**: Low
**Status**: Complete

- [x] Task 6.1: Add unit tests for incremental update
  - Parse source, apply edit, call update(), verify tree reflects change
- [x] Task 6.2: Add test for changeset_to_input_edits mapping
  - Known ChangeSet + Rope -> verify InputEdit byte offsets and positions
- [x] Task 6.3: Add multi-language tests
  - Verify highlighting for Rust, Python, JavaScript at minimum
- [x] Task 6.4: Update any existing tests that use `SyntaxHighlighter::new()` or `LanguageRegistry`
  - `crates/termcode-plugin/tests/integration.rs` (uses `LanguageRegistry::new()`)
  - `crates/termcode-term/src/mouse.rs:358` (uses `LanguageRegistry::new()`)
- [x] Task 6.5: Run full test suite: `cargo test --workspace`
- [x] Task 6.6: Run clippy: `cargo clippy --workspace` (0 warnings)
- [x] Task 6.7: Run format check: `cargo fmt --check`

## 5. Quality Gate

- [x] Build success: `cargo build`
- [x] Tests pass: `cargo test --workspace`
- [x] Lint pass: `cargo clippy --workspace` (0 warnings)
- [x] Format pass: `cargo fmt --check`
- [ ] Manual verification: Open a Rust file and confirm accurate syntax highlighting
- [ ] Manual verification: Open a Python file and confirm highlighting
- [ ] Manual verification: Open an unsupported file type and confirm no crash (graceful degradation)
- [ ] Manual verification: Edit a file and confirm highlighting updates incrementally (no visual glitches)

## 6. Notes

### Implementation Considerations

- The `tree-sitter-highlight` crate's `Highlighter::highlight()` method creates a new highlighter instance per call. Consider caching or reusing the `Highlighter` instance per render if profiling shows overhead.
- `HighlightConfiguration::new()` is expensive (compiles the query). It should be created once per language and shared via `Arc` across documents with the same language. Consider storing `Arc<HighlightConfiguration>` in `LanguageConfig` after query loading.
- The `highlight_names` list must be set on the `HighlightConfiguration` to map highlight indices to scope names. This list should match the theme's known scopes.
- For the Rope-to-bytes callback in `parse_with()`, use `Rope::chunk_at_byte()` to get the chunk containing the requested byte offset, then return a slice from that chunk. This avoids allocating a full String.

### Patterns to Avoid (from Analysis)

- Do NOT use `Rope::to_string()` for tree-sitter input; use chunk-based callback
- Do NOT highlight the entire document on every render; use viewport-scoped extraction
- Do NOT block the event loop on initial parse of very large files (synchronous is fine for typical files, but be aware of the upper bound)
- Do NOT tightly couple tree-sitter types with `termcode-view` layer; keep tree-sitter types in `termcode-syntax`

### Key Design Decisions

1. **Grammar loading**: Compile-time crate dependencies (not runtime .so loading). Simpler, more reliable, follows Helix pattern.
2. **Incremental parse wiring**: In `Document` methods (apply_transaction, undo, redo), not in `App`. Keeps syntax update close to buffer mutation.
3. **Rope integration**: Chunk-based callback for `parse_with()`. Performance-critical for large files.
4. **HighlightConfiguration sharing**: Via `Arc` per language, created once during query loading.
5. **ChangeSet access**: Add `ops()` accessor to ChangeSet rather than making the field public.

### Potential Issues

- Grammar crate version compatibility: All grammar crates must be compatible with `tree-sitter = "0.24"`. Some grammar crates may have moved to 0.25+. Pin exact versions during dependency resolution.
- TypeScript has two sub-languages (typescript and tsx). `tree-sitter-typescript` provides both. Need to handle this in LanguageConfig (may need two separate configs or a combined approach).
- Query file size: Some highlight query files can be large (500+ lines). This is loaded once at startup, so not a performance concern, but the files should be well-curated.
- The `ChangeSet.ops` field is currently private with no accessor. Phase 2 adds one. If this change to termcode-core is undesirable, an alternative is to compute InputEdits by replaying the transaction against the rope positions.

## 7. Implementation Notes

### Phase 1 (2026-04-01)

- Created: 11 highlights.scm query files
- Modified: 2 Cargo.toml files (workspace + termcode-syntax)
- Risk: Low
- Notes: All 11 grammar crate dependencies added. tree-sitter-toml 0.20 requires version bridge.

### Phase 2 (2026-04-01)

- Modified: 4 files (language.rs, transaction.rs, app.rs, Cargo.toml)
- Risk: Medium
- Notes: Added grammar field to LanguageConfig, load_queries method, Bash language, TOML bridge via unsafe transmute due to tree-sitter 0.20 vs 0.24 ABI difference. Added ChangeSet::ops() accessor.

### Phase 3 (2026-04-01)

- Modified: 5 files (highlighter.rs full rewrite, Cargo.toml, document.rs, editor.rs, editor_view.rs)
- Risk: High
- Notes: Upgraded tree-sitter from 0.24 to 0.25 to resolve ABI version mismatch (grammar ABI 15 requires tree-sitter >= 0.25). Complete rewrite of SyntaxHighlighter with tree-sitter Parser + HighlightConfiguration. Pulled forward Document::open and rendering integration since they were needed for compilation.

### Phase 4 (2026-04-01)

- Modified: 2 files (document.rs, highlighter.rs)
- Risk: Medium
- Notes: Wired incremental syntax updates in apply_transaction/undo/redo. Added changeset_to_input_edits conversion helper.

### Phase 5 (2026-04-01)

- Already completed as part of Phase 3
- Notes: Viewport-scoped highlighting via highlight_lines() pre-computed before per-line render loop.

### Phase 6 (2026-04-01)

- Modified: 5 query files (fixed invalid node types), highlighter.rs (added tests)
- Risk: Low
- Notes: Fixed query compatibility issues across bash (no return keyword), c/cpp (no preprocessor literals), markdown (block-only grammar), javascript/typescript (super is named node, not keyword). All 11 grammars validated by per-grammar unit tests. 23 total syntax tests added.

### Key Deviations from Plan

- tree-sitter upgraded from 0.24 to 0.25 (ABI version incompatibility)
- highlight_lines uses Rope::to_string for the highlight path (tree-sitter-highlight API requires &[u8])
- Several query files needed corrections for grammar node type compatibility
- Phases 4 and 5 partially completed during Phase 3 due to build dependencies

---

Last Updated: 2026-04-01
Status: Complete (manual verification pending)
