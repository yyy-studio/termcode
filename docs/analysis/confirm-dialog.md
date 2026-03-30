# Analysis: Unsaved Changes Confirmation Dialog

## Impact Summary

### Directly Modified Files (7)

| File                                            | Change                                                                         | Risk     |
| ----------------------------------------------- | ------------------------------------------------------------------------------ | -------- |
| `crates/termcode-view/src/confirm.rs`           | NEW: ConfirmDialog, ConfirmAction types                                        | Low      |
| `crates/termcode-view/src/lib.rs`               | Add `pub mod confirm;`                                                         | Low      |
| `crates/termcode-view/src/editor.rs`            | Add `confirm_dialog: Option<ConfirmDialog>` field                              | Low      |
| `crates/termcode-term/src/app.rs`               | handle_key intercept, handle_close_tab split, Ctrl+Q guard, handle_mouse guard | **High** |
| `crates/termcode-term/src/ui/confirm_dialog.rs` | NEW: ConfirmDialogWidget                                                       | Medium   |
| `crates/termcode-term/src/ui/mod.rs`            | Add `pub mod confirm_dialog;`                                                  | Low      |
| `crates/termcode-term/src/render.rs`            | Add confirm dialog overlay after help_popup                                    | Low      |

### Referenced Files (read-only)

| File                                        | Role                                      |
| ------------------------------------------- | ----------------------------------------- |
| `crates/termcode-view/src/document.rs`      | DocumentId, is_modified(), display_name() |
| `crates/termcode-view/src/tab.rs`           | Tab.content, TabContent enum              |
| `crates/termcode-term/src/ui/help_popup.rs` | Reference widget pattern                  |
| `crates/termcode-theme/src/theme.rs`        | Theme color fields                        |

## Reusable Patterns

1. **HelpPopupWidget**: Manual centered popup with rounded borders (help_popup.rs) - reference for widget structure
2. **Input blocking**: `help_visible` check in handle_key() - but dialog must come BEFORE it
3. **Editor optional state**: `Option<T>` pattern (completion, hover) for dialog visibility
4. **Theme colors**: `theme.ui.{background,foreground,border,selection}.to_ratatui()`
5. **Save flow**: `editor.save_document(doc_id)` + `lsp_notify_did_save()` (app.rs:750)
6. **Close flow**: `handle_close_tab()` with plugin hook + LSP close + editor.close_document

## Risks & Mitigations

| #   | Risk                                       | Severity | Mitigation                                                         |
| --- | ------------------------------------------ | -------- | ------------------------------------------------------------------ |
| R1  | Save failure inconsistent state            | HIGH     | Catch error, show status_message, dismiss dialog, abort close/quit |
| R2  | Ctrl+Q bypasses dialog (wrong check order) | HIGH     | Dialog check MUST be FIRST in handle_key(), before Ctrl+Q          |
| R3  | QuitAll partial save failure               | HIGH     | Save all, collect errors, abort quit on any failure                |
| R4  | Mouse events not blocked during dialog     | MEDIUM   | Add early return in handle_mouse()                                 |
| R5  | OnClose hook fires before confirmation     | MEDIUM   | Defer hook to after dialog confirm                                 |
| R6  | Stale DocumentId in dialog                 | LOW      | Validate doc_id exists before executing action                     |
| R7  | Borrow checker conflict                    | MEDIUM   | Extract locals before mutating editor                              |

## Conflicts Resolved

- **overlay.rs vs manual rendering**: overlay.rs uses square corners, spec requires rounded corners matching help_popup. Decision: use manual rendering approach from HelpPopupWidget.

## Design Recommendation

- **Architecture**: none (existing patterns sufficient)
- **UX**: none (spec already defines complete UX)
- **Layers**: single (Rust terminal only)
