<p align="center">
  <h1 align="center">termcode</h1>
  <p align="center">A fast, lightweight terminal code editor built in Rust.</p>
  <p align="center">
    <a href="#installation">Installation</a> &middot;
    <a href="#features">Features</a> &middot;
    <a href="#usage">Usage</a> &middot;
    <a href="#configuration">Configuration</a> &middot;
    <a href="#themes">Themes</a>
  </p>
</p>

---

**termcode** is a modern terminal-based code editor that combines the speed of terminal editors with IDE-grade features. Built from scratch in Rust with a modular crate architecture, it delivers native performance, full LSP support, and first-class CJK character handling.

## Why termcode?

- **Instant startup** -- opens files in milliseconds, not seconds
- **Zero runtime dependencies** -- single static binary, no Node.js, no Electron
- **True IDE features** -- autocompletion, hover docs, go-to-definition, diagnostics
- **CJK-native** -- Korean, Chinese, Japanese characters render correctly everywhere
- **Modular architecture** -- 8 focused crates, clean dependency layers, easy to extend

## Features

### Editor

| Feature                 | Details                                                                                   |
| ----------------------- | ----------------------------------------------------------------------------------------- |
| **Modal editing**       | 6 modes -- Normal, Edit, File Explorer, Search, Fuzzy Finder, Command Palette             |
| **Syntax highlighting** | Tree-sitter based. Rust, Python, JavaScript, TypeScript, Go, C, C++, TOML, JSON, Markdown |
| **LSP integration**     | Autocomplete, hover info, go-to-definition, real-time diagnostics                         |
| **Fuzzy file finder**   | `Ctrl+P` -- fast fuzzy search with smart scoring (word boundaries, path proximity)        |
| **Search & Replace**    | `Ctrl+F` / `Ctrl+H` -- case-insensitive, match counter, replace all                       |
| **Command palette**     | `Ctrl+Shift+P` -- searchable command list with theme switcher                             |
| **Multi-tab**           | Open multiple files, navigate with `Alt+Left/Right`, close with `Ctrl+W`                  |
| **Unsaved protection**  | Confirmation dialog on close/quit when files have unsaved changes                         |
| **File explorer**       | `Ctrl+B` -- tree view sidebar with `.gitignore` awareness                                 |
| **Image viewer**        | View images in tabs -- PNG, JPG, GIF, BMP, WebP, ICO, TIFF, AVIF (Sixel/Kitty/iTerm2)     |
| **Lua plugins**         | Custom commands, editor API, hook system (`on_open`, `on_save`, `on_close`, etc.)         |
| **Undo/Redo**           | Branching history with full transaction support                                           |
| **Mouse support**       | Click, drag select, scroll wheel, tab/sidebar click                                       |

### Technical

| Feature              | Details                                                                  |
| -------------------- | ------------------------------------------------------------------------ |
| **Rope buffer**      | O(log n) edits via `ropey` -- handles large files efficiently            |
| **Atomic saves**     | Write-to-temp + rename prevents data loss on crash                       |
| **Encoding**         | UTF-8, UTF-16 LE/BE, BOM detection, auto line-ending (LF/CRLF)           |
| **Unicode width**    | Full-width CJK characters, combining marks, emoji handled correctly      |
| **System clipboard** | Copy/Cut/Paste via system clipboard (`Ctrl+C/X/V`)                       |
| **Diagnostics**      | Inline underlines, gutter icons, error/warning navigation with `[` / `]` |
| **True color**       | 24-bit RGB color rendering                                               |

## Installation

### From source

```bash
# Requires Rust 1.85+
git clone https://github.com/user/termcode.git
cd termcode
cargo install --path .
```

### Build from source

```bash
cargo build --release
# Binary at target/release/termcode
```

## Usage

```bash
# Open a file
termcode path/to/file.rs

# Open a directory (starts in file explorer)
termcode .

# Open with no arguments (empty editor)
termcode
```

### Keybindings

#### Normal Mode

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

#### Edit Mode

| Key                    | Action                |
| ---------------------- | --------------------- |
| `Esc`                  | Return to Normal mode |
| Any character          | Insert at cursor      |
| `Backspace` / `Delete` | Delete character      |
| `Enter`                | New line              |
| `Home` / `End`         | Line start / end      |
| Arrow keys             | Move cursor           |

## Configuration

Configuration files are loaded from the standard config directory:

- **Editor config** -- `config.toml`
- **Keybindings** -- `keybindings.toml`
- **Themes** -- `runtime/themes/*.toml`

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

[insert]
"ctrl+space" = "completion.trigger"
```

## Themes

Ships with **3 built-in themes**:

- **One Dark** (default)
- **Gruvbox Dark**
- **Catppuccin Mocha**

Switch themes via the command palette (`Ctrl+Shift+P` > Themes).

### Custom Themes

Create a `.toml` file in `runtime/themes/`:

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

## Architecture

termcode is built as 8 modular crates with strict dependency layers:

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

- **termcode-core** -- Buffer (Rope), Position, Selection, Transaction, History
- **termcode-theme** -- Theme loading, color resolution, syntax scope mapping
- **termcode-syntax** -- Tree-sitter integration, language registry
- **termcode-config** -- TOML config & keybinding loading
- **termcode-view** -- Editor state, Document, View, commands (frontend-agnostic)
- **termcode-lsp** -- LSP client, JSON-RPC transport, capability negotiation
- **termcode-term** -- Terminal UI, widgets, event loop, clipboard, LSP bridge

> **TEA (The Elm Architecture):** All state changes flow through `Event -> Update -> Render`. Widgets never mutate state during rendering.

## License

MIT
