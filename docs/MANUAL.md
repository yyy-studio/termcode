# termcode User Manual

---

## Table of Contents

1. [Introduction](#1-introduction)
2. [Installation](#2-installation)
3. [Getting Started](#3-getting-started)
4. [User Interface](#4-user-interface)
5. [Editor Modes](#5-editor-modes)
6. [Complete Keybinding Reference](#6-complete-keybinding-reference)
7. [Editing](#7-editing)
8. [File Explorer](#8-file-explorer)
9. [Search and Replace](#9-search-and-replace)
10. [Fuzzy File Finder](#10-fuzzy-file-finder)
11. [Command Palette](#11-command-palette)
12. [LSP Integration](#12-lsp-integration)
13. [Configuration](#13-configuration)
14. [Keybinding Customization](#14-keybinding-customization)
15. [Themes](#15-themes)
16. [Session Management](#16-session-management)
17. [Mouse Support](#17-mouse-support)
18. [Troubleshooting](#18-troubleshooting)

---

## 1. Introduction

### What is termcode

termcode is a terminal-based code viewer and editor written in Rust. It provides a modern editing experience entirely within your terminal, combining a file explorer sidebar, tabbed editor, syntax highlighting, LSP integration, and a fuzzy file finder into a single, lightweight binary.

### Key Differentiators

- **Integrated file explorer + editor in a single binary.** No external dependencies or plugins are required to browse and edit your project files. The sidebar tree view and editor pane coexist seamlessly.
- **Modal editing.** termcode uses a Normal/Insert mode paradigm inspired by Vim, allowing efficient keyboard-driven navigation and editing.
- **LSP support.** Built-in Language Server Protocol client provides autocomplete, diagnostics, hover information, and go-to-definition for any language with an LSP-compliant server.
- **Lightweight and fast.** The entire application is approximately 10,000 lines of Rust across an 8-crate workspace, rendering only visible lines for performance.
- **Themeable.** Ships with three built-in themes (One Dark, Gruvbox Dark, Catppuccin Mocha) and supports user-created themes via TOML files.
- **Session persistence.** Automatically saves and restores your open files, tabs, and cursor positions between sessions.
- **Configurable keybindings.** All keybindings can be overridden via a TOML configuration file.

### Supported Platforms

termcode runs on any platform that supports a modern terminal emulator with 24-bit (true color) support:

- **macOS** (Terminal.app, iTerm2, Alacritty, WezTerm, Kitty)
- **Linux** (any terminal with true color support)
- **Windows** (Windows Terminal, ConEmu; native console not recommended)

The editor uses crossterm for terminal I/O, which provides cross-platform compatibility.

### Supported Languages (Syntax Highlighting)

termcode includes built-in syntax highlighting for the following languages:

| Language   | File Extensions               |
| ---------- | ----------------------------- |
| Rust       | `.rs`                         |
| Python     | `.py`, `.pyi`                 |
| JavaScript | `.js`, `.mjs`, `.cjs`         |
| TypeScript | `.ts`, `.tsx`                 |
| Go         | `.go`                         |
| C          | `.c`, `.h`                    |
| C++        | `.cpp`, `.cc`, `.cxx`, `.hpp` |
| TOML       | `.toml`                       |
| JSON       | `.json`                       |
| Markdown   | `.md`, `.markdown`            |

---

## 2. Installation

### Prerequisites

- **Rust toolchain**: You need Rust 1.70 or later. Install it via [rustup](https://rustup.rs/):

  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  ```

- **A terminal with true color support**: termcode uses 24-bit RGB colors. Most modern terminal emulators support this.

### Building from Source

Clone the repository and build:

```bash
git clone <repository-url> termcode
cd termcode
cargo build --release
```

The release binary will be located at:

```
target/release/termcode
```

You can copy this binary to a directory on your PATH for convenient access:

```bash
cp target/release/termcode ~/.local/bin/
```

### Development Build

For development (faster compilation, debug symbols):

```bash
cargo build
cargo run -- .
```

### Running Tests

```bash
cargo test --workspace     # Run all 91 tests
cargo clippy --workspace   # Lint (should produce 0 warnings)
cargo fmt --check          # Check formatting
```

---

## 3. Getting Started

### First Launch

Launch termcode with no arguments to open an empty editor. If a previous session exists for the current directory, it will be restored automatically:

```bash
termcode
```

### Opening Files and Directories

```bash
termcode .                # Open current directory with file explorer sidebar
termcode src/main.rs      # Open a specific file
termcode /path/to/project # Open a directory
```

When you open a **directory**, the file explorer sidebar is shown and focused. When you open a **file**, the sidebar is hidden and the file is loaded into the editor.

### CLI Arguments

```
termcode [PATH]

Arguments:
  [PATH]  File or directory to open (optional)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

If `PATH` is a file, its parent directory is used as the project root for the file explorer. If `PATH` is a directory, it becomes the project root. If no path is given, the current working directory is used as the project root, and the editor starts empty (or restores the previous session).

### Basic Navigation

Once the editor is open:

1. If the file explorer is focused (FileExplorer mode), use `j`/`k` or arrow keys to navigate the tree, and press `Enter` to open a file.
2. Press `Tab` or `Esc` to leave the file explorer and enter Normal mode in the editor.
3. In Normal mode, use `h`/`j`/`k`/`l` or arrow keys to move the cursor.
4. Press `i` to enter Insert mode and begin typing.
5. Press `Esc` to return to Normal mode.
6. Press `Ctrl+S` to save the current file.
7. Press `Ctrl+Q` or `Ctrl+C` (without a selection) to quit.

---

## 4. User Interface

### Layout Overview

termcode's interface consists of five main regions arranged as follows:

```
+---------------------------------------------------------------+
| Top Bar (termcode title)                                      |
+------------------+--------------------------------------------+
|                  | Tab Bar: [ main.rs | lib.rs | config.toml ]|
|   File Explorer  +--------------------------------------------+
|   Sidebar        |                                            |
|                  |  Line Numbers | Editor Area                |
|   src/           |     1         | fn main() {               |
|     main.rs      |     2         |     println!("hello");    |
|     lib.rs       |     3         | }                         |
|   Cargo.toml     |               |                           |
|                  |               |                           |
+------------------+--------------------------------------------+
| Status Bar: NORMAL | Ln 1, Col 0 | UTF-8 | rust | 0 errors  |
+---------------------------------------------------------------+
```

### Top Bar

The top bar spans the full width of the terminal and displays the application name ("termcode"). It can be hidden via the `show_top_bar` configuration option.

### File Explorer Sidebar

The sidebar occupies the left portion of the screen (default width: 30 columns) and displays a tree view of your project's files and directories. Key features:

- **Toggle visibility**: Press `Ctrl+B` to show/hide the sidebar. When shown, focus moves to the file explorer. Pressing `Ctrl+B` again when already focused hides it.
- **Navigation**: Use `j`/`k` or Up/Down arrows to move the selection.
- **Expand/Collapse**: Press `l`/Right to expand a directory, `h`/Left to collapse it. Press `Enter` on a directory to toggle expansion.
- **Open files**: Press `Enter` on a file to open it in the editor.
- **.gitignore awareness**: Files and directories listed in `.gitignore` are excluded from the tree.
- **Scroll**: The tree scrolls automatically to keep the selected item visible.

### Tab Bar

The tab bar appears below the top bar (or at the very top if the top bar is hidden) and shows all open files as tabs:

- **Active tab**: Highlighted with a distinct background color matching the editor background.
- **Inactive tabs**: Shown with a darker background.
- **Modified indicator**: A bullet character appears before the filename when the file has unsaved changes (e.g., `. main.rs`).
- **Switch tabs**: Press `Alt+Right` / `Alt+Left` to cycle through tabs, or click on a tab with the mouse.
- **Close tab**: Press `Ctrl+W` to close the current tab.

The tab bar can be hidden via the `show_tab_bar` configuration option.

### Editor Area

The main editing area consists of:

- **Gutter (line numbers)**: Displayed on the left side. Supports four styles: absolute, relative, relative+absolute, and hidden. The current line's number is highlighted. The gutter width adjusts automatically based on the total number of lines.
- **Code area**: The main text editing region. Only lines within the viewport are rendered for performance.
- **Cursor**: Block cursor in Normal mode; positioned on the character. In Insert mode, the cursor can be positioned after the last character on a line.
- **Cursor line highlight**: The line containing the cursor has a subtle background highlight (`cursor_line_bg` color).
- **Diagnostic indicators**: Error, warning, info, and hint icons appear in the gutter next to lines with LSP diagnostics. Diagnostic text is underlined inline.
- **Search matches**: When a search is active, all matches are highlighted in the search match color, with the current match in a distinct active color.

### Status Bar

The status bar at the bottom of the screen shows:

- **Mode indicator**: Displays the current editor mode (NORMAL, INSERT, EXPLORER, SEARCH, FUZZY, PALETTE).
- **Cursor position**: Shows the current line and column (e.g., `Ln 42, Col 10`).
- **Encoding**: Displays the file encoding (UTF-8).
- **Language**: Shows the detected language for the current file (e.g., `rust`, `python`).
- **Diagnostics summary**: Shows the count of errors and warnings from LSP (e.g., `2 errors, 1 warning`).
- **Status messages**: Transient messages (save confirmation, error messages, etc.) appear in the status bar.

---

## 5. Editor Modes

termcode uses a modal editing paradigm with six distinct modes. The current mode is always shown in the status bar.

### Normal Mode

**Purpose**: Navigation and command execution. This is the default mode when the editor starts.

**Key characteristics**:

- Cursor movement via `h`/`j`/`k`/`l` or arrow keys.
- Single-key commands for common operations (e.g., `x` to delete a character, `i` to enter Insert mode).
- The cursor sits ON a character (cannot be positioned past the last character on a line).
- No text is inserted when typing alphanumeric keys; they are interpreted as commands.

**Entering Normal mode**: Press `Esc` from Insert mode, or `Tab`/`Esc` from File Explorer mode.

**Cursor movement**:
| Key | Action |
|------------------|-------------------------|
| `h` or Left | Move cursor left |
| `j` or Down | Move cursor down |
| `k` or Up | Move cursor up |
| `l` or Right | Move cursor right |
| `g` or Home | Go to beginning of file |
| `G` (Shift+G) or End | Go to end of file |
| PageUp | Move one page up |
| PageDown | Move one page down |

**Editing**:
| Key | Action |
|------------------|-------------------------------|
| `x` or Delete | Delete character under cursor |
| `i` | Enter Insert mode |

**LSP**:
| Key | Action |
|------------------|-------------------------------|
| `]` | Jump to next diagnostic |
| `[` | Jump to previous diagnostic |
| `Shift+K` | Show hover info (LSP) |
| `F12` or `Ctrl+D`| Go to definition (LSP) |

### Insert Mode

**Purpose**: Text input. Characters you type are inserted into the document.

**Entering Insert mode**: Press `i` in Normal mode.

**Key characteristics**:

- All printable characters (including those typed with Shift held) are inserted at the cursor position.
- The cursor can be positioned after the last character on a line (one position beyond the end).
- Backspace deletes the character before the cursor.
- Delete removes the character under the cursor.
- Enter inserts a newline.
- Arrow keys move the cursor without leaving Insert mode.

**Exiting Insert mode**: Press `Esc` to return to Normal mode.

**Autocomplete**:

- When an LSP server is active, typing trigger characters (e.g., `.` in Rust) automatically shows the autocomplete popup.
- Press `Ctrl+Space` to manually trigger autocomplete.
- When the autocomplete popup is visible:
  - `Up`/`Down` to navigate items.
  - `Enter` or `Tab` to accept the selected completion.
  - `Esc` to dismiss the popup.
  - Typing any other character dismisses the popup and inserts the character.

### File Explorer Mode

**Purpose**: Navigating the file tree in the sidebar.

**Entering**: Press `Ctrl+B` to toggle the sidebar and enter File Explorer mode. Also entered automatically when clicking on a sidebar item.

**Navigation**:
| Key | Action |
|------------------|-------------------------------|
| `j` or Down | Move selection down |
| `k` or Up | Move selection up |
| `Enter` | Open file or toggle directory |
| `l` or Right | Expand directory |
| `h` or Left | Collapse directory / go to parent |
| `Esc` or `Tab` | Return to Normal mode |

When pressing `h`/Left on a collapsed directory or a file, the selection moves to the parent directory.

### Search Mode

**Purpose**: Finding and replacing text in the current document.

**Entering**: Press `Ctrl+F` for search, or `Ctrl+H` for search and replace.

**Behavior**:

- A search bar appears at the top of the editor area with a text input field.
- As you type, the search runs incrementally and all matches are highlighted in the document.
- Search is **case-insensitive literal** (not regex).
- The current match index and total count are displayed.

**Navigation and actions**:
| Key | Action |
|------------------|-----------------------------------------------|
| Type characters | Update search query (incremental) |
| Backspace | Delete last character from query |
| `Enter` | Jump to next match |
| `Shift+Enter` | Jump to previous match |
| `Tab` | Switch focus between search and replace fields |
| `Ctrl+H` | Toggle replace mode while in search |
| `Enter` (in replace field) | Replace current match |
| `Ctrl+Alt+Enter` | Replace all matches |
| `Esc` | Close search and return to Normal mode |

### Fuzzy Finder Mode

**Purpose**: Quickly opening files by typing part of the filename.

**Entering**: Press `Ctrl+P`.

**Behavior**:

- An overlay popup appears with a text input and a list of files.
- Files are loaded from the project root, respecting `.gitignore`.
- As you type, files are filtered and ranked by fuzzy match score.
- The best matches appear at the top.

**Navigation**:
| Key | Action |
|------------------|-------------------------------|
| Type characters | Filter file list |
| Backspace | Delete last character |
| `Up` or `Ctrl+K` | Move selection up |
| `Down` or `Ctrl+J`| Move selection down |
| `Enter` | Open selected file |
| `Esc` | Close finder, return to Normal mode |

### Command Palette Mode

**Purpose**: Discovering and executing commands, switching themes.

**Entering**: Press `Ctrl+Shift+P`.

**Behavior**:

- An overlay popup appears listing all registered commands.
- Type to fuzzy-filter the command list.
- Selecting "Select Theme" opens a secondary palette listing available themes.

**Navigation**:
| Key | Action |
|------------------|-------------------------------|
| Type characters | Filter command list |
| Backspace | Delete last character |
| `Up` or `Ctrl+K` | Move selection up |
| `Down` or `Ctrl+J`| Move selection down |
| `Enter` | Execute selected command |
| `Esc` | Close palette, return to Normal mode |

---

## 6. Complete Keybinding Reference

### Global Keybindings

These work in every mode:

| Key            | Command ID            | Action                                                  |
| -------------- | --------------------- | ------------------------------------------------------- |
| `Ctrl+Q`       | _(built-in)_          | Quit the editor                                         |
| `Ctrl+C`       | `clipboard.copy`      | Copy selection (or quit if no selection / double-press) |
| `Ctrl+S`       | `file.save`           | Save the current file                                   |
| `Ctrl+B`       | `view.toggle_sidebar` | Toggle sidebar / enter File Explorer mode               |
| `Ctrl+F`       | `search.open`         | Open search                                             |
| `Ctrl+H`       | `search.open_replace` | Open search and replace                                 |
| `Ctrl+P`       | `fuzzy.open`          | Open fuzzy file finder                                  |
| `Ctrl+Shift+P` | `palette.open`        | Open command palette                                    |
| `Alt+Right`    | `tab.next`            | Switch to next tab                                      |
| `Alt+Left`     | `tab.prev`            | Switch to previous tab                                  |
| `Ctrl+W`       | _(built-in)_          | Close current tab                                       |
| `Ctrl+Z`       | `edit.undo`           | Undo                                                    |
| `Ctrl+Y`       | `edit.redo`           | Redo                                                    |
| `Ctrl+V`       | `clipboard.paste`     | Paste from system clipboard                             |
| `Ctrl+X`       | `clipboard.cut`       | Cut selection to system clipboard                       |

### Normal Mode Keybindings

| Key                 | Command ID         | Action                        |
| ------------------- | ------------------ | ----------------------------- |
| `j` / Down          | `cursor.down`      | Move cursor down              |
| `k` / Up            | `cursor.up`        | Move cursor up                |
| `h` / Left          | `cursor.left`      | Move cursor left              |
| `l` / Right         | `cursor.right`     | Move cursor right             |
| PageDown            | `cursor.page_down` | Page down                     |
| PageUp              | `cursor.page_up`   | Page up                       |
| `g` / Home          | `cursor.home`      | Go to beginning of file       |
| `G` (Shift+G) / End | `cursor.end`       | Go to end of file             |
| `i`                 | `mode.insert`      | Enter Insert mode             |
| `x` / Delete        | `edit.delete_char` | Delete character under cursor |
| `]`                 | `diagnostic.next`  | Jump to next diagnostic       |
| `[`                 | `diagnostic.prev`  | Jump to previous diagnostic   |
| `Ctrl+D`            | `goto.definition`  | Go to definition (LSP)        |
| `F12`               | `goto.definition`  | Go to definition (LSP)        |
| `Shift+K`           | `lsp.hover`        | Show hover info (LSP)         |

### Insert Mode Keybindings

| Key             | Command ID               | Action                         |
| --------------- | ------------------------ | ------------------------------ |
| `Esc`           | `mode.normal`            | Return to Normal mode          |
| Backspace       | `edit.backspace`         | Delete character before cursor |
| Delete          | `edit.delete_char`       | Delete character under cursor  |
| Enter           | `edit.newline`           | Insert newline                 |
| Up              | `cursor.up`              | Move cursor up                 |
| Down            | `cursor.down`            | Move cursor down               |
| Left            | `cursor.left`            | Move cursor left               |
| Right           | `cursor.right`           | Move cursor right              |
| `Ctrl+Space`    | `lsp.trigger_completion` | Manually trigger autocomplete  |
| Printable chars | _(built-in)_             | Insert character at cursor     |

When the **autocomplete popup** is visible:

| Key           | Action                              |
| ------------- | ----------------------------------- |
| Down          | Select next completion item         |
| Up            | Select previous completion item     |
| Enter / Tab   | Accept selected completion          |
| Esc           | Dismiss autocomplete popup          |
| Any other key | Dismiss popup, process key normally |

### File Explorer Keybindings

| Key         | Command ID          | Action                            |
| ----------- | ------------------- | --------------------------------- |
| `j` / Down  | `explorer.down`     | Move selection down               |
| `k` / Up    | `explorer.up`       | Move selection up                 |
| Enter       | `explorer.enter`    | Open file or toggle directory     |
| `l` / Right | `explorer.expand`   | Expand directory                  |
| `h` / Left  | `explorer.collapse` | Collapse directory / go to parent |
| `Esc`       | `mode.normal`       | Return to Normal mode             |
| `Tab`       | `mode.normal`       | Return to Normal mode             |

### Search Mode Keybindings

| Key              | Command ID           | Action                                                   |
| ---------------- | -------------------- | -------------------------------------------------------- |
| `Esc`            | `search.close`       | Close search                                             |
| Enter            | `search.next`        | Next match (or replace current if replace field focused) |
| `Shift+Enter`    | `search.prev`        | Previous match                                           |
| `Tab`            | _(built-in)_         | Toggle focus between search and replace fields           |
| `Ctrl+H`         | _(built-in)_         | Toggle replace mode on/off                               |
| `Ctrl+Alt+Enter` | `search.replace_all` | Replace all matches                                      |
| Backspace        | _(built-in)_         | Delete character from query/replace text                 |
| Printable chars  | _(built-in)_         | Append to query/replace text                             |

### Fuzzy Finder Keybindings

| Key             | Command ID    | Action                      |
| --------------- | ------------- | --------------------------- |
| `Esc`           | `fuzzy.close` | Close finder                |
| Up              | `fuzzy.up`    | Move selection up           |
| `Ctrl+K`        | `fuzzy.up`    | Move selection up           |
| Down            | `fuzzy.down`  | Move selection down         |
| `Ctrl+J`        | `fuzzy.down`  | Move selection down         |
| Enter           | _(built-in)_  | Open selected file          |
| Backspace       | _(built-in)_  | Delete character from query |
| Printable chars | _(built-in)_  | Append to query             |

### Command Palette Keybindings

| Key             | Command ID      | Action                                 |
| --------------- | --------------- | -------------------------------------- |
| `Esc`           | `palette.close` | Close palette                          |
| Up              | `palette.up`    | Move selection up                      |
| `Ctrl+K`        | `palette.up`    | Move selection up                      |
| Down            | `palette.down`  | Move selection down                    |
| `Ctrl+J`        | `palette.down`  | Move selection down                    |
| Enter           | _(built-in)_    | Execute selected command / apply theme |
| Backspace       | _(built-in)_    | Delete character from query            |
| Printable chars | _(built-in)_    | Append to query                        |

### Mouse Actions

| Action                    | Effect                                           |
| ------------------------- | ------------------------------------------------ |
| Left click on editor      | Place cursor at clicked position                 |
| Left click on line number | Select entire line                               |
| Left click on tab         | Switch to clicked tab                            |
| Left click on sidebar     | Select and open the clicked file/directory       |
| Left drag in editor       | Select text (drag selection from anchor to head) |
| Scroll wheel up           | Scroll view up by 3 lines                        |
| Scroll wheel down         | Scroll view down by 3 lines                      |

---

## 7. Editing

### Text Insertion

To insert text, enter Insert mode by pressing `i` in Normal mode. Then type normally. Every printable character (including those typed with Shift) is inserted at the current cursor position. The cursor advances to the right after each character.

Special insertions in Insert mode:

- **Enter**: Inserts a newline character, moving subsequent text to the next line.
- **Tab**: Inserts spaces based on the `tab_size` setting (when `insert_spaces` is true) or a literal tab character.

### Deletion

| Operation                      | Normal Mode       | Insert Mode |
| ------------------------------ | ----------------- | ----------- |
| Delete character under cursor  | `x` or Delete     | Delete      |
| Delete character before cursor | _(not available)_ | Backspace   |

In Normal mode, `x` or Delete removes the character under the cursor. In Insert mode, Backspace removes the character before the cursor, and Delete removes the character under the cursor.

### Undo and Redo

- **Undo**: `Ctrl+Z` -- reverses the last editing operation.
- **Redo**: `Ctrl+Y` -- re-applies the last undone operation.

Undo/Redo operate on the transaction history. Each discrete editing operation (character insertion, deletion, paste, replace) creates a transaction that can be undone and redone.

### Selection

**Mouse drag selection**: Click and drag in the editor area to select text. The selection starts at the click position (anchor) and extends to the current drag position (head). The selected text is highlighted with the theme's selection color.

**Line number click**: Clicking on a line number in the gutter selects the entire line.

After making a selection, you can:

- Copy it with `Ctrl+C`
- Cut it with `Ctrl+X`
- The selection is cleared when the cursor moves via keyboard.

### Copy, Cut, and Paste

termcode integrates with the system clipboard via the `arboard` library:

| Operation | Keybinding | Behavior                                                                                                                                                |
| --------- | ---------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Copy      | `Ctrl+C`   | Copies the selected text to the system clipboard. If no selection exists, quits the editor (acts as interrupt). Double-press within 500ms always quits. |
| Cut       | `Ctrl+X`   | Copies the selected text to the system clipboard and deletes it from the document.                                                                      |
| Paste     | `Ctrl+V`   | Inserts text from the system clipboard at the cursor position.                                                                                          |

### Saving Files

- **Manual save**: Press `Ctrl+S` to save the current file.
- **Atomic save**: termcode performs atomic saves -- the file content is written to a temporary location and then moved to the target path, preventing data corruption from interrupted writes.
- **Modified indicator**: The tab shows a bullet character (`.`) before the filename when the file has unsaved changes.
- **Save confirmation**: After a successful save, a message appears briefly in the status bar.

---

## 8. File Explorer

### Opening Directories

When you launch termcode with a directory path, the file explorer sidebar is shown and focused:

```bash
termcode .
termcode /path/to/project
```

When you open a file directly, the sidebar is hidden but the parent directory is set as the project root. You can reveal the sidebar at any time with `Ctrl+B`.

### Tree Navigation

The file explorer displays a hierarchical tree of files and directories. Directories are shown with a disclosure indicator:

- **Collapsed directory**: shown with a right-pointing triangle (or similar indicator)
- **Expanded directory**: shown with a down-pointing triangle

Navigation keys:

- `j` / Down: Move selection to the next item
- `k` / Up: Move selection to the previous item
- The selection wraps around when you reach the beginning or end of the visible tree.

### Expanding and Collapsing

- `l` / Right: Expand the selected directory (no effect if already expanded or if the item is a file).
- `h` / Left: Collapse the selected directory. If the selected item is a file or an already-collapsed directory, the selection jumps to the parent directory.
- `Enter`: Toggle expansion of a directory; open a file.

### Opening Files from the Tree

Press `Enter` on a file to open it in the editor:

- If the file is already open in a tab, the existing tab is activated (no duplicate tabs).
- The editor switches to Normal mode after opening a file.
- The file is loaded with syntax highlighting based on its extension.

### .gitignore Awareness

The file explorer respects `.gitignore` files in your project. Files and directories that match gitignore patterns are excluded from the tree display. This keeps the tree focused on relevant source files and avoids clutter from build artifacts, node_modules, and other ignored paths.

### Scroll Behavior

The file explorer automatically scrolls to keep the selected item visible within the sidebar viewport. The viewport height is determined by the terminal height minus the top bar, tab bar, and status bar.

---

## 9. Search and Replace

### Opening Search

- **Search only**: Press `Ctrl+F`. A search input bar appears below the tab bar.
- **Search and Replace**: Press `Ctrl+H`. Both search and replace input fields appear.

If you are already in Search mode, pressing `Ctrl+H` toggles the replace field on and off.

### Search Behavior

- **Case-insensitive literal search**: The search query is matched literally (not as a regular expression) and is case-insensitive.
- **Incremental search**: As you type characters into the search field, matches are found and highlighted in real-time.
- **Match highlighting**: All matches in the document are highlighted with the `search_match` color. The current match (the one the cursor will jump to) is highlighted with the `search_match_active` color.
- **Match count**: The total number of matches and the current match index are displayed.

### Navigating Matches

Once you have typed a search query:

| Key           | Action                 |
| ------------- | ---------------------- |
| `Enter`       | Jump to next match     |
| `Shift+Enter` | Jump to previous match |

In Normal mode (after closing search), you can also navigate matches with `n` (next) and `N` (previous) if matches remain highlighted.

The editor view scrolls automatically to center the current match, respecting the `scroll_off` setting.

### Opening Replace

Press `Ctrl+H` to open the replace field alongside the search field. Both fields are visible simultaneously.

### Switching Between Fields

Press `Tab` to toggle focus between the search field and the replace field. The focused field has the active cursor.

### Replacing

| Action                | Key / Behavior                                  |
| --------------------- | ----------------------------------------------- |
| Replace current match | Press `Enter` when the replace field is focused |
| Replace all matches   | Press `Ctrl+Alt+Enter`                          |

**Replace current**: Replaces only the currently highlighted match with the replace text, then advances to the next match.

**Replace all**: Replaces all matches in the document at once. Replacements are applied in reverse document order to preserve byte offsets. After replacement, the search is re-run to update the match list.

### Closing Search

Press `Esc` to close the search bar and return to Normal mode. Match highlights are cleared.

---

## 10. Fuzzy File Finder

### Opening the Fuzzy Finder

Press `Ctrl+P` to open the fuzzy file finder. An overlay popup appears centered in the editor area.

### How It Works

1. On first open, the finder loads all files from the project root directory.
2. Files matching `.gitignore` patterns are excluded.
3. Type part of a filename (or path) to filter the list.
4. Files are ranked by a fuzzy matching score -- the best matches appear at the top of the list.
5. The query is matched against the relative file path from the project root.

### Typing to Filter

As you type characters, the file list updates in real-time. The matching is fuzzy, meaning the characters you type do not need to be contiguous in the filename. For example, typing `mrs` would match `main.rs`.

### Score-Based Ranking

Files are sorted by how well they match the query:

- Consecutive character matches score higher.
- Matches at the start of a filename or path segment score higher.
- Shorter paths with more matching characters rank higher.

### Opening the Selected File

- Use `Up`/`Down` or `Ctrl+K`/`Ctrl+J` to move the selection highlight.
- Press `Enter` to open the selected file. If the file is already open in a tab, the existing tab is activated.
- Press `Esc` to close the finder without opening a file.

### .gitignore Awareness

Like the file explorer, the fuzzy finder respects `.gitignore` files and excludes ignored files from the list.

---

## 11. Command Palette

### Opening the Command Palette

Press `Ctrl+Shift+P` to open the command palette. An overlay popup appears listing all available commands.

### Available Commands

The following commands are registered in the command palette:

| Command ID               | Display Name         | Description                                                |
| ------------------------ | -------------------- | ---------------------------------------------------------- |
| `file.save`              | Save File            | Save the current file to disk                              |
| `edit.delete_char`       | Delete Character     | Delete the character under the cursor                      |
| `edit.backspace`         | Backspace            | Delete the character before the cursor                     |
| `edit.newline`           | Insert Newline       | Insert a newline at the cursor                             |
| `edit.undo`              | Undo                 | Undo the last editing operation                            |
| `edit.redo`              | Redo                 | Redo the last undone operation                             |
| `cursor.up`              | Cursor Up            | Move cursor up one line                                    |
| `cursor.down`            | Cursor Down          | Move cursor down one line                                  |
| `cursor.left`            | Cursor Left          | Move cursor left one character                             |
| `cursor.right`           | Cursor Right         | Move cursor right one character                            |
| `cursor.page_up`         | Page Up              | Scroll and move cursor up one page                         |
| `cursor.page_down`       | Page Down            | Scroll and move cursor down one page                       |
| `cursor.home`            | Go to Beginning      | Move cursor to the beginning of the file                   |
| `cursor.end`             | Go to End            | Move cursor to the end of the file                         |
| `mode.insert`            | Enter Insert Mode    | Switch to Insert mode                                      |
| `mode.normal`            | Enter Normal Mode    | Switch to Normal mode                                      |
| `tab.next`               | Next Tab             | Switch to the next tab                                     |
| `tab.prev`               | Previous Tab         | Switch to the previous tab                                 |
| `view.toggle_sidebar`    | Toggle Sidebar       | Show/hide the file explorer sidebar                        |
| `search.open`            | Find                 | Open search                                                |
| `search.open_replace`    | Find and Replace     | Open search and replace                                    |
| `search.next`            | Find Next            | Jump to next search match                                  |
| `search.prev`            | Find Previous        | Jump to previous search match                              |
| `search.replace_current` | Replace              | Replace current match                                      |
| `search.replace_all`     | Replace All          | Replace all matches                                        |
| `search.close`           | Close Search         | Close the search bar                                       |
| `fuzzy.open`             | Open File            | Open fuzzy file finder                                     |
| `fuzzy.close`            | Close Finder         | Close fuzzy file finder                                    |
| `palette.open`           | Command Palette      | Open the command palette                                   |
| `palette.close`          | Close Palette        | Close the command palette                                  |
| `diagnostic.next`        | Next Diagnostic      | Jump to next LSP diagnostic                                |
| `diagnostic.prev`        | Previous Diagnostic  | Jump to previous LSP diagnostic                            |
| `goto.definition`        | Go to Definition     | Go to definition (LSP)                                     |
| `lsp.hover`              | Show Hover Info      | Show hover information (LSP)                               |
| `lsp.trigger_completion` | Trigger Completion   | Manually trigger autocomplete (LSP)                        |
| `clipboard.copy`         | Copy to Clipboard    | Copy selection to system clipboard                         |
| `clipboard.cut`          | Cut to Clipboard     | Cut selection to system clipboard                          |
| `clipboard.paste`        | Paste from Clipboard | Paste from system clipboard                                |
| `theme.list`             | Select Theme         | Open theme selection palette                               |
| `line_numbers.toggle`    | Toggle Line Numbers  | Cycle: absolute -> relative -> relative+absolute -> hidden |

### Theme Switching Flow

1. Open the command palette with `Ctrl+Shift+P`.
2. Type "theme" to filter, then select "Select Theme" and press `Enter`.
3. A secondary palette opens listing all available themes (e.g., "one-dark", "gruvbox-dark", "catppuccin-mocha").
4. Type to filter themes, use `Up`/`Down` to navigate, and press `Enter` to apply the selected theme.
5. The theme is applied immediately and a confirmation message appears in the status bar.

### Fuzzy Filtering

The command palette uses fuzzy matching on command display names. Type any portion of the command name to narrow the list. For example, typing "save" will match "Save File", and typing "undo" will match "Undo".

---

## 12. LSP Integration

### What is LSP

The Language Server Protocol (LSP) is a standard protocol for communication between code editors and language servers. Language servers provide features like autocomplete, diagnostics, hover info, and go-to-definition for a specific programming language. termcode includes a built-in LSP client that communicates with external language servers.

### Configuring Language Servers

Language servers are configured in your `config.toml` file using `[[lsp]]` array sections:

```toml
[[lsp]]
language = "rust"
command = "rust-analyzer"
args = []

[[lsp]]
language = "python"
command = "pylsp"
args = []

[[lsp]]
language = "typescript"
command = "typescript-language-server"
args = ["--stdio"]
```

Each entry specifies:

- `language`: The language identifier (must match the language ID from the built-in language registry, e.g., "rust", "python", "typescript").
- `command`: The executable name of the language server. It must be available on your PATH.
- `args`: An array of command-line arguments to pass to the server (default: empty).

**Important**: You must install the language servers yourself. termcode only launches and communicates with them. Common language servers:

| Language   | Server                     | Install Command                                  |
| ---------- | -------------------------- | ------------------------------------------------ |
| Rust       | rust-analyzer              | `rustup component add rust-analyzer`             |
| Python     | pylsp                      | `pip install python-lsp-server`                  |
| TypeScript | typescript-language-server | `npm i -g typescript-language-server typescript` |
| Go         | gopls                      | `go install golang.org/x/tools/gopls@latest`     |
| C/C++      | clangd                     | Available via system package manager             |

### Supported Features

#### Autocomplete

- **Automatic trigger**: When you type a trigger character (e.g., `.` or `::` in Rust), the autocomplete popup appears automatically.
- **Manual trigger**: Press `Ctrl+Space` in Insert mode to request completions at the current position.
- **Popup navigation**: Use `Up`/`Down` to move through the list, `Enter`/`Tab` to accept, `Esc` to dismiss.
- **Insert text**: The selected completion's insert text replaces the text at the cursor position.
- The popup automatically dismisses when you press any key other than navigation keys.

#### Diagnostics

Diagnostics are errors, warnings, info messages, and hints reported by the language server.

- **Gutter icons**: An icon appears in the gutter next to lines that have diagnostics. The icon color reflects the severity (error=red, warning=yellow, info=blue, hint=cyan).
- **Inline underlines**: Diagnostic ranges in the code are underlined with the corresponding severity color.
- **Navigation**: Press `]` in Normal mode to jump to the next diagnostic. Press `[` to jump to the previous diagnostic. The diagnostic message appears in the status bar.
- **Wrap-around**: When navigating diagnostics, if you pass the last diagnostic, navigation wraps to the first one (and vice versa).
- **Status bar count**: The total number of errors and warnings is shown in the status bar.

#### Hover Information

- Press `Shift+K` in Normal mode to request hover information at the cursor position.
- If the server returns hover data, a tooltip appears near the cursor showing type information, documentation, or other details.
- The hover tooltip is dismissed when you press any key.

#### Go to Definition

- Press `F12` or `Ctrl+D` in Normal mode to jump to the definition of the symbol under the cursor.
- If the definition is in the current file, the cursor jumps to it.
- If the definition is in another file, that file is opened in a new tab and the cursor is placed at the definition location.

### LSP Document Lifecycle

termcode manages the LSP document lifecycle automatically:

1. **didOpen**: Sent when a file is opened in the editor. If the language server for that language is not yet running, it is started first.
2. **didChange**: Sent after every editing operation (character insertion, deletion, paste, replace). Uses full document sync with 100ms debouncing.
3. **didSave**: Sent when the file is saved with `Ctrl+S`.
4. **didClose**: Sent when a tab is closed with `Ctrl+W`.

The LSP client uses asynchronous communication via tokio, with JSON-RPC 2.0 over stdin/stdout transport.

---

## 13. Configuration

### Config File Location

termcode looks for configuration in the following locations:

1. **User config**: `~/.config/termcode/config.toml` (on macOS/Linux)
2. **Project config**: `config/config.toml` in the project root (for development)

If no configuration file is found, default values are used for all settings. If a configuration file exists but contains parse errors, a warning is logged and defaults are used.

### Complete Configuration Reference

Below is a fully annotated configuration file with all options and their defaults:

```toml
# Theme name. Must match a .toml file in the runtime/themes/ directory.
# Built-in options: "one-dark", "gruvbox-dark", "catppuccin-mocha"
theme = "one-dark"

[editor]
# Number of spaces per tab stop.
# Default: 4
tab_size = 4

# Insert spaces instead of tab characters when pressing Tab.
# Default: true
insert_spaces = true

# Automatically save files (not yet implemented in UI).
# Default: false
auto_save = false

# Delay in milliseconds for auto-save (used with auto_save).
# Default: 1000
# auto_save_delay_ms = 1000

# Enable word wrapping (reserved for future use).
# Default: false
word_wrap = false

# Line number display style.
# Options: "absolute", "relative", "relative_absolute", "none"
#   absolute          - Shows absolute line numbers (1, 2, 3, ...)
#   relative          - Shows distance from current line (2, 1, 0, 1, 2, ...)
#   relative_absolute - Shows relative numbers except current line shows absolute
#   none              - Hides line numbers entirely
# Default: "absolute"
line_numbers = "absolute"

# Number of lines to keep visible above/below the cursor when scrolling.
# Default: 5
scroll_off = 5

# Enable mouse support (click, scroll, drag).
# Default: true
mouse_enabled = true

[ui]
# Width of the file explorer sidebar in columns.
# Default: 30
sidebar_width = 30

# Whether the sidebar is visible on startup.
# Default: true
sidebar_visible = true

# Show minimap (reserved for future use).
# Default: false
show_minimap = false

# Show the tab bar.
# Default: true
show_tab_bar = true

# Show the top bar (title bar).
# Default: true
show_top_bar = true

# LSP server configurations.
# Add one [[lsp]] section per language server.

# [[lsp]]
# language = "rust"
# command = "rust-analyzer"
# args = []

# [[lsp]]
# language = "python"
# command = "pylsp"
# args = []

# [[lsp]]
# language = "typescript"
# command = "typescript-language-server"
# args = ["--stdio"]
```

### Editor Options Detail

| Option               | Type    | Default      | Description                     |
| -------------------- | ------- | ------------ | ------------------------------- |
| `tab_size`           | integer | `4`          | Number of spaces per tab stop   |
| `insert_spaces`      | boolean | `true`       | Use spaces instead of tabs      |
| `auto_save`          | boolean | `false`      | Enable auto-save                |
| `auto_save_delay_ms` | integer | `1000`       | Auto-save delay in milliseconds |
| `word_wrap`          | boolean | `false`      | Enable word wrapping (future)   |
| `line_numbers`       | string  | `"absolute"` | Line number style               |
| `scroll_off`         | integer | `5`          | Scroll margin in lines          |
| `mouse_enabled`      | boolean | `true`       | Enable mouse support            |

### UI Options Detail

| Option            | Type    | Default | Description                |
| ----------------- | ------- | ------- | -------------------------- |
| `sidebar_width`   | integer | `30`    | Sidebar width in columns   |
| `sidebar_visible` | boolean | `true`  | Sidebar visible on startup |
| `show_minimap`    | boolean | `false` | Show minimap (future)      |
| `show_tab_bar`    | boolean | `true`  | Show the tab bar           |
| `show_top_bar`    | boolean | `true`  | Show the top bar           |

### LSP Server Configuration

Each `[[lsp]]` section configures one language server:

| Field      | Type         | Required | Description                          |
| ---------- | ------------ | -------- | ------------------------------------ |
| `language` | string       | Yes      | Language identifier (e.g., "rust")   |
| `command`  | string       | Yes      | Server executable name               |
| `args`     | string array | No       | Command-line arguments (default: []) |

---

## 14. Keybinding Customization

### Keybinding File Location

Custom keybindings are defined in:

```
~/.config/termcode/keybindings.toml
```

For development, the project-local file `config/keybindings.toml` is also supported.

### File Format

The keybinding file is a TOML file with sections for global bindings and per-mode bindings:

```toml
# Global keybindings (active in all modes)
[global]
"ctrl+d" = "goto.definition"

# Normal mode keybindings
[mode.normal]
"g" = "cursor.home"
"shift+g" = "cursor.end"

# Insert mode keybindings
[mode.insert]

# File Explorer mode keybindings
[mode.file_explorer]

# Search mode keybindings
[mode.search]

# Fuzzy Finder mode keybindings
[mode.fuzzy_finder]

# Command Palette mode keybindings
[mode.command_palette]
```

Each entry is a key-value pair where:

- **Key** (left side): A quoted string describing the key combination.
- **Value** (right side): A quoted string containing the command ID to execute.

### Key Combo Syntax

Key combinations are written as modifier names joined by `+` with the key name at the end:

```
[modifier+[modifier+]]key
```

**Available modifier keys**:

- `ctrl` (or `control`)
- `alt`
- `shift`

**Available key names**:

- Letters: `a` through `z` (always lowercase in the combo string)
- Numbers: `0` through `9`
- Special keys: `enter` (or `return`), `esc` (or `escape`), `backspace`, `delete` (or `del`), `tab`, `space`
- Arrow keys: `up`, `down`, `left`, `right`
- Navigation: `pageup`, `pagedown`, `home`, `end`
- Function keys: `f1` through `f12`

**Examples**:

```toml
"ctrl+s"         # Ctrl + S
"ctrl+shift+p"   # Ctrl + Shift + P
"alt+left"       # Alt + Left arrow
"f12"            # F12 function key
"enter"          # Enter key
"shift+g"        # Shift + G (uppercase G)
"ctrl+k"         # Ctrl + K
```

**Note on Shift+letter**: When using `shift+` with a letter key, the parser automatically converts the letter to uppercase internally. Write `"shift+g"` in the config, and it will match pressing Shift+G.

**Note on `+` key**: The literal `+` key cannot be directly bound using this syntax. To bind a plus key, use `"shift+="` instead.

### Override Examples

```toml
[global]
# Remap Ctrl+D to toggle sidebar instead of go-to-definition
"ctrl+d" = "view.toggle_sidebar"

# Add Ctrl+Shift+S as an additional save binding
"ctrl+shift+s" = "file.save"

[mode.normal]
# Use 'o' to enter insert mode (in addition to 'i')
# Note: this adds a new binding; existing 'i' binding is unchanged
"o" = "mode.insert"

# Override 'g' to mean something else
"g" = "cursor.end"

[mode.insert]
# Use Ctrl+Backspace for undo
"ctrl+backspace" = "edit.undo"
```

### Validation

When keybinding overrides are loaded:

1. The key combo string is parsed. If it is invalid (e.g., `"ctrl+"`, `""`), a warning is logged and the binding is **skipped**.
2. The command ID is validated against the command registry. If the command does not exist (e.g., `"nonexistent.command"`), a warning is logged and the binding is **skipped**.
3. If the key combo already has a binding, the existing binding is **replaced** with the new command.
4. If the key combo does not have an existing binding, a **new** binding is added.

Invalid bindings are silently skipped at runtime -- they do not prevent the editor from starting.

---

## 15. Themes

### Built-in Themes

termcode ships with three built-in themes:

| Theme Name       | File                                   | Description                |
| ---------------- | -------------------------------------- | -------------------------- |
| One Dark         | `runtime/themes/one-dark.toml`         | Atom-inspired dark theme   |
| Gruvbox Dark     | `runtime/themes/gruvbox-dark.toml`     | Retro groove dark theme    |
| Catppuccin Mocha | `runtime/themes/catppuccin-mocha.toml` | Soothing pastel dark theme |

The default theme is **One Dark**.

### Theme File Format

Theme files are TOML files with four sections: `[meta]`, `[palette]`, `[ui]`, and `[scopes]`.

```toml
[meta]
name = "My Custom Theme"

[palette]
# Named colors that can be referenced in [ui] and [scopes]
bg = "#282c34"
fg = "#abb2bf"
red = "#e06c75"
green = "#98c379"
yellow = "#e5c07b"
blue = "#61afef"
magenta = "#c678dd"
cyan = "#56b6c2"
gutter = "#4b5263"
comment = "#5c6370"

[ui]
# UI element colors. Values can be:
#   - A palette name (e.g., "bg", "fg", "red")
#   - A hex color string (e.g., "#282c34")
background = "bg"
foreground = "fg"
cursor = "#528bff"
selection = "#3e4451"
cursor_line_bg = "#2c3038"
line_number = "gutter"
line_number_active = "fg"
status_bar_bg = "#21252b"
status_bar_fg = "fg"
tab_active_bg = "bg"
tab_inactive_bg = "#21252b"
sidebar_bg = "#21252b"
sidebar_fg = "fg"
border = "#181a1f"
error = "red"
warning = "yellow"
info = "blue"
hint = "cyan"
search_match = "yellow"
search_match_active = "#d19a66"

[scopes]
# Syntax highlighting scopes.
# Each entry maps a scope name to a style definition.
# Style properties: fg, bg (color values), bold, italic, underline, strikethrough (booleans)
"keyword" = { fg = "magenta" }
"comment" = { fg = "comment", italic = true }
"string" = { fg = "green" }
"function" = { fg = "blue" }
"type" = { fg = "yellow" }
"variable" = { fg = "red" }
"constant" = { fg = "cyan" }
"operator" = { fg = "cyan" }
```

### Creating Custom Themes

1. Create a new `.toml` file in the `runtime/themes/` directory (next to the binary, or in the project root).
2. Follow the theme file format described above.
3. The filename (without `.toml` extension) becomes the theme identifier used in `config.toml` and the theme palette.
4. Switch to your theme by setting `theme = "my-theme-name"` in `config.toml`, or by using the command palette at runtime.

### Runtime Theme Switching

You can switch themes without restarting the editor:

1. Press `Ctrl+Shift+P` to open the command palette.
2. Select "Select Theme" (or type "theme" to filter).
3. Choose a theme from the list and press `Enter`.
4. The theme is applied immediately.

### All UI Color Slots

The `[ui]` section supports the following color slots:

| Slot                  | Description                                   |
| --------------------- | --------------------------------------------- |
| `background`          | Main editor background color                  |
| `foreground`          | Default text color                            |
| `cursor`              | Cursor color                                  |
| `selection`           | Background color for selected text            |
| `cursor_line_bg`      | Background highlight for the current line     |
| `line_number`         | Color for non-active line numbers             |
| `line_number_active`  | Color for the current line number             |
| `status_bar_bg`       | Status bar background                         |
| `status_bar_fg`       | Status bar text color                         |
| `tab_active_bg`       | Active tab background                         |
| `tab_inactive_bg`     | Inactive tab background                       |
| `sidebar_bg`          | File explorer sidebar background              |
| `sidebar_fg`          | File explorer sidebar text color              |
| `border`              | Border/separator color                        |
| `error`               | Error diagnostic color                        |
| `warning`             | Warning diagnostic color                      |
| `info`                | Info diagnostic color                         |
| `hint`                | Hint diagnostic color                         |
| `search_match`        | Background color for search matches           |
| `search_match_active` | Background color for the current search match |

### All Syntax Scope Names

The `[scopes]` section maps syntax scope names to styles. Scope resolution uses hierarchical fallback: if `"function.macro"` is not defined, termcode falls back to `"function"`, then to the default foreground color.

The following scopes are used by the built-in One Dark theme and are recognized by the highlighter:

| Scope Name                  | Typical Usage                    |
| --------------------------- | -------------------------------- |
| `attribute`                 | Attributes / decorators          |
| `comment`                   | Comments                         |
| `constant`                  | Constants                        |
| `constant.numeric`          | Numeric literals                 |
| `constant.character`        | Character literals               |
| `constant.character.escape` | Escape sequences in strings      |
| `constructor`               | Constructors                     |
| `function`                  | Function names                   |
| `function.macro`            | Macro invocations                |
| `function.builtin`          | Built-in functions               |
| `keyword`                   | Keywords                         |
| `keyword.control`           | Control flow keywords            |
| `keyword.control.return`    | Return keyword                   |
| `keyword.control.import`    | Import/use keywords              |
| `keyword.operator`          | Operator keywords (and, or, not) |
| `keyword.function`          | Function definition keyword (fn) |
| `label`                     | Labels / lifetimes               |
| `namespace`                 | Namespaces / modules             |
| `operator`                  | Operators (+, -, \*, etc.)       |
| `punctuation`               | General punctuation              |
| `punctuation.bracket`       | Brackets ((), [], {})            |
| `punctuation.delimiter`     | Delimiters (commas, semicolons)  |
| `string`                    | String literals                  |
| `string.special`            | Special strings (regex, etc.)    |
| `type`                      | Type names                       |
| `type.builtin`              | Built-in types (i32, str, etc.)  |
| `variable`                  | Variable names                   |
| `variable.builtin`          | Built-in variables (self, this)  |
| `variable.parameter`        | Function parameters              |
| `tag`                       | HTML/XML tags                    |
| `special`                   | Special tokens                   |

### Style Properties

Each scope style supports the following properties:

| Property        | Type    | Description                            |
| --------------- | ------- | -------------------------------------- |
| `fg`            | string  | Foreground color (palette name or hex) |
| `bg`            | string  | Background color (palette name or hex) |
| `bold`          | boolean | Bold text (default: false)             |
| `italic`        | boolean | Italic text (default: false)           |
| `underline`     | boolean | Underlined text (default: false)       |
| `strikethrough` | boolean | Strikethrough text (default: false)    |

---

## 16. Session Management

### Auto-Save on Exit

When you quit termcode (via `Ctrl+Q` or `Ctrl+C`), the current session state is automatically saved. This includes all open tabs and their cursor positions.

### Auto-Restore on Startup

When you launch termcode without specifying a file (i.e., opening a directory or no arguments), the previous session for that project root is automatically restored. Each file from the session is re-opened with its saved cursor position, and the previously active tab is re-selected.

### Session File Location

Session files are stored in:

```
~/.config/termcode/sessions/
```

Each session file is named using a deterministic FNV-1a hash of the canonicalized project root path, producing a filename like `<16-hex-digit-hash>.json`. This means each project directory has its own separate session.

### What Is Saved

A session file (JSON format) contains:

| Field                   | Description                                 |
| ----------------------- | ------------------------------------------- |
| `root`                  | Absolute path of the project root directory |
| `files`                 | Array of open files, each with:             |
| `files[].path`          | Absolute path to the file                   |
| `files[].cursor_line`   | Cursor line number (0-indexed)              |
| `files[].cursor_column` | Cursor column number (0-indexed)            |
| `active_tab`            | Index of the active tab (0-indexed)         |

### Behavior with Missing Files

On session restore, each file path is checked for existence. Files that no longer exist on disk are silently skipped. If all files in the session have been deleted, the session is not restored (the editor starts empty).

---

## 17. Mouse Support

Mouse support is enabled by default and can be disabled by setting `mouse_enabled = false` in `config.toml`.

### Click to Place Cursor

Left-clicking in the editor area moves the cursor to the clicked position. The click position is mapped to the correct line and column, accounting for the gutter width and scroll offset. If you click past the end of a line, the cursor is placed at the last character of that line.

Clicking in the editor area also switches the mode to Normal (or stays in Insert if already in Insert mode).

### Click on Tabs to Switch

Left-clicking on a tab in the tab bar switches to that tab. The click is matched against the computed tab label positions.

### Click on File Explorer to Select/Open

Left-clicking on an item in the file explorer sidebar:

- Selects the clicked item.
- Switches to File Explorer mode.
- Opens the item (if a file, it opens in the editor; if a directory, it toggles expansion).

### Scroll Wheel

- **Scroll up**: Scrolls the view up by 3 lines.
- **Scroll down**: Scrolls the view down by 3 lines.

Scrolling does not move the cursor -- only the viewport.

### Drag to Select Text

Left-clicking and dragging in the editor area creates a text selection:

- The selection anchor is set at the initial click position.
- As you drag, the selection extends to the current mouse position.
- The selected text is highlighted with the theme's selection color.
- After dragging, you can copy the selection with `Ctrl+C` or cut it with `Ctrl+X`.

### Click on Line Numbers

Left-clicking on a line number in the gutter selects the entire line. The selection spans from the start of the clicked line to the start of the next line.

---

## 18. Troubleshooting

### LSP Server Not Starting

**Symptoms**: No autocomplete, no diagnostics, no hover info.

**Solutions**:

1. **Verify the server is installed**: Run the server command directly in your terminal (e.g., `rust-analyzer --version`). If it is not found, install it.
2. **Check your config**: Ensure you have an `[[lsp]]` section in your `config.toml` with the correct `language` and `command` values.
3. **Verify the language ID matches**: The `language` field must exactly match termcode's built-in language identifiers: `rust`, `python`, `javascript`, `typescript`, `go`, `c`, `cpp`, `toml`, `json`, `markdown`.
4. **Check the status bar**: When a server starts successfully, a message like "LSP: rust server started" appears. Error messages also appear in the status bar.
5. **PATH issues**: The language server executable must be on your PATH. If installed in a non-standard location, use the full path in the `command` field.

### Clipboard Not Working

**Symptoms**: Copy/paste operations show "Clipboard unavailable" error.

**Solutions**:

1. **macOS**: The clipboard should work out of the box via the `arboard` library.
2. **Linux**: Ensure you have `xclip` or `xsel` installed (for X11), or `wl-copy`/`wl-paste` (for Wayland). The `arboard` crate requires these system utilities.
3. **SSH sessions**: Clipboard access is typically not available in SSH sessions. Consider using terminal-level clipboard integration (e.g., OSC 52 if your terminal supports it).

### Theme Not Loading

**Symptoms**: Editor uses default colors, or an error message about theme loading appears.

**Solutions**:

1. **Verify the theme file exists**: Check that a `.toml` file with the theme name exists in `runtime/themes/` (e.g., `runtime/themes/one-dark.toml`).
2. **Check for TOML syntax errors**: Ensure the theme file is valid TOML. Common issues include missing quotes around strings or invalid hex color codes.
3. **Hex color format**: Colors must be in `#rrggbb` format (6 hex digits, preceded by `#`). Short forms like `#fff` are not supported.
4. **Runtime directory location**: termcode looks for themes in a `runtime/` directory next to the binary, then falls back to `runtime/` in the current working directory. Ensure themes are in the correct location.

### Session Not Restoring

**Symptoms**: Editor starts empty even though you had files open previously.

**Solutions**:

1. **Same directory**: Sessions are tied to the project root directory. Make sure you are launching termcode from the same directory (or with the same path argument) as before.
2. **File specified on command line**: Sessions are only restored when no file is specified. Running `termcode src/main.rs` opens that specific file instead of restoring the session.
3. **Files deleted**: If all files from the saved session no longer exist on disk, the session is discarded.
4. **Permissions**: Ensure the sessions directory (`~/.config/termcode/sessions/`) is writable.

### Performance with Large Files

**Symptoms**: Slow rendering or high memory usage with very large files.

**Information**:

1. **Viewport-scoped rendering**: termcode only highlights and renders lines within the visible viewport, so scrolling through large files should remain responsive.
2. **Rope data structure**: The text buffer uses the `ropey` crate's Rope data structure, which provides O(log n) character and line access even for very large files.
3. **LSP didChange sync**: Document changes send the full document content to the language server. For very large files, this may cause a brief delay. The 100ms debounce helps mitigate rapid successive changes.
4. **Syntax highlighting**: The current implementation uses keyword-based highlighting (not tree-sitter incremental parsing), which may be slower for very large files with complex syntax.

### Editor Does Not Start

**Symptoms**: Error message or crash on startup.

**Solutions**:

1. **Terminal true color support**: Ensure your terminal emulator supports 24-bit (true color). Test with: `echo -e "\033[38;2;255;100;0mTRUE COLOR TEST\033[0m"` -- if the text appears orange, your terminal supports true color.
2. **Terminal size**: termcode requires a minimum terminal size to render. Ensure your terminal window is at least 40 columns wide and 10 rows tall.
3. **Path does not exist**: If you specify a path that does not exist, termcode will print an error and exit. Verify the path is correct.

---

## Appendix A: Architecture Overview

termcode is organized as an 8-crate Cargo workspace:

| Crate             | Purpose                                                                      |
| ----------------- | ---------------------------------------------------------------------------- |
| `termcode-core`   | Text buffer (Rope), position, selection, transaction, history, encoding      |
| `termcode-syntax` | Keyword-based syntax highlighting, language registry                         |
| `termcode-theme`  | Theme engine (TOML), style/color types, palette                              |
| `termcode-view`   | Document, View, Editor state, FileExplorer, Tab, Search, Fuzzy, Palette      |
| `termcode-config` | AppConfig, keybinding parser, default paths                                  |
| `termcode-lsp`    | LSP client (JSON-RPC, async transport, registry)                             |
| `termcode-plugin` | Plugin system stub (future: Lua via mlua)                                    |
| `termcode-term`   | TUI: app loop, event handling, widgets, rendering, mouse, clipboard, session |

The architecture follows The Elm Architecture (TEA): Event -> Update -> Render. No widget mutates state during rendering. All user actions route through a CommandRegistry using named command IDs.

---

## Appendix B: Version Information

termcode version information is available via:

```bash
termcode --version
```

---

_This manual covers termcode as of its current implementation. Features marked as "future" or "reserved" are planned but not yet available._
