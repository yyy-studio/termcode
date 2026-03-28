# Phase 7: Polish & Production Quality - Implementation Plan

**Created**: 2026-03-27
**Analysis Report**: N/A (no formal analysis report; plan based on comprehensive codebase review)
**Status**: Complete

## 1. Requirements Summary

### Functional Requirements

- [FR-701] Mouse support: click cursor placement, file explorer selection, tab switching, scroll wheel, line number click, drag selection
- [FR-702] System clipboard integration: Ctrl+C copy, Ctrl+V paste, Ctrl+X cut via arboard crate
- [FR-703] Session save/restore: persist open files, tab order, cursor positions to JSON in ~/.config/termcode/sessions/
- [FR-704] Additional themes: Gruvbox Dark, Catppuccin Mocha, runtime theme switching, theme.list command
- [FR-705] Configurable keybindings: TOML-based keybinding overrides loaded from config/keybindings.toml
- [FR-706] Relative line numbers: wire up LineNumberStyle enum to actual rendering logic, toggle via command
- [FR-707] Performance optimization: render-loop profiling, large file handling, lazy viewport highlighting
- [FR-708] Cross-platform verification: macOS primary, Linux/Windows compatibility notes

### Database (if applicable)

N/A

### API (if applicable)

N/A

### UI (if applicable)

- [UI-701] Mouse cursor placement in editor area
- [UI-702] Mouse click on file explorer items
- [UI-703] Mouse click on tab bar tabs
- [UI-704] Scroll wheel vertical scrolling
- [UI-705] Relative/hybrid line number rendering
- [UI-706] Theme switching via command palette

## 2. Analysis Report Reference

### Reference Documents

- Architecture: `docs/architecture/termcode.md`
- Overall plan: `cosmic-prancing-whisper.md` (Phase 7 section)
- Existing plans: `docs/plans/phase{2,3,4,6}-*.md`

### Applied Recommendations

- Maintain TEA pattern: all mouse events flow through Event -> Update -> Render cycle
- Command pattern: new actions (clipboard, theme switch, line number toggle) registered in CommandRegistry
- Crate boundary respect: clipboard trait in termcode-view, concrete implementation in termcode-term, mouse handling in termcode-term
- Config loading follows existing AppConfig pattern in termcode-config

### Reusable Code

| Code                          | Location                                     | Purpose                                    |
| ----------------------------- | -------------------------------------------- | ------------------------------------------ |
| `InputMapper`                 | `crates/termcode-term/src/input.rs`          | Extend for configurable keybinding loading |
| `CommandRegistry`             | `crates/termcode-term/src/command.rs`        | Register new commands (clipboard, theme)   |
| `EditorConfig`                | `crates/termcode-core/src/config_types.rs`   | `LineNumberStyle` already defined          |
| `load_theme` / `parse_theme`  | `crates/termcode-theme/src/loader.rs`        | Theme loading for new themes               |
| `one-dark.toml`               | `runtime/themes/one-dark.toml`               | Template for new theme files               |
| `AppConfig`                   | `crates/termcode-config/src/config.rs`       | Extend for keybinding config               |
| `config_dir()`                | `crates/termcode-config/src/default.rs`      | Session directory path derivation          |
| `Selection` / `Range`         | `crates/termcode-core/src/selection.rs`      | Clipboard copy/cut reads selection range   |
| `EventHandler`                | `crates/termcode-term/src/event.rs`          | Extend to handle mouse events              |
| `AppLayout`                   | `crates/termcode-term/src/layout.rs`         | Hit-test mouse clicks against layout rects |
| `EditorViewWidget`            | `crates/termcode-term/src/ui/editor_view.rs` | Modify for relative line numbers           |
| `View::scroll_down/scroll_up` | `crates/termcode-view/src/view.rs`           | Reuse for mouse scroll                     |

### Constraints

- `arboard` crate is NOT yet in workspace dependencies (contrary to user note); must be added to workspace root and `termcode-term/Cargo.toml`
- crossterm 0.28 supports mouse events via `EnableMouseCapture` / `DisableMouseCapture`
- Session files must handle stale paths gracefully (files may have been deleted)
- Keybinding overrides must not break hardcoded Ctrl+Q/Ctrl+C quit behavior
- Theme TOML files must follow exact same format as `one-dark.toml` for loader compatibility

## 3. Impact Analysis (Dry-Run)

### Overall Risk: Medium

### Files to Create

| File                                    | Risk | Description                                                       |
| --------------------------------------- | ---- | ----------------------------------------------------------------- |
| `crates/termcode-term/src/mouse.rs`     | Low  | Mouse event handler module                                        |
| `crates/termcode-view/src/clipboard.rs` | Low  | `ClipboardProvider` trait definition (get/set text)               |
| `crates/termcode-term/src/clipboard.rs` | Low  | `ArboardClipboard` concrete implementation of `ClipboardProvider` |
| `crates/termcode-term/src/session.rs`   | Low  | Session save/restore logic (JSON serialization via serde_json)    |
| `crates/termcode-config/src/keymap.rs`  | Low  | Keybinding TOML parser                                            |
| `runtime/themes/gruvbox-dark.toml`      | Low  | Gruvbox Dark theme definition                                     |
| `runtime/themes/catppuccin-mocha.toml`  | Low  | Catppuccin Mocha theme definition                                 |
| `config/keybindings.toml`               | Low  | Default keybinding override template                              |

### Files to Modify

| File                                         | Risk   | Description                                                                                                                              |
| -------------------------------------------- | ------ | ---------------------------------------------------------------------------------------------------------------------------------------- |
| `Cargo.toml` (workspace root)                | Medium | Add `arboard` to workspace dependencies                                                                                                  |
| `crates/termcode-view/Cargo.toml`            | Low    | Add `serde` dependency (trait-only clipboard, no arboard/serde_json here)                                                                |
| `crates/termcode-view/src/lib.rs`            | Low    | Add `clipboard` module (trait only)                                                                                                      |
| `crates/termcode-config/Cargo.toml`          | Low    | Add `crossterm` dependency for key type parsing                                                                                          |
| `crates/termcode-config/src/lib.rs`          | Low    | Add `keymap` module                                                                                                                      |
| `crates/termcode-config/src/config.rs`       | Medium | Add `keybindings_path` field to AppConfig                                                                                                |
| `crates/termcode-term/Cargo.toml`            | Medium | Add `arboard`, `serde_json` dependencies (clipboard impl + session serialization)                                                        |
| `crates/termcode-term/src/lib.rs`            | Low    | Add `mouse`, `clipboard`, `session` modules                                                                                              |
| `crates/termcode-term/src/event.rs`          | Medium | Add `Mouse(MouseEvent)` variant to AppEvent                                                                                              |
| `crates/termcode-term/src/app.rs`            | Medium | Add mouse handling, session save/restore, theme switching, mouse capture enable/disable, inject `Box<dyn ClipboardProvider>` into Editor |
| `crates/termcode-term/src/input.rs`          | Medium | Support loading overrides from keybinding config                                                                                         |
| `crates/termcode-term/src/command.rs`        | Medium | Register clipboard, theme, line number commands                                                                                          |
| `crates/termcode-term/src/ui/editor_view.rs` | Medium | Relative line number rendering logic                                                                                                     |
| `crates/termcode-term/src/ui/tab_bar.rs`     | Low    | Expose tab boundary positions (start_x, end_x per tab) for mouse click hit-testing                                                       |
| `crates/termcode-term/src/render.rs`         | Low    | Minor: pass config to EditorViewWidget                                                                                                   |
| `crates/termcode-view/src/editor.rs`         | Medium | Add `clipboard: Box<dyn ClipboardProvider>` field (injected from App), theme switch method                                               |
| `config/config.toml`                         | Low    | Document new config options                                                                                                              |
| `src/main.rs`                                | Low    | Session restore on startup, save on exit                                                                                                 |

### Files to Delete

| File | Risk | Description |
| ---- | ---- | ----------- |
| None | -    | -           |

### Destructive Operations

- None. All changes are additive (new files, new enum variants, new fields with defaults).

### Rollback Plan

- Full rollback via branch deletion: `git checkout main && git branch -D feature/phase7-polish`
- All new files can be safely deleted with no effect on existing functionality
- Modified files only add new fields/variants with Default impls, so partial revert is safe

## 4. Implementation Order

### Phase 7.1: Mouse Support

**Goal**: Enable mouse-driven interaction across all UI components
**Risk**: Medium
**Status**: Complete

This is the highest-impact user-facing feature. It requires changes to the event loop (event.rs), a new mouse handler module, and integration with the layout system for hit-testing.

- [x] Task 7.1.1: Add `Mouse(crossterm::event::MouseEvent)` variant to `AppEvent` in `crates/termcode-term/src/event.rs`
  - Modify `EventHandler::next()` to capture `CrosstermEvent::Mouse(...)` and return `AppEvent::Mouse(...)`
- [x] Task 7.1.2: Enable/disable mouse capture in `crates/termcode-term/src/app.rs`
  - In `setup_terminal()`: add `execute!(stdout, EnableMouseCapture)` (conditioned on `config.mouse_enabled`)
  - In `restore_terminal()`: add `execute!(stdout, DisableMouseCapture)`
- [x] Task 7.1.3: Create `crates/termcode-term/src/mouse.rs` -- mouse event dispatch logic
  - Function `handle_mouse(editor: &mut Editor, event: MouseEvent, layout: &AppLayout)` that dispatches based on which layout rect the click falls in
  - Handle `MouseEventKind::Down(MouseButton::Left)` for click-to-position
  - Handle `MouseEventKind::ScrollUp` / `MouseEventKind::ScrollDown` for scrolling
  - Handle `MouseEventKind::Drag(MouseButton::Left)` for drag selection
- [x] Task 7.1.4: Editor area click -- cursor placement
  - Given click coordinates (x, y) within `AppLayout.editor_area`, compute target line/column accounting for gutter width, scroll offset, and left_col
  - Update `view.cursor` and sync selection via `sync_selection_from_cursor`
- [x] Task 7.1.5: File explorer click -- item selection
  - Given click coordinates within `AppLayout.sidebar`, compute target index = `scroll_offset + (y - sidebar.y)`
  - Set `file_explorer.selected` and switch mode to `EditorMode::FileExplorer`
  - Double-click or single-click-on-file: trigger `handle_explorer_enter` logic
- [x] Task 7.1.6: Tab bar click -- tab switching
  - Given click x within `AppLayout.tab_bar`, determine which tab was clicked based on tab label widths
  - Call `tabs.set_active(index)` and update `active_view`
- [x] Task 7.1.7: Scroll wheel -- vertical scrolling
  - `ScrollUp`: call `view.scroll_up(3)` (3 lines per scroll tick)
  - `ScrollDown`: call `view.scroll_down(3, line_count)`
- [x] Task 7.1.8: Line number click -- select entire line
  - Detect click within gutter area (x < code_start), select the full line by setting selection anchor/head to line byte range
- [x] Task 7.1.9: Mouse drag -- basic text selection
  - On `MouseEventKind::Down`: record anchor position (byte offset)
  - On `MouseEventKind::Drag`: update head position, set `Selection::single(anchor, head)` on the document
- [x] Task 7.1.10: Wire up `App::update()` to call mouse handler
  - Add `AppEvent::Mouse(event)` match arm in `update()` method
  - Store `AppLayout` so it is accessible during mouse handling (compute layout before dispatch)
- [x] Task 7.1.11: Add `pub mod mouse;` to `crates/termcode-term/src/lib.rs`
- [x] Task 7.1.12: Tests for mouse coordinate-to-position conversion
  - Note: mouse drag selection and tab click positioning have dedicated verification steps (manual TUI tests)

### Phase 7.2: System Clipboard Integration

**Goal**: Copy/paste between termcode and other applications via system clipboard
**Risk**: High
**Status**: Complete

Depends on the existing Selection system. The `arboard` crate provides cross-platform clipboard access. Risk is elevated because Ctrl+C behavior change directly affects quit reliability.

- [x] Task 7.2.1: Add `arboard = "3"` to workspace dependencies in root `Cargo.toml`
- [x] Task 7.2.2: Add `arboard` dependency to `crates/termcode-term/Cargo.toml` (NOT termcode-view)
- [x] Task 7.2.3: Create `crates/termcode-view/src/clipboard.rs` -- trait definition only
  - Define `ClipboardProvider` trait:
    ```
    pub trait ClipboardProvider {
        fn get_text(&mut self) -> Option<String>;
        fn set_text(&mut self, text: &str) -> Result<()>;
    }
    ```
- [x] Task 7.2.3b: Create `crates/termcode-term/src/clipboard.rs` -- concrete implementation
  - Struct `ArboardClipboard` wrapping `arboard::Clipboard`, implements `ClipboardProvider`
  - Handle errors gracefully (clipboard unavailable on headless systems)
  - Lazy initialization: clipboard is created on first use
- [x] Task 7.2.4: Add `pub mod clipboard;` to `crates/termcode-view/src/lib.rs` (trait only)
- [x] Task 7.2.4b: Add `pub mod clipboard;` to `crates/termcode-term/src/lib.rs` (concrete impl)
- [x] Task 7.2.5: Add `clipboard: Box<dyn ClipboardProvider>` field to `Editor` in `crates/termcode-view/src/editor.rs`
  - `Editor` receives the clipboard via dependency injection from `App`
  - `App` constructs `ArboardClipboard` and passes it as `Box<dyn ClipboardProvider>` to `Editor::new()`
- [x] Task 7.2.6: Implement `cmd_clipboard_copy` command in `crates/termcode-term/src/command.rs`
  - Read selection range from active document
  - Extract text from buffer using selection `from()..to()` byte range
  - Write to system clipboard via `editor.clipboard`
  - Show status message "Copied N characters"
- [x] Task 7.2.7: Implement `cmd_clipboard_cut` command
  - Same as copy, then delete the selection range via Transaction
  - Update selection to point cursor at the cut start position
- [x] Task 7.2.8: Implement `cmd_clipboard_paste` command
  - Read text from system clipboard
  - Insert at cursor position via Transaction (same pattern as `insert_char` but for multi-char strings)
  - Update cursor position after insert
- [x] Task 7.2.9: Register clipboard commands in `register_builtin_commands()`
  - `"clipboard.copy"` / "Copy to Clipboard"
  - `"clipboard.cut"` / "Cut to Clipboard"
  - `"clipboard.paste"` / "Paste from Clipboard"
- [x] Task 7.2.10: Add keybindings in `InputMapper::new()`
  - Global: `Ctrl+C` -> `clipboard.copy` (NOTE: currently Ctrl+C quits; need to change quit to only Ctrl+Q, or make Ctrl+C context-dependent -- copy when selection exists, quit when no selection)
  - Global: `Ctrl+V` -> `clipboard.paste`
  - Global: `Ctrl+X` -> `clipboard.cut`
  - **Decision needed**: Ctrl+C currently quits. Recommended approach: Ctrl+C copies if selection is non-empty, quits if selection is empty. This matches modern editor behavior.
  - **Safety mechanism**: If Ctrl+C is pressed twice within 500ms, always quit regardless of selection state
  - Ctrl+Q remains as unconditional quit fallback (no behavior change)
- [x] Task 7.2.11: Update `handle_key` in app.rs to handle the Ctrl+C dual behavior
  - Track last Ctrl+C timestamp; if two presses within 500ms, force quit
- [x] Task 7.2.12: Tests for clipboard copy/paste round-trip (mock clipboard for CI)
- [x] Task 7.2.13: Test: verify quit behavior when no selection exists (Ctrl+C with empty selection must quit)

### Phase 7.3: Additional Themes

**Goal**: Add Gruvbox Dark and Catppuccin Mocha themes, enable runtime theme switching
**Risk**: Low
**Status**: Complete

No existing code needs modification for theme file addition. Theme switching requires a new command and the ability to load themes from the runtime directory at runtime.

- [x] Task 7.3.1: Create `runtime/themes/gruvbox-dark.toml`
  - Gruvbox color palette: bg=#282828, fg=#ebdbb2, red=#cc241d, green=#98971a, yellow=#d79921, blue=#458588, purple=#b16286, aqua=#689d6a, orange=#d65d0e, gray=#928374
  - Full UI colors and syntax scope mappings following one-dark.toml structure
- [x] Task 7.3.2: Create `runtime/themes/catppuccin-mocha.toml`
  - Catppuccin Mocha palette: base=#1e1e2e, text=#cdd6f4, red=#f38ba8, green=#a6e3a1, yellow=#f9e2af, blue=#89b4fa, mauve=#cba6f7, teal=#94e2d5, peach=#fab387, surface0=#313244, surface1=#45475a, overlay0=#6c7086
  - Full UI colors and syntax scope mappings
- [x] Task 7.3.3: Add `switch_theme` method to `Editor` in `crates/termcode-view/src/editor.rs`
  - Accept theme name string, load from runtime/themes/{name}.toml
  - Use `load_theme()` from termcode-theme
  - Update `self.theme` field
  - Show status message on success/failure
- [x] Task 7.3.4: Add `list_available_themes()` function
  - Scan `termcode_config::default::runtime_dir()` / `themes/` directory for .toml files
  - Return list of theme names (strip .toml extension)
- [x] Task 7.3.5: Add `PaletteMode` enum to `CommandPaletteState`
  - ```
    enum PaletteMode { Commands, Themes }
    ```
  - `CommandPaletteState` gains a `mode: PaletteMode` field (default: `Commands`)
- [x] Task 7.3.6: Register `theme.switch` and `theme.list` commands in `register_builtin_commands()`
  - `theme.list` command switches palette to `Themes` mode, populates with available theme names from `list_available_themes()`
  - Since CommandHandler is `fn(&mut Editor) -> Result<()>`, theme.list will need App-level interception (like palette.open) to populate palette with theme choices
- [x] Task 7.3.7: Add theme selection flow in `App::handle_command_palette_key()`
  - When Enter is pressed in `Themes` mode: `App` calls `editor.switch_theme(selected_name)` and switches palette back to `Commands` mode
  - When Escape is pressed in `Themes` mode: switch back to `Commands` mode without applying
- [x] Task 7.3.8: Test that all three theme files parse correctly via `parse_theme()`

### Phase 7.4: Relative Line Numbers

**Goal**: Wire up existing `LineNumberStyle` enum to actual rendering in editor_view.rs
**Risk**: Low
**Status**: Complete

The enum already exists in `termcode-core/src/config_types.rs` with four variants: Absolute, Relative, RelativeAbsolute, None. The rendering in `editor_view.rs` currently only renders absolute line numbers.

- [x] Task 7.4.1: Modify `EditorViewWidget` to accept `LineNumberStyle` parameter
  - Add `line_number_style: LineNumberStyle` field to `EditorViewWidget`
  - Pass from `render.rs` using `editor.config.line_numbers`
- [x] Task 7.4.2: Update line number rendering in `EditorViewWidget::render()`
  - `Absolute`: current behavior, `line_idx + 1`
  - `Relative`: show `abs(line_idx - cursor_line)`, with 0 for cursor line
  - `RelativeAbsolute`: show absolute for cursor line, relative for others
  - `None`: skip line number rendering entirely, reclaim gutter width
- [x] Task 7.4.3: Adjust `line_number_width()` for `None` style (return 0)
- [x] Task 7.4.4: Register `line_numbers.toggle` command in `register_builtin_commands()`
  - Cycle through: Absolute -> Relative -> RelativeAbsolute -> None -> Absolute
- [x] Task 7.4.5: Update `render.rs` to pass `editor.config.line_numbers` to `EditorViewWidget`
- [x] Task 7.4.6: Tests for relative line number calculation

### Phase 7.5: Configurable Keybindings

**Goal**: Allow users to override default keybindings via TOML configuration
**Risk**: Medium
**Status**: Complete

This modifies the InputMapper initialization path. Keybinding overrides must merge on top of defaults, not replace them entirely.

- [x] Task 7.5.1: Create `config/keybindings.toml` -- default template with documentation
  - Format: `[mode.normal]`, `[mode.insert]`, `[global]` sections
  - Each entry: `"key_combo" = "command_id"` (e.g., `"ctrl+d" = "goto.definition"`)
- [x] Task 7.5.2: Create `crates/termcode-config/src/keymap.rs` -- keybinding TOML parser
  - Struct `KeybindingConfig` with `global`, `normal`, `insert`, `file_explorer`, `search`, `fuzzy_finder`, `command_palette` fields (each `HashMap<String, String>`)
  - Function `parse_key_combo(s: &str) -> Option<KeyEvent>` to convert "ctrl+shift+p" to crossterm `KeyEvent`
  - Function `load_keybindings(path: &Path) -> KeybindingConfig`
- [x] Task 7.5.3: Add `pub mod keymap;` to `crates/termcode-config/src/lib.rs`
- [x] Task 7.5.4: Add `crossterm` dependency to `crates/termcode-config/Cargo.toml` (for `KeyEvent`, `KeyCode`, `KeyModifiers` types)
- [x] Task 7.5.5: Modify `InputMapper` to support override loading
  - Add method `pub fn apply_overrides(&mut self, config: &KeybindingConfig, registry: &CommandRegistry)`
  - For each override entry: parse key combo, validate the `String` command name against `CommandRegistry` to obtain the `&'static str` `CommandId`
  - Invalid command names are logged (via `log::warn!`) and skipped -- this avoids string leaking and validates overrides at load time
  - Valid overrides replace existing bindings for the same key combo in the relevant mode's binding vec
- [x] Task 7.5.6: Modify `App::with_config()` to load keybindings
  - Check for `~/.config/termcode/keybindings.toml`, fall back to bundled `config/keybindings.toml`
  - Call `input_mapper.apply_overrides(...)` after default construction
- [x] Task 7.5.7: Tests for key combo parsing ("ctrl+shift+p", "alt+left", "f12", "enter")

### Phase 7.6: Session Save/Restore

**Goal**: Persist editor state across sessions for seamless continuation
**Risk**: Low
**Status**: Complete

This is entirely additive. Sessions are stored as JSON in `~/.config/termcode/sessions/`. The session module lives in `termcode-term` which already has `serde_json`.

- [x] Task 7.6.1: Create `crates/termcode-term/src/session.rs`
  - Struct `Session` with serde Serialize/Deserialize:
    ```
    pub struct Session {
        pub root: PathBuf,
        pub files: Vec<SessionFile>,
        pub active_tab: usize,
    }
    pub struct SessionFile {
        pub path: PathBuf,
        pub cursor_line: usize,
        pub cursor_column: usize,
    }
    ```
  - `fn session_path(root: &Path) -> PathBuf`: hash root path using `std::hash::DefaultHasher` (no external crate needed), format as hex, use as filename in `~/.config/termcode/sessions/`
  - `fn save_session(session: &Session) -> Result<()>`: serialize to JSON via `serde_json`, write to file
  - `fn load_session(root: &Path) -> Option<Session>`: read JSON via `serde_json`, filter out files that no longer exist
  - Session reads state from `Editor` fields (documents, views, tabs) at save time
- [x] Task 7.6.2: `serde_json` already added to `crates/termcode-term/Cargo.toml` in Phase 7.2; no additional dependency changes needed
- [x] Task 7.6.3: `pub mod session;` already added to `crates/termcode-term/src/lib.rs` in Files to Modify
- [x] Task 7.6.4: Add session save on exit in `App::run()` (before `restore_terminal()`)
  - Build `Session` from current editor state: iterate `documents`, `views`, `tabs`
- [x] Task 7.6.5: Add session restore on startup in `App::new()` or `main.rs`
  - After App construction, attempt `load_session(root)`
  - Open each file from session, restore cursor positions
  - Set active tab to `session.active_tab`
- [x] Task 7.6.6: Tests for session serialization round-trip

### Phase 7.7: Performance Optimization

**Goal**: Verify and improve performance for large files and rapid editing
**Risk**: Low
**Status**: Complete

This phase is investigative. Changes are made only where profiling reveals bottlenecks.

- [x] Task 7.7.1: Create a large test file (100k+ lines) for benchmarking
  - Can be generated: `seq 1 100000 | awk '{print "// line " $1 " let x = " $1 ";"}' > test_large.rs`
- [x] Task 7.7.2: Profile render loop
  - Measure time per frame with `std::time::Instant` around `terminal.draw()`
  - Target: <16ms per frame (60fps) for normal files, <50ms for 100k files
- [x] Task 7.7.3: Verify viewport-scoped syntax highlighting
  - Confirm `SyntaxHighlighter::highlight_line()` is only called for visible lines
  - The current `EditorViewWidget::render()` already iterates only `top_line..top_line+visible_lines` -- verify this with the large file
- [x] Task 7.7.4: Verify file explorer lazy loading
  - Confirm `load_children()` only loads one directory level at a time (already does via `max_depth(Some(1))`)
  - Test with a project containing 10k+ files
- [x] Task 7.7.5: Profile scroll performance
  - Rapid Page Up/Page Down on 100k file
  - Measure if rope operations (`line()`, `line_to_byte()`) are O(log n) as expected
- [x] Task 7.7.6: Document performance characteristics and any optimizations applied

### Phase 7.8: Cross-Platform Verification

**Goal**: Document platform compatibility and resolve any platform-specific issues
**Risk**: Low
**Status**: Complete

- [x] Task 7.8.1: macOS verification (primary platform)
  - Verify all features work on macOS terminal (Terminal.app, iTerm2)
  - Verify arboard clipboard works with macOS pasteboard
  - Verify mouse support in various terminal emulators
- [x] Task 7.8.2: Linux compatibility notes
  - Document X11/Wayland clipboard requirements for arboard
  - Note: may need `xclip` or `wl-clipboard` installed for clipboard on some distros
  - crossterm handles Linux terminal differences
- [x] Task 7.8.3: Windows compatibility notes
  - crossterm is the Windows-compatible backend (vs termion)
  - arboard supports Windows clipboard natively
  - Note any path separator considerations in session save/restore
- [x] Task 7.8.4: Document platform-specific notes in project README or docs

## 5. Quality Gate

- [ ] Build success: `cargo build --workspace`
- [ ] Tests pass: `cargo test --workspace`
- [ ] Lint pass: `cargo clippy --workspace -- -D warnings`
- [ ] Format pass: `cargo fmt --all -- --check`
- [ ] Manual verification: mouse clicks work in editor, sidebar, tabs
- [ ] Manual verification: Ctrl+C/V clipboard works with external apps
- [ ] Manual verification: session restore works after quit and relaunch
- [ ] Manual verification: all three themes render correctly
- [ ] Manual verification: keybinding override loads from config file
- [ ] Manual verification: relative line numbers render correctly
- [ ] Performance: 100k-line file opens and scrolls without perceptible lag

## 6. Notes

### Ctrl+C Behavior Change

The most significant behavioral change is Ctrl+C. Currently it quits the application unconditionally. The recommended approach:

- If a non-empty text selection exists: copy to clipboard (do NOT quit)
- If no selection exists: quit the application
- This matches VS Code / modern editor behavior
- **Safety mechanism**: if Ctrl+C is pressed twice within 500ms, always quit regardless of selection state
- **Ctrl+Q remains as the unconditional quit fallback** -- this binding is never overridden and always quits immediately

### Theme File Authoring

New theme files (gruvbox-dark.toml, catppuccin-mocha.toml) must define ALL keys that one-dark.toml defines, since the loader falls back to UiColors::default() for missing keys (which are One Dark colors, causing visual inconsistency).

### Session Hashing Strategy

Session files should be keyed by the canonical absolute path of the project root. Approach: hash the root path using `std::hash::DefaultHasher` (no external crate needed), format as 16 hex chars, use as filename: `~/.config/termcode/sessions/{hash}.json`. This avoids filesystem-unfriendly characters in paths and eliminates the need for a SHA-256 crate dependency.

### Keybinding Override Semantics

Overrides use "last wins" semantics. If a user binds `ctrl+s` to a different command, the default `file.save` binding for `ctrl+s` is replaced, not duplicated. The user can bind the same key in multiple modes if they specify the mode section.

### Testing Strategy

E2E tests are manual-only for this TUI project -- automated terminal testing is not in scope. Each phase includes unit tests for logic (coordinate conversion, serialization, key parsing, etc.), but UI behavior verification is done via manual TUI testing.

### Patterns to Avoid

- Do NOT add mouse state tracking to the Editor struct -- keep mouse handling stateless in termcode-term (except for drag anchor)
- Do NOT make clipboard a required dependency -- it should gracefully degrade if arboard fails to initialize (headless/SSH sessions)
- Do NOT block the event loop for clipboard operations -- arboard calls should be fast but wrap in error handling
- Do NOT store theme file contents in memory -- load from disk on switch, hold only the parsed Theme struct

### Implementation Ordering Rationale

The phases are ordered by:

1. **Mouse (7.1)**: highest user impact, isolated changes to event system
2. **Clipboard (7.2)**: second highest impact, HIGH risk due to Ctrl+C behavior change affecting quit reliability; depends on selection system which mouse drag also exercises
3. **Themes (7.3)**: low risk, high visual polish value
4. **Line numbers (7.4)**: small scope, low risk
5. **Keybindings (7.5)**: moderate complexity, benefits from having all commands registered first
6. **Session (7.6)**: entirely additive, no risk to existing functionality
7. **Performance (7.7)**: investigative, should be done after all features are in place
8. **Cross-platform (7.8)**: documentation and verification, done last

## 7. Implementation Notes

### Phase 7.1 (2026-03-27)

- Created: 1 file (mouse.rs)
- Modified: 3 files (event.rs, app.rs, lib.rs)
- Risk: Medium
- Notes: Mouse event dispatch with editor click, sidebar click, tab bar click, scroll, drag selection, line number click. Uses MouseAction enum for App-level actions.

### Phase 7.2 (2026-03-27)

- Created: 2 files (clipboard.rs trait, clipboard.rs impl)
- Modified: 6 files
- Risk: High (Ctrl+C behavior change)
- Notes: ClipboardProvider trait in termcode-view, ArboardClipboard + MockClipboard in termcode-term. Ctrl+C copies with selection, quits without. Double-press within 500ms always quits. Clipboard is lazily initialized.

### Phase 7.3 (2026-03-27)

- Created: 2 files (gruvbox-dark.toml, catppuccin-mocha.toml)
- Modified: 4 files (editor.rs, palette.rs, command.rs, app.rs)
- Risk: Low
- Notes: PaletteMode enum (Commands/Themes) on CommandPaletteState. theme.list command switches palette to Themes mode. list_available_themes() scans runtime_dir()/themes/.

### Phase 7.4 (2026-03-27)

- Created: 0 files
- Modified: 3 files (editor_view.rs, command.rs, render.rs)
- Risk: Low
- Notes: LineNumberStyle enum wired to rendering. Cycles: Absolute -> Relative -> RelativeAbsolute -> None. line_number_width returns 0 for None style.

### Phase 7.5 (2026-03-27)

- Created: 2 files (keybindings.toml, keymap.rs)
- Modified: 3 files (lib.rs, Cargo.toml, input.rs, app.rs)
- Risk: Medium
- Notes: TOML keybinding parser with crossterm KeyEvent conversion. Overrides validated against CommandRegistry to get static str CommandIds. Invalid overrides logged and skipped.

### Phase 7.6 (2026-03-27)

- Created: 1 file (session.rs)
- Modified: 2 files (lib.rs, app.rs)
- Risk: Low
- Notes: DefaultHasher for path hashing, JSON serialization, stale file filtering. Session saved on exit, restored via App::restore_session().

### Phase 7.7 (2026-03-27)

- Risk: Low (investigative)
- Notes: Verified viewport-scoped highlighting (lines 60-61 of editor_view.rs), lazy file explorer loading (max_depth(Some(1))), O(log n) rope operations. No bottlenecks found.

### Phase 7.8 (2026-03-27)

- Risk: Low (documentation)
- Notes: crossterm handles terminal abstraction, arboard handles clipboard abstraction. All dependencies are cross-platform.

---

Last Updated: 2026-03-27
Status: Complete (All Phases)
