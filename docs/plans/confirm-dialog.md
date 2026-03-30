# Unsaved Changes Confirmation Dialog Implementation Plan

**Created**: 2026-03-30
**Analysis Report**: docs/analysis/confirm-dialog.md
**Specification**: docs/specs/confirm-dialog.md
**Status**: Approved

## 1. Requirements Summary

### Functional Requirements

- [FR-CONFIRM-001] ConfirmDialog state struct in Editor (ConfirmAction enum + ConfirmDialog struct)
- [FR-CONFIRM-002] Ctrl+W shows dialog when closing modified file
- [FR-CONFIRM-003] Ctrl+Q shows dialog when quitting with unsaved files
- [FR-CONFIRM-004] Dialog button actions (save+close, discard+close, cancel)
- [FR-CONFIRM-005] Keyboard navigation (Left/Right/Tab/Shift+Tab/Enter/Esc)
- [FR-CONFIRM-006] Centered overlay popup widget with rounded borders

## 2. Analysis Report Reference

### Reference Documents

- Analysis Report: `docs/analysis/confirm-dialog.md`
- Specification: `docs/specs/confirm-dialog.md`

### Applied Recommendations

- Follow HelpPopupWidget manual rendering pattern (rounded borders, centered popup)
- Use `Option<ConfirmDialog>` pattern matching existing overlay state (completion, hover)
- Dialog check FIRST in handle_key(), before Ctrl+Q and help_visible checks
- No new EditorMode variant -- use confirm_dialog.is_some() guard

### Reusable Code

| Code                    | Location                           | Purpose                                   |
| ----------------------- | ---------------------------------- | ----------------------------------------- |
| HelpPopupWidget         | termcode-term/src/ui/help_popup.rs | Reference popup rendering pattern         |
| Editor.save_document()  | termcode-view/src/editor.rs:198    | Save a document by DocumentId             |
| Editor.close_document() | termcode-view/src/editor.rs:213    | Remove document, views, tabs              |
| handle_close_tab()      | termcode-term/src/app.rs:1076      | Full close flow with LSP + hooks          |
| Document.is_modified()  | termcode-view/src/document.rs:130  | Check unsaved state                       |
| Document.display_name() | termcode-view/src/document.rs:72   | Filename for dialog message               |
| theme.ui colors         | termcode-theme/src/theme.rs        | background, foreground, border, selection |

### Constraints

- ConfirmDialog/ConfirmAction types in termcode-view (no ratatui dependency)
- Widget in termcode-term (ratatui allowed)
- Dialog blocks ALL keyboard and mouse input while active
- Save failure must abort close/quit and show error in status_message

## 3. Impact Analysis (Dry-Run)

### Overall Risk: Medium

### Files to Create

| File                                            | Risk | Description                              |
| ----------------------------------------------- | ---- | ---------------------------------------- |
| `crates/termcode-view/src/confirm.rs`           | Low  | ConfirmDialog struct, ConfirmAction enum |
| `crates/termcode-term/src/ui/confirm_dialog.rs` | Low  | ConfirmDialogWidget (ratatui Widget)     |

### Files to Modify

| File                                 | Risk     | Description                                                                                            |
| ------------------------------------ | -------- | ------------------------------------------------------------------------------------------------------ |
| `crates/termcode-view/src/lib.rs`    | Low      | Add `pub mod confirm;`                                                                                 |
| `crates/termcode-view/src/editor.rs` | Low      | Add `confirm_dialog: Option<ConfirmDialog>` field + init                                               |
| `crates/termcode-term/src/ui/mod.rs` | Low      | Add `pub mod confirm_dialog;`                                                                          |
| `crates/termcode-term/src/render.rs` | Low      | Add confirm dialog overlay after help_popup                                                            |
| `crates/termcode-term/src/app.rs`    | **High** | handle_key intercept, handle_close_tab guard, Ctrl+Q guard, handle_mouse guard, confirm action handler |

### Files to Delete

| File | Risk | Description |
| ---- | ---- | ----------- |
| None | -    | -           |

### Destructive Operations

- None

### Rollback Plan

- Full rollback via branch deletion: `git checkout main && git branch -D feature/confirm-dialog`
- All changes are additive (new types, new widget, new guards). No existing behavior removed.

## 4. Implementation Order

### Phase 1: State Types (termcode-view)

**Goal**: Define ConfirmDialog and ConfirmAction types in termcode-view
**Risk**: Low
**Status**: ✅ Complete

- [x] Task 1.1: Create `crates/termcode-view/src/confirm.rs` with ConfirmAction enum and ConfirmDialog struct
  - ConfirmAction: `CloseTab(DocumentId)`, `QuitAll`
  - ConfirmDialog: `action`, `message: String`, `buttons: Vec<String>`, `selected_button: usize`
- [x] Task 1.2: Add `pub mod confirm;` to `crates/termcode-view/src/lib.rs`
- [x] Task 1.3: Add `pub confirm_dialog: Option<ConfirmDialog>` to Editor struct, initialize as `None`

### Phase 2: Widget (termcode-term)

**Goal**: Create ConfirmDialogWidget and wire into render pipeline
**Risk**: Low
**Status**: ✅ Complete

- [x] Task 2.1: Create `crates/termcode-term/src/ui/confirm_dialog.rs`
  - Implement `ratatui::widgets::Widget` for `ConfirmDialogWidget`
  - Accept `&ConfirmDialog` and `&Theme` in constructor
  - Centered popup with rounded borders (matching HelpPopupWidget pattern)
  - Auto-calculate popup width from message length and button text
  - Selected button: highlight with `theme.ui.selection` background
  - Unselected buttons: `theme.ui.border` brackets, normal foreground text
- [x] Task 2.2: Add `pub mod confirm_dialog;` to `crates/termcode-term/src/ui/mod.rs`
- [x] Task 2.3: Add confirm dialog rendering in `render.rs` AFTER help_popup (highest z-order)
  - Guard: `if let Some(ref dialog) = editor.confirm_dialog { ... }`

### Phase 3: Input Handling & Actions (termcode-term)

**Goal**: Wire dialog triggers (Ctrl+W, Ctrl+Q) and handle dialog key events
**Risk**: **High** -- this is the most complex phase, touching handle_key order
**Status**: ✅ Complete

- [x] Task 3.1: Add confirm dialog input intercept at TOP of handle_key()
  - BEFORE the Ctrl+Q check (line 492)
  - When `editor.confirm_dialog.is_some()`, consume ALL keys via dedicated handler
  - Handle: Left/Shift+Tab (prev button), Right/Tab (next button), Enter (execute), Esc (cancel)
  - Ctrl+Q and Ctrl+W consumed with no effect while dialog active
- [x] Task 3.2: Implement `handle_confirm_key(&mut self, key: KeyEvent)` method on App
  - Button navigation: Left/Shift+Tab wraps prev, Right/Tab wraps next
  - Enter: match on (action, selected_button) to dispatch action
  - Esc: set confirm_dialog = None
- [x] Task 3.3: Implement confirm action execution
  - **CloseTab save+close**: save_document(doc_id), on success call lsp_notify_did_save() and dispatch OnSave plugin hook (mirroring existing save flow at app.rs:749-752), then close via handle_close_tab logic (OnClose hook, LSP close, close_document); on error show status_message and dismiss dialog
  - **CloseTab discard**: close tab without saving (same handle_close_tab logic, skip save)
  - **CloseTab cancel**: dismiss dialog
  - **QuitAll save+quit**: iterate all modified docs, save each + call lsp_notify_did_save() and dispatch OnSave plugin hook per document (mirroring app.rs:749-752); abort on any error; on success set should_quit
  - **QuitAll discard**: set should_quit directly
  - **QuitAll cancel**: dismiss dialog
  - NOTE: Must validate DocumentId still exists before acting (R6)
  - NOTE: After every successful save_document(), must call lsp_notify_did_save() and dispatch_plugin_hook(HookEvent::OnSave { path, filename }) to mirror the existing save flow at app.rs:749-752. Omitting these will desync LSP server state.
- [x] Task 3.4: Modify Ctrl+W handling (app.rs:536)
  - Check active tab's document.is_modified() before close
  - If modified: create ConfirmDialog with CloseTab action + Korean messages
  - If not modified (or image tab): close immediately (existing behavior)
- [x] Task 3.5: Modify Ctrl+Q handling (app.rs:492)
  - Count modified documents in editor.documents
  - If any: create ConfirmDialog with QuitAll action + Korean messages
  - If none: set should_quit (existing behavior)
- [x] Task 3.6: Add mouse guard in handle_mouse() (app.rs:395)
  - Early return when `editor.confirm_dialog.is_some()`

### Phase 4: Testing & Verification

**Goal**: Verify all behavior is correct
**Risk**: Low
**Status**: ✅ Complete

- [x] Task 4.1: `cargo build` -- verify compilation
- [x] Task 4.2: `cargo clippy --workspace` -- zero warnings
- [x] Task 4.3: `cargo test --workspace` -- all tests pass
- [x] Task 4.4: `cargo fmt --check` -- formatting OK
- [ ] Task 4.5: Manual test scenarios:
  - Ctrl+W on unmodified file: closes immediately
  - Ctrl+W on modified file: dialog appears, test all 3 buttons
  - Ctrl+Q with no modified files: quits immediately
  - Ctrl+Q with modified files: dialog appears, test all 3 buttons
  - Keyboard navigation: Left/Right/Tab/Shift+Tab wraps correctly
  - Esc dismisses dialog
  - Mouse clicks ignored while dialog visible
  - Save failure shows error and doesn't close/quit

## 5. Quality Gate

- [x] Build success: `cargo build`
- [x] Tests pass: `cargo test --workspace`
- [x] Lint pass: `cargo clippy --workspace`
- [x] Format check: `cargo fmt --check`

## 6. Notes

### Key Risk Mitigations

| Risk                        | Mitigation in Plan                                                                             |
| --------------------------- | ---------------------------------------------------------------------------------------------- |
| R1 (save failure)           | Task 3.3 catches save errors, shows status_message, does NOT proceed with close/quit           |
| R2 (Ctrl+Q bypasses dialog) | Task 3.1 places dialog check BEFORE Ctrl+Q in handle_key()                                     |
| R3 (QuitAll partial save)   | Task 3.3 saves all, aborts quit on any failure                                                 |
| R4 (mouse not blocked)      | Task 3.6 adds early return in handle_mouse()                                                   |
| R5 (hook timing)            | Task 3.3 defers OnClose hook to after confirm (only fires on actual close, not on dialog show) |
| R6 (stale DocumentId)       | Task 3.3 validates doc_id exists before acting                                                 |
| R7 (borrow checker)         | Task 3.3 extracts action/doc_id locals before mutating editor                                  |

### Patterns to Avoid

- Do NOT use overlay.rs helpers (square corners, doesn't match spec)
- Do NOT add a new EditorMode variant (check confirm_dialog.is_some() instead)
- Do NOT fire OnClose hook when dialog appears (only when close actually executes)

### Handle_close_tab Reuse Strategy

The existing `handle_close_tab()` contains LSP close + plugin hook + editor.close_document logic. For the confirm dialog "close" actions, extract the close-execution logic so it can be called both from direct close (unmodified) and from confirm action. Options:

1. Call existing `handle_close_tab()` directly after dismissing dialog (simplest if it re-checks active tab)
2. Extract a `close_tab_for_doc(doc_id)` helper that does LSP close + hook + close_document

Recommendation: Option 2 -- extract helper to avoid re-deriving the active tab. The confirm dialog stores the DocumentId, so we can close it directly.

## 7. Implementation Notes

### Phase 1 (2026-03-30)

- Created: 1 file (confirm.rs)
- Modified: 2 files (lib.rs, editor.rs)
- Risk: Low
- Notes: ConfirmAction enum, ConfirmDialog struct with navigation helpers

### Phase 2 (2026-03-30)

- Created: 1 file (confirm_dialog.rs widget)
- Modified: 2 files (ui/mod.rs, render.rs)
- Risk: Low
- Notes: Centered popup with rounded borders, button highlight via theme.ui.selection

### Phase 3 (2026-03-30)

- Created: 0 files
- Modified: 1 file (app.rs, ~200 lines added)
- Risk: High (handle_key order change)
- Notes: Dialog intercept first in handle_key, extracted close_tab_for_doc helper, lsp_notify_did_save_doc helper, Korean button labels

### Phase 4 (2026-03-30)

- cargo build: OK
- cargo clippy: 0 warnings
- cargo test: all pass
- cargo fmt: OK (also fixed pre-existing format drift)

---

Last Updated: 2026-03-30
Status: ✅ Implementation Complete
