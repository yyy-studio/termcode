# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Test Commands

```bash
cargo build                          # Build all crates
cargo test --workspace               # Run all tests
cargo test -p termcode-core          # Run tests for a single crate
cargo test -p termcode-core -- test_name  # Run a specific test
cargo clippy --workspace             # Lint (must be 0 warnings)
cargo fmt --check                    # Format check
cargo fmt                            # Auto-format
cargo run -- .                       # Run with file explorer on current dir
cargo run -- path/to/file.rs         # Open a specific file
```

## Architecture

### Crate Dependency Layers (strict downward-only)

```
Layer 0: termcode-core, termcode-theme          (no internal deps)
Layer 1: termcode-syntax, termcode-config       (deps: core, theme)
Layer 2: termcode-view, termcode-lsp            (deps: core, syntax/config, theme)
Layer 3: termcode-plugin, termcode-term          (deps: all above)
Layer 4: termcode (binary in src/main.rs)        (deps: term)
```

**Critical boundary rules:**

- `termcode-view` is frontend-agnostic: no ratatui, no arboard, no terminal deps. It defines traits (e.g., `ClipboardProvider`), implementations live in `termcode-term`.
- `termcode-lsp` uses primitive types (`&str` URIs, `String` text) in its API, never `Document` or `Editor`. This prevents view<->lsp cycles.
- `termcode-term` owns `LspRegistry`, `ArboardClipboard`, and the tokio async runtime. It bridges async LSP with the synchronous event loop via `mpsc` channels.
- `EditorConfig`, `LineNumberStyle`, and `FileTreeStyle` are defined in `termcode-core` (not config) so `termcode-view` can use them without depending on `termcode-config`.

### Core Patterns

**TEA (The Elm Architecture):** All state changes flow through `Event -> Update -> Render`. The main loop in `App::run()` (app.rs) does: initial render, then loop { drain LSP events via `try_recv`, poll crossterm events, update state, render }. Widgets never mutate state during rendering.

**Command Pattern:** Every user action is a named command (`CommandId = &'static str`) registered in `CommandRegistry`. Commands that need `App`-level access (e.g., `palette.open`, `goto.definition`, `lsp.hover`) are registered with a noop handler and intercepted in `App::handle_key()` before dispatch. All other commands receive `&mut Editor` only.

**Rope-only buffer:** All document text is stored as `ropey::Rope`. Position conversion between line/column (`Position`) and byte offsets (used by `Selection`, `Transaction`) goes through `Buffer::pos_to_byte()` / `byte_to_pos()`.

**Overlay rendering:** Search, fuzzy finder, command palette, completion, and hover are rendered as overlays on top of the editor area (rendered last in `render.rs`). Each has state in `Editor` and a dedicated widget.

### Key Data Flow

```
User Input → EventHandler (crossterm poll) → AppEvent::Key
  → InputMapper.resolve(mode, key) → CommandId
  → CommandRegistry.execute(id, &mut editor) → Editor state mutation
  → render(frame, &editor) → Ratatui widgets read Editor state
```

LSP events flow separately:

```
LspBridge (tokio runtime) → mpsc::UnboundedSender<AppEvent::Lsp>
  → App drains lsp_event_rx via try_recv before crossterm poll
  → Updates diagnostics/completion/hover state in Editor
```

### State Ownership

- `Editor` (termcode-view): owns all documents, views, tabs, file explorer, search/fuzzy/palette state, completion/hover state, theme, config. Single source of truth for the model layer.
- `App` (termcode-term): owns `Editor`, `CommandRegistry`, `InputMapper`, `LspBridge`, `EventHandler`, terminal. Handles the event loop and bridges between layers.
- `Document`: owns `Buffer` (Rope), `Selection`, `History`, syntax highlighter, diagnostics, LSP version.
- `View`: viewport into a document (scroll state, cursor position, area dimensions).

### EditorMode

Six modes: `Normal`, `Insert`, `FileExplorer`, `Search`, `FuzzyFinder`, `CommandPalette`. Mode determines which keybindings are active in `InputMapper`. The `CommandPalette` has sub-modes via `PaletteMode` enum (`Commands`, `Themes`).

### Adding a New Command

1. Write handler: `fn cmd_foo(editor: &mut Editor) -> anyhow::Result<()>` in `command.rs`
2. Register: `registry.register(cmd!("category.foo", "Foo", cmd_foo))` in `register_builtin_commands()`
3. Bind key: add to `InputMapper::default()` in `input.rs` under the appropriate mode
4. If the command needs App-level access (LSP, clipboard, quit), intercept it in `App::handle_key()` like `palette.open`

### Adding a New Widget

1. Create `crates/termcode-term/src/ui/my_widget.rs`
2. Implement `ratatui::widgets::Widget` trait
3. Add module to `ui/mod.rs`
4. Render in `render.rs` (overlays go after the editor view)

### Adding a New Theme

1. Create `runtime/themes/my-theme.toml` following the structure in `one-dark.toml`
2. Sections: `[meta]`, `[palette]`, `[ui]` (20 color slots), `[scopes]` (syntax highlight scopes), `[icons]` + `[icons.extensions]` (optional file type emoji overrides)
3. Theme is automatically discovered by `list_available_themes()` scanning `runtime/themes/`

### Configuration Loading

Config is loaded once at startup in `App::new()`:

1. User config: `~/.config/termcode/config.toml`
2. Project-local config: `config/config.toml` (overrides user config if present)

File tree display is controlled by two flat bools under `[ui]` in config (uses `#[serde(flatten)]` on `FileTreeStyle` struct):

- `tree_style = true|false` — show tree lines (├── └──)
- `show_file_type_emoji = true|false` — show file type emoji icons

File type icons are configured per-theme via `[icons]` section (directory_open, directory_closed, file_default) and `[icons.extensions]` table (extension → emoji). User overrides merge on top of defaults.

## Important Technical Details

- `CommandHandler = fn(&mut Editor) -> anyhow::Result<()>` uses fn pointers, not closures. Commands that need a char argument (insert_char) are handled as special cases in `App`.
- `CommandId = &'static str`. Keybinding overrides validate user strings against `CommandRegistry` at load time to obtain the static reference.
- `Transaction` must be committed to `History` BEFORE applying to `Buffer` (captures original rope state for inverse computation).
- `History::undo()/redo()` return owned `Transaction` (not references) to avoid borrow checker conflicts with `Buffer::apply()`.
- LSP `didChange` must be sent after every document mutation (not just typed chars — also backspace, delete, undo, redo, search-replace).
- `Document.version` must be incremented on every mutation including undo/redo (LSP requires monotonically increasing versions).
- Atomic file save: write to tempfile, then rename. Implemented in `Buffer::save_to_file()`.
- `Ctrl+C` is copy-only (no quit behavior). `Ctrl+Q` is the sole quit command.
- Overlay text inputs track `cursor_pos` as character index, converted to byte index via `char_to_byte_index()` before `String::insert()/remove()`.
- Search `find_matches()` uses case-insensitive literal matching on `&str` (not `&Rope`). Caller converts Rope to String. Matches are non-overlapping. Replace operations apply in reverse byte-offset order.
