# Unsaved Changes Confirmation Dialog

Show a confirmation dialog when the user attempts to close a modified file (Ctrl+W) or quit with unsaved files (Ctrl+Q), preventing accidental data loss.

## Code Reference Checklist

| Item                    | Result                                                                                                                                                                                                |
| ----------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Similar feature exists? | No. `Ctrl+W` calls `handle_close_tab()` and `Ctrl+Q` sets `should_quit = true` immediately, without checking `is_modified()`.                                                                         |
| Reference pattern       | `help_visible: bool` in `Editor` + `HelpPopupWidget` in `termcode-term/src/ui/help_popup.rs`. Overlay rendered last in `render.rs`. `HelpPopupWidget` draws a centered popup with rounded borders.    |
| Technical constraints   | State must live in `Editor` (`termcode-view`, no ratatui). Widget lives in `termcode-term`. Dialog must block all other input while active (similar to `help_visible` intercepting keys in `app.rs`). |

### Key files

| File                                        | Role                                             |
| ------------------------------------------- | ------------------------------------------------ |
| `crates/termcode-view/src/editor.rs`        | `Editor` struct, `EditorMode`, state ownership   |
| `crates/termcode-view/src/document.rs`      | `Document.is_modified()`, `DocumentId`           |
| `crates/termcode-view/src/tab.rs`           | `Tab.modified`, `TabContent`                     |
| `crates/termcode-term/src/app.rs`           | `handle_key()`, `handle_close_tab()`, quit logic |
| `crates/termcode-term/src/render.rs`        | Overlay rendering order                          |
| `crates/termcode-term/src/ui/help_popup.rs` | Reference widget (centered popup pattern)        |
| `crates/termcode-term/src/ui/overlay.rs`    | Shared overlay utilities                         |

---

## Functional Requirements

### FR-CONFIRM-001: ConfirmDialog State

- **Description**: Add a `ConfirmDialog` state struct to `Editor` that represents an active confirmation dialog.
- **Priority**: High
- **Status**: Draft
- **Code Reference**: `Editor` struct in `crates/termcode-view/src/editor.rs:57`
- **Details**:
  - New types in `termcode-view` (no ratatui dependency):

    ```rust
    pub enum ConfirmAction {
        CloseTab(DocumentId),
        QuitAll,
    }

    pub struct ConfirmDialog {
        pub action: ConfirmAction,
        pub message: String,
        pub buttons: Vec<String>,       // e.g. ["Save", "Don't Save", "Cancel"]
        pub selected_button: usize,     // index of currently focused button
    }
    ```

  - Add `pub confirm_dialog: Option<ConfirmDialog>` to `Editor`.
  - When `confirm_dialog` is `Some`, the dialog is visible and blocks all other input.

### FR-CONFIRM-002: Ctrl+W Close Modified File

- **Description**: When closing a tab with unsaved changes via Ctrl+W, show a confirmation dialog instead of closing immediately.
- **Priority**: High
- **Status**: Draft
- **Code Reference**: `handle_close_tab()` in `crates/termcode-term/src/app.rs:1076`, Ctrl+W handling at `app.rs:536`
- **Details**:
  - **Modified file**: Set `editor.confirm_dialog` to `Some(ConfirmDialog)` with:
    - `action`: `ConfirmAction::CloseTab(doc_id)`
    - `message`: `"'{filename}'에 저장되지 않은 변경 사항이 있습니다."` (filename from `Document.path`)
    - `buttons`: `["저장 후 닫기", "저장 안 하고 닫기", "취소"]`
    - `selected_button`: `0`
  - **Unmodified file**: Close immediately (current behavior, no dialog).
  - **Image tab**: Close immediately (images are never modified).

### FR-CONFIRM-003: Ctrl+Q Quit with Unsaved Files

- **Description**: When quitting via Ctrl+Q with any unsaved files, show a confirmation dialog instead of quitting immediately.
- **Priority**: High
- **Status**: Draft
- **Code Reference**: Ctrl+Q handling in `crates/termcode-term/src/app.rs:492`
- **Details**:
  - **Any modified documents**: Set `editor.confirm_dialog` to `Some(ConfirmDialog)` with:
    - `action`: `ConfirmAction::QuitAll`
    - `message`: `"저장되지 않은 파일이 {count}개 있습니다."` (count = number of modified documents)
    - `buttons`: `["전부 저장 후 종료", "저장 안 하고 종료", "취소"]`
    - `selected_button`: `0`
  - **No modified documents**: Quit immediately (current behavior).
  - Count modified documents by iterating `editor.documents` and checking `is_modified()`.

### FR-CONFIRM-004: Dialog Button Actions

- **Description**: Define what each button does when confirmed.
- **Priority**: High
- **Status**: Draft
- **Code Reference**: `save_document()` at `editor.rs:198`, `close_document()` at `editor.rs:213`
- **Details**:

  **For CloseTab(doc_id):**

  | Button              | Action                                                                            |
  | ------------------- | --------------------------------------------------------------------------------- |
  | "저장 후 닫기"      | Call `save_document(doc_id)`, then close tab (existing `handle_close_tab` logic). |
  | "저장 안 하고 닫기" | Close tab without saving (existing `handle_close_tab` logic).                     |
  | "취소"              | Set `confirm_dialog = None`. No action.                                           |

  **For QuitAll:**

  | Button              | Action                                                                     |
  | ------------------- | -------------------------------------------------------------------------- |
  | "전부 저장 후 종료" | Save all modified documents via `save_document()`, then set `should_quit`. |
  | "저장 안 하고 종료" | Set `should_quit` without saving.                                          |
  | "취소"              | Set `confirm_dialog = None`. No action.                                    |
  - If save fails (e.g., no file path), show error in `status_message` and do NOT proceed with close/quit.

### FR-CONFIRM-005: Keyboard Navigation

- **Description**: Navigate and interact with the confirmation dialog using keyboard.
- **Priority**: High
- **Status**: Draft
- **Code Reference**: `handle_key()` in `crates/termcode-term/src/app.rs:491`
- **Details**:
  - Dialog input handling must be checked BEFORE all other key handlers in `handle_key()` (after Ctrl+Q/Ctrl+C interception is removed when dialog is active).
  - When `confirm_dialog` is `Some`, consume ALL key events (dialog blocks other input).

  | Key              | Action                                                        |
  | ---------------- | ------------------------------------------------------------- |
  | Left / Shift+Tab | Move `selected_button` to previous (wrap around)              |
  | Right / Tab      | Move `selected_button` to next (wrap around)                  |
  | Enter            | Execute the action for `selected_button` (see FR-CONFIRM-004) |
  | Escape           | Cancel (same as selecting "취소")                             |
  - While the dialog is active, Ctrl+Q and Ctrl+W must be consumed without effect (no re-triggering).

### FR-CONFIRM-006: Dialog Widget Rendering

- **Description**: Render the confirmation dialog as a centered overlay popup.
- **Priority**: High
- **Status**: Draft
- **Code Reference**: `HelpPopupWidget` in `crates/termcode-term/src/ui/help_popup.rs`, render order in `render.rs:184`
- **Details**:
  - New file: `crates/termcode-term/src/ui/confirm_dialog.rs`
  - Implement `ratatui::widgets::Widget` for `ConfirmDialogWidget`.
  - Register module in `crates/termcode-term/src/ui/mod.rs`.
  - Layout:
    ```
    ╭──────────────────────────────────────╮
    │                                      │
    │  'filename'에 저장되지 않은 변경       │
    │  사항이 있습니다.                      │
    │                                      │
    │  [저장 후 닫기] [저장 안 하고 닫기] [취소] │
    │                                      │
    ╰──────────────────────────────────────╯
    ```
  - Use rounded corner borders (╭╮╰╯) matching `HelpPopupWidget` style.
  - Theme colors: background from `theme.ui.background`, foreground from `theme.ui.foreground`, border from `theme.ui.border`.
  - Selected button: highlight with `theme.ui.selection` background or inverted colors.
  - Unselected buttons: rendered with `theme.ui.border` brackets and normal foreground text.
  - Popup size: auto-calculated from message length and button count. Minimum width to fit all buttons on one line.
  - Render AFTER `help_popup` in `render.rs` (highest z-order, rendered last).

---

## Implementation Notes

### State Placement

- `ConfirmDialog` struct and `ConfirmAction` enum go in a new file `crates/termcode-view/src/confirm.rs` (or inline in `editor.rs`).
- `Editor.confirm_dialog: Option<ConfirmDialog>` — `None` means no dialog, `Some` means dialog is visible and blocking.

### Event Flow

```
Ctrl+W pressed
  → Check active tab's document.is_modified()
  → If modified: editor.confirm_dialog = Some(CloseTab dialog)
  → If not modified: close immediately (current behavior)

Ctrl+Q pressed
  → Count modified documents
  → If any: editor.confirm_dialog = Some(QuitAll dialog)
  → If none: should_quit = true (current behavior)

Any key while confirm_dialog is Some
  → Route to confirm dialog handler (consume event)
  → On Enter: execute action, set confirm_dialog = None
  → On Esc: set confirm_dialog = None
```

### Render Order in render.rs

```
... existing overlays ...
help_popup overlay
confirm_dialog overlay  ← NEW (last = highest z-order)
```

### No New EditorMode

The dialog does NOT add a new `EditorMode` variant. Instead, input blocking is handled by checking `editor.confirm_dialog.is_some()` at the top of `handle_key()`. This avoids modifying the mode enum and input mapper, keeping the change minimal.

---

## Out of Scope

- Custom keybinding configuration for dialog actions
- Configurable dialog appearance via `config.toml`
- Multiple file selection for partial save (save some, discard others)
- "Save As" option in the dialog
- Untitled/new file save-as prompt (assumes all documents have a path; if no path, save fails with error message)
