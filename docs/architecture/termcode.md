# Termcode Architecture Blueprint

**Date**: 2026-03-27
**Architect**: yyy-architect
**Project**: termcode -- Terminal-based code viewer/editor with integrated file explorer

## 1. Architecture Decision

### Chosen Approach

**Layered workspace architecture** inspired by Helix editor's proven crate separation, adapted for termcode's unique file-explorer-first design. The project is organized as a Cargo workspace with 8 focused crates, using an event-driven architecture with The Elm Architecture (TEA) pattern for state management and rendering.

The key differentiator -- an integrated file explorer sidebar alongside a tabbed code editor in a single binary -- demands a layout engine that treats the sidebar and editor as co-equal first-class components, not an afterthought.

### Trade-offs Considered

| Option                                | Pros                                                          | Cons                                                  | Decision                         |
| ------------------------------------- | ------------------------------------------------------------- | ----------------------------------------------------- | -------------------------------- |
| Monolithic single crate               | Simple to start, less boilerplate                             | Poor compile times, tight coupling, hard to test      | Rejected: does not scale         |
| Workspace with 8 crates (Helix-style) | Clear boundaries, parallel compilation, testable in isolation | More initial setup, inter-crate API design needed     | **Selected**                     |
| Dynamic plugin-only arch (like Micro) | Maximum extensibility                                         | Complexity explosion, Lua perf overhead on core paths | Rejected: core must be fast Rust |

### Patterns Applied

- **TEA (The Elm Architecture)**: State update cycle of `Event -> Update -> View` keeps rendering pure and state transitions explicit
- **Command Pattern**: All user actions map to named commands, enabling keybinding remapping and plugin hooks
- **Observer/Event Bus**: Internal event system for decoupled communication between crates (e.g., file-explorer notifying editor of file open)
- **Rope Buffer**: O(log n) insertions/deletions for large files via the `ropey` crate
- **Immediate-mode rendering**: Ratatui's rendering model -- rebuild the entire UI each frame from current state

## 2. Project Structure

### Cargo Workspace Layout

```
termcode/
├── Cargo.toml                    # Workspace root + [package] for binary
├── Cargo.lock
├── README.md
├── CLAUDE.md                     # Project conventions
├── LICENSE
│
├── crates/
│   ├── termcode-core/            # Buffer, rope, selection, transaction primitives
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── buffer.rs         # Rope-based text buffer
│   │       ├── selection.rs      # Cursor and multi-selection
│   │       ├── transaction.rs    # Atomic edit operations, undo/redo
│   │       ├── history.rs        # Undo/redo DAG
│   │       ├── position.rs       # Line/column/byte position types
│   │       ├── diagnostic.rs    # Diagnostic type (owned by core, used by view/lsp)
│   │       ├── config_types.rs  # EditorConfig, LineNumberStyle (used by view, deserialized by config)
│   │       └── encoding.rs       # File encoding detection/conversion
│   │
│   ├── termcode-syntax/          # Tree-sitter integration, syntax highlighting
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── highlighter.rs    # Tree-sitter highlight engine
│   │       ├── language.rs       # Language registry and grammar loading
│   │       ├── query.rs          # Highlight query management
│   │       └── indent.rs         # Tree-sitter based auto-indent
│   │
│   ├── termcode-view/            # Document, view, editor state (frontend-agnostic)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── document.rs       # Document = buffer + syntax + history + metadata
│   │       ├── view.rs           # Single editor viewport (scroll, gutter)
│   │       ├── editor.rs         # Global editor state: all documents, views, config
│   │       ├── tab.rs            # Tab management
│   │       ├── pane.rs           # Split pane tree (horizontal/vertical)
│   │       ├── file_explorer.rs  # File tree model: nodes, expand/collapse state
│   │       ├── clipboard.rs      # System clipboard abstraction
│   │       └── register.rs       # Yank/paste registers
│   │
│   ├── termcode-lsp/             # Language Server Protocol client
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── client.rs         # LSP client per language server
│   │       ├── transport.rs      # JSON-RPC stdio transport
│   │       ├── registry.rs       # Running server registry
│   │       └── types.rs          # LSP type wrappers
│   │
│   ├── termcode-theme/           # Theme engine: parsing, color resolution
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── theme.rs          # Theme struct and color mapping
│   │       ├── loader.rs         # TOML theme file parser
│   │       ├── style.rs          # Resolved Style type (fg, bg, modifiers)
│   │       └── palette.rs        # Named color palette support
│   │
│   ├── termcode-plugin/          # Lua plugin runtime
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── runtime.rs        # Lua VM lifecycle, sandboxing
│   │       ├── api.rs            # Rust functions exposed to Lua
│   │       ├── loader.rs         # Plugin discovery and loading
│   │       ├── event.rs          # Plugin hook/event registration
│   │       └── types.rs          # Lua <-> Rust type conversions
│   │
│   ├── termcode-term/            # Terminal UI: widgets, rendering, event loop
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── app.rs            # Main application struct, TEA loop
│   │       ├── event.rs          # Event types and crossterm event polling
│   │       ├── input.rs          # Key event -> Command mapping
│   │       ├── command.rs        # Command registry and execution
│   │       ├── layout.rs         # Top-level layout engine (sidebar + editor + bars)
│   │       ├── ui/
│   │       │   ├── mod.rs
│   │       │   ├── editor_view.rs    # Code editor widget
│   │       │   ├── file_explorer.rs  # File tree sidebar widget
│   │       │   ├── tab_bar.rs        # Tab strip widget
│   │       │   ├── status_bar.rs     # Bottom status bar widget
│   │       │   ├── top_bar.rs        # Top info/menu bar widget
│   │       │   ├── command_palette.rs # Ctrl+Shift+P overlay
│   │       │   ├── fuzzy_finder.rs   # Ctrl+P file picker overlay
│   │       │   ├── search.rs         # Search/replace overlay
│   │       │   ├── popup.rs          # Generic popup/menu component
│   │       │   └── minimap.rs        # Code minimap widget
│   │       └── render.rs         # Frame rendering orchestration
│   │
│   └── termcode-config/          # Configuration loading and schema
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           ├── config.rs         # Main config struct
│           ├── keymap.rs         # Keybinding configuration
│           ├── loader.rs         # TOML config file loading
│           └── default.rs        # Default configuration values
│
├── src/
│   └── main.rs                   # Binary entrypoint: CLI args, launch app
│
├── runtime/                      # Runtime resources (shipped with binary or installed)
│   ├── themes/
│   │   ├── default.toml
│   │   ├── gruvbox.toml
│   │   ├── catppuccin.toml
│   │   └── one-dark.toml
│   ├── grammars/                 # Compiled tree-sitter grammars (.so/.dylib)
│   ├── queries/                  # Tree-sitter highlight queries per language
│   │   ├── rust/
│   │   │   └── highlights.scm
│   │   ├── python/
│   │   ├── javascript/
│   │   ├── typescript/
│   │   └── ...
│   └── plugins/                  # Built-in Lua plugins
│       └── example/
│           └── init.lua
│
├── config/                       # Default config (copied to ~/.config/termcode/)
│   ├── config.toml               # Main configuration
│   └── keybindings.toml          # Keybinding overrides
│
├── docs/
│   ├── architecture/
│   │   └── termcode.md           # This file
│   ├── specs/
│   └── analysis/
│
├── tests/                        # Integration tests
│   ├── integration/
│   └── fixtures/
│
└── benches/                      # Performance benchmarks
    └── buffer_bench.rs
```

### Root Cargo.toml (Workspace + Binary)

```toml
[package]
name = "termcode"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
repository.workspace = true
default-run = "termcode"

[[bin]]
name = "termcode"
path = "src/main.rs"

[dependencies]
termcode-term = { path = "crates/termcode-term" }
anyhow.workspace = true
clap = { version = "4", features = ["derive"] }

[workspace]
resolver = "2"
members = [
    ".",
    "crates/termcode-core",
    "crates/termcode-syntax",
    "crates/termcode-view",
    "crates/termcode-lsp",
    "crates/termcode-theme",
    "crates/termcode-plugin",
    "crates/termcode-term",
    "crates/termcode-config",
]

[workspace.package]
version = "0.1.0"
edition = "2024"
rust-version = "1.85"
license = "MIT"
repository = "https://github.com/user/termcode"

[workspace.dependencies]
# Core
ropey = "1.6"
tree-sitter = "0.24"
tree-sitter-highlight = "0.24"

# TUI
ratatui = "0.29"
crossterm = "0.28"

# Async
tokio = { version = "1", features = ["full"] }

# Plugin
mlua = { version = "0.10", features = ["lua54", "vendored", "async", "serialize"] }

# Config & Serialization
toml = "0.8"
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# Utilities
anyhow = "1"
thiserror = "2"
log = "0.4"
env_logger = "0.11"
once_cell = "1"
parking_lot = "0.12"
nucleo = "0.5"              # Fuzzy matching (from Helix)
globset = "0.4"
ignore = "0.4"              # .gitignore-aware file walking
arboard = "3"               # System clipboard
encoding_rs = "0.8"         # Encoding detection

# LSP
lsp-types = "0.97"

# Git
gix = { version = "0.68", default-features = false, features = ["status"] }
```

## 3. Crate Dependency Graph

```
Layer 0 (leaf, no internal deps):
  ┌──────────────┐   ┌──────────────┐
  │ termcode-    │   │ termcode-    │
  │ core         │   │ theme        │
  │ (rope, pos)  │   │ (style, TOML)│
  └──────┬───────┘   └──────┬───────┘
         │                  │
Layer 1 (depends on Layer 0):
  ┌──────▼──────────────────▼──┐   ┌──────▼──────────────┐
  │ termcode-syntax            │   │ termcode-config     │
  │ (tree-sitter, highlighting)│   │ (AppConfig, keymap) │
  │ deps: core, theme          │   │ deps: core, theme   │
  └─────────────┬──────────────┘   └──────┬──────────────┘
                │                         │
Layer 2 (depends on Layer 0-1):           │
  ┌──────────────────────────────┐  ┌─────▼──────────────┐
  │ termcode-view                │  │ termcode-lsp       │
  │ (Document, Editor, Tab, Pane)│  │ (LSP client)       │
  │ deps: core, syntax, theme   │  │ deps: core, config  │
  └─────────────┬────────────────┘  └────────┬───────────┘
                │                            │
Layer 3 (depends on Layer 0-2):              │
  ┌─────────────▼────────┐  ┌───────────────▼┐
  │ termcode-plugin      │  │ termcode-term   │
  │ deps: view           │  │ deps: ALL       │
  └──────────────────────┘  │ (owns LspRegistry,│
                            │  bridges lsp↔view) │
                            └────────┬───────────┘
                                     │
Layer 4 (binary):
                              ┌──────▼───────┐
                              │  termcode    │
                              │  (main.rs)   │
                              │  deps: term  │
                              └──────────────┘
```

Arrow direction: ▼ = "depends on" (points from consumer to dependency).

**Dependency rules** (strictly enforced):

- `termcode-core` depends on nothing internal (external crates: ropey, encoding_rs, serde for Deserialize on config_types)
- `termcode-theme` depends on nothing internal (only serde, toml)
- `termcode-syntax` depends on `termcode-core` (needs position types) and `termcode-theme` (needs style types)
- `termcode-lsp` depends on `termcode-core` and `termcode-config` (needs position types + LspServerConfig; NO dependency on view/document)
- `termcode-view` depends on `termcode-core`, `termcode-syntax`, `termcode-theme` (NOT lsp or config -- avoids cycles; EditorConfig is defined in core)
- `termcode-config` depends on `termcode-core`, `termcode-theme` (LspServerConfig and FileExplorerConfig are self-contained in config; EditorConfig lives in core)
- `termcode-plugin` depends on `termcode-view` (exposes editor state to Lua)
- `termcode-term` depends on all of the above; owns `LspRegistry` and bridges lsp ↔ view

**Note on LSP config types**: `LspServerConfig` is defined in `termcode-config` (not in `termcode-lsp`)
as a plain data struct. `termcode-lsp` depends on `termcode-config` to receive it. This keeps the
dependency direction: config → (nothing internal besides core/theme), lsp → core + config.

## 4. Component Design

### 4.1 termcode-core: Text Buffer Primitives

**Responsibility**: Rope-based text buffer with selection, transaction, and undo/redo -- zero UI awareness.

```rust
// crates/termcode-core/src/buffer.rs

/// A text buffer backed by a Rope for O(log n) edits on large files.
pub struct Buffer {
    text: Rope,
    encoding: Encoding,
    line_ending: LineEnding,
    modified: bool,
}

impl Buffer {
    pub fn from_reader<R: Read>(reader: R, encoding: Option<Encoding>) -> Result<Self>;
    pub fn text(&self) -> &Rope;
    pub fn line(&self, idx: usize) -> RopeSlice<'_>;
    pub fn line_count(&self) -> usize;
    pub fn byte_to_pos(&self, byte: usize) -> Position;
    pub fn pos_to_byte(&self, pos: Position) -> usize;
    pub fn apply(&mut self, transaction: &Transaction) -> Result<()>;
    pub fn is_modified(&self) -> bool;
}

// crates/termcode-core/src/position.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Position {
    pub line: usize,   // 0-indexed
    pub column: usize, // 0-indexed, grapheme cluster offset
}

// crates/termcode-core/src/selection.rs

/// A single selection range with anchor (fixed) and head (moves).
#[derive(Debug, Clone, Copy)]
pub struct Range {
    pub anchor: usize, // byte offset
    pub head: usize,   // byte offset
}

/// Multiple cursors/selections. Always contains at least one range.
#[derive(Debug, Clone)]
pub struct Selection {
    ranges: Vec<Range>,
    primary: usize, // index of the primary selection
}

impl Selection {
    pub fn point(pos: usize) -> Self;
    pub fn single(anchor: usize, head: usize) -> Self;
    pub fn primary(&self) -> Range;
    pub fn ranges(&self) -> &[Range];
    pub fn transform<F: FnMut(Range) -> Range>(&self, f: F) -> Self;
    pub fn map(&self, transaction: &Transaction) -> Self;
}

// crates/termcode-core/src/transaction.rs

/// An atomic set of text changes that can be applied or undone.
#[derive(Debug, Clone)]
pub struct Transaction {
    changes: ChangeSet,
    selection: Option<Selection>,
}

impl Transaction {
    pub fn insert(text: &str, pos: usize) -> Self;
    pub fn delete(range: std::ops::Range<usize>) -> Self;
    pub fn replace(range: std::ops::Range<usize>, text: &str) -> Self;
    pub fn compose(self, other: Transaction) -> Transaction;
    pub fn invert(&self, original: &Rope) -> Transaction;
    pub fn apply(&self, rope: &mut Rope) -> Result<()>;
}

// crates/termcode-core/src/history.rs

/// Branching undo/redo history.
pub struct History {
    revisions: Vec<Revision>,
    current: usize,
}

pub struct Revision {
    transaction: Transaction,
    inverse: Transaction,
    timestamp: Instant,
    parent: usize,
}

impl History {
    pub fn commit(&mut self, transaction: Transaction, original: &Rope);
    pub fn undo(&mut self) -> Option<&Transaction>;
    pub fn redo(&mut self) -> Option<&Transaction>;
}

// crates/termcode-core/src/diagnostic.rs

/// Diagnostic type lives in core so both view and lsp can use it
/// without creating a dependency between them.
#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub range: (Position, Position),
    pub severity: DiagnosticSeverity,
    pub message: String,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Info,
    Hint,
}

// crates/termcode-core/src/config_types.rs
//
// Editor option types live in core so termcode-view can use them
// without depending on termcode-config.
// termcode-config deserializes TOML into these types.

#[derive(Debug, Clone, Deserialize)]
pub struct EditorConfig {
    pub tab_size: usize,
    pub insert_spaces: bool,
    pub auto_save: bool,
    pub auto_save_delay_ms: u64,
    pub word_wrap: bool,
    pub line_numbers: LineNumberStyle,
    pub cursor_style: CursorStyle,
    pub scroll_off: usize,
    pub mouse_enabled: bool,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub enum LineNumberStyle {
    Absolute,
    Relative,
    RelativeAbsolute,
    None,
}
```

### 4.2 termcode-syntax: Tree-sitter Highlighting

**Responsibility**: Parse source code, produce highlight spans, manage language grammars.

```rust
// crates/termcode-syntax/src/highlighter.rs

use tree_sitter_highlight::{Highlighter as TsHighlighter, HighlightConfiguration, HighlightEvent};

/// Manages Tree-sitter parsing and highlighting for a single document.
pub struct SyntaxHighlighter {
    parser: tree_sitter::Parser,
    tree: Option<tree_sitter::Tree>,
    config: Arc<HighlightConfiguration>,
    language_id: LanguageId,
}

impl SyntaxHighlighter {
    pub fn new(language: &LanguageConfig) -> Result<Self>;
    /// Full parse (initial open).
    pub fn parse(&mut self, source: &Rope) -> Result<()>;
    /// Incremental re-parse after edits.
    pub fn update(&mut self, source: &Rope, edits: &[InputEdit]) -> Result<()>;
    /// Get highlight spans for a line range (for rendering visible viewport only).
    pub fn highlight_lines(&self, source: &Rope, range: std::ops::Range<usize>)
        -> Vec<HighlightSpan>;
}

#[derive(Debug, Clone)]
pub struct HighlightSpan {
    pub byte_range: std::ops::Range<usize>,
    pub highlight: HighlightName,  // e.g., "keyword", "function", "string"
}

// crates/termcode-syntax/src/language.rs

/// Arc<str> instead of &'static str: languages are loaded dynamically from
/// the runtime directory at startup, so IDs cannot be compile-time constants.
/// Arc<str> is cheap to clone and compare, and works as a HashMap key.
///
/// Convention: LanguageId values are always lowercase ASCII identifiers
/// (e.g., "rust", "python", "typescript"). This same convention applies to
/// LspServerConfig.language (String) in termcode-config. The bridge code in
/// termcode-term matches them via simple string equality (id.as_ref() == language).
pub type LanguageId = Arc<str>;

pub struct LanguageConfig {
    pub id: LanguageId,
    pub name: String,
    pub file_extensions: Vec<String>,
    pub grammar: tree_sitter::Language,
    pub highlight_query: String,
    pub indent_query: Option<String>,
}

/// Global registry of available languages.
pub struct LanguageRegistry {
    languages: HashMap<LanguageId, Arc<LanguageConfig>>,
    extension_map: HashMap<String, LanguageId>,
}

impl LanguageRegistry {
    pub fn load_from_runtime_dir(path: &Path) -> Result<Self>;
    pub fn detect_language(&self, path: &Path) -> Option<LanguageId>;
    pub fn get(&self, id: &str) -> Option<&Arc<LanguageConfig>>;
}
```

### 4.3 termcode-view: Editor State Model

**Responsibility**: The "model" layer -- all editor state, documents, views, tabs, the file explorer tree. Frontend-agnostic.

```rust
// crates/termcode-view/src/editor.rs

/// Top-level editor state. Single source of truth for the entire application.
/// Note: LspRegistry is NOT owned here -- it lives in App (termcode-term)
/// to avoid a circular dependency between termcode-view and termcode-lsp.
/// Diagnostic type is defined in termcode-core (no lsp dependency needed).
pub struct Editor {
    pub documents: SlotMap<DocumentId, Document>,
    pub pane_tree: PaneTree,
    pub file_explorer: FileExplorer,
    pub theme: Theme,
    pub config: EditorConfig,
    pub language_registry: Arc<LanguageRegistry>,
    pub registers: RegisterFile,
    pub clipboard: Box<dyn ClipboardProvider>,
    pub status_message: Option<(String, Severity)>,
    pub mode: EditorMode,
    pub search: SearchState,              // Search/replace state (termcode-view::search)
    pub fuzzy_finder: FuzzyFinderState,   // Fuzzy file finder state (termcode-view::fuzzy)
    pub command_palette: CommandPaletteState, // Command palette state (termcode-view::palette)
    pub exit_requested: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorMode {
    Normal,
    Insert,
    FileExplorer,
    Search,          // Ctrl+F / Ctrl+H search/replace overlay
    FuzzyFinder,     // Ctrl+P fuzzy file finder overlay
    CommandPalette,  // Ctrl+Shift+P command palette overlay
}

impl Editor {
    pub fn open_file(&mut self, path: &Path) -> Result<DocumentId>;
    pub fn save_document(&mut self, doc_id: DocumentId) -> Result<()>;
    pub fn close_document(&mut self, doc_id: DocumentId) -> Result<()>;
    pub fn active_document(&self) -> Option<&Document>;
    pub fn active_document_mut(&mut self) -> Option<&mut Document>;
    pub fn active_view(&self) -> Option<&View>;
    pub fn switch_theme(&mut self, theme_name: &str) -> Result<()>;
}

// crates/termcode-view/src/document.rs

pub struct Document {
    pub id: DocumentId,
    pub buffer: Buffer,
    pub path: Option<PathBuf>,
    pub selection: Selection,
    pub history: History,
    pub syntax: Option<SyntaxHighlighter>,
    pub language_id: Option<LanguageId>,
    pub diagnostics: Vec<Diagnostic>,
    pub last_saved_revision: usize,
}

// crates/termcode-view/src/view.rs

/// A viewport into a document (one pane in a split layout).
pub struct View {
    pub id: ViewId,
    pub doc_id: DocumentId,
    pub scroll: ScrollState,
    pub gutter_width: u16,
    pub area: Rect,            // Assigned during layout
}

pub struct ScrollState {
    pub top_line: usize,
    pub left_col: usize,
}

// crates/termcode-view/src/pane.rs

/// Binary tree of panes supporting horizontal and vertical splits.
pub enum PaneTree {
    Leaf(ViewId),
    Split {
        direction: SplitDirection,
        ratio: f32,
        left: Box<PaneTree>,
        right: Box<PaneTree>,
    },
}

#[derive(Debug, Clone, Copy)]
pub enum SplitDirection {
    Horizontal,
    Vertical,
}

impl PaneTree {
    pub fn active_view_id(&self) -> ViewId;
    pub fn split(&mut self, direction: SplitDirection, new_view: ViewId);
    pub fn close(&mut self, view_id: ViewId);
    pub fn layout(&self, area: Rect) -> Vec<(ViewId, Rect)>;
}

// crates/termcode-view/src/tab.rs

pub struct TabManager {
    tabs: Vec<Tab>,
    active: usize,
}

pub struct Tab {
    pub label: String,
    pub doc_id: DocumentId,
    pub modified: bool,
}

// crates/termcode-view/src/file_explorer.rs

/// File tree model for the sidebar.
pub struct FileExplorer {
    pub root: PathBuf,
    pub tree: Vec<FileNode>,
    pub selected: usize,
    pub visible: bool,
    pub width: u16,
}

pub struct FileNode {
    pub path: PathBuf,
    pub name: String,
    pub kind: FileNodeKind,
    pub depth: usize,
    pub expanded: bool,        // Only meaningful for directories
    pub git_status: Option<GitStatus>,
}

#[derive(Debug, Clone, Copy)]
pub enum FileNodeKind {
    File,
    Directory,
    Symlink,
}

#[derive(Debug, Clone, Copy)]
pub enum GitStatus {
    Modified,
    Added,
    Deleted,
    Untracked,
    Ignored,
    Clean,
}

impl FileExplorer {
    pub fn open(root: PathBuf) -> Result<Self>;
    pub fn toggle_expand(&mut self, index: usize) -> Result<()>;
    pub fn refresh(&mut self) -> Result<()>;
    pub fn selected_path(&self) -> Option<&Path>;
    pub fn move_selection(&mut self, delta: i32);
    pub fn flatten_visible(&self) -> Vec<&FileNode>;
}
```

### 4.4 termcode-theme: Theme Engine

**Responsibility**: Parse theme files, resolve highlight names to terminal styles.

```rust
// crates/termcode-theme/src/theme.rs

/// A fully resolved theme mapping highlight scopes to styles.
pub struct Theme {
    pub name: String,
    pub palette: Palette,
    pub scopes: HashMap<String, Style>,  // "keyword" -> Style, "function" -> Style
    pub ui: UiColors,
}

pub struct UiColors {
    pub background: Color,
    pub foreground: Color,
    pub cursor: Color,
    pub selection: Color,
    pub line_number: Color,
    pub line_number_active: Color,
    pub status_bar_bg: Color,
    pub status_bar_fg: Color,
    pub tab_active_bg: Color,
    pub tab_inactive_bg: Color,
    pub sidebar_bg: Color,
    pub sidebar_fg: Color,
    pub border: Color,
    pub error: Color,
    pub warning: Color,
    pub info: Color,
    pub hint: Color,
}

impl Theme {
    pub fn resolve(&self, highlight_name: &str) -> Style;
}

// crates/termcode-theme/src/style.rs

#[derive(Debug, Clone, Copy)]
pub struct Style {
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum Color {
    Rgb(u8, u8, u8),
    Indexed(u8),
    Named(NamedColor),
}

// crates/termcode-theme/src/loader.rs

/// Theme TOML format:
/// [palette]
/// red = "#e06c75"
///
/// [scopes]
/// keyword = { fg = "red", bold = true }
/// "function.name" = { fg = "#61afef" }
///
/// [ui]
/// background = "#282c34"
pub fn load_theme(path: &Path) -> Result<Theme>;
pub fn load_builtin_themes(runtime_dir: &Path) -> Result<HashMap<String, Theme>>;
```

### 4.5 termcode-plugin: Lua Plugin Runtime

**Responsibility**: Manage Lua VM, expose editor API to plugins, handle plugin lifecycle.

```rust
// crates/termcode-plugin/src/runtime.rs

/// The Lua plugin runtime. One Lua VM for the entire editor.
pub struct PluginRuntime {
    lua: Lua,
    loaded_plugins: Vec<PluginInfo>,
    hooks: HookRegistry,
}

pub struct PluginInfo {
    pub name: String,
    pub version: String,
    pub path: PathBuf,
    pub enabled: bool,
}

impl PluginRuntime {
    pub fn new() -> Result<Self>;
    pub fn load_plugin(&mut self, path: &Path) -> Result<()>;
    pub fn emit_hook(&self, hook: &str, args: LuaMultiValue) -> Result<()>;
    pub fn execute(&self, code: &str) -> Result<LuaValue>;
}

// crates/termcode-plugin/src/api.rs

/// Register all Rust functions into the Lua `termcode` global table.
///
/// Lua plugins access the editor via:
///   termcode.open_file("path")
///   termcode.get_current_line()
///   termcode.insert_text("hello")
///   termcode.register_command("my_command", function() ... end)
///   termcode.on("buffer_write_pre", function(doc) ... end)
///   termcode.set_keymap("normal", "<C-x>", "my_command")
///
pub fn register_api(lua: &Lua, editor: Arc<Mutex<Editor>>) -> Result<()>;

// crates/termcode-plugin/src/event.rs

/// Named hooks that plugins can subscribe to.
pub enum Hook {
    BufferOpen,
    BufferWritePre,
    BufferWritePost,
    BufferClose,
    CharInsertPre,      // Before character insertion (e.g., for auto-pairs)
    CharInsertPost,     // After character insertion (e.g., for snippets)
    CursorMoved,
    ModeChanged,
    FileExplorerOpen,
    CommandExecuted,
}

pub struct HookRegistry {
    hooks: HashMap<String, Vec<LuaFunction>>,
}
```

### 4.6 termcode-term: Terminal UI and Event Loop

**Responsibility**: The "view" and "controller" -- rendering widgets, dispatching events, running the main loop.

```rust
// crates/termcode-term/src/app.rs

/// Main application struct. Owns everything.
pub struct App {
    editor: Editor,
    terminal: Terminal<CrosstermBackend<Stdout>>,
    plugin_runtime: PluginRuntime,
    event_rx: mpsc::UnboundedReceiver<AppEvent>,
    event_tx: mpsc::UnboundedSender<AppEvent>,
    command_registry: CommandRegistry,
    should_quit: bool,
}

impl App {
    pub async fn new(config: AppConfig) -> Result<Self>;

    /// The main TEA loop: Event -> Update -> Render (consistent with TEA pattern).
    pub async fn run(&mut self) -> Result<()> {
        // Initial render before entering the loop
        self.render()?;

        loop {
            // 1. Wait for next event
            let event = self.next_event().await?;

            // 2. Update state based on event
            self.update(event)?;

            // 3. Check exit
            if self.should_quit { break; }

            // 4. Render updated state
            self.render()?;
        }
        Ok(())
    }

    fn render(&mut self) -> Result<()>;
    fn update(&mut self, event: AppEvent) -> Result<()>;
    async fn next_event(&mut self) -> Result<AppEvent>;
}

// crates/termcode-term/src/event.rs

/// All events the app can process.
pub enum AppEvent {
    /// Raw terminal input
    Input(crossterm::event::Event),
    /// Named command to execute
    Command(CommandId, Option<Vec<String>>),
    /// LSP response arrived
    Lsp(LspResponse),
    /// File system change detected
    FileChanged(PathBuf),
    /// Plugin-generated event
    Plugin(String, serde_json::Value),
    /// Resize terminal
    Resize(u16, u16),
    /// Tick for periodic tasks (auto-save, etc.)
    Tick,
}

// crates/termcode-term/src/command.rs

pub type CommandId = &'static str;
pub type CommandFn = Box<dyn Fn(&mut Editor, &[String]) -> Result<()> + Send>;

pub struct CommandRegistry {
    commands: HashMap<CommandId, CommandEntry>,
}

pub struct CommandEntry {
    pub id: CommandId,
    pub name: String,          // Display name for command palette
    pub description: String,
    pub handler: CommandFn,
    pub default_keybinding: Option<KeyEvent>,
}

impl CommandRegistry {
    pub fn register(&mut self, entry: CommandEntry);
    pub fn execute(&self, id: CommandId, editor: &mut Editor, args: &[String]) -> Result<()>;
    pub fn search(&self, query: &str) -> Vec<&CommandEntry>;  // For command palette fuzzy search
}

// All built-in commands registered at startup:
pub fn register_builtin_commands(registry: &mut CommandRegistry) {
    // File commands
    registry.register(cmd!("file.open", "Open File", file_open));
    registry.register(cmd!("file.save", "Save File", file_save));
    registry.register(cmd!("file.save_as", "Save As", file_save_as));
    registry.register(cmd!("file.close", "Close File", file_close));

    // Edit commands
    registry.register(cmd!("edit.insert_char", "Insert Character", edit_insert_char));
    registry.register(cmd!("edit.delete_char", "Delete Character", edit_delete_char));
    registry.register(cmd!("edit.backspace", "Backspace", edit_backspace));
    registry.register(cmd!("edit.newline", "New Line", edit_newline));
    registry.register(cmd!("edit.undo", "Undo", edit_undo));
    registry.register(cmd!("edit.redo", "Redo", edit_redo));
    registry.register(cmd!("edit.cut", "Cut", edit_cut));
    registry.register(cmd!("edit.copy", "Copy", edit_copy));
    registry.register(cmd!("edit.paste", "Paste", edit_paste));

    // View commands
    registry.register(cmd!("view.split_horizontal", "Split Horizontal", view_split_h));
    registry.register(cmd!("view.split_vertical", "Split Vertical", view_split_v));
    registry.register(cmd!("view.toggle_sidebar", "Toggle Sidebar", view_toggle_sidebar));
    registry.register(cmd!("view.toggle_minimap", "Toggle Minimap", view_toggle_minimap));

    // Navigation
    registry.register(cmd!("goto.line", "Go to Line", goto_line));
    registry.register(cmd!("goto.file", "Go to File (Fuzzy)", goto_file));
    registry.register(cmd!("goto.definition", "Go to Definition", goto_definition));

    // Search
    registry.register(cmd!("search.find", "Find", search_find));
    registry.register(cmd!("search.replace", "Find and Replace", search_replace));
    registry.register(cmd!("search.project", "Search in Project", search_project));

    // Tab
    registry.register(cmd!("tab.next", "Next Tab", tab_next));
    registry.register(cmd!("tab.prev", "Previous Tab", tab_prev));
    registry.register(cmd!("tab.close", "Close Tab", tab_close));
}

// crates/termcode-term/src/input.rs

/// Maps key events to commands based on current mode and keybinding config.
pub struct InputMapper {
    keymaps: HashMap<EditorMode, HashMap<KeySequence, CommandId>>,
}

impl InputMapper {
    pub fn from_config(config: &KeymapConfig) -> Self;
    pub fn resolve(&self, mode: EditorMode, key: KeyEvent) -> Option<CommandId>;
}

/// Supports multi-key sequences like "g g" for go-to-top.
pub struct KeySequence {
    keys: Vec<KeyEvent>,
}

// crates/termcode-term/src/layout.rs

/// Computes the layout rectangles for the entire terminal area.
///
/// ┌─────────────────────────────────────────────────┐
/// │                   Top Bar                        │ <- 1 row
/// ├────────────┬────────────────────────────────────┤
/// │            │          Tab Bar                    │ <- 1 row
/// │   File     ├────────────────────────────────────┤
/// │  Explorer  │                                     │
/// │  Sidebar   │          Editor Area                │ <- remaining
/// │            │         (pane splits)               │
/// │  (toggle)  │                                     │
/// │            │                                     │
/// ├────────────┴────────────────────────────────────┤
/// │                 Status Bar                       │ <- 1 row
/// └─────────────────────────────────────────────────┘
///
pub fn compute_layout(area: Rect, sidebar_visible: bool, sidebar_width: u16) -> AppLayout;

pub struct AppLayout {
    pub top_bar: Rect,
    pub sidebar: Option<Rect>,
    pub tab_bar: Rect,
    pub editor_area: Rect,    // The pane tree lays out within this
    pub status_bar: Rect,
}
```

### 4.7 termcode-config: Configuration

```rust
// crates/termcode-config/src/config.rs

#[derive(Debug, Deserialize)]
pub struct AppConfig {
    pub editor: EditorConfig,
    pub ui: UiConfig,
    pub theme: String,
    pub keybindings: KeymapConfig,
    pub file_explorer: FileExplorerConfig,
    pub plugins: PluginConfig,
    pub lsp: Vec<LspServerConfig>,
    pub languages: HashMap<String, LanguageOverride>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FileExplorerConfig {
    pub show_hidden: bool,
    pub show_gitignored: bool,
    pub icons: bool,
}

/// Defined here in termcode-config (not in termcode-lsp).
/// termcode-lsp depends on termcode-config to receive this type.
#[derive(Debug, Clone, Deserialize)]
pub struct LspServerConfig {
    pub language: String,      // matches TOML field: [[lsp]] language = "rust"
    pub command: String,
    pub args: Vec<String>,
}

/// Note: EditorConfig and LineNumberStyle are defined in termcode-core
/// (not here) so that termcode-view can use them without depending on
/// termcode-config. AppConfig deserializes TOML into these core types.
/// See termcode-core for the struct definitions.
///
/// EditorConfig fields: tab_size, insert_spaces, auto_save,
///   auto_save_delay_ms, word_wrap, line_numbers, cursor_style,
///   scroll_off, mouse_enabled
///
/// LineNumberStyle variants: Absolute, Relative, RelativeAbsolute, None

#[derive(Debug, Deserialize)]
pub struct UiConfig {
    pub sidebar_width: u16,
    pub sidebar_visible: bool,
    pub show_minimap: bool,
    pub show_tab_bar: bool,
    pub show_top_bar: bool,
    pub border_style: BorderStyle,
}
```

### 4.8 termcode-lsp: Language Server Protocol

```rust
// crates/termcode-lsp/src/client.rs

/// A running LSP client connected to one language server.
pub struct LspClient {
    server_name: String,
    process: Child,
    writer: BufWriter<ChildStdin>,
    pending_requests: HashMap<RequestId, oneshot::Sender<lsp_types::Value>>,
}

impl LspClient {
    pub async fn start(config: &LspServerConfig) -> Result<Self>;
    pub async fn initialize(&mut self, root: &Path) -> Result<()>;
    pub async fn text_document_completion(&mut self, params: CompletionParams)
        -> Result<CompletionResponse>;
    pub async fn text_document_definition(&mut self, params: GotoDefinitionParams)
        -> Result<GotoDefinitionResponse>;
    pub async fn text_document_hover(&mut self, params: HoverParams) -> Result<Option<Hover>>;
    /// Uses core types only (no Document dependency) to avoid lsp -> view cycle.
    pub fn notify_did_open(&mut self, uri: &str, language_id: &str, version: i32, text: &str) -> Result<()>;
    pub fn notify_did_change(&mut self, uri: &str, version: i32, changes: &[TextDocumentContentChangeEvent])
        -> Result<()>;
    pub async fn shutdown(&mut self) -> Result<()>;
}

// crates/termcode-lsp/src/registry.rs

/// Keys are language name strings (e.g., "rust", "python") matching
/// LspServerConfig.language. LanguageId (Arc<str>) lives in termcode-syntax
/// which lsp does not depend on, so we use String here.
pub struct LspRegistry {
    clients: HashMap<String, LspClient>,
    configs: HashMap<String, LspServerConfig>,
}

/// Note: LspServerConfig is defined in termcode-config, not here.
/// LspRegistry receives it from App at initialization.
/// See termcode-config/src/config.rs for the struct definition.
```

## 5. Data Flow

### 5.1 Main Event Loop (TEA Pattern)

```
┌─────────────────────────────────────────────────────────────┐
│                        EVENT SOURCES                         │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌────────────┐ │
│  │ Terminal  │  │  Timer   │  │   LSP    │  │  Plugin    │ │
│  │ (crossterm│  │ (tick)   │  │ (server  │  │ (Lua async │ │
│  │  events)  │  │          │  │  response)│  │  callback) │ │
│  └─────┬─────┘  └────┬─────┘  └─────┬────┘  └─────┬──────┘ │
│        └──────┬───────┴──────────────┴─────────────┘        │
│               ▼                                              │
│     ┌─────────────────┐                                      │
│     │  mpsc channel   │  (all events unified into AppEvent)  │
│     └────────┬────────┘                                      │
│              ▼                                               │
│  ┌───────────────────────┐                                   │
│  │   INPUT MAPPER        │  Key event -> Command resolution  │
│  │   (mode-aware)        │                                   │
│  └───────────┬───────────┘                                   │
│              ▼                                               │
│  ┌───────────────────────┐                                   │
│  │  COMMAND REGISTRY     │  Execute the resolved command     │
│  │  (dispatch)           │                                   │
│  └───────────┬───────────┘                                   │
│              ▼                                               │
│  ┌───────────────────────┐                                   │
│  │  PLUGIN HOOKS (pre)   │  "buffer_write_pre" etc.          │
│  └───────────┬───────────┘                                   │
│              ▼                                               │
│  ┌───────────────────────┐                                   │
│  │   STATE UPDATE        │  Mutate Editor state              │
│  │   (Editor methods)    │  (buffer, selection, tabs, etc.)  │
│  └───────────┬───────────┘                                   │
│              ▼                                               │
│  ┌───────────────────────┐                                   │
│  │  PLUGIN HOOKS (post)  │  "buffer_write_post" etc.         │
│  └───────────┬───────────┘                                   │
│              ▼                                               │
│  ┌───────────────────────┐                                   │
│  │   RENDER              │  terminal.draw(|frame| { ... })   │
│  │   (immediate mode)    │  Read Editor state, paint widgets │
│  └───────────────────────┘                                   │
└─────────────────────────────────────────────────────────────┘
```

### 5.2 File Open Flow

| Step | Component                  | Action                                          | Data                             |
| ---- | -------------------------- | ----------------------------------------------- | -------------------------------- |
| 1    | InputMapper                | User presses `Ctrl+O` or clicks file tree       | `AppEvent::Command("file.open")` |
| 2    | CommandRegistry            | Dispatches to `file_open` handler               | `CommandId`                      |
| 3    | Editor::open_file          | Read file from disk, detect encoding            | `PathBuf` -> `Vec<u8>`           |
| 4    | Buffer::from_reader        | Build Rope from file contents                   | `Rope`                           |
| 5    | LanguageRegistry           | Detect language from extension                  | `LanguageId`                     |
| 6    | SyntaxHighlighter::new     | Initialize tree-sitter parser                   | `SyntaxHighlighter`              |
| 7    | SyntaxHighlighter::parse   | Parse full AST                                  | `tree_sitter::Tree`              |
| 8    | Document                   | Assemble buffer + syntax + history              | `Document`                       |
| 9    | Editor                     | Insert into documents map, create View, add Tab | `DocumentId`                     |
| 10   | LspClient::notify_did_open | Notify language server                          | `DidOpenTextDocument`            |
| 11   | PluginRuntime::emit_hook   | Fire `buffer_open` hook                         | Lua callback                     |
| 12   | Render                     | Next frame paints the new document              | `Frame`                          |

### 5.3 Keystroke-to-Edit Flow

```
Key('a') in Insert mode
    -> InputMapper: resolves to built-in command "edit.insert_char"
    -> CommandRegistry::execute("edit.insert_char", args=["a"])
    -> PluginRuntime::emit_hook("char_insert_pre")
    -> Transaction::insert("a", cursor_pos)
    -> Buffer::apply(transaction)
    -> Selection::map(transaction)  -- update cursor position
    -> SyntaxHighlighter::update(edits)  -- incremental re-parse
    -> History::commit(transaction)
    -> LspClient::notify_did_change
    -> PluginRuntime::emit_hook("char_insert_post")
    -> Render
```

Note: Even printable character insertion in Insert mode goes through the
CommandRegistry via the "edit.insert_char" command. This preserves the
principle that ALL actions flow through commands, enabling plugins to
intercept character input (e.g., auto-pairs, snippets).

## 6. API Contracts

### 6.1 Plugin Lua API

The Lua API is the primary extension point. Plugins interact through a `termcode` global table.

```lua
-- Plugin init.lua structure
local M = {}

M.name = "my-plugin"
M.version = "0.1.0"

function M.setup(opts)
    -- Register commands
    termcode.register_command("my_plugin.greet", function()
        local line = termcode.get_current_line()
        termcode.notify("Current line: " .. line)
    end)

    -- Register keybinding
    termcode.set_keymap("normal", "<leader>g", "my_plugin.greet")

    -- Subscribe to hooks
    termcode.on("buffer_open", function(doc)
        termcode.notify("Opened: " .. doc.path)
    end)

    -- Add status bar segment
    termcode.add_status_item("right", function()
        return "My Plugin"
    end)
end

return M
```

**Available Lua functions** (exposed from Rust via mlua):

| Category | Function                              | Description           |
| -------- | ------------------------------------- | --------------------- |
| Buffer   | `termcode.get_current_line()`         | Current line text     |
| Buffer   | `termcode.get_line(n)`                | Get line n            |
| Buffer   | `termcode.insert_text(text)`          | Insert at cursor      |
| Buffer   | `termcode.delete_selection()`         | Delete selected text  |
| Buffer   | `termcode.get_selection()`            | Get selected text     |
| Cursor   | `termcode.get_cursor()`               | Returns {line, col}   |
| Cursor   | `termcode.set_cursor(line, col)`      | Move cursor           |
| File     | `termcode.open_file(path)`            | Open file in new tab  |
| File     | `termcode.save()`                     | Save current file     |
| File     | `termcode.get_filepath()`             | Current file path     |
| Command  | `termcode.register_command(id, fn)`   | Register new command  |
| Command  | `termcode.execute_command(id)`        | Execute a command     |
| Keymap   | `termcode.set_keymap(mode, key, cmd)` | Bind key to command   |
| Hook     | `termcode.on(event, callback)`        | Subscribe to event    |
| UI       | `termcode.notify(msg)`                | Show status message   |
| UI       | `termcode.add_status_item(pos, fn)`   | Add status bar widget |
| Config   | `termcode.get_option(key)`            | Read config value     |
| Config   | `termcode.set_option(key, val)`       | Set config value      |

### 6.2 Theme TOML Format

```toml
# runtime/themes/one-dark.toml

[meta]
name = "One Dark"
author = "termcode"

[palette]
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
background = "bg"
foreground = "fg"
cursor = "#528bff"
selection = "#3e4451"
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

[scopes]
"attribute" = { fg = "yellow" }
"comment" = { fg = "comment", italic = true }
"constant" = { fg = "cyan" }
"constant.numeric" = { fg = "yellow" }
"function" = { fg = "blue" }
"function.macro" = { fg = "magenta" }
"keyword" = { fg = "magenta" }
"keyword.control" = { fg = "magenta" }
"operator" = { fg = "cyan" }
"punctuation" = { fg = "fg" }
"string" = { fg = "green" }
"type" = { fg = "yellow" }
"variable" = { fg = "red" }
"variable.builtin" = { fg = "red" }
"tag" = { fg = "red" }
```

### 6.3 Configuration TOML Format

```toml
# config/config.toml

theme = "one-dark"

[editor]
tab_size = 4
insert_spaces = true
auto_save = false
auto_save_delay_ms = 1000
word_wrap = false
line_numbers = "relative_absolute"
cursor_style = "block"
scroll_off = 5
mouse_enabled = true

[ui]
sidebar_width = 30
sidebar_visible = true
show_minimap = false
show_tab_bar = true
show_top_bar = true
border_style = "rounded"

[file_explorer]
show_hidden = false
show_gitignored = false
icons = true

[[lsp]]
language = "rust"
command = "rust-analyzer"
args = []

[[lsp]]
language = "python"
command = "pyright-langserver"
args = ["--stdio"]

[[lsp]]
language = "typescript"
command = "typescript-language-server"
args = ["--stdio"]

[plugins]
plugin_dir = "~/.config/termcode/plugins"
enabled = ["example"]
```

## 7. Error Handling Strategy

| Error Type                 | Handling                            | User Message                                    |
| -------------------------- | ----------------------------------- | ----------------------------------------------- |
| File I/O (open/save)       | Return error, show in status bar    | "Cannot open file: permission denied"           |
| Encoding detection failure | Fall back to UTF-8, warn            | "File encoding unknown, opened as UTF-8"        |
| Tree-sitter parse failure  | Degrade gracefully, no highlighting | "Syntax highlighting unavailable for this file" |
| LSP connection failure     | Log, continue without LSP           | "Language server failed to start"               |
| Plugin Lua error           | Catch, log, disable plugin, notify  | "Plugin 'x' error: ..."                         |
| Config parse error         | Fall back to defaults, warn         | "Config error at line N, using defaults"        |
| Out of memory (huge file)  | Refuse to open beyond limit         | "File too large (>500MB)"                       |
| Terminal resize            | Recompute layout, re-render         | (silent)                                        |

## 8. Security Considerations

- [ ] **Plugin sandboxing**: Lua VM has limited filesystem access (configurable allowed paths). No network access by default. Plugins cannot execute arbitrary system commands without explicit permission in config.
- [ ] **File permission checks**: Warn before writing to read-only files. Respect file ownership.
- [ ] **No remote code execution**: Plugins are loaded only from configured directories, never auto-downloaded.
- [ ] **Sensitive file warning**: Warn when opening files like `.env`, `id_rsa`, `credentials.json`.
- [ ] **Input validation**: All config values validated against schema with bounded ranges (e.g., tab_size 1-16).

## 9. Performance Considerations

| Concern                       | Strategy                                                                                                       |
| ----------------------------- | -------------------------------------------------------------------------------------------------------------- |
| Large files (>10MB)           | Rope data structure gives O(log n) operations. Lazy syntax highlighting (only visible viewport).               |
| Syntax highlighting           | Incremental Tree-sitter parsing after edits. Only highlight visible lines during render.                       |
| File explorer with many files | Lazy expansion -- only read directory contents when expanded. Use `ignore` crate for .gitignore-aware walking. |
| Rendering                     | Ratatui's immediate mode diffing -- only flushes changed cells to terminal.                                    |
| Fuzzy finder                  | `nucleo` crate (same as Helix) for parallel fuzzy matching.                                                    |
| LSP                           | Async communication. Debounce `didChange` notifications (100ms).                                               |
| Memory                        | One Rope per open document. Close background documents to free memory if needed.                               |
| Startup time                  | Lazy-load tree-sitter grammars. Load config synchronously (fast TOML parse). Defer plugin init.                |

## 10. Testing Strategy

| Layer           | Test Type   | Coverage Target                                                                  | Framework                                |
| --------------- | ----------- | -------------------------------------------------------------------------------- | ---------------------------------------- |
| termcode-core   | Unit tests  | Buffer operations, selection math, transaction compose/invert, history undo/redo | `#[cfg(test)]` built-in                  |
| termcode-syntax | Unit tests  | Language detection, highlight span generation                                    | Built-in + fixture files                 |
| termcode-view   | Unit tests  | Document state, pane tree layout, file explorer expand/collapse, tab management  | Built-in                                 |
| termcode-theme  | Unit tests  | TOML parsing, color resolution, scope fallback                                   | Built-in                                 |
| termcode-config | Unit tests  | Config loading, default merging, validation                                      | Built-in                                 |
| termcode-plugin | Integration | Lua API calls modify editor state correctly                                      | mlua + custom test harness               |
| termcode-term   | Integration | Key sequence -> command -> state change -> expected render output                | `ratatui::backend::TestBackend`          |
| termcode-lsp    | Integration | Mock server, verify protocol messages                                            | tokio test runtime                       |
| End-to-end      | E2E         | Open file, edit, save, split pane, use sidebar                                   | Custom test driver with virtual terminal |
| Performance     | Benchmark   | Buffer ops on 1M line file, render frame time                                    | `criterion` crate                        |

## 11. Implementation Sequence

### Phase 1: Foundation (Weeks 1-3) -- MVP: View a file with syntax highlighting

**Goal**: Open termcode, see a syntax-highlighted file, scroll around.

1. Scaffold workspace with all 8 crate stubs
2. `termcode-core`: Buffer (Rope), Position, read file from disk
3. `termcode-theme`: Parse one built-in theme (One Dark)
4. `termcode-syntax`: Tree-sitter integration for Rust language (one language to start)
5. `termcode-config`: Minimal config struct with hardcoded defaults
6. `termcode-view`: Document (buffer + syntax), single View with scroll
7. `termcode-term`: Basic event loop, render a single file with line numbers and highlighting
8. `src/main.rs`: CLI arg to open a file

**Deliverable**: `termcode src/main.rs` opens and displays the file with syntax highlighting, scroll with arrow keys.

### Phase 2: File Explorer + Tabs (Weeks 4-5) -- The differentiator

**Goal**: Toggleable sidebar with file tree, open files in tabs.

1. `termcode-view`: FileExplorer model, TabManager
2. `termcode-term/ui/file_explorer.rs`: File tree widget with expand/collapse
3. `termcode-term/ui/tab_bar.rs`: Tab strip widget
4. `termcode-term/ui/top_bar.rs`: Path + file info bar
5. `termcode-term/ui/status_bar.rs`: Line/col, filetype, encoding
6. `termcode-term/layout.rs`: Full layout engine with sidebar toggle
7. Navigation keybindings (switch between sidebar and editor)

**Deliverable**: Full VS Code-like layout with sidebar file browser and tabbed editor.

### Phase 3: Editing (Weeks 6-8) -- Become a real editor

**Goal**: Insert and edit text, undo/redo, save files.

1. `termcode-core`: Transaction, Selection (multi-cursor), History
2. `termcode-view`: Editor modes (Normal, Insert), Document editing methods
3. `termcode-term`: Command registry, input mapper, mode-aware keybindings
4. Basic editing commands: insert, delete, backspace, enter, select, cut/copy/paste
5. Undo/redo
6. File save (with encoding preservation)
7. Bracket matching, auto-indent
8. `termcode-term/ui/editor_view.rs`: Cursor rendering, selection highlighting

**Deliverable**: Functional text editor with insert mode, undo/redo, save.

### Phase 4: Search, Fuzzy Finder, Command Palette (Weeks 9-10)

**Goal**: In-file search/replace, fuzzy file finder, command palette.

1. `termcode-view/search.rs`: SearchState with literal case-insensitive matching (regex deferred)
2. `termcode-view/fuzzy.rs`: FuzzyFinderState with built-in fuzzy scoring (no nucleo for MVP)
3. `termcode-view/palette.rs`: CommandPaletteState with fuzzy filter on command names
4. `termcode-term/ui/overlay.rs`: Shared overlay rendering (frame, input line, result list)
5. `termcode-term/ui/search.rs`: Search/replace overlay (Ctrl+F, Ctrl+H)
6. `termcode-term/ui/fuzzy_finder.rs`: Centered file picker overlay (Ctrl+P)
7. `termcode-term/ui/command_palette.rs`: Centered command list overlay (Ctrl+Shift+P)
8. Three new `EditorMode` variants: Search, FuzzyFinder, CommandPalette
9. `InputMapper` extended with per-mode keybinding vectors for overlay modes
10. `CommandRegistry::list_commands()` for palette population

**Deliverable**: In-file search/replace, fuzzy file finder, command palette. All three overlays render on top of editor area.

### Phase 5: Plugin System (Weeks 11-13)

**Goal**: Lua plugins can extend the editor.

1. `termcode-plugin`: Lua VM setup, API registration
2. Plugin loader (scan directories, call `setup()`)
3. Hook system (pre/post events)
4. Command registration from Lua
5. Keybinding registration from Lua
6. Status bar extension from plugins
7. Example plugins: word count, auto-pairs, trailing whitespace trimmer

**Deliverable**: Working plugin system with example plugins.

### Phase 6: LSP + Git (Weeks 14-17)

**Goal**: IDE-level features.

1. `termcode-lsp`: Client, transport, registry
2. Diagnostics display (inline errors/warnings)
3. Autocomplete popup
4. Go-to-definition, hover
5. `gix` integration: branch in status bar, file status icons in explorer
6. Additional tree-sitter languages (Python, JS, TS, Go, C/C++, Java, etc.)

**Deliverable**: LSP autocomplete, diagnostics, git status indicators.

### Phase 7: Polish (Weeks 18-20)

**Goal**: Production quality.

1. Mouse support (click to place cursor, scroll, click tabs, click file tree)
2. System clipboard integration (`arboard`)
3. Session save/restore
4. Configurable keybindings (full TOML keybinding config)
5. Multiple themes shipped, theme switching at runtime
6. Minimap widget
7. Relative line numbers
8. Performance optimization (profile and optimize render loop, large file handling)
9. Cross-platform testing (macOS, Linux, Windows)
10. Documentation and README

**Deliverable**: Release candidate.

## 12. Key Crate Dependencies (Final)

| Crate                   | Version | Purpose                               |
| ----------------------- | ------- | ------------------------------------- |
| `ratatui`               | 0.29    | Terminal UI framework                 |
| `crossterm`             | 0.28    | Cross-platform terminal backend       |
| `ropey`                 | 1.6     | Rope data structure for text buffers  |
| `tree-sitter`           | 0.24    | Incremental parsing                   |
| `tree-sitter-highlight` | 0.24    | Syntax highlighting                   |
| `mlua`                  | 0.10    | Lua 5.4 bindings for plugin system    |
| `tokio`                 | 1       | Async runtime (LSP, plugins)          |
| `serde`                 | 1       | Serialization framework               |
| `toml`                  | 0.8     | Config/theme file parsing             |
| `lsp-types`             | 0.97    | LSP protocol types                    |
| `nucleo`                | 0.5     | Fuzzy matching (Helix's fuzzy finder) |
| `ignore`                | 0.4     | .gitignore-aware file walking         |
| `arboard`               | 3       | System clipboard                      |
| `gix`                   | 0.68    | Git integration                       |
| `encoding_rs`           | 0.8     | Character encoding                    |
| `anyhow`                | 1       | Application error handling            |
| `thiserror`             | 2       | Library error types                   |
| `parking_lot`           | 0.12    | Fast mutexes for shared state         |
| `slotmap`               | 1       | Slot map for document/view IDs        |
| `criterion`             | 0.5     | Benchmarking                          |

## Handoff Notes for yyy-plan

### Critical Decisions

1. **TEA architecture is mandatory**: Every state change goes through the `Event -> Update -> Render` cycle. No widget should mutate state directly during rendering.
2. **Command pattern is the only way to trigger actions**: Even internal actions (like "open file from explorer click") dispatch through the CommandRegistry. This ensures plugins can hook into everything.
3. **Rope is the only buffer representation**: Never store file contents as `String` or `Vec<u8>`. Always use `ropey::Rope`.
4. **Tree-sitter highlighting is viewport-scoped**: Never highlight the entire file. Only compute highlights for visible lines + a small buffer.
5. **File explorer is a first-class citizen**: It has its own mode (`EditorMode::FileExplorer`), its own keybindings, and its own state in `Editor`. It is not an afterthought overlay.
6. **Crate boundaries must be respected**: `termcode-core` must never depend on `termcode-view`. Dependency arrows only point downward in the crate graph.

### Files to Create (Phase 1 -- MVP)

| File                                             | Purpose                                             |
| ------------------------------------------------ | --------------------------------------------------- |
| `Cargo.toml`                                     | Workspace root                                      |
| `src/main.rs`                                    | Binary entrypoint                                   |
| `crates/termcode-core/Cargo.toml`                | Core crate manifest                                 |
| `crates/termcode-core/src/lib.rs`                | Core module declarations                            |
| `crates/termcode-core/src/buffer.rs`             | Rope-based text buffer                              |
| `crates/termcode-core/src/position.rs`           | Position types                                      |
| `crates/termcode-core/src/diagnostic.rs`         | Diagnostic type (stub, populated by LSP in Phase 6) |
| `crates/termcode-core/src/config_types.rs`       | EditorConfig, LineNumberStyle (used by view)        |
| `crates/termcode-core/src/encoding.rs`           | Encoding detection                                  |
| `crates/termcode-syntax/Cargo.toml`              | Syntax crate manifest                               |
| `crates/termcode-syntax/src/lib.rs`              | Syntax module declarations                          |
| `crates/termcode-syntax/src/highlighter.rs`      | Tree-sitter highlight engine                        |
| `crates/termcode-syntax/src/language.rs`         | Language registry                                   |
| `crates/termcode-theme/Cargo.toml`               | Theme crate manifest                                |
| `crates/termcode-theme/src/lib.rs`               | Theme module declarations                           |
| `crates/termcode-theme/src/theme.rs`             | Theme struct                                        |
| `crates/termcode-theme/src/loader.rs`            | Theme TOML parser                                   |
| `crates/termcode-theme/src/style.rs`             | Style types                                         |
| `crates/termcode-view/Cargo.toml`                | View crate manifest                                 |
| `crates/termcode-view/src/lib.rs`                | View module declarations                            |
| `crates/termcode-view/src/document.rs`           | Document model                                      |
| `crates/termcode-view/src/view.rs`               | Viewport state                                      |
| `crates/termcode-view/src/editor.rs`             | Global editor state                                 |
| `crates/termcode-config/Cargo.toml`              | Config crate manifest                               |
| `crates/termcode-config/src/lib.rs`              | Config loading                                      |
| `crates/termcode-config/src/config.rs`           | Config struct                                       |
| `crates/termcode-config/src/default.rs`          | Default values                                      |
| `crates/termcode-term/Cargo.toml`                | Terminal crate manifest                             |
| `crates/termcode-term/src/lib.rs`                | Terminal module declarations                        |
| `crates/termcode-term/src/app.rs`                | Main application loop                               |
| `crates/termcode-term/src/event.rs`              | Event types                                         |
| `crates/termcode-term/src/render.rs`             | Frame rendering                                     |
| `crates/termcode-term/src/ui/mod.rs`             | UI widget module                                    |
| `crates/termcode-term/src/ui/editor_view.rs`     | Code editor widget                                  |
| `crates/termcode-term/src/ui/overlay.rs`         | Shared overlay rendering (frame, input, list)       |
| `crates/termcode-term/src/ui/search.rs`          | Search/replace overlay widget                       |
| `crates/termcode-term/src/ui/fuzzy_finder.rs`    | Fuzzy file finder overlay widget                    |
| `crates/termcode-term/src/ui/command_palette.rs` | Command palette overlay widget                      |
| `crates/termcode-lsp/Cargo.toml`                 | LSP crate manifest (stub for workspace)             |
| `crates/termcode-lsp/src/lib.rs`                 | LSP module stub (empty, compiles)                   |
| `crates/termcode-plugin/Cargo.toml`              | Plugin crate manifest (stub for workspace)          |
| `crates/termcode-plugin/src/lib.rs`              | Plugin module stub (empty, compiles)                |
| `runtime/themes/one-dark.toml`                   | Default theme                                       |
| `runtime/queries/rust/highlights.scm`            | Rust highlight queries                              |
| `config/config.toml`                             | Default configuration                               |

### Files to Modify (None -- Greenfield Project)

This is a new project. No existing files need modification.
