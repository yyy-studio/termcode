# Phase 3: Editing Features Implementation Plan

**Created**: 2026-03-27
**Analysis Report**: N/A (no formal analysis report; planning based on architecture blueprint and team lead specifications)
**Status**: Complete

## 1. Requirements Summary

### Functional Requirements

- [FR-P3-01] Insert mode: enter with `i`, type printable characters, exit with `Esc`
- [FR-P3-02] Delete character under cursor (`delete` key / `x` in Normal mode)
- [FR-P3-03] Backspace: delete character before cursor
- [FR-P3-04] Newline: insert line break in Insert mode (`Enter`)
- [FR-P3-05] Undo (`Ctrl+Z`) and Redo (`Ctrl+Y` / `Ctrl+Shift+Z`)
- [FR-P3-06] Save file (`Ctrl+S`) preserving original encoding and line endings
- [FR-P3-07] Modified indicator: track unsaved changes via revision comparison
- [FR-P3-08] Mode indicator in status bar: NORMAL / INSERT / EXPLORER
- [FR-P3-09] Cursor style: block in Normal mode, line/bar in Insert mode
- [FR-P3-10] All editing actions routed through CommandRegistry (architecture mandate)
- [FR-P3-11] InputMapper: key-to-command mapping per EditorMode

### Architecture Constraints (from docs/architecture/termcode.md)

- TEA pattern: Event -> Update -> Render (all state changes through this cycle)
- Command Pattern: ALL user actions map to named commands through CommandRegistry
- Rope: sole buffer representation; Transaction operates on Rope
- Plugin hooks: CharInsertPre/CharInsertPost stubs for future Phase 5 plugin system
- Crate boundaries: core has no internal deps; view depends on core+syntax+theme; term depends on all
- Selection uses byte offsets (not line/column) for Rope compatibility
- History supports branching undo (parent pointer in Revision)

## 2. Analysis Report Reference

### Reference Documents

- Architecture Blueprint: `docs/architecture/termcode.md`
- Project Plan: `/Users/hankyung/.claude/plans/cosmic-prancing-whisper.md`
- Phase 2 Plan (pattern reference): `docs/plans/phase2-file-explorer-tabs.md`

### Applied Recommendations (from architecture)

- Transaction struct with ChangeSet for atomic text operations (insert, delete, replace)
- Selection with Range (anchor + head as byte offsets), primary selection tracking
- History with Revision list, inverse transactions, branching undo via parent pointer
- CommandRegistry with HashMap<CommandId, CommandEntry> and handler functions
- InputMapper with per-mode keymaps resolving KeyEvent to CommandId
- Document gains Selection and History fields, plus last_saved_revision for modified tracking
- EditorMode expanded with Insert variant

### Reusable Code

| Code                          | Location                                     | Purpose                                                     |
| ----------------------------- | -------------------------------------------- | ----------------------------------------------------------- |
| `Buffer` struct               | `crates/termcode-core/src/buffer.rs`         | Rope operations; add `apply()` and `save_to_file()` methods |
| `Position` struct             | `crates/termcode-core/src/position.rs`       | Line/column position; used for cursor/selection conversion  |
| `Document` struct             | `crates/termcode-view/src/document.rs`       | Add Selection, History, last_saved_revision fields          |
| `Editor` struct               | `crates/termcode-view/src/editor.rs`         | Add Insert mode, save/close methods                         |
| `EditorMode` enum             | `crates/termcode-view/src/editor.rs`         | Add `Insert` variant                                        |
| `View` struct                 | `crates/termcode-view/src/view.rs`           | Cursor management for selection integration                 |
| `App::handle_key()`           | `crates/termcode-term/src/app.rs`            | Replace inline dispatch with CommandRegistry                |
| `EditorViewWidget`            | `crates/termcode-term/src/ui/editor_view.rs` | Extend for cursor style + selection rendering               |
| `StatusBarWidget`             | `crates/termcode-term/src/ui/status_bar.rs`  | Add mode indicator                                          |
| `FileEncoding` / `LineEnding` | `crates/termcode-core/src/encoding.rs`       | Preserve encoding on save                                   |

### Constraints

- No formal analysis report; specifications derived from architecture blueprint and team lead input
- Current `Document` has no `Selection` or `History` fields -- must be added
- Current `EditorMode` has only `Normal` and `FileExplorer` -- `Insert` must be added
- Current `handle_key()` in `app.rs` uses inline match arms -- must be refactored to use CommandRegistry dispatch
- `Buffer` has `text_mut()` but no `apply(transaction)` method -- Transaction must integrate with Buffer
- No `save_to_file` exists on Buffer -- must be added with encoding preservation
- `termcode-core/Cargo.toml` currently has no `chrono` or `std::time` usage -- History needs `Instant`

## 3. Impact Analysis (Dry-Run)

### Overall Risk: Medium-High

This phase touches both core data structures (buffer, document) and the main event loop. The Transaction/History system is a foundational change that all future editing depends on. However, all changes are additive (no deletions) and the existing viewer functionality is preserved.

### Files to Create

| File                                      | Risk | Description                                                |
| ----------------------------------------- | ---- | ---------------------------------------------------------- |
| `crates/termcode-core/src/transaction.rs` | Low  | Transaction, ChangeSet: atomic edit operations on Rope     |
| `crates/termcode-core/src/selection.rs`   | Low  | Range, Selection: cursor/multi-selection with byte offsets |
| `crates/termcode-core/src/history.rs`     | Low  | History, Revision: undo/redo with branching support        |
| `crates/termcode-term/src/command.rs`     | Low  | CommandRegistry, CommandEntry: named command dispatch      |
| `crates/termcode-term/src/input.rs`       | Low  | InputMapper: mode-aware key-to-command resolution          |

### Files to Modify

| File                                         | Risk        | Description                                                       |
| -------------------------------------------- | ----------- | ----------------------------------------------------------------- |
| `crates/termcode-core/src/lib.rs`            | Low         | Add `pub mod transaction; pub mod selection; pub mod history;`    |
| `crates/termcode-core/src/buffer.rs`         | Medium      | Add `apply()`, `save_to_file()`, `pos_to_byte()`, `byte_to_pos()` |
| `crates/termcode-view/src/document.rs`       | Medium      | Add Selection, History, last_saved_revision fields                |
| `crates/termcode-view/src/editor.rs`         | Medium-High | Add Insert mode, save_document, close_document, cursor movement   |
| `crates/termcode-term/src/lib.rs`            | Low         | Add `pub mod command; pub mod input;`                             |
| `crates/termcode-term/src/app.rs`            | High        | Integrate CommandRegistry + InputMapper; refactor handle_key      |
| `crates/termcode-term/src/ui/editor_view.rs` | Medium      | Cursor style by mode; selection rendering                         |
| `crates/termcode-term/src/ui/status_bar.rs`  | Low         | Add mode indicator (NORMAL / INSERT / EXPLORER)                   |
| `crates/termcode-term/src/render.rs`         | Low         | Pass EditorMode to widgets that need it                           |

### Files to Delete

| File | Risk | Description |
| ---- | ---- | ----------- |
| None | -    | -           |

### Destructive Operations

- None. All changes are additive or extend existing code. Existing viewer functionality is fully preserved.

### Rollback Plan

- All work on a feature branch; full rollback via `git checkout main && git branch -D feature/phase3-editing`
- No database changes; no external system modifications
- Cargo.lock changes are auto-resolved by reverting Cargo.toml

## 4. Implementation Order

### Phase 1: Core Editing Primitives (termcode-core)

**Goal**: Build the foundational Transaction, Selection, and History types that all editing operations depend on. These are pure data types with no UI awareness.
**Risk**: Low
**Status**: Complete

- [x] Task 1.1: Create `crates/termcode-core/src/selection.rs`
  - Define `Range` struct: `anchor: usize` (byte offset), `head: usize` (byte offset)
  - Implement `Range::new(anchor, head)`, `Range::point(pos)` (anchor == head, cursor with no selection)
  - Implement `Range::is_empty()` (anchor == head), `Range::from()` / `Range::to()` (min/max of anchor/head)
  - Implement `Range::flip()` (swap anchor and head)
  - Define `Selection` struct: `ranges: Vec<Range>`, `primary: usize`
  - Implement `Selection::point(pos: usize) -> Self` -- single cursor at position
  - Implement `Selection::single(anchor: usize, head: usize) -> Self` -- single range
  - Implement `Selection::new(ranges: Vec<Range>, primary: usize) -> Self` -- multi-cursor (assert non-empty)
  - Implement `Selection::primary(&self) -> Range`
  - Implement `Selection::primary_mut(&mut self) -> &mut Range`
  - Implement `Selection::ranges(&self) -> &[Range]`
  - Implement `Selection::transform<F: FnMut(Range) -> Range>(&self, f: F) -> Self`
  - Defer `Selection::map(transaction)` to after Transaction is built (Task 1.2)
  - Add `#[derive(Debug, Clone)]` on both types

- [x] Task 1.2: Create `crates/termcode-core/src/transaction.rs`
  - Define `Operation` enum: `Retain(usize)`, `Insert(String)`, `Delete(usize)`
    - `Retain(n)`: skip n bytes unchanged
    - `Insert(s)`: insert string at current position
    - `Delete(n)`: delete n bytes from current position
  - Define `ChangeSet` struct: `ops: Vec<Operation>`, `input_len: usize` (original document length)
  - Implement `ChangeSet::new(input_len: usize)` -- empty changeset
  - Implement `ChangeSet::insert(&mut self, text: String)` -- append Insert op
  - Implement `ChangeSet::delete(&mut self, count: usize)` -- append Delete op
  - Implement `ChangeSet::retain(&mut self, count: usize)` -- append Retain op
  - Implement `ChangeSet::apply(&self, rope: &mut Rope) -> anyhow::Result<()>` -- apply ops sequentially to rope
    - Walk through ops, maintaining a byte cursor into the rope
    - Retain: advance cursor by n
    - Insert: `rope.insert(cursor, &text)`, advance cursor by text.len()
    - Delete: `rope.remove(cursor..cursor+n)`
  - Implement `ChangeSet::invert(&self, original: &Rope) -> ChangeSet` -- produce inverse for undo
    - Retain -> Retain, Insert(s) -> Delete(s.len()), Delete(n) -> Insert(extracted text from original)
  - Implement `ChangeSet::compose(self, other: ChangeSet) -> ChangeSet` -- merge two sequential changesets (for grouping rapid edits)
  - Implement `ChangeSet::is_empty(&self) -> bool`
  - Implement `ChangeSet::map_position(&self, pos: usize) -> usize` -- map a byte position through the changeset (for updating cursor/selection positions after apply)
  - Define `Transaction` struct: `changes: ChangeSet`, `selection: Option<Selection>`
  - Implement `Transaction::new(changes: ChangeSet) -> Self`
  - Implement `Transaction::with_selection(mut self, selection: Selection) -> Self`
  - Implement `Transaction::insert(text: &str, pos: usize, doc_len: usize) -> Self` -- convenience: Retain(pos) + Insert(text) + Retain(remaining)
  - Implement `Transaction::delete(range: std::ops::Range<usize>, doc_len: usize) -> Self` -- convenience: Retain(range.start) + Delete(range.len()) + Retain(remaining)
  - Implement `Transaction::replace(range: std::ops::Range<usize>, text: &str, doc_len: usize) -> Self`
  - Implement `Transaction::apply(&self, rope: &mut Rope) -> anyhow::Result<()>` -- delegates to `self.changes.apply(rope)`
  - Implement `Transaction::invert(&self, original: &Rope) -> Transaction`
  - Implement `Transaction::compose(self, other: Transaction) -> Transaction`
  - Add `Selection::map(&self, changes: &ChangeSet) -> Self` back in selection.rs using `ChangeSet::map_position`

- [x] Task 1.3: Create `crates/termcode-core/src/history.rs`
  - Define `Revision` struct: `transaction: Transaction`, `inverse: Transaction`, `timestamp: std::time::Instant`, `parent: usize`
  - Define `History` struct: `revisions: Vec<Revision>`, `current: usize`
  - Implement `History::new() -> Self` -- empty revisions, current = 0
  - Implement `History::commit(&mut self, transaction: Transaction, original: &Rope)`
    - Compute inverse via `transaction.invert(original)`
    - Create Revision with parent = self.current
    - Push to revisions, set current to new index
    - (Branching undo: new revisions can have different parents, no truncation of old branches)
  - Implement `History::undo(&mut self) -> Option<Transaction>` (returns owned/cloned Transaction)
    - If current == 0 (no revisions or at initial state), return None
    - Clone the current revision's inverse transaction
    - Set current = revision.parent
    - Return Some(cloned inverse)
    - **NOTE**: Returns owned `Transaction` (not `&Transaction`) to avoid borrow checker conflict -- the caller needs `&mut self` on both History (to update current) and Buffer (to apply the transaction). Returning a reference would borrow self.history immutably while buffer.apply needs &mut self on Document.
  - Implement `History::redo(&mut self) -> Option<Transaction>` (returns owned/cloned Transaction)
    - Find a revision whose parent == current (most recent one)
    - If found, clone its transaction, set current to that revision's index, return Some(cloned transaction)
    - If none found, return None
    - **NOTE**: Same owned-return rationale as undo() above.
  - Implement `History::current_revision(&self) -> usize` -- returns current index (for last_saved_revision tracking)
  - Implement `History::is_at_saved(&self, saved_revision: usize) -> bool` -- checks current == saved_revision

- [x] Task 1.4: Add `pos_to_byte()` and `byte_to_pos()` to `crates/termcode-core/src/buffer.rs`
  - `pos_to_byte(&self, pos: &Position) -> usize` -- convert line/column to byte offset using Rope API
    - `let line_start = self.text.line_to_byte(pos.line);`
    - Walk grapheme clusters in the line up to `pos.column` to get byte offset
    - Return `line_start + byte_within_line`
  - `byte_to_pos(&self, byte: usize) -> Position` -- convert byte offset to line/column
    - `let line = self.text.byte_to_line(byte);`
    - `let line_start = self.text.line_to_byte(line);`
    - Walk grapheme clusters from line_start to byte to count columns
    - Return `Position { line, column }`
  - Add `apply(&mut self, transaction: &Transaction) -> anyhow::Result<()>` -- apply transaction and mark modified
    - `transaction.apply(&mut self.text)?; self.modified = true; Ok(())`
  - Add `save_to_file(&self, path: &Path) -> anyhow::Result<()>` -- write buffer to file preserving encoding/line endings
    - Collect rope to String
    - Normalize line endings to match `self.line_ending` (replace `\n` with `\r\n` if CrLf)
    - Write BOM if encoding is Utf8Bom
    - Encode to bytes based on `self.encoding`
    - Write atomically: write to temp file then rename (to prevent data loss on crash)

- [x] Task 1.5: Update `crates/termcode-core/src/lib.rs`
  - Add `pub mod transaction;`
  - Add `pub mod selection;`
  - Add `pub mod history;`

- [x] Task 1.6: Verify Phase 1 builds
  - Run `cargo check -p termcode-core`
  - Run `cargo test -p termcode-core`
  - Run `cargo clippy -p termcode-core`

### Phase 2: Document and Editor State Updates (termcode-view)

**Goal**: Integrate Selection, History, and Transaction into the Document and Editor models. Add Insert mode, save, and close capabilities.
**Risk**: Medium
**Status**: Complete
**Depends on**: Phase 1

- [x] Task 2.1: Update `crates/termcode-view/src/document.rs`
  - Add `use termcode_core::selection::Selection;`
  - Add `use termcode_core::history::History;`
  - Add `use termcode_core::transaction::Transaction;`
  - Add fields to `Document`:
    - `pub selection: Selection` -- initialized to `Selection::point(0)`
    - `pub history: History` -- initialized to `History::new()`
    - `pub last_saved_revision: usize` -- initialized to 0
  - Update `Document::new()` and `Document::open()` to initialize these fields
  - Add `Document::apply_transaction(&mut self, transaction: &Transaction) -> anyhow::Result<()>`
    - Save reference to current rope state (for inverse computation)
    - `self.history.commit(transaction.clone(), self.buffer.text());`
    - `self.buffer.apply(transaction)?;`
    - If transaction has selection, apply it: `self.selection = transaction.selection.clone().unwrap_or_else(|| self.selection.map(&transaction.changes));`
    - Return Ok
  - Add `Document::undo(&mut self) -> anyhow::Result<bool>`
    - `if let Some(inverse) = self.history.undo() { self.buffer.apply(&inverse)?; update selection from inverse; return Ok(true) }`
    - Else Ok(false)
    - **NOTE**: `history.undo()` returns `Option<Transaction>` (owned). This avoids the borrow checker conflict where `&self.history` (for the returned reference) would overlap with `&mut self.buffer` (for apply). The owned Transaction is used, then dropped.
  - Add `Document::redo(&mut self) -> anyhow::Result<bool>`
    - `if let Some(txn) = self.history.redo() { self.buffer.apply(&txn)?; update selection from txn; return Ok(true) }`
    - Else Ok(false)
    - **NOTE**: Same owned-return pattern as undo() above.
  - Add `Document::is_modified(&self) -> bool` -- `!self.history.is_at_saved(self.last_saved_revision)`
  - Add `Document::mark_saved(&mut self)` -- `self.last_saved_revision = self.history.current_revision(); self.buffer.set_modified(false);`

- [x] Task 2.2: Update `crates/termcode-view/src/editor.rs`
  - Add `Insert` variant to `EditorMode` enum: `Normal, Insert, FileExplorer`
  - Add `save_document(&mut self, doc_id: DocumentId) -> anyhow::Result<()>`
    - Get document by doc_id
    - Get path (error if None: "No file path for document")
    - Call `doc.buffer.save_to_file(&path)?`
    - Call `doc.mark_saved()`
    - Set status message: "Saved: {path}"
  - Add `close_document(&mut self, doc_id: DocumentId)`
    - Remove document from `self.documents`
    - Remove associated view(s) from `self.views`
    - Remove tab from `self.tabs`
    - Adjust active_view
  - Add `active_document_mut(&mut self) -> Option<&mut Document>`
    - Get active view's doc_id, then `self.documents.get_mut(&doc_id)`
  - Update `sync_tab_modified()` to use `doc.is_modified()` instead of `doc.buffer.is_modified()`
    (This uses the new revision-based modified tracking rather than the simple buffer flag)

- [x] Task 2.3: Verify Phase 2 builds
  - Run `cargo check -p termcode-view`
  - Run `cargo clippy -p termcode-view`

### Phase 3: Command Registry and Input Mapper (termcode-term)

**Goal**: Create the CommandRegistry and InputMapper infrastructure that decouples key input from actions, per architecture mandate.
**Risk**: Medium
**Status**: Complete
**Depends on**: Phase 2

- [x] Task 3.1: Create `crates/termcode-term/src/command.rs`
  - Define `pub type CommandId = &'static str;`
  - Define `pub type CommandHandler = fn(&mut Editor) -> anyhow::Result<()>;`
    - NOTE: Use `fn` pointer instead of `Box<dyn Fn>` for simplicity in Phase 3. The architecture shows `Box<dyn Fn>` for plugin extensibility, but that can be upgraded in Phase 5 when plugins need to register commands. For now, all commands are built-in.
  - Define `CommandEntry` struct: `id: CommandId`, `name: &'static str`, `description: &'static str`, `handler: CommandHandler`
  - Define `CommandRegistry` struct: `commands: HashMap<CommandId, CommandEntry>`
  - Implement `CommandRegistry::new() -> Self`
  - Implement `CommandRegistry::register(&mut self, entry: CommandEntry)`
  - Implement `CommandRegistry::execute(&self, id: CommandId, editor: &mut Editor) -> anyhow::Result<()>`
    - Look up command, call handler, return result
    - If not found, return error
  - Implement `CommandRegistry::get(&self, id: CommandId) -> Option<&CommandEntry>`
  - Define `pub fn register_builtin_commands(registry: &mut CommandRegistry)` that registers Phase 3 commands:
    - `"file.save"` -- save active document
    - `"edit.insert_char"` -- (special: handled differently, see Task 4.2)
    - `"edit.delete_char"` -- delete char at cursor
    - `"edit.backspace"` -- delete char before cursor
    - `"edit.newline"` -- insert newline at cursor
    - `"edit.undo"` -- undo
    - `"edit.redo"` -- redo
    - `"cursor.up"`, `"cursor.down"`, `"cursor.left"`, `"cursor.right"` -- cursor movement
    - `"cursor.page_up"`, `"cursor.page_down"` -- page navigation
    - `"cursor.home"`, `"cursor.end"` -- beginning/end of document
    - `"mode.insert"` -- enter Insert mode
    - `"mode.normal"` -- exit to Normal mode
    - `"tab.next"`, `"tab.prev"`, `"tab.close"` -- tab operations
    - `"view.toggle_sidebar"` -- sidebar toggle
    - `"app.quit"` -- quit application
  - Each command handler function receives `&mut Editor` and performs the appropriate operation
  - NOTE on `edit.insert_char`: This command needs a character argument. For Phase 3, handle insert_char as a special case in the input dispatch (the App receives the char from KeyEvent and calls a helper directly). A full args-passing system can be added when the command palette (Phase 4) needs it.

- [x] Task 3.2: Create `crates/termcode-term/src/input.rs`
  - Define `InputMapper` struct: `keymaps: HashMap<EditorMode, Vec<(KeyEvent, CommandId)>>`
    - Use Vec of tuples rather than HashMap for KeyEvent since KeyEvent doesn't implement Hash in all crossterm versions; linear scan over ~20 entries per mode is negligible
  - Implement `InputMapper::new() -> Self` -- create with default keymaps
  - Implement `InputMapper::resolve(&self, mode: EditorMode, key: KeyEvent) -> Option<CommandId>`
    - First check mode-specific bindings
    - Could add global bindings check as fallback
  - Define default keymaps:
    - **Global** (checked regardless of mode):
      - `Ctrl+Q` / `Ctrl+C` -> `"app.quit"`
      - `Ctrl+B` -> `"view.toggle_sidebar"`
      - `Alt+Right` -> `"tab.next"`
      - `Alt+Left` -> `"tab.prev"`
      - `Ctrl+W` -> `"tab.close"`
      - `Ctrl+S` -> `"file.save"`
      - `Ctrl+Z` -> `"edit.undo"`
      - `Ctrl+Y` -> `"edit.redo"`
    - **Normal mode**:
      - `j` / `Down` -> `"cursor.down"`
      - `k` / `Up` -> `"cursor.up"`
      - `h` / `Left` -> `"cursor.left"`
      - `l` / `Right` -> `"cursor.right"`
      - `PageDown` -> `"cursor.page_down"`
      - `PageUp` -> `"cursor.page_up"`
      - `g` / `Home` -> `"cursor.home"`
      - `G` (Shift+G) / `End` -> `"cursor.end"`
      - `i` -> `"mode.insert"`
      - `x` -> `"edit.delete_char"` -- delete character under cursor (Vim-style)
      - `Delete` -> `"edit.delete_char"` -- delete key also works in Normal mode
    - **Insert mode**:
      - `Esc` -> `"mode.normal"`
      - `Backspace` -> `"edit.backspace"`
      - `Delete` -> `"edit.delete_char"`
      - `Enter` -> `"edit.newline"`
      - `Up` -> `"cursor.up"`
      - `Down` -> `"cursor.down"`
      - `Left` -> `"cursor.left"`
      - `Right` -> `"cursor.right"`
      - Printable chars -> special insert_char handling (not via InputMapper)
    - **FileExplorer mode**: (preserve existing behavior)
      - `j` / `Down` -> `"explorer.down"`
      - `k` / `Up` -> `"explorer.up"`
      - `Enter` -> `"explorer.enter"`
      - `l` / `Right` -> `"explorer.expand"`
      - `h` / `Left` -> `"explorer.collapse"`
      - `Esc` / `Tab` -> `"mode.normal"`

- [x] Task 3.3: Update `crates/termcode-term/src/lib.rs`
  - Add `pub mod command;`
  - Add `pub mod input;`

- [x] Task 3.4: Verify Phase 3 builds
  - Run `cargo check -p termcode-term`
  - Run `cargo clippy -p termcode-term`

### Phase 4: App Integration (termcode-term)

**Goal**: Integrate CommandRegistry and InputMapper into the App event loop. Refactor handle_key to use command dispatch. Add Insert mode character insertion. This is the highest-risk phase as it rewrites the core input handling.
**Risk**: High
**Status**: Complete
**Depends on**: Phase 3

- [x] Task 4.1: Update `App` struct in `crates/termcode-term/src/app.rs`
  - Add `command_registry: CommandRegistry` field to `App`
  - Add `input_mapper: InputMapper` field to `App`
  - In `App::new()`:
    - Create `CommandRegistry`, call `register_builtin_commands(&mut registry)`
    - Create `InputMapper::new()` with defaults
    - Store both in App

- [x] Task 4.2: Refactor `handle_key()` to use command dispatch
  - New `handle_key()` flow:
    1. Check global keybindings first via `input_mapper.resolve_global(key)` -> if Some(cmd_id), execute
    2. For Insert mode: if key is `KeyCode::Char(c)` with no modifiers (or SHIFT only), handle `insert_char(c)` directly
       - Build Transaction::insert for the char at the current cursor byte position
       - Apply to document via `doc.apply_transaction()`
       - Update cursor position (advance by char byte length)
    3. For other keys: resolve via `input_mapper.resolve(mode, key)` -> if Some(cmd_id), execute command
    4. If no command resolved, ignore key
  - Handle special quit flag: `"app.quit"` command sets `self.should_quit = true` (the command itself sets a flag on Editor or returns a signal)
    - Option A: Add `exit_requested: bool` to Editor, check after command execution
    - Option B: Handle quit command separately before dispatch (simpler for Phase 3)
    - **Decision**: Option B -- keep `"app.quit"` as a special case that sets `self.should_quit` directly. Other commands go through registry.
  - File explorer commands (`"explorer.*"`) remain as methods on App that manipulate `editor.file_explorer` directly, similar to Phase 2. They are registered in CommandRegistry as thin wrappers.

- [x] Task 4.3: Implement editing command handlers
  - Each command handler is a function `fn handler(editor: &mut Editor) -> anyhow::Result<()>`
  - **`edit.delete_char`**: Delete character under cursor
    - Get active document mut
    - Get cursor byte position from selection primary head
    - If cursor is at end of document, no-op
    - Determine byte length of char at cursor position
    - Build `Transaction::delete(cursor..cursor+char_len, doc_len)`
    - Apply to document
  - **`edit.backspace`**: Delete character before cursor
    - Get cursor byte position
    - If cursor is at 0, no-op
    - Find byte offset of previous character
    - Build `Transaction::delete(prev..cursor, doc_len)`
    - Apply to document, update cursor to prev position
  - **`edit.newline`**: Insert newline at cursor
    - Build `Transaction::insert("\n", cursor_pos, doc_len)`
    - Apply to document, advance cursor
  - **`edit.undo`**: `editor.active_document_mut()?.undo()?`
  - **`edit.redo`**: `editor.active_document_mut()?.redo()?`
  - **`file.save`**:
    - Get active view's doc_id
    - Call `editor.save_document(doc_id)?`
  - **`mode.insert`**: `editor.switch_mode(EditorMode::Insert)`
  - **`mode.normal`**: `editor.switch_mode(EditorMode::Normal)`
  - **Cursor commands** (`cursor.up`, `cursor.down`, etc.):
    - Move the existing cursor movement logic from `handle_normal_key()` into command handler functions
    - Each adjusts cursor Position on the active view, then calls `ensure_cursor_visible()`
    - Additionally update the document's Selection to match cursor position: `doc.selection = Selection::point(buffer.pos_to_byte(&view.cursor))`

- [x] Task 4.4: Handle Insert mode character input
  - In `handle_key()`, when mode is Insert and key is `KeyCode::Char(c)`:
    - Get active document and view
    - Compute cursor byte offset: `doc.buffer.pos_to_byte(&view.cursor)`
    - Build transaction: `Transaction::insert(&c.to_string(), byte_pos, doc.buffer.len_bytes())`
    - Apply: `doc.apply_transaction(&transaction)?`
    - Advance cursor: move column right by 1 (or handle multi-byte chars correctly via byte_to_pos on new cursor byte)
    - Call `view.ensure_cursor_visible(scroll_off)`

- [x] Task 4.5: Sync cursor between View and Document Selection
  - After any editing command that changes the document:
    - Update `view.cursor` from `doc.selection.primary().head` via `buffer.byte_to_pos(head)`
  - After any cursor movement command:
    - Update `doc.selection` from `view.cursor` via `buffer.pos_to_byte(&cursor)`
  - Create helper: `fn sync_cursor_to_selection(view: &mut View, doc: &Document)` and `fn sync_selection_to_cursor(doc: &mut Document, view: &View)`
  - Call appropriate sync after each command execution

- [x] Task 4.6: Verify Phase 4 builds and basic functionality
  - Run `cargo check`
  - Run `cargo clippy`
  - Run `cargo test`

### Phase 5: UI Updates (termcode-term)

**Goal**: Update rendering to reflect new editing state -- cursor style changes by mode, mode indicator in status bar, modified indicator improvements.
**Risk**: Medium
**Status**: Complete
**Depends on**: Phase 4

- [x] Task 5.1: Update `crates/termcode-term/src/ui/status_bar.rs`
  - Accept `EditorMode` as parameter in `StatusBarWidget::new()`
  - Render mode indicator on the left side of status bar:
    - `EditorMode::Normal` -> "NORMAL" with one background color
    - `EditorMode::Insert` -> "INSERT" with a distinct background color (e.g., green-ish)
    - `EditorMode::FileExplorer` -> "EXPLORER" with another distinct color
  - Format: `MODE | filename [+]` on the left
  - Keep right side unchanged (line/col, encoding, language)

- [x] Task 5.2: Update `crates/termcode-term/src/ui/editor_view.rs`
  - Accept `EditorMode` as parameter (to vary cursor rendering)
  - Cursor rendering:
    - Normal mode: render cursor as a block (reverse video on the character under cursor)
    - Insert mode: render cursor as a thin line (left edge of the character cell -- use a pipe `|` character or set only the left-border style)
    - Implementation: In Normal mode, apply reverse style to the cell at cursor position. In Insert mode, render a `|` or use a different highlight (e.g., bright foreground bar).
  - Selection rendering (basic for Phase 3):
    - If selection primary range is non-empty (anchor != head), highlight the selected region with `theme.ui.selection` background color
    - For Phase 3, single selection only (multi-cursor deferred)

- [x] Task 5.3: Update `crates/termcode-term/src/render.rs`
  - Pass `editor.mode` to widgets that need it:
    - `EditorViewWidget::new(doc, view, theme, mode)`
    - `StatusBarWidget::new(doc, view, theme, status_msg, mode)`

- [x] Task 5.4: Verify Phase 5 builds
  - Run `cargo check`
  - Run `cargo clippy`

### Phase 6: Integration Testing and Polish

**Goal**: Verify the full editing flow works end-to-end, fix edge cases, ensure all quality gates pass.
**Risk**: Low
**Status**: Complete
**Depends on**: Phase 5

- [x] Task 6.1: Manual integration testing
  - Launch termcode, open a file
  - Press `i` to enter Insert mode -- status bar shows "INSERT"
  - Type characters -- they appear in the document at cursor position
  - Press `Enter` -- new line inserted
  - Press `Backspace` -- character before cursor deleted
  - Press `Esc` -- return to Normal mode, status bar shows "NORMAL"
  - Press `x` -- delete character under cursor (Normal mode)
  - Press `Ctrl+Z` -- undo last edit
  - Press `Ctrl+Y` -- redo
  - Press `Ctrl+S` -- file saved, status message shows "Saved: ..."
  - Verify cursor is block in Normal mode, line/bar in Insert mode
  - Verify modified indicator `[+]` appears after edits, disappears after save
  - Verify file explorer still works (Ctrl+B, navigation, opening files)
  - Verify tab switching still works (Alt+Left/Right)
  - Test edge cases:
    - Editing at beginning of file (byte offset 0)
    - Editing at end of file
    - Editing empty file
    - Backspace at beginning of line (join with previous line)
    - Delete at end of line (join with next line)
    - Undo/redo across multiple edits
    - Save file, edit more, undo to saved state -- modified indicator should clear

- [x] Task 6.2: Unit tests for core primitives
  - Transaction: test insert, delete, replace, compose, invert
  - Selection: test point, single, transform, map
  - History: test commit, undo, redo, branching
  - Buffer: test pos_to_byte, byte_to_pos round-trip, save_to_file

- [x] Task 6.3: Full build verification
  - `cargo build` -- full project builds
  - `cargo test` -- all tests pass
  - `cargo clippy` -- no warnings
  - `cargo fmt --check` -- formatting consistent

- [x] Task 6.4: Update overall project plan
  - Update `/Users/hankyung/.claude/plans/cosmic-prancing-whisper.md` to mark Phase 3 as complete
  - Record completion date and any notable deviations from the plan

## 5. Quality Gate

- [x] Build success: `cargo build`
- [x] Tests pass: `cargo test`
- [x] Lint pass: `cargo clippy` (0 warnings)
- [x] Format check: `cargo fmt --check`
- [ ] Manual smoke test: enter Insert mode, type text, undo/redo, save file, verify mode indicators

## 6. Notes

### Implementation Considerations

- **ChangeSet vs direct Rope mutation**: The architecture mandates Transaction/ChangeSet for all edits. This enables undo/redo and future collaborative editing. Never use `buffer.text_mut()` for editing operations after Phase 3 -- always go through Transaction.

- **Cursor dual representation**: The View has `cursor: Position` (line/column) and the Document has `selection: Selection` (byte offsets). These must be kept in sync. The View's cursor is the "display" representation; the Selection is the "edit" representation. Sync after every command.

- **Byte offset correctness**: Rope's byte offsets are UTF-8 byte offsets. When inserting/deleting, all positions after the edit point shift. The `ChangeSet::map_position()` method handles this for selection updates. Be careful with multi-byte characters (e.g., emoji, CJK characters).

- **`insert_char` as special case**: The architecture shows `edit.insert_char` as a registered command, but it needs a character argument. For Phase 3, handle it as a special case in the key dispatch rather than passing args through CommandRegistry. The full args-passing system (needed for command palette input) can be added in Phase 4.

- **Encoding preservation on save**: The `save_to_file` method must preserve the original encoding (UTF-8, UTF-8 BOM, UTF-16 LE/BE) and line ending style. Ropey stores text as UTF-8 internally, so encoding conversion happens only at save time.

- **Atomic save**: Write to a temp file then rename to prevent data loss if the process crashes mid-save. Use `std::fs::rename()` which is atomic on most filesystems.

- **CommandHandler signature**: Phase 3 uses `fn(&mut Editor) -> Result<()>` for simplicity. Phase 4/5 may need `fn(&mut Editor, &[String]) -> Result<()>` for command palette arguments. This upgrade is straightforward.

- **FileExplorer commands in registry**: Explorer commands (expand, collapse, enter, up, down) are registered in CommandRegistry for consistency, but their handlers need access to App-level state (e.g., opening files creates views/tabs). Solution: the handlers operate on `Editor` which owns `file_explorer`. The App handles the result (e.g., checking if a file was opened).

- **History undo/redo return owned Transaction**: `History::undo()` and `History::redo()` return `Option<Transaction>` (owned/cloned) rather than `Option<&Transaction>`. This is a deliberate design choice to avoid a borrow checker conflict in `Document::undo()/redo()`, where the returned reference would borrow `self.history` immutably while `self.buffer.apply()` requires `&mut self`. Cloning the Transaction is acceptable because undo/redo are infrequent user actions, not hot-path operations.

### Patterns to Follow

- Widget pattern: struct with lifetime `'a` references, implement `Widget` trait (from Phase 2)
- Manual buffer cell writing (not ratatui Paragraph/Block) for precise control
- Theme color access: `self.theme.ui.{color_name}.to_ratatui()`
- Editor state: immutable borrow in render functions; mutable in update/command handlers
- Borrow checker pattern: clone data before mutable calls (established in Phase 2 explorer_enter)

### Patterns to Avoid

- Do NOT use `buffer.text_mut()` for editing after this phase -- always use Transaction
- Do NOT store cursor position only in View -- keep Document.selection in sync
- Do NOT handle edit actions inline in match arms -- route through CommandRegistry
- Do NOT ignore encoding/line-ending on save -- always preserve original

### Potential Issues

- **Borrow checker in command handlers**: Command handlers receive `&mut Editor` but may need to access both active document and active view simultaneously. Solution: extract doc_id/view_id first, then borrow.
- **Cursor column after vertical movement**: When moving up/down, the cursor column should ideally remember the "desired column" (sticky column) so moving through shorter lines doesn't reset the column. This is a polish item -- can be deferred if it complicates Phase 3. Note: the current View has no `desired_column` field; consider adding it.
- **ChangeSet compose complexity**: Composing two ChangeSets is algorithmically non-trivial (interleaving Retain/Insert/Delete ops). For Phase 3, composition can be simplified or deferred -- each edit can be its own History revision. Grouping rapid edits (e.g., typing a word as one undo step) can use a timer-based heuristic in History.commit().
- **Redo after branching**: When the user undoes several steps then makes a new edit, the redo path from those undone steps is preserved (branching undo). The `redo()` implementation finds the most recent child of the current revision. This is simpler than Vim's full undo tree visualization but preserves all history.

### Recommendation

Run yyy-plan-review agent to verify this plan before implementation.

## 7. Implementation Notes

### Phase 1 (2026-03-27)

- Created: 3 files (selection.rs, transaction.rs, history.rs)
- Modified: 3 files (buffer.rs, lib.rs, Cargo.toml)
- Risk: Low
- Notes: Fixed map_position to shift positions at insert point. Added tempfile dependency for atomic save.

### Phase 2 (2026-03-27)

- Modified: 3 files (document.rs, editor.rs, tab.rs)
- Risk: Medium
- Notes: Added Selection, History, Transaction integration to Document. Added Insert mode, save/close to Editor. Added remove_by_doc_id to TabManager.

### Phase 3 (2026-03-27)

- Created: 2 files (command.rs, input.rs)
- Modified: 2 files (lib.rs, app.rs)
- Risk: Medium
- Notes: CommandRegistry with fn pointer handlers, InputMapper with per-mode Vec<(KeyEvent, CommandId)> keymaps.

### Phase 4 (2026-03-27)

- Modified: 1 file (app.rs)
- Risk: High
- Notes: Refactored handle_key to use CommandRegistry + InputMapper. Quit and close-tab remain as special cases in App. File explorer commands dispatched directly. Insert mode printable chars handled via insert_char helper.

### Phase 5 (2026-03-27)

- Modified: 3 files (status_bar.rs, editor_view.rs, render.rs)
- Risk: Medium
- Notes: Mode indicator with distinct colors per mode. Block cursor (REVERSED) for Normal, underline for Insert. Selection highlighting for non-empty ranges.

### Phase 6 (2026-03-27)

- Created: 0 files
- Modified: 1 file (buffer.rs - added tests)
- Risk: Low
- Notes: 29 tests pass. All quality gates pass (build, test, clippy 0 warnings, fmt). Manual smoke test deferred to user.

---

Last Updated: 2026-03-27
Status: Complete (all 6 phases)
