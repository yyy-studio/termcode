<p align="center">
  <img src="logo.png" alt="termcode" width="480">
</p>

<p align="center">
  A fast, lightweight terminal code editor built in Rust.
</p>

<p align="center">
  <a href="#installation">Installation</a> &bull;
  <a href="#getting-started">Getting Started</a> &bull;
  <a href="#features">Features</a> &bull;
  <a href="#keybindings">Keybindings</a> &bull;
  <a href="#configuration">Configuration</a> &bull;
  <a href="#themes">Themes</a> &bull;
  <a href="#plugins">Plugins</a> &bull;
  <a href="#architecture">Architecture</a> &bull;
  <a href="#contributing">Contributing</a>
</p>

---

<p align="center">
  <img src="screenshots/screenshot1.png" alt="termcode screenshot">
</p>

**termcode** is a modern terminal-based code editor that combines the speed of terminal editors with IDE-grade features. Built from scratch in Rust with a modular crate architecture, it delivers native performance, full LSP support, and first-class CJK character handling.

## Highlights

- **Instant startup** -- opens files in milliseconds, not seconds
- **Zero runtime dependencies** -- single static binary, no Node.js, no Electron
- **True IDE features** -- autocompletion, hover docs, go-to-definition, diagnostics
- **CJK-native** -- Korean, Chinese, Japanese characters render correctly everywhere
- **Extensible** -- Lua plugin system with full editor API and lifecycle hooks

## Installation

### Quick install (recommended)

```bash
curl -fsSL https://raw.githubusercontent.com/yyy-studio/termcode/main/install.sh | sh
```

Automatically detects your platform, downloads the latest release, and installs to `~/.local/bin/`.

### Pre-built binaries

Download from [GitHub Releases](https://github.com/yyy-studio/termcode/releases):

| Platform              | Download                                    |
| --------------------- | ------------------------------------------- |
| macOS (Apple Silicon) | `termcode-aarch64-apple-darwin.tar.gz`      |
| macOS (Intel)         | `termcode-x86_64-apple-darwin.tar.gz`       |
| Linux (x86_64)        | `termcode-x86_64-unknown-linux-gnu.tar.gz`  |
| Linux (ARM64)         | `termcode-aarch64-unknown-linux-gnu.tar.gz` |
| Windows (x86_64)      | `termcode-x86_64-pc-windows-msvc.zip`       |

### From source (requires Rust 1.85+)

```bash
git clone https://github.com/yyy-studio/termcode.git
cd termcode
cargo install --path .
```

## Features

### Editor

| Feature                 | Details                                                                                      |
| ----------------------- | -------------------------------------------------------------------------------------------- |
| **Modal editing**       | 6 modes -- Normal, Edit, File Explorer, Search, Fuzzy Finder, Command Palette                |
| **Syntax highlighting** | Tree-sitter based -- Rust, Python, JS, TS, Go, C, C++, HTML, CSS, Bash, TOML, JSON, Markdown |
| **LSP integration**     | Autocomplete, hover info, go-to-definition, real-time diagnostics                            |
| **Fuzzy file finder**   | `Ctrl+P` -- fast fuzzy search with smart scoring                                             |
| **Search & Replace**    | `Ctrl+F` / `Ctrl+H` -- case-insensitive, match counter, replace all                          |
| **Command palette**     | `Ctrl+Shift+P` -- searchable command list with theme switcher                                |
| **Multi-tab**           | Open multiple files, navigate with `Alt+Left/Right`, close with `Ctrl+W`                     |
| **Unsaved protection**  | Confirmation dialog on close/quit when files have unsaved changes                            |
| **File explorer**       | `Ctrl+B` -- tree view sidebar with `.gitignore` awareness                                    |
| **Image viewer**        | View images in tabs -- PNG, JPG, GIF, BMP, WebP, ICO, TIFF, AVIF                             |
| **Lua plugins**         | Custom commands, editor API, hook system                                                     |
| **Undo/Redo**           | Branching history with full transaction support                                              |
| **Mouse support**       | Click, drag select, scroll wheel, tab/sidebar click                                          |

### Under the Hood

| Feature              | Details                                                             |
| -------------------- | ------------------------------------------------------------------- |
| **Rope buffer**      | O(log n) edits via `ropey` -- handles large files efficiently       |
| **Atomic saves**     | Write-to-temp + rename prevents data loss on crash                  |
| **Encoding**         | UTF-8, UTF-16 LE/BE, BOM detection, auto line-ending (LF/CRLF)      |
| **Unicode width**    | Full-width CJK characters, combining marks, emoji handled correctly |
| **System clipboard** | Copy/Cut/Paste via system clipboard (`Ctrl+C/X/V`)                  |
| **Diagnostics**      | Inline underlines, gutter icons, error/warning navigation           |
| **True color**       | 24-bit RGB color rendering                                          |

## Getting Started

### Launch

```bash
termcode path/to/file.rs    # Open a file
termcode .                   # Open directory in file explorer
termcode                     # Empty editor
```

### Options

| Flag              | Description   |
| ----------------- | ------------- |
| `-h`, `--help`    | Print help    |
| `-V`, `--version` | Print version |

### Quick Workflow

1. **Open a project** -- `termcode .` to start with the file explorer
2. **Navigate files** -- `Ctrl+B` to toggle sidebar, `Ctrl+P` to fuzzy find
3. **Edit** -- press `i` to enter Edit mode, `Esc` to return to Normal mode
4. **Save** -- `Ctrl+S`
5. **Search** -- `Ctrl+F` to search, `Ctrl+H` to search & replace
6. **Tabs** -- open multiple files, switch with `Alt+Left` / `Alt+Right`, close with `Ctrl+W`
7. **Commands** -- `Ctrl+Shift+P` to open command palette (theme switch, all commands)
8. **LSP** -- auto-activates if a language server is configured (see [Configuration](#configuration))
9. **Quit** -- `Ctrl+Q`

## Keybindings

### Normal Mode

| Key                          | Action                          |
| ---------------------------- | ------------------------------- |
| `h` `j` `k` `l` / Arrow keys | Move cursor                     |
| `0` / `Home`                 | Go to line start                |
| `$` / `End`                  | Go to line end                  |
| `g`                          | Go to document start            |
| `G`                          | Go to document end              |
| `PageUp` / `PageDown`        | Page up / down                  |
| `i`                          | Enter Edit mode                 |
| `x` / `Delete`               | Delete character                |
| `Shift+K`                    | LSP hover info                  |
| `Ctrl+P`                     | Fuzzy file finder               |
| `Ctrl+F`                     | Search                          |
| `Ctrl+H`                     | Search & Replace                |
| `Ctrl+Shift+P` / `:`         | Command palette                 |
| `Ctrl+B`                     | Toggle file explorer            |
| `Ctrl+D` / `F12`             | Go to definition                |
| `Ctrl+Z`                     | Undo                            |
| `Ctrl+Y`                     | Redo                            |
| `Ctrl+S`                     | Save                            |
| `Ctrl+W`                     | Close tab (confirms if unsaved) |
| `Alt+Left` / `Alt+Right`     | Previous / next tab             |
| `]` / `[`                    | Next / previous diagnostic      |
| `Ctrl+C`                     | Copy selection                  |
| `Ctrl+Q`                     | Quit (confirms if unsaved)      |
| `F1` / `?`                   | Help                            |

### Edit Mode

| Key                    | Action                |
| ---------------------- | --------------------- |
| `Esc`                  | Return to Normal mode |
| Any character          | Insert at cursor      |
| `Backspace` / `Delete` | Delete character      |
| `Enter`                | New line              |
| `Home` / `End`         | Line start / end      |
| Arrow keys             | Move cursor           |

### Keymap Presets

The table above describes the built-in keymap, a hybrid of VS Code shortcuts and
a small modal layer. If you would rather have the keys your usual editor uses,
pick a preset in `config.toml`:

```toml
[keymap]
preset = "vim"   # "vscode" | "vim" | "helix"
```

| Preset   | Style                                                                                    |
| -------- | ---------------------------------------------------------------------------------------- |
| `vscode` | Always in Insert mode, VS Code shortcuts, readline keys (`Ctrl+A`/`Ctrl+E`) while typing |
| `vim`    | Modal, with real multi-key sequences: `gg`, `dd`, `yy`, `gd`, `]d`, `ZQ`                 |
| `helix`  | Modal, `Space` leader plus the `g` / `[` / `]` prefixes                                  |

A preset **replaces** the whole keymap rather than layering on top of it, so it
never inherits a default binding that contradicts it. Your `keybindings.toml`
overrides still apply on top of the preset.

The `vscode` preset has no modal layer: it opens ready to type, `Esc` does not
drop you into a mode with nothing bound, and closing an overlay returns you to
the buffer. A preset opts into that with `[meta] initial_mode = "insert"`.

Presets live in `runtime/keymaps/*.toml`; drop your own into
`~/.config/termcode/keymaps/` to add one — that directory is searched first, so
a file there also replaces a shipped preset of the same name.

Switch keymaps for the current session from the command palette
(**Select Keymap**); `[keymap] preset` still decides what loads at startup.

Two keys are deliberately not preset-controlled:

- `Ctrl+Q` always quits, so a broken keymap can never trap you in the editor.
- Terminal multiplexers and shells claim some keys before termcode sees them.
  Each preset documents where it steps around `Ctrl+B` (tmux prefix), `Ctrl+Z`
  (suspend) and `Ctrl+Shift+P` (indistinguishable from `Ctrl+P` in most
  terminals).

While a multi-key sequence is half-typed, the pending keys show in the status
bar. `chord_timeout_ms` (default 1000 ms) is how long the editor waits for the
next key, measured from the last one. If the sequence is abandoned while you are
typing — into the buffer, the search box, or the finder — the keys are entered as
text rather than lost.

All keybindings are customizable via `keybindings.toml`. See [Configuration](#configuration).

## Configuration

termcode stores all user data under `~/.config/termcode/`:

```
~/.config/termcode/
  config.toml          # editor settings
  keybindings.toml     # custom keybindings
  themes/              # custom themes (.toml)
  plugins/             # Lua plugins
  sessions/            # auto-saved sessions
```

Config is loaded from (in order):

1. `~/.config/termcode/config.toml` -- user config
2. `./config/config.toml` -- project-local override

### config.toml

```toml
[editor]
tab_size = 4
insert_spaces = true
word_wrap = false
line_numbers = "relative"     # "absolute", "relative", "relative_absolute", "none"
scroll_off = 5
mouse_enabled = true
auto_save = false
auto_save_delay_ms = 1000

[ui]
sidebar_width = 30
sidebar_visible = true
show_tab_bar = true
show_top_bar = true

[lsp.rust]
command = "rust-analyzer"
args = []

[lsp.python]
command = "pyright-langserver"
args = ["--stdio"]
```

### keybindings.toml

```toml
[normal]
"ctrl+p" = "fuzzy_finder.open"
"ctrl+f" = "search.open"

[edit]
"ctrl+space" = "completion.trigger"
```

## Themes

Ships with **3 built-in themes**:

- **One Dark** (default)
- **Gruvbox Dark**
- **Catppuccin Mocha**

Switch themes via the command palette (`Ctrl+Shift+P` > Themes).

### Custom Themes

Create a `.toml` file in `~/.config/termcode/themes/`:

```toml
[meta]
name = "My Theme"

[palette]
bg = "#1a1b26"
fg = "#c0caf5"

[ui]
background = "bg"
foreground = "fg"
cursor = "#f7768e"
selection = "#283457"
# ... 20+ configurable UI color slots

[scopes]
"keyword" = { fg = "#bb9af7" }
"function" = { fg = "#7aa2f7" }
"string" = { fg = "#9ece6a" }
"comment" = { fg = "#565f89", modifiers = ["italic"] }
```

## Plugins

termcode supports **Lua plugins** for extending editor functionality.

### Capabilities

- **Custom commands** -- register commands accessible from the command palette
- **Editor API** -- read/write buffer text, cursor position, selection, file info
- **Hook system** -- respond to lifecycle events: `on_open`, `on_save`, `on_close`, `on_mode_change`, `on_cursor_move`, `on_buffer_change`, `on_tab_switch`, `on_ready`
- **Status bar** -- display messages from plugins
- **Logging** -- `log.info()`, `log.warn()`, `log.error()`, `log.debug()`

### Example Plugin

```lua
-- ~/.config/termcode/plugins/hello/init.lua

plugin.register_command("hello.greet", "Say hello", function()
    local name = editor.get_filename() or "world"
    editor.set_status("Hello, " .. name .. "!")
end)

hooks.on_save = function(event)
    log.info("Saved: " .. (event.filename or "unknown"))
end
```

## Architecture

termcode is built as **8 modular crates** with strict dependency layers:

```
                    termcode (binary)
                        |
                   termcode-term        (terminal, event loop, ratatui)
                    /        \
           termcode-plugin  termcode-view   termcode-lsp
                |              |               |
          termcode-config  termcode-syntax
                \              /
            termcode-core  termcode-theme
```

| Crate               | Role                                                       |
| ------------------- | ---------------------------------------------------------- |
| **termcode-core**   | Buffer (Rope), Position, Selection, Transaction, History   |
| **termcode-theme**  | Theme loading, color resolution, syntax scope mapping      |
| **termcode-syntax** | Tree-sitter integration, language registry                 |
| **termcode-config** | TOML config & keybinding loading                           |
| **termcode-view**   | Editor state, Document, View, commands (frontend-agnostic) |
| **termcode-lsp**    | LSP client, JSON-RPC transport, capability negotiation     |
| **termcode-plugin** | Lua plugin runtime, hook system, editor API bindings       |
| **termcode-term**   | Terminal UI, widgets, event loop, clipboard, LSP bridge    |

> All state changes flow through **Event -> Update -> Render** (TEA architecture). Widgets never mutate state during rendering.

## Contributing

Contributions are welcome! Here's how to get started:

```bash
git clone https://github.com/user/termcode.git
cd termcode
cargo build
cargo test --workspace
```

Before submitting a PR:

```bash
cargo clippy --workspace    # Must be 0 warnings
cargo fmt --check           # Must pass
cargo test --workspace      # Must pass
```

## License

[MIT](LICENSE)
