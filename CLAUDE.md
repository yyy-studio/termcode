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

**Command Pattern:** Every user action is a named command (`CommandId = &'static str`) registered in `CommandRegistry`. Commands that need `App`-level access (e.g., `palette.open`, `goto.definition`, `lsp.hover`, `tab.close`, `app.quit`, `explorer.*`) are registered with a noop handler and intercepted in `App::dispatch_command()` / `App::handle_key()` before dispatch. All other commands receive `&mut Editor` only. Registration is mandatory even for app-level commands: `InputMapper` validates every binding against the registry and silently drops unknown ids. `register_hidden()` keeps a command bindable but out of the command palette (`explorer.*`, `fuzzy.up/down`, `palette.up/down`).

**Rope-only buffer:** All document text is stored as `ropey::Rope`. Position conversion between line/column (`Position`) and byte offsets (used by `Selection`, `Transaction`) goes through `Buffer::pos_to_byte()` / `byte_to_pos()`.

**Overlay rendering:** Search, fuzzy finder, command palette, completion, and hover are rendered as overlays on top of the editor area (rendered last in `render.rs`). Each has state in `Editor` and a dedicated widget.

### Key Data Flow

```
User Input → EventHandler (crossterm poll) → AppEvent::Key
  → App::feed_key → InputMapper.resolve_key(mode, key)
      → KeyResolution::{Match(CommandId) | Pending | NoMatch}
  → CommandRegistry.execute(id, &mut editor) → Editor state mutation
  → render(frame, &editor, &image_cache, &input_mapper)
```

`Pending` means the key is a prefix of a longer binding; it is consumed and
`App.chord_started` restarts a timer that `AppEvent::Tick` expires, making
`chord_timeout_ms` an inter-key gap rather than a whole-sequence budget.

Two ways to end a chord early: `abandon_pending_chord()` drops the keys (mouse
input, confirm dialog, `Ctrl+Q` — contexts where typing them would be wrong),
`recover_pending_chord()` enters them as text where they were typed (timeout,
completion popup stealing a key).

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

1. Write handler: `fn cmd_foo(editor: &mut Editor) -> anyhow::Result<()>` in `command.rs` — word motions live in `command/motion.rs`, line-oriented editing in `command/line_edit.rs`
2. Register: `registry.register(CommandEntry { id: "category.foo", name: "Foo", handler: cmd_foo })` in `register_builtin_commands()`, or `register_hidden()` to keep it bindable but out of the command palette
3. Bind key: add to `InputMapper::new()` in `input.rs` under the appropriate mode, plus any preset in `runtime/keymaps/` that should carry it
4. If the command needs App-level access (LSP, clipboard, quit), register it with `cmd_noop` in `register_app_level_commands()` and intercept it in `App::dispatch_command()` like `palette.open`
5. If it mutates the buffer, add its id to `is_document_mutation()` in `app.rs` so LSP `didChange` and the `OnBufferChange` hook still fire

### Adding a New Widget

1. Create `crates/termcode-term/src/ui/my_widget.rs`
2. Implement `ratatui::widgets::Widget` trait
3. Add module to `ui/mod.rs`
4. Render in `render.rs` (overlays go after the editor view)

### Plugin System

Plugins are sandboxed Lua scripts discovered from `~/.config/termcode/plugins/` and `runtime/plugins/`. Each plugin has a `plugin.toml` manifest and an `init.lua` entry point.

**Sandbox restrictions:** Restricted Lua stdlib (base, string, table, math, utf8, safe os subset). Instruction and memory limits prevent resource exhaustion. Per-plugin `require()` is scoped to the plugin directory (no path traversal).

**Hook events:** `on_open`, `on_save`, `on_close`, `on_mode_change`, `on_cursor_move`, `on_buffer_change`, `on_tab_switch`, `on_ready`. Plugins register hooks via `termcode.on("event_name", handler)`.

**Plugin commands:** Plugins register commands via `termcode.register_command("name", handler)` which become available in the command palette. Commands execute through thread-local `EDITOR_PTR` for safe mutable access during execution.

**Deferred actions:** Plugins cannot directly mutate app state. Instead, actions like `OpenFile` and `ExecuteCommand` are queued and processed after hook/command execution completes.

### Image Viewer

Images open in tabs alongside text documents via `TabContent::Image(ImageId)`. Frontend-agnostic metadata (`ImageEntry`: path, format, file_size, dimensions) lives in `termcode-view`. Decoded pixel data is cached in `App.image_cache: HashMap<ImageId, Mutex<StatefulProtocol>>` in `termcode-term`, rendered via `ratatui-image`.

### Tab System

`TabManager` manages `Vec<Tab>` where each `Tab` holds a `TabContent` enum (`Document(DocumentId)` or `Image(ImageId)`). Navigation: `Alt+Left`/`Alt+Right`. When an image tab is active, `active_view` is `None` (no cursor/viewport).

### Adding a New Theme

1. Create `runtime/themes/my-theme.toml` following the structure in `one-dark.toml`
2. Sections: `[meta]`, `[palette]`, `[ui]` (20 color slots), `[scopes]` (syntax highlight scopes), `[icons]` + `[icons.extensions]` (optional file type emoji overrides)
3. Theme is automatically discovered by `list_available_themes()` scanning `runtime/themes/`

### Keymap Presets

`InputMapper` stores bindings as `(Vec<KeyEvent>, CommandId)`, so multi-key
chords (`gg`, `space f`, `ctrl+k ctrl+p`) are first-class. Resolution rules:

- Mode table is searched **before** the global table, so a preset can reclaim a
  global shortcut inside one mode (Vim's Insert-mode `Ctrl+W`).
- Global bindings are **not** consulted in the overlay modes (Search, fuzzy
  finder, command palette); those keys belong to the overlay's text input.
- Each table is settled completely (exact match, then prefix) before the next is
  consulted, so a mode chord is not pre-empted by a same-prefix global binding.
- An exact match fires immediately, so a binding that is also the prefix of a
  longer one makes the longer one unreachable _within its table_.
  `tests/keymap_presets.rs` asserts no shipped preset does this, checking each
  mode table both alone and unioned with `[global]`.
- A sequence that turns out to be unbound is discarded, not retried key-by-key
  — except in Insert mode, where the buffered keys are typed as text so a chord
  like `j k` cannot swallow a literal `j`. If the key that ended the chord
  carries no text (`Esc`, `Enter`, ...) it is then re-resolved on its own, since
  the mapper only ever saw it as the tail of a dead sequence. The overlay
  handlers and the chord-timeout path recover their keys the same way.
- The pending chord lives only in `InputMapper`; `render()` reads it via
  `pending_display()`. Do not mirror it into `Editor` — every key path that
  bypasses the mapper (mouse, confirm dialog) would desynchronise it.

`Ctrl+Q` and the file-explorer/overlay navigation commands are intercepted in
`App` rather than run from the registry, so the help popup carries an
`ALWAYS_AVAILABLE` fallback for `app.quit`.

Keymap presets are searched via `keymap_dirs()`, which puts
`~/.config/termcode/keymaps/` **before** the runtime directories so a user file
can replace a shipped preset by name. A file that fails to parse is logged and
skipped rather than masking a working one further down.

`config.toml`'s `[keymap] preset = "<name>"` loads `runtime/keymaps/<name>.toml`
and **replaces** the built-in map entirely; `keybindings.toml` overrides then
apply on top. Both paths go through `build_input_mapper()` in `app.rs`, which is
called twice: once at startup and again after plugin commands register, so a
keymap can bind them.

`Ctrl+Q` stays hardcoded in `handle_key()` as an escape hatch that no keymap can
remove.

### Resting Mode (`Editor::default_mode`)

The editor boots in `EditorMode::Normal`, which makes a keymap with no modal
layer unusable: nothing would be bound. A preset declares `[meta] initial_mode =
"insert"` (only `vscode` does) and `App::with_config` sets `editor.default_mode`
from it.

`Editor::switch_to_default_mode()` is what every "we're done, go back" path
calls -- `cmd_mode_normal`, overlay close, opening a file from the finder or
explorer, clicking into the editor. Under a modal keymap `default_mode` is
`Normal`, so those paths behave exactly as before; under `vscode` they return to
Insert. Insert is downgraded to Normal when `active_view` is `None`, since there
is nothing to type into.

The resting mode is settled at the top of `App::run()`, not in `with_config()`:
callers open startup files between the two, and Insert needs a view to exist.

### Adding a New Keymap Preset

1. Create `runtime/keymaps/my-keymap.toml` with `[meta]`, `[global]`, and
   `[mode.*]` sections (see `vim.toml`)
2. Bind every command the preset needs -- there is no inheritance from defaults
3. Set `[meta] initial_mode = "insert"` if the keymap has no modal layer
4. Add the name to `PRESETS` in `crates/termcode-term/tests/keymap_presets.rs`
   so it is checked for unparsable keys, unknown commands, shadowed chords, and
   unknown sections

Serde ignores what it does not recognise, so a misspelled `[mode.nromal]` would
bind nothing at all. `KeymapPreset::warnings()` (and `KeybindingConfig::warnings()`
for `keybindings.toml`) reports those sections plus an unusable
`meta.initial_mode`; `App` shows them in the status bar at startup and on a live
keymap switch.

### Configuration Loading

Config is loaded once at startup in `App::new()`:

1. User config: `~/.config/termcode/config.toml`
2. Project-local config: `config/config.toml` (overrides user config if present)

File tree display is controlled by two flat bools under `[ui]` in config (uses `#[serde(flatten)]` on `FileTreeStyle` struct):

- `tree_style = true|false` — show tree lines (├── └──)
- `show_file_type_emoji = true|false` — show file type emoji icons

File type icons are configured per-theme via `[icons]` section (directory_open, directory_closed, file_default) and `[icons.extensions]` table (extension → emoji). User overrides merge on top of defaults.

### File Explorer

Tree-based with lazy expansion. Uses `ignore::WalkBuilder` for `.gitignore`-aware traversal. `FileNode` tracks kind (File/Directory/Symlink), depth, and expanded state. `toggle_expand()` inserts/removes children inline; `refresh()` preserves existing expansion state.

### CI & Release

- **CI** (`.github/workflows/ci.yml`): fmt check → clippy → test (ubuntu/macos/windows matrix) → build
- **Release** (`.github/workflows/release.yml`): Tag-triggered multi-platform build (aarch64/x86_64 darwin/linux + windows). Cross-compilation via `cross`. Archives include `runtime/` directory. Creates GitHub Release with artifacts.

### Runtime Directory

```
runtime/
  themes/      # Built-in themes (one-dark, gruvbox-dark, catppuccin-mocha, lazygit)
  keymaps/     # Keymap presets (vscode, vim, helix)
  plugins/     # Example plugins (each has plugin.toml + init.lua)
  queries/     # Tree-sitter highlight queries per language
```

User overrides: `~/.config/termcode/themes/`, `~/.config/termcode/keymaps/`, and `~/.config/termcode/plugins/` are also scanned.

## Important Technical Details

- `CommandHandler = fn(&mut Editor) -> anyhow::Result<()>` uses fn pointers, not closures. Commands that need a char argument (insert_char) are handled as special cases in `App`.
- `CommandId = &'static str`. Keybinding overrides validate user strings against `CommandRegistry` at load time to obtain the static reference.
- `Transaction` must be committed to `History` BEFORE applying to `Buffer` (captures original rope state for inverse computation).
- `History::undo()/redo()` return owned `Transaction` (not references) to avoid borrow checker conflicts with `Buffer::apply()`.
- LSP `didChange` must be sent after every document mutation (not just typed chars — also backspace, delete, undo, redo, search-replace).
- `Document.version` must be incremented on every mutation including undo/redo (LSP requires monotonically increasing versions).
- Atomic file save: write to tempfile, then rename. Implemented in `Buffer::save_to_file()`.
- `Ctrl+C` is copy-only (no quit behavior). It is a `global` binding, so like
  every global it does not apply in the overlay modes (search, fuzzy finder,
  command palette), where a keymap must bind it per-mode to reach it.
  `Ctrl+Q` is intercepted before the keymap and is the sole guaranteed quit.
- Overlay text inputs track `cursor_pos` as character index, converted to byte index via `char_to_byte_index()` before `String::insert()/remove()`.
- Search `find_matches()` uses case-insensitive literal matching on `&str` (not `&Rope`). Caller converts Rope to String. Matches are non-overlapping. Replace operations apply in reverse byte-offset order.
