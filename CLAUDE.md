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

**TEA (The Elm Architecture):** All state changes flow through `Event -> Update -> Render`. The main loop in `App::run()` (app.rs) does: an initial `settle_and_draw`, then loop { drain LSP events via `try_recv`, poll crossterm events, update state, `settle_and_draw` }. `settle_and_draw` is the whole window between the last event and the next frame -- see `### Settling the Frame`. Widgets never mutate state during rendering.

**Command Pattern:** Every user action is a named command (`CommandId = &'static str`) registered in `CommandRegistry`. Commands that need `App`-level access (e.g., `palette.open`, `goto.definition`, `lsp.hover`, `tab.close`, `app.quit`, `explorer.*`) are registered with a noop handler and intercepted in `App::dispatch_command()` / `App::handle_key()` before dispatch. All other commands receive `&mut Editor` only. Registration is mandatory even for app-level commands: `InputMapper` validates every binding against the registry and silently drops unknown ids. `register_hidden()` keeps a command bindable but out of the command palette (`explorer.*`, `fuzzy.up/down`, `palette.up/down`).

**Rope-only buffer:** All document text is stored as `ropey::Rope`. Position conversion between line/column (`Position`) and byte offsets (used by `Selection`, `Transaction`) goes through `Buffer::pos_to_byte()` / `byte_to_pos()`.

**Overlay rendering:** Search, fuzzy finder, command palette, completion, and hover are rendered as overlays on top of the editor area (rendered last in `render.rs`). Each has state in `Editor` and a dedicated widget.

Every popup casts a shadow: `overlay::render_shadow` dims the band its
rectangle would fall on, offset two columns right and one row down (two to one,
because a cell is twice as tall as it is wide). It _dims_ rather than fills, so
the text behind stays readable, and it is called from `render_overlay_frame` --
a widget that draws its own frame (`confirm_dialog`, `help_popup`,
`completion`, `hover`) calls it directly. `Indexed` and `Reset` colours have no
channels to scale and take the theme's shaded background instead.

An overlay that owns a mode owns the wheel with it (`mouse::handle_wheel`):
it moves the overlay's own list where there is one and is swallowed where
there is not, never reaching the buffer behind. Text sliding around under a
popup reads as the input having gone through to the editor, which is exactly
what has not happened. Where the pointer is does not come into it -- a wheel
that fell through whenever the pointer sat outside the popup would be the leak
this closes. The settings screen returns `MouseAction::ScrollSettings` instead
of moving anything itself, so the category pane, the value picker and live
preview all go down the same path the arrow keys use.

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

Events produced off the event loop's thread flow separately, through one
channel drained before crossterm is polled:

```
LspBridge (tokio runtime) ─┐
update::spawn_check (std thread) ─┴→ mpsc::UnboundedSender<AppEvent>
  → App drains async_event_rx via try_recv before crossterm poll
  → Updates diagnostics/completion/hover state, or App.update_status
```

`App.async_event_tx` is the sending half, kept on `App` so a check can be
started at any time; `LspBridge` is handed its own clone and may not exist at
all.

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

**Hook events:** `on_open`, `on_before_save`, `on_save`, `on_close`, `on_mode_change`, `on_cursor_move`, `on_buffer_change`, `on_tab_switch`, `on_ready`. Plugins register hooks via `plugin.on("event_name", handler)`.

**Plugin commands:** Plugins register commands via `plugin.register_command("name", "description", handler)`, which the loader exposes as `plugin.<plugin-name>.<name>` in the command palette. Commands execute through thread-local `EDITOR_PTR` for safe mutable access during execution.

The Lua globals are `plugin`, `editor` and `log` (see `runtime/plugins/example/init.lua`) -- there is no `termcode` table.

`on_before_save` fires _before_ the write, so a handler that rewrites the
buffer has its edit land in the saved file -- `on_save` fires after and cannot.
A handler may itself queue `file.save`; `App.in_before_save` is what stops the
two calling each other forever. The confirm dialog's Save+Close and Save-all
name the document they are saving, and the whole plugin API reads and writes
the _active_ one, so those saves fire the hook only when the two coincide
(`dispatch_before_save_hook_for`).

**Deferred actions:** Plugins cannot directly mutate app state. Instead, actions like `OpenFile` and `ExecuteCommand` are queued and processed after hook/command execution completes.

### Image Viewer

Images open in tabs alongside text documents via `TabContent::Image(ImageId)`. Frontend-agnostic metadata (`ImageEntry`: path, format, file_size, dimensions) lives in `termcode-view`. Decoded pixel data is cached in `App.image_cache: HashMap<ImageId, Mutex<StatefulProtocol>>` in `termcode-term`, rendered via `ratatui-image`.

### Tab System

`TabManager` manages `Vec<Tab>` where each `Tab` holds a `TabContent` enum (`Document(DocumentId)` or `Image(ImageId)`). Navigation: `Alt+Left`/`Alt+Right`. When an image tab is active, `active_view` is `None` (no cursor/viewport).

### Adding a New Theme

1. Create `runtime/themes/my-theme.toml` following the structure in `one-dark.toml`
2. Sections: `[meta]`, `[palette]`, `[ui]` (21 color slots), `[scopes]` (syntax highlight scopes), `[icons]` + `[icons.extensions]` (optional file type emoji overrides, plus the explorer toolbar's `new_file`/`new_folder`/`refresh`/`copy_path` glyphs)
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
the rows needs `App`) is a two-pane popup: a fixed list of categories
(Appearance, Editor, Keybindings, Plugins, Update) and the rows of the selected
one. It resolves keys like the other overlays -- mode table only, no global fallback --
so a shortcut cannot navigate away from a screen the user is in the middle of.

The popup floats over the **whole frame**, not over the editor area: what it
edits belongs to the sidebar as much as to the editor. `ui::settings::popup_area`
is the single source of its geometry -- a share of the frame, capped, centred,
and `None` where the terminal is too small for the two panes, in which case
nothing is drawn. `App` sizes `settings.visible_height` and the value picker's
rows from that same function rather than from the frame, since paging by a
screenful only works if both agree on how tall the screen is.

`SettingsState` (termcode-view) holds only the cursor, the rows, and the capture
flag. It never builds the rows: the available themes, keymaps, plugins and
bindings are known to `App`, not to `termcode-view`, so `App::reload_settings_items`
fills them the same way the command palette is fed its command list. Everything
about a row that the frontend needs -- the value, where it is written, whether it
needs a restart -- travels in `SettingItem`.

The screen is modal for the mouse as well as the keyboard. Every region behind
the popup switches the mode when it is clicked -- the tree, the tab bar, the
editor area, the top bar's own buttons -- so a click that missed used to close a
screen the user was in the middle of. `mouse::handle_mouse` now answers clicks
on the screen itself and swallows everything else, exactly as it does for the
confirm dialog; dismissing is `Esc`'s job. The wheel still goes through
`handle_wheel`, which already owns the rule that a popup keeps its notches.

`ui::settings::layout` is the single source of the screen's geometry -- the
popup, the two panes, the divider column and the value list -- shared by the
widget and `mouse.rs`, as `confirm_dialog::layout` and
`explorer_toolbar::buttons` are. It carries `first_category` and `first_item`
with the rects, since both panes scroll and a row number alone does not say
which entry it is.

`mouse.rs` decides *which row the pointer is on* and nothing else
(`MouseAction::SettingsCategory` / `SettingsItem` / `SettingsOption`).
Rebuilding the category's rows, saving a value and previewing a theme are all
`App`'s, and `click_settings_*` go back through `move_selection`,
`picker_move` and `run_settings_command` rather than writing the state
directly: deciding it twice is how the mouse and the keyboard drift apart.

A click on an item selects it and a second click on the row already selected
runs it -- the same select-then-act the tree and the confirm dialog use, so
changing a keymap or starting an install is not one misplaced click away, and a
double click (two presses) activates. The value list owns the screen while it is
open, for the mouse as `run_picker_command` does for the keyboard.

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
   `editor.tab_size` and `editor.line_numbers` then call
   `command::ensure_h_scroll`: the first moves every display column on the
   cursor's line, the second changes the gutter and so the width of the code
   area, and a `left_col` computed against the old ones can leave the cursor off
   the side of the screen. That call is the shared one, not a second copy of the
   arithmetic -- and it belongs here rather than in `sync_viewport_metrics`,
   which runs every frame and would drag `left_col` back to the cursor in the
   middle of a scrollbar drag.
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

`SettingValue::Action` is the one row that is neither read from nor written to
a config file: it is a button, and activating it returns `SettingsAction::Invoke`
carrying the row index rather than `Changed`. The row's `id` -- not its label --
is what `App::run_update_action` dispatches on, so renaming a button cannot
change what it does. `persist_config_value` refuses one outright: there is
nothing to store.

`config.toml` keys that nothing reads (`ui.show_tab_bar`, `ui.show_top_bar`,
`ui.show_minimap`, `editor.word_wrap`, `editor.auto_save*`) are deliberately not
listed on this screen: a settings row that does nothing when toggled is worse
than no row.

### Update Check

`update.rs` asks the GitHub releases API whether a newer tag exists, on a
detached `std::thread`, and posts the answer back as `AppEvent::Update`. It is
started from `App::run()` and **not** from `with_config`, so constructing an
`App` -- which every test does -- never opens a socket.

The answer is cached in `~/.config/termcode/update.json` for a day
(`UpdateCache::is_fresh`); a start within that window makes no request and still
knows what it knew. `Check Now` passes `force = true`, which is what that button
means. A clock wound backwards reads as "just checked" rather than "fresh
forever", which is the harmless direction: being a day late with a version
number costs nothing.

`Version` compares numerically because string comparison puts `0.10.0` before
`0.9.0`, and a pre-release sorts before the release it leads to. A tag it cannot
parse becomes `UpdateStatus::Failed` rather than a version of zero, which would
claim every release is newer.

`ReleaseSource` is a trait so the decision -- cache, request, comparison -- can
be tested without a network; CI has none, and the button's own `spawn_check` is
deliberately left out of the tests rather than given a seam that only tests use.

Installing is handed straight back to `install.sh`. That script already knows
where the binary and the `runtime/` directory go, that macOS quarantines a
downloaded executable, and how to leave an existing `config.toml` alone; a
second implementation inside the editor would have to learn all of it again and
would be the copy that goes stale. What lives here is the decision of *whether*
to run it.

`install_readiness()` refuses unless the running executable is
`~/.local/bin/termcode` -- `default::installed_binary_path`, which sits beside
`config_dir` because it is the same knowledge: what layout the installer
creates. A binary from `cargo install`, a package manager or `target/` belongs
to whoever put it there. Windows is refused outright -- there is no `install.ps1`
-- and so is a missing `sh` or `curl`. Where it refuses, the row is an `Info`
saying why rather than a button: a button that always refuses is worse than no
button.

The install itself runs from `App::run()` **after** `restore_terminal`, gated on
`App.pending_install`, because a shell script needs the terminal out of raw mode
and out of the alternate screen. The editor is not restarted afterwards: the
process still running is the old one, and re-executing it would run the old code
while claiming to be the update.

Two presses arm it (`App.install_armed`, holding the row index), and any other
settings command takes the arming back. Unsaved work is **refused** rather than
routed through the quit dialog: that dialog can be cancelled, which would leave
an install armed for whenever the editor next quit -- an action the user had
backed out of, happening later, for a different reason.

`App.release_source` and `App.install_check` are function-pointer seams. Both
answers depend on the machine the editor is running on -- a network, and where
this binary sits -- which a test out of `target/` can never satisfy, and the
logic behind the Install button quits the editor.

The category pane derives where it starts from the selection and its own height
(`ui::settings::category_scroll`) rather than keeping a scroll offset in the
state: the list is fixed and five entries long, so there is nothing to keep in
step. Without it a short terminal draws the first few categories and hides the
highlight, which is a screen with no way to tell where you are.

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
place, keeping it inside the tree it belongs to, while Enter _enters_ it --
`navigate_into()` makes it the root. The mouse splits the row the same way: a
single click on the `▶`/`▼` chevron expands (`MouseAction::ToggleExplorerExpand`),
a double click anywhere else re-roots. `ui::file_explorer::chevron_span()` is the
single source of the chevron's columns, shared by the widget and `mouse.rs`, and
returns `None` where there is nothing to click -- a file, the `..` row, or
`show_file_type_emoji = false`. Hit-testing is in _logical_ columns
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
_width_ rather than a bool is what separates a press that resized from one that
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

### Tab Width

`display_width::TabStops` is the single source of what a `\t` is worth. A tab's
width is a function of the **column it starts at** -- it advances to the next
multiple of `tab_size` -- so there is deliberately no `fn width(ch)` for buffer
text: a per-character width function cannot answer for a tab, and the answer the
old one gave (0, from `UnicodeWidthChar::width`) is exactly what let the renderer
draw a 4-column indent that every measurement path read as 0. `next_col(col, ch)`
is the only place the arithmetic exists; `col_at_char`, `char_at_col`,
`width_capped_chars` and the widget's own drawing loops are all folds over it, so
a tab, a CJK glyph and a combining mark cannot disagree. A new mapping (a
minimap, word wrap, a whitespace glyph) folds `next_col` too -- it does not write
`(col / n + 1) * n` a second time.

`tab_size` reaches the widget as `&EditorConfig` (`EditorViewWidget::new`'s 6th
parameter, carrying `line_numbers` with it so the constructor stays at 7
arguments) and everything else as `TabStops::from_config(&editor.config)`, built
on the spot from the `&Editor` the caller already holds. There is **no `Default`
impl**, on purpose: `TabStops::default()` would be a silent `tab_size` of 4, the
constant this type removed. `TabStops::new` clamps to `1..=64`, both ends in the
one place: a hand-edited `config.toml` can say `tab_size = 0` and this is the
only code that divides by it, and it can equally say a number near `usize::MAX`,
on which `(col / size + 1) * size` overflows.

The tab-unaware side is the `ui_*` free functions (`ui_char_width`,
`ui_str_width`, `ui_col_at_char`), for **UI strings** only -- a status bar
segment, a tab label, a hover line, a settings row, a dialog button, a one-line
query. None of those can contain a literal tab. The naming is the separation:
`TabStops` is a type you must construct with a `tab_size`, `ui_*` plainly says
what it is for, and a call site cannot pick the wrong half by accident.

Tab stops are counted from **column 0 of the line**, never from `left_col`.
Counting them from the scroll position would make scrolling horizontally move
where the stops fall, which is why `ui::scrollbar::content_width` still does not
depend on `left_col` even though it now takes a `TabStops`.

`ui::editor_view::visible_span` is the one clipping rule the main render loop,
the search highlight and the selection highlight share: a tab is clipped **per
column**, so one straddling either edge of the viewport draws the columns that
fit, while every other character is all-or-nothing -- half a CJK glyph is not a
glyph. The cursor sits at its character's **first** column (a tab's first
column, where the renderer starts painting it), which is what makes the widget's
REVERSED cell and `render::cursor_screen_position` name the same cell and keeps
the click round trip idempotent. That holds at any `left_col` because both sides
answer _nothing_ outside the viewport: the widget draws no block, and
`cursor_screen_position` returns `None` rather than the nearest edge cell --
it is the completion and hover popups' anchor, and an anchor clamped onto a cell
the cursor is not in points the popup at a place the user sees no cursor. What
then happens to the popup that asked is in `### Settling the Frame`: no anchor
means it is closed, not left `visible` and undrawn.
Clicking inside a tab therefore moves the cursor to the tab's start -- visible
behaviour, and the same answer already given for the second cell of a CJK
character.

### Editor Scrollbars

`AppLayout::editor_scrollbar` and `AppLayout::editor_hscrollbar` are the single
source of the two bars' geometry -- the last column and the last row of the
region below the tab bar, both carved out of `editor_area` by `compute_layout`
rather than subtracted inside `EditorViewWidget`. Everything that derives
geometry from `editor_area` (`view.area_height`/`area_width`, the cursor clamp in
`render.rs`, click and drag mapping in `mouse.rs`, `cmd_page_up/down`,
`ensure_h_scroll`) is therefore correct with no arithmetic of its own, exactly as
with `sidebar_divider`.

The two carves are independent guards -- a frame can be wide enough to spare a
column and too short to spare a row -- and the vertical track is
`editor_area.height` tall, **not** the full text region. That is load-bearing:
`ui::scrollbar::thumb`'s `max_offset` and `View::scroll_down`'s `max_top` are the
same number only while the track and `view.area_height` are. Widening the track
back over the corner would make a drag to the bottom of the track stop one line
short of where the wheel gets to. The 1x1 corner where the two would meet
therefore belongs to neither, and `AppLayout::editor_scrollbar_corner()` derives
it from the two rects.

Both are reserved whatever the tab holds and whether or not anything overflows,
so text never reflows -- not vertically when a long line scrolls into view, not
horizontally when switching between a short file, a long one and an image. Only
the **thumb** is drawn (`THUMB_GLYPH` / `H_THUMB_GLYPH`, one cell each); the
track stays the editor background, and content that fits gets no thumb at all --
a thumb filling the whole track reads as scrollable content that will not move.
The branches with nothing to draw still call `scrollbar::blank`, and so do the
row's gutter columns and the corner: not against staleness (ratatui resets the
back buffer before every draw) but for the _background_, because `Cell::reset`
leaves `bg: Color::Reset`, which the backend emits as the terminal's default, so
an unpainted region would show as a stripe or a notch beside the editor's own
background.

`ui::scrollbar::thumb` and its inverse `offset_for_thumb` are **axis-neutral**
and shared by both widgets and by `mouse.rs`: `total` is however many units of
content there are, `offset` how far in the viewport starts. Both endpoints are
exact (start → offset 0, `max_offset` → `offset + length == track_len`); the
middle is approximate, and one thumb cell covers many lines or columns. A track
one cell long is the degenerate case where the thumb fills it and has nowhere to
travel -- reachable far more easily horizontally, from a wide gutter in a narrow
pane -- and `offset_for_thumb` answers `0` there, because that is what the thumb
it would draw stands for.

The horizontal track covers the **code area only**: `ui::scrollbar::h_track(row,
gutter_width)` drops the gutter and its separator column, because the gutter does
not scroll. The row is the whole reserved strip and the track is what is left of
it; `compute_layout` cannot cut the track itself, since the gutter width depends
on the line count.

The horizontal scroll total is `ui::scrollbar::content_width`: the widest line
**currently on screen**, bounded by `SCAN_BUDGET` (50,000). There is
deliberately no document-wide maximum-width cache -- it would have to be
invalidated on every edit, undo, redo and LSP-applied change. The visible
consequence is that the thumb resizes as the document scrolls vertically.

The budget is spent **across the visible lines, not per line**, and it bounds
characters examined as well as columns counted: the `RopeSlice` is walked
lazily, a line is measured only as far as what is left of the budget, and once
it is gone the remaining lines are not measured at all. A frame therefore costs
one budget however tall the viewport is, and never O(line length) -- not even
for a line built out of zero-width characters, which a column budget alone would
not bound. `display_width::TabStops::width_capped_chars` is the shared scan; it
takes a character iterator rather than a `&str` precisely so no line has to be
collected into a `String` first, which used to make the scan O(line length)
whatever the cap said.

The **character** half of the cap is justified by zero-width characters alone --
combining marks. A run of tabs used to be the other example, back when a tab was
measured as zero columns; it is no longer one, since a tab now advances the
width to its next stop and so fills the column half after `SCAN_BUDGET /
tab_size` characters. The character half stays: without it a line of combining
marks is still walked to its end.

What the total is a function of is the whole design: the document, `top_line`,
`view.area_height` and the track width -- and **not** `left_col`. A horizontal
drag writes `left_col` and nothing else, so it cannot move the scale it is being
measured against. The press, every drag event and the frame drawn after the
release all measure the same thing and get the same number: a held pointer
settles on the _first_ event, and letting go does not shift the thumb.

Getting there took removing two things that had grown around each other. The
budget was once `left_col + k`, which grew the total with every scroll rightwards
so the thumb never approached the end of the track -- the destination receded as
fast as it was approached. The **floor** at `left_col + code_width` was then kept
for a different reason (below) and reintroduced the same feedback, and
`ScrollbarDrag::Horizontal` grew a latched `total` to break it. Latching worked
and cost two defects of its own: the thumb jumped on release, where the latched
number and the fresh one differed, and an `Up` lost outside the terminal left the
latch behind for later frames to be drawn through -- a thumb for content that is
not on screen. A constant budget and no floor leave nothing to latch.

The floor existed to keep a thumb on screen when `left_col` is past every visible
line -- a long line scrolled right along and then scrolled off the top. Without
it `total` is just what fits, `thumb` returns `None`, and the bar is empty while
the code area shows blank columns. What leads back is a **rule** rather than a
number: `handle_hscrollbar_press` treats a press on an empty track as "return to
column 0" instead of swallowing it. That is the one place the two bars differ,
and it is the state only the horizontal one can be in -- `top_line` past the
document is not reachable, `left_col` past the content is.

The price of the budget is a horizon: a line wider than 50,000 columns cannot be
dragged past that point, and the cursor motions (`End`, search) are what reach
further. Carried out there by the cursor, `left_col` exceeds the total, and
`thumb`'s `offset.min(max_offset)` pins the thumb to the right end of the track
rather than overflowing it -- so it is still there to grab and drag back.

`ui::scrollbar::hscroll_total(editor, code_width)` is the single source of the
number, and both `render.rs` and `mouse.rs` go through it. Two matching call
sites would be one drift away from a thumb drawn where the pointer is not. The
viewport height inside it comes from `view.area_height` rather than from
`AppLayout`, for the same one-source reason.

The vertical press is tested **before** `editor_area` in `handle_left_click`
because the column is inside that region's columns -- otherwise a press on the
thumb would place the cursor on the last visible character of a line. The
horizontal press is tested there too, for symmetry and against a future re-carve:
the row does not overlap `editor_area`, so nothing presently depends on the
order. A press off either thumb centres it under the pointer and the drag carries
on under the same rule -- except on an empty horizontal track, where there is no
thumb to centre and the press returns the view to column 0 instead. The grab
point lives in `Editor.scrollbar_drag`, whose `ScrollbarDrag` carries the
**axis** and nothing else, so only one drag can be live -- a pointer has one
button and a drag has one axis, and two `Option`s would make an impossible state
representable. It is cleared on every `Down`, so an `Up` lost outside the
terminal cannot leave a thumb stuck to the pointer; nothing is drawn through it
either way, since neither bar's total is read out of it. Nothing here moves the
cursor, the selection or the mode, and no `MouseAction` variant is involved:
scrolling is pure `Editor` state and `App` has nothing to decide.

A popup owns both bars as it owns the wheel: `popup_is_up` guards press and drag
alike, so a press on either scrollbar while the search bar, the fuzzy finder, the
command palette or the settings screen is up moves nothing. It is swallowed
rather than dismissing the popup -- a scrollbar does not change the mode, and
closing one from a scroll gesture would.

### Settling the Frame

`App::settle` is `sync_viewport_metrics` then `dismiss_popups_without_a_cursor`,
and it runs after **every event** as well as before every frame
(`settle_and_draw` is `settle`, then `sync_tab_modified`, then the draw).
Per event, because both halves are read by the _next event_ rather than only by
the draw: events arrive in batches -- the coalescing loop takes everything the
terminal already has before drawing anything -- so the `Enter` behind a wheel
notch used to accept a completion the notch had already stranded off screen, and
a resize correction used to land on a scroll the user made after the resize.
Settling is not drawing: it costs the batch no frames, and neither half does any
work in the ordinary case.

`event_loop` calls `settle_and_draw` before waiting for the first event as well
as at the tail of each iteration: a view still at `area_height = 0` measures
nothing, so the first frame would come up with one scrollbar of the two. It is
generic over the ratatui backend, and events come from `App.event_handler`
(a `Box<dyn EventSource>`), so a test drives the real loop with a scripted
source and a `TestBackend` rather than copying its order into the test body --
`the_loop_draws_before_it_waits_for_an_event` is what holds the pre-loop draw in
place.

`sync_viewport_metrics` is the only writer of `view.area_width` /
`view.area_height`, so it is the only place that can **see** the viewport change
size. It compares before assigning, and on a change re-corrects the scroll --
`ensure_cursor_visible` vertically, `command::ensure_h_scroll` horizontally --
so the cursor is inside the viewport again before the frame is drawn. One
correction point covers every path that can resize `editor_area`: a terminal
resize, the sidebar's width and visibility, the theme's `pane_focus_style` /
`panel_borders`, and a tab switch onto a view last sized under different
geometry. None of them knows this exists, and a fifth path added tomorrow does
not have to.

Not at those call sites, because at every one of them `view.area_width` is still
the _previous_ frame's -- the metrics do not refresh until the top of the next
iteration -- so an immediate correction computes `code_width` from the old width
and misses by exactly the delta being applied. The exceptions are `tab_size` and
`line_numbers` in `apply_config_value`, which **do** call `ensure_h_scroll`
immediately: neither resizes `editor_area` (a gutter is carved out of it, not
added to it), so change detection cannot see them, and `area_width` is already
current there. Removing those calls "because the resize path covers it" puts the
cursor back off the side of a tab-indented line.

The correction runs **only** on a change, and that is a hard constraint rather
than an optimisation. Correcting every frame would pull `left_col` back toward
the cursor while a horizontal drag held the thumb, and `mouse.rs`'s drag
invariants never call `sync_viewport_metrics`, so they would all keep passing
while the running editor misbehaved.
`a_frame_that_did_not_change_size_leaves_the_scroll_alone` is what states the
constraint directly.

A resize _can_ arrive mid-drag, and then the axis the drag writes is skipped and
the other one is not. `ScrollbarDrag` already names the axis, so this is a
`matches!` and no new state. Skipping both axes would consume the size change
and never get it back -- the cursor would stay lost for the rest of the drag and
after it -- and remembering that a correction is owed would be a latch with a
lifetime, which is exactly what `ScrollbarDrag`'s deleted `total` was.

`dismiss_popups_without_a_cursor` closes a completion or hover popup whose
anchor has gone: `visible` means visible. The two render call sites are
`if let Some(anchor)`, so without this the popup stops being drawn while its
flag stays `true`, and `Enter` accepts an item nobody can see. "No anchor" is
not a proxy for "not drawn" -- `render.rs`'s tests already assert that
`cursor_screen_position` answers `None` exactly when the widget reverses no
cell -- so every path that can hide the cursor is covered without being
enumerated. Both popups are treated alike, hover included: a second flag that
does not mean visible is the trap being closed.

A completion also ends when the cursor **leaves the word it was asked about**
(`invalidate_completion_that_moved_away`, in `process_event`). `accept_completion`
replaces the span from `trigger_position` to the cursor and refuses only a
different line, so a click further along the same line left that span covering
text the user never typed. The test is the document's version, not a list of
commands: typing moves the cursor _and_ bumps the version, while a click or an
arrow key moves it alone, so a command added later cannot forget to be listed.
The keyboard needs none of this -- `handle_completion_popup_key` already
dismisses the popup on any key it does not consume -- which is why the mouse was
where this showed.

The order of those two steps is the behaviour. The correction gets first
refusal, so a **resize keeps** a popup -- the cursor comes back and the anchor
with it -- and only a real scroll away from the cursor (wheel, scrollbar drag,
scrollbar press) closes one. Reversing the two lines is a behaviour change that
`a_resize_does_not_close_a_completion_popup` catches.

None of this happens during render: `render` takes `&Editor` and must keep
taking it (TEA). `AppEvent::Resize` is not where the frame is settled either --
it only updates `App.terminal_size`, for the benefit of `handle_mouse`, which
hit-tests against that field and can run against a click coalesced into the same
batch. The size the draw uses is always `terminal.size()`.

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
  plugins/     # Shipped plugins (each has plugin.toml + init.lua): `example`,
               # and `comment`, whose `plugin.comment.toggle` comments the
               # cursor line or the selected lines. Presets cannot bind it --
               # `tests/keymap_presets.rs` checks every preset command against a
               # registry with no plugins loaded -- so it is bound from
               # `keybindings.toml`, which is re-applied after plugins register.
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
