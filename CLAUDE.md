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

Seven modes: `Normal`, `Insert`, `FileExplorer`, `Search`, `FuzzyFinder`, `CommandPalette`, `Settings`. Mode determines which keybindings are active in `InputMapper`. The `CommandPalette` has sub-modes via `PaletteMode` enum (`Commands`, `Themes`, `Keymaps`).

Adding a mode touches every exhaustive `match` on it: `InputMapper::mode_table`/`tables`, `mode_section_name`, `ModeBindings` (config), `mode_to_string` (plugin API), and the status bar. The cursor is a filled block in every mode -- `editor_view.rs` does not branch on the mode for it.

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

**Hook events:** `on_open`, `on_save`, `on_close`, `on_mode_change`, `on_cursor_move`, `on_buffer_change`, `on_tab_switch`, `on_ready`. Plugins register hooks via `plugin.on("event_name", handler)`.

**Plugin commands:** Plugins register commands via `plugin.register_command("name", "description", handler)`, which the loader exposes as `plugin.<plugin-name>.<name>` in the command palette. Commands execute through thread-local `EDITOR_PTR` for safe mutable access during execution.

The Lua globals are `plugin`, `editor` and `log` (see `runtime/plugins/example/init.lua`) -- there is no `termcode` table.

**Deferred actions:** Plugins cannot directly mutate app state. Instead, actions like `OpenFile` and `ExecuteCommand` are queued and processed after hook/command execution completes.

### Image Viewer

Images open in tabs alongside text documents via `TabContent::Image(ImageId)`. Frontend-agnostic metadata (`ImageEntry`: path, format, file_size, dimensions) lives in `termcode-view`. Decoded pixel data is cached in `App.image_cache: HashMap<ImageId, Mutex<StatefulProtocol>>` in `termcode-term`, rendered via `ratatui-image`.

### Tab System

`TabManager` manages `Vec<Tab>` where each `Tab` holds a `TabContent` enum (`Document(DocumentId)` or `Image(ImageId)`). Navigation: `Alt+Left`/`Alt+Right`. When an image tab is active, `active_view` is `None` (no cursor/viewport).

### Adding a New Theme

1. Create `runtime/themes/my-theme.toml` following the structure in `one-dark.toml`
2. Sections: `[meta]`, `[palette]`, `[ui]` (20 color slots), `[scopes]` (syntax highlight scopes), `[icons]` + `[icons.extensions]` (optional file type emoji overrides, plus the explorer toolbar's `new_file`/`new_folder`/`refresh`/`copy_path` glyphs)
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

The default preset is `vscode` (`KeymapConfig::default`), so an editor with no
config file is modeless. An **empty** `preset = ""` is what selects the built-in
hybrid keymap: omitting the key means the default, so the settings screen writes
the empty string there rather than removing the key.

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
4. `[mode.settings]` is optional: a preset that omits it entirely inherits the
   built-in settings keys, since that mode consults no global table and would
   otherwise have none. Declaring even one binding there takes over the whole
   section, as with every other mode -- so a partial section is the one way to
   end up with a half-usable screen (`Esc` still closes it regardless;
   `handle_settings_key` falls back to that when nothing is bound)
5. Add the name to `PRESETS` in `crates/termcode-term/tests/keymap_presets.rs`
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

### Settings Screen

`EditorMode::Settings` (`F2`, the top bar button, or `settings.open` from the
palette -- the button goes through `MouseAction::OpenSettings`, since building
the rows needs `App`) is a two-pane
screen: a fixed list of categories (Appearance, Editor, Keybindings, Plugins)
and the rows of the selected one. It resolves keys like the other overlays --
mode table only, no global fallback -- so a shortcut cannot navigate away from
a screen the user is in the middle of.

`SettingsState` (termcode-view) holds only the cursor, the rows, and the capture
flag. It never builds the rows: the available themes, keymaps, plugins and
bindings are known to `App`, not to `termcode-view`, so `App::reload_settings_items`
fills them the same way the command palette is fed its command list. Everything
about a row that the frontend needs -- the value, where it is written, whether it
needs a restart -- travels in `SettingItem`.

The screen is three levels deep and the arrows move between them, never editing:
`settings.focus_in` (Right) steps categories → settings, `settings.focus_out`
(Left) steps back out and also closes an open value list. `settings.activate`
(Enter **and** Space) is the only thing that changes anything.

`SettingValue::Choice` and `SettingValue::Int` are therefore **not** stepped in
place. Activating one opens a `SettingsPicker` -- a list floating over the
screen, driven by the same `[mode.settings]` bindings -- and only
`picker_commit` writes back to the row. Stepping in place applied every value on
the way to the wanted one, so walking to `vim` ran the editor under `helix` en
route and could take away the keys being used to walk. A number's list is built
from its range by `step`, and `set_choice` turns the chosen position back into
the number. While the picker is open `run_picker_command` swallows everything
else, so no command can act on the screen behind it.

`SettingItem::live_preview` opts a row into applying each option as the
highlight passes over it (`SettingsAction::Preview`), reverting on cancel
(`PreviewReverted`); neither saves. Only the theme sets it -- a colour scheme
cannot take the keyboard away, and a keymap can.

Changing a row does two things, in `App::apply_and_save_setting`:

1. `apply_config_value` matches the row's dotted path and updates the running
   editor. Settings the editor only reads at startup (`editor.mouse_enabled`,
   anything under `plugins`) fall through it and are flagged `restart_required`.
2. `persist_config_value` writes it to the config file **that was actually
   loaded** -- `App.config_path`, which `App::new` points at the project-local
   `config/config.toml` when that is what it read.

Writes go through `termcode_config::writer`, which edits the TOML document with
`toml_edit` and replaces only the key that changed. Re-serialising `AppConfig`
would drop the user's comments, reorder their keys, and materialise defaults
they never wrote. `set_value` replaces an existing value _in place_ rather than
re-inserting the key, because a comment above a setting is attached to the key.

Keybinding rows are captured, not typed: `Enter` starts a capture, each key is
appended, `Enter` commits (so `g g` is possible), and `Esc` cancels -- which is
why `Esc` itself cannot be bound from this screen. The captured keys are
serialised by `key_sequence_to_config`, **not** by the status-bar formatter:
that one prints `PgUp`/`Shift+Tab`, neither of which `parse_key_combo` reads
back as the same key. Anything written here round-trips through the parser
before it is stored, so what is in memory is what the file will produce.

A rebinding is written into the section the command is already bound in
(`InputMapper::binding_scope`), defaulting to `[global]`. Moving `search.next`
into `[global]` would leave it unreachable, since the overlay modes never
consult that table. `keybindings.toml` is indexed by key, so a new binding
**adds** to the command rather than replacing it; the row shows every binding
(`InputMapper::bindings_for`) instead of the first, and `InputMapper::conflicts`
reports sequences the new one collides with or shadows.

`config.toml` keys that nothing reads (`ui.show_tab_bar`, `ui.show_top_bar`,
`ui.show_minimap`, `editor.word_wrap`, `editor.auto_save*`) are deliberately not
listed on this screen: a settings row that does nothing when toggled is worse
than no row.

### File Explorer

Tree-based with lazy expansion. Uses `ignore::WalkBuilder` for `.gitignore`-aware traversal. `FileNode` tracks kind (File/Directory/Symlink), depth, and expanded state. `toggle_expand()` inserts/removes children inline; `refresh()` preserves existing expansion state.

The sidebar's first row is the explorer toolbar (`ui/explorer_toolbar.rs`): the
project name plus the New File / New Folder / Refresh / Copy Path buttons. Under
`pane_focus_style = "title_bar"` it takes over the pane title row -- it carries
the same focus styling, so nothing is lost -- and under the other styles it
takes the tree's first line. Buttons are dropped from the left as the sidebar
narrows, and `explorer_toolbar::buttons()` is the single source of their
positions, shared by the widget and `mouse.rs`.

The button glyphs are the theme's `[icons]` `new_file` / `new_folder` /
`refresh` / `copy_path` (default 📄 📁 🔄 📋). They are emoji rather than the
more obvious symbols (⚙ ⟳ ⎘) because emoji are East Asian **Wide** -- exactly
two columns in every terminal -- while those symbols are Ambiguous and would
shift the whole header. `ToolbarLabels::resolve()` falls back to ASCII (`+F`,
`+D`, `R`, `C`) when `ui.show_file_type_emoji` is off: a terminal that cannot
draw the tree's icons cannot draw these either. Widths come from
`unicode_width`, never `str::len`, and the widget blanks the cell a wide glyph
covers (ratatui's diff skips it, so it is never emitted).

Creating an entry is inline rather than a mode of its own: `FileExplorer.new_entry`
holds a `NewEntryInput` (kind, parent directory, tree row, depth, name, cursor)
and the tree draws it as an extra row where the entry will land. While it is
open, `App::handle_key` routes every key to `handle_new_entry_key` before the
keymap sees it, so a name may contain letters the explorer binds (`n`, `y`,
...). `Enter` commits, `Esc` cancels, and leaving the explorer (`switch_mode`,
`switch_to_default_mode`, a click in the tree) drops the half-typed name --
nothing else feeds that row. `commit_new_entry()` rejects an empty name, one
that escapes the parent (`..`, an absolute path) and one that already exists,
keeping the row open so the name can be corrected. A new file is opened in a tab
straight away; a new directory leaves focus in the tree.

The tree's first node is a `..` row (`FileNode::is_parent`), unless the root is
the filesystem root and has no parent. It is a real node so selection,
scrolling and mouse hit-testing stay plain row indices, and `is_parent` keeps it
out of everything that would treat it as the directory it points at:
`toggle_expand` and `refresh_node` ignore it, `new_entry_target` creates in the
current root instead, and `selected_path()` returns `None` there so
`explorer.copy_path` has nothing to copy. Enter (and Right) on it calls
`navigate_to_parent()`, which re-roots the tree one level up and selects the
directory it came out of. It absolutises the root first (`std::path::absolute`,
not `canonicalize`, so symlinks stay unresolved): the editor is usually opened
on `.`, and `Path::new(".").parent()` is the empty path, which lists nothing.

Enter and the arrow keys do different things to a directory: Right expands it in
place, keeping it inside the tree it belongs to, while Enter *enters* it --
`navigate_into()` makes it the root. The mouse splits the row the same way: a
single click on the `▶`/`▼` chevron expands (`MouseAction::ToggleExplorerExpand`),
a double click anywhere else re-roots. `ui::file_explorer::chevron_span()` is the
single source of the chevron's columns, shared by the widget and `mouse.rs`, and
returns `None` where there is nothing to click -- a file, the `..` row, or
`show_file_type_emoji = false`. Hit-testing is in *logical* columns
(`x - sidebar.x + scroll_left`), since the tree scrolls horizontally. Both re-rootings go through `set_root()`,
which drops the old tree wholesale (its expansion state belongs to paths at a
different depth) and lands the selection on the first real entry rather than on
the `..` row, so Enter does not bounce straight back out. Re-rooting moves what
the fuzzy finder and the toolbar's project name follow; it does not touch the
LSP root, nor the session, which is keyed on `App.session_root` captured at
startup so walking the root around cannot move where it is written.

The seam between the sidebar and the editor is a drag handle.
`AppLayout::sidebar_divider` is the single source of its columns -- always the
sidebar's **last** column, whatever is drawn there (the panel border, the focus
border, or plain tree padding), so one rule covers every `pane_focus_style` and
`panel_borders` combination. `mouse.rs` tests it before the regions that also
contain that column, which costs the last column of the tree its click-to-select
under the styles that draw no border there.

A `Drag` event says nothing about where the drag began, so the press is
remembered in `FileExplorer.resizing` as the width at that moment. Keeping the
*width* rather than a bool is what separates a press that resized from one that
never moved: only the former returns `MouseAction::SidebarResized`, and only
that writes `ui.sidebar_width` to the config file. A press always clears the
field first, so an `Up` lost outside the terminal cannot leave the divider stuck
to the pointer. The bounds live in `layout.rs` (`clamp_sidebar_width`) and are
shared with the settings screen's `Sidebar Width` row, so neither path can
produce a width the other rejects.

`explorer.copy_path` puts the selected entry's absolute path on the system
clipboard. The path is joined with the working directory rather than
canonicalised, so symlinks stay unresolved and Windows' verbatim `\\?\` prefix
never appears.

### Confirm Dialog

The unsaved-changes dialog (`ConfirmAction::CloseTab` / `QuitAll`) is modal for
the mouse as well as the keyboard: `mouse::handle_mouse` answers clicks on its
buttons and swallows everything else, so nothing behind it can be clicked or
scrolled while it is up. A button is clicked twice: the first click moves the
focus to it, the second runs it (`MouseAction::ConfirmSelected` ->
`execute_confirm_action`) -- the same select-then-act as the tree, because
discarding unsaved work should not be one misplaced click away. A click that
misses does nothing, since dismissing is `Esc`'s job and a stray click must not
discard the choice.

`ui::confirm_dialog::layout()` is the single source of the dialog's geometry --
the popup rect, the button row, and each button's columns -- shared by the
widget and `mouse.rs`. It centres itself in the whole frame, which is why
`AppLayout` carries `frame`. It returns `None` for an area too small to draw in,
and the widget then draws nothing, so there is never a button to click that was
not rendered.

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
