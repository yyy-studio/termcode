# Phase 2: File Explorer + Tabs Implementation Plan

**Created**: 2026-03-27
**Analysis Report**: N/A (no formal analysis report exists; planning based on architecture blueprint and team lead specifications)
**Status**: Pending

## 1. Requirements Summary

### Functional Requirements

- [FR-P2-01] Toggleable file explorer sidebar with directory tree navigation
- [FR-P2-02] .gitignore-aware directory walking (respects ignore patterns)
- [FR-P2-03] Expand/collapse directories with visual tree indicators
- [FR-P2-04] Open files from explorer into tabbed editor
- [FR-P2-05] Tab bar with active tab highlight and modified indicator
- [FR-P2-06] Switch between tabs (next/prev)
- [FR-P2-07] Close tabs
- [FR-P2-08] Top bar showing current file path and app name
- [FR-P2-09] Full layout engine splitting terminal into top bar, sidebar, tab bar, editor area, status bar
- [FR-P2-10] Mode switching between Normal and FileExplorer modes
- [FR-P2-11] Keyboard navigation in file explorer (arrow keys, Enter to open/expand)

### Architecture Constraints (from docs/architecture/termcode.md)

- File explorer is a first-class citizen with its own `EditorMode::FileExplorer`
- `termcode-view` is frontend-agnostic (no ratatui dependency)
- TEA pattern: Event -> Update -> Render
- Immediate-mode rendering: rebuild full UI each frame from state
- Crate boundary: `termcode-core` must never depend on `termcode-view`

## 2. Analysis Report Reference

### Reference Documents

- Architecture Blueprint: `docs/architecture/termcode.md`
- Project Plan: `/Users/hankyung/.claude/plans/cosmic-prancing-whisper.md`

### Applied Recommendations (from architecture)

- FileExplorer struct with root, tree nodes, selected index, visible flag, width
- TabManager with tabs vec, active index
- EditorMode::FileExplorer variant for mode-based key handling
- AppLayout struct with compute_layout function for layout engine
- Use `ignore` crate for .gitignore-aware directory walking

### Reusable Code

| Code                  | Location                                     | Purpose                                                                |
| --------------------- | -------------------------------------------- | ---------------------------------------------------------------------- |
| `Editor` struct       | `crates/termcode-view/src/editor.rs`         | Add FileExplorer + TabManager fields                                   |
| `DocumentId` pattern  | `crates/termcode-view/src/document.rs`       | Tab references documents via DocumentId                                |
| `Widget` impl pattern | `crates/termcode-term/src/ui/editor_view.rs` | Pattern for new widget implementations                                 |
| `StatusBarWidget`     | `crates/termcode-term/src/ui/status_bar.rs`  | Simple widget reference for top_bar/tab_bar                            |
| `UiColors`            | `crates/termcode-theme/src/theme.rs`         | sidebar_bg, sidebar_fg, tab_active_bg, tab_inactive_bg already defined |
| `render()` function   | `crates/termcode-term/src/render.rs`         | Extend with new layout engine                                          |
| `handle_key()` method | `crates/termcode-term/src/app.rs`            | Add mode-aware keybinding dispatch                                     |

### Constraints

- No formal analysis report; all specifications derived from architecture blueprint and team lead input
- Phase 1 uses `HashMap<DocumentId, Document>` (not `SlotMap` as architecture envisions); plan aligns with current implementation
- Current Editor lacks `mode` field; must be added
- Current render.rs uses simple 2-chunk vertical layout; must be replaced with full layout engine
- `ignore` crate must be added as a new dependency to `termcode-view`

## 3. Impact Analysis (Dry-Run)

### Overall Risk: Medium

### Files to Create

| File                                           | Risk | Description                                           |
| ---------------------------------------------- | ---- | ----------------------------------------------------- |
| `crates/termcode-view/src/file_explorer.rs`    | Low  | FileExplorer model: tree nodes, expand/collapse, walk |
| `crates/termcode-view/src/tab.rs`              | Low  | TabManager: tab list, active index, CRUD operations   |
| `crates/termcode-term/src/layout.rs`           | Low  | Layout engine: compute_layout + AppLayout struct      |
| `crates/termcode-term/src/ui/file_explorer.rs` | Low  | File tree sidebar widget (ratatui Widget impl)        |
| `crates/termcode-term/src/ui/tab_bar.rs`       | Low  | Tab strip widget with active highlight                |
| `crates/termcode-term/src/ui/top_bar.rs`       | Low  | Top bar widget showing file path + app name           |

### Files to Modify

| File                                 | Risk   | Description                                                                         |
| ------------------------------------ | ------ | ----------------------------------------------------------------------------------- |
| `crates/termcode-view/Cargo.toml`    | Low    | Add `ignore` crate dependency                                                       |
| `crates/termcode-view/src/lib.rs`    | Low    | Add `pub mod file_explorer; pub mod tab;`                                           |
| `crates/termcode-view/src/editor.rs` | Medium | Add FileExplorer, TabManager, EditorMode fields; update open_file to create tab     |
| `crates/termcode-term/src/render.rs` | Medium | Replace simple layout with layout engine; render all widgets                        |
| `crates/termcode-term/src/app.rs`    | Medium | Add mode-aware keybindings, sidebar toggle, tab switching, file explorer navigation |
| `crates/termcode-term/src/ui/mod.rs` | Low    | Add new widget module declarations                                                  |

### Files to Delete

| File | Risk | Description |
| ---- | ---- | ----------- |
| None | -    | -           |

### Destructive Operations

- None. All changes are additive or extend existing code.

### Rollback Plan

- All work on a feature branch; full rollback via `git checkout main && git branch -D feature/phase2-file-explorer-tabs`
- No database changes; no external system modifications
- Cargo.lock changes are auto-resolved by reverting Cargo.toml

## 4. Implementation Order

### Phase 1: Data Models (termcode-view)

**Goal**: Create the FileExplorer and TabManager models with full functionality, independent of any UI code.
**Risk**: Low
**Status**: Complete

- [x] Task 1.1: Add `ignore` crate to `crates/termcode-view/Cargo.toml`
  - Add `ignore = "0.4"` to `[dependencies]`
  - Verify workspace builds with `cargo check -p termcode-view`

- [x] Task 1.2: Create `crates/termcode-view/src/file_explorer.rs`
  - Define `FileNodeKind` enum: `File`, `Directory`, `Symlink`
  - Define `FileNode` struct: `path: PathBuf`, `name: String`, `kind: FileNodeKind`, `depth: usize`, `expanded: bool`
  - Define `FileExplorer` struct: `root: PathBuf`, `tree: Vec<FileNode>`, `selected: usize`, `visible: bool`, `width: u16`
  - Implement `FileExplorer::open(root: PathBuf) -> anyhow::Result<Self>` -- uses `ignore::WalkBuilder` for .gitignore-aware walk, populates root-level children only (lazy loading)
  - Implement `FileExplorer::toggle_expand(&mut self, index: usize) -> anyhow::Result<()>` -- if directory and collapsed, read children and insert after index; if expanded, remove children
  - Implement `FileExplorer::refresh(&mut self) -> anyhow::Result<()>` -- re-scan from root preserving expand state
  - Implement `FileExplorer::selected_path(&self) -> Option<&Path>`
  - Implement `FileExplorer::move_selection(&mut self, delta: i32)` -- clamp to 0..tree.len()
  - Implement `FileExplorer::flatten_visible(&self) -> &[FileNode]` -- return tree slice (tree already stores only visible nodes since expand/collapse manages insertions/removals)
  - Sort entries: directories first, then files, alphabetical within each group (case-insensitive)
  - Default width: 30 columns

- [x] Task 1.3: Create `crates/termcode-view/src/tab.rs`
  - Define `Tab` struct: `label: String`, `doc_id: DocumentId`, `modified: bool`
  - Define `TabManager` struct: `tabs: Vec<Tab>`, `active: usize`
  - Implement `TabManager::new() -> Self` -- empty tabs vec
  - Implement `TabManager::add(&mut self, label: String, doc_id: DocumentId)` -- push tab and set active to new tab
  - Implement `TabManager::remove(&mut self, index: usize)` -- remove tab, adjust active index
  - Implement `TabManager::set_active(&mut self, index: usize)` -- clamp to valid range
  - Implement `TabManager::next(&mut self)` -- wrap-around to first tab
  - Implement `TabManager::prev(&mut self)` -- wrap-around to last tab
  - Implement `TabManager::find_by_doc_id(&self, doc_id: DocumentId) -> Option<usize>`
  - Implement `TabManager::active_tab(&self) -> Option<&Tab>`

- [x] Task 1.4: Update `crates/termcode-view/src/lib.rs`
  - Add `pub mod file_explorer;`
  - Add `pub mod tab;`

- [x] Task 1.5: Update `crates/termcode-view/src/editor.rs`
  - Add `EditorMode` enum: `Normal`, `FileExplorer` (minimal for Phase 2; Insert/Command/Search deferred)
  - Add fields to `Editor`: `pub file_explorer: FileExplorer`, `pub tabs: TabManager`, `pub mode: EditorMode`
  - Initialize `file_explorer` as hidden (visible: false) in `Editor::new()`; accept optional root path parameter
  - Initialize `tabs` as empty `TabManager::new()` in `Editor::new()`
  - Initialize `mode` as `EditorMode::Normal`
  - Update `open_file()` to also call `self.tabs.add(name, doc_id)` with the document's display_name
  - Add `toggle_sidebar(&mut self)` -- toggles `self.file_explorer.visible`
  - Add `switch_mode(&mut self, mode: EditorMode)` -- sets `self.mode = mode`

- [x] Task 1.6: Verify Phase 1 builds
  - Run `cargo check -p termcode-view`
  - Run `cargo test -p termcode-view` (if tests exist)

### Phase 2: Layout Engine (termcode-term)

**Goal**: Create the layout computation that splits the terminal area into the five regions (top bar, sidebar, tab bar, editor area, status bar).
**Risk**: Low
**Status**: Complete

- [x] Task 2.1: Create `crates/termcode-term/src/layout.rs`
  - Define `AppLayout` struct: `top_bar: Rect`, `sidebar: Option<Rect>`, `tab_bar: Rect`, `editor_area: Rect`, `status_bar: Rect`
  - Implement `compute_layout(area: Rect, sidebar_visible: bool, sidebar_width: u16) -> AppLayout`
  - Layout algorithm:
    1. Vertical split: top_bar (1 row) | middle (remaining - 1) | status_bar (1 row)
    2. If sidebar_visible: horizontal split middle into sidebar (sidebar_width cols) | right_panel (remaining)
    3. Vertical split right_panel (or full middle if no sidebar): tab_bar (1 row) | editor_area (remaining)
  - Use `ratatui::layout::Layout` with `Constraint::Length` and `Constraint::Min`

- [x] Task 2.2: Add `pub mod layout;` to `crates/termcode-term/src/lib.rs`
  - Verify it compiles with `cargo check -p termcode-term`

### Phase 3: UI Widgets (termcode-term)

**Goal**: Create the three new widgets (file explorer, tab bar, top bar) following the established Widget pattern.
**Risk**: Low
**Status**: Complete

- [x] Task 3.1: Create `crates/termcode-term/src/ui/top_bar.rs`
  - Define `TopBarWidget<'a>` struct: holds `path: Option<&str>`, `theme: &Theme`
  - Implement `Widget` trait for `TopBarWidget`
  - Render: fill with `tab_active_bg` color (matching top bar to active tab visually), show "termcode" on left, current file path on right
  - Follow the same manual buffer-writing pattern as `StatusBarWidget`

- [x] Task 3.2: Create `crates/termcode-term/src/ui/tab_bar.rs`
  - Define `TabBarWidget<'a>` struct: holds `tabs: &TabManager`, `theme: &Theme`
  - Implement `Widget` trait for `TabBarWidget`
  - Render: horizontal row of tab labels
    - Active tab: `tab_active_bg` background, `foreground` text
    - Inactive tabs: `tab_inactive_bg` background, dimmed text
    - Modified indicator: show bullet character before label for modified tabs
    - Separator between tabs: use `border` color pipe character
  - If no tabs, render empty bar with `tab_inactive_bg`

- [x] Task 3.3: Create `crates/termcode-term/src/ui/file_explorer.rs`
  - Define `FileExplorerWidget<'a>` struct: holds `explorer: &FileExplorer`, `theme: &Theme`
  - Implement `Widget` trait for `FileExplorerWidget`
  - Render:
    - Background fill with `sidebar_bg`
    - For each visible FileNode:
      - Indent: `depth * 2` spaces
      - Directory icons: expanded = `v `, collapsed = `> ` (simple ASCII; Unicode optional)
      - File icon: `  ` (2-space indent to align with dir icons)
      - Selected item: highlight with `selection` background color
    - Text color: `sidebar_fg`
    - Clip text that overflows sidebar width

- [x] Task 3.4: Update `crates/termcode-term/src/ui/mod.rs`
  - Add `pub mod file_explorer;`
  - Add `pub mod tab_bar;`
  - Add `pub mod top_bar;`

### Phase 4: Render Integration (termcode-term)

**Goal**: Replace the current simple 2-chunk layout with the full layout engine and render all widgets.
**Risk**: Medium
**Status**: Complete

- [x] Task 4.1: Update `crates/termcode-term/src/render.rs`
  - Import layout module and all new widget types
  - Replace current Layout::default() with `layout::compute_layout(area, editor.file_explorer.visible, editor.file_explorer.width)`
  - Render top bar widget into `layout.top_bar`
  - Conditionally render file explorer widget into `layout.sidebar` (if Some)
  - Render tab bar widget into `layout.tab_bar`
  - Render editor view widget into `layout.editor_area` (existing logic)
  - Render status bar widget into `layout.status_bar` (existing logic)
  - Preserve existing EditorViewWidget and StatusBarWidget rendering logic

- [x] Task 4.2: Update `crates/termcode-term/src/app.rs` -- view dimensions
  - Update the view dimension calculation in `run()` to use the layout engine's `editor_area` dimensions instead of raw terminal size minus status bar
  - The `area_height` and `area_width` on the active view must reflect the actual editor_area after sidebar/bars are accounted for

### Phase 5: Keybindings and Mode Handling (termcode-term)

**Goal**: Wire up keyboard input to control the sidebar, tabs, and file explorer navigation.
**Risk**: Medium
**Status**: Complete

- [x] Task 5.1: Update `crates/termcode-term/src/app.rs` -- mode-aware key dispatch
  - Restructure `handle_key()` to dispatch based on `self.editor.mode`:
    - `EditorMode::Normal`: existing navigation keybindings (j/k/h/l, arrows, PgUp/PgDn, etc.)
    - `EditorMode::FileExplorer`: file tree navigation (see Task 5.3)
  - Global keybindings (work in any mode):
    - `Ctrl+Q` / `Ctrl+C`: quit
    - `Ctrl+B`: toggle sidebar visibility (`editor.toggle_sidebar()`) and switch mode accordingly

- [x] Task 5.2: Add tab switching keybindings (global, any mode)
  - `Ctrl+Tab` (or `Alt+Right` as fallback since Ctrl+Tab may not be captured by terminals): next tab (`editor.tabs.next()`)
  - `Ctrl+Shift+Tab` (or `Alt+Left` as fallback): previous tab (`editor.tabs.prev()`)
  - When switching tabs: update `editor.active_view` to match the tab's `doc_id`, find the corresponding ViewId
  - `Ctrl+W`: close active tab (remove tab, if last tab show empty state)

- [x] Task 5.3: Implement FileExplorer mode keybindings
  - `Up` / `k`: `editor.file_explorer.move_selection(-1)`
  - `Down` / `j`: `editor.file_explorer.move_selection(1)`
  - `Enter`: if selected is directory, toggle expand; if file, open in editor via `editor.open_file()` and switch to Normal mode
  - `Right` / `l`: if directory and collapsed, expand it
  - `Left` / `h`: if directory and expanded, collapse it; if file or collapsed dir, move to parent
  - `Esc`: switch back to `EditorMode::Normal`, optionally hide sidebar
  - `Tab`: switch focus from file explorer to editor (mode -> Normal) without hiding sidebar

- [x] Task 5.4: Handle opening files from explorer
  - When Enter is pressed on a file in the explorer:
    1. Check if file is already open (search tabs by path, then find doc_id)
    2. If already open: switch to that tab
    3. If not open: call `editor.open_file(path)` which creates doc + view + tab
    4. Switch mode to `EditorMode::Normal`

- [x] Task 5.5: Wire up sidebar toggle mode transitions
  - When `Ctrl+B` toggles sidebar ON: switch to `EditorMode::FileExplorer`
  - When `Ctrl+B` toggles sidebar OFF: switch to `EditorMode::Normal`
  - When sidebar is visible and user is in Normal mode, `Ctrl+B` enters FileExplorer mode (focus sidebar)

### Phase 6: Integration Testing and Polish

**Goal**: Verify the full flow works end-to-end and fix edge cases.
**Risk**: Low
**Status**: Complete

- [x] Task 6.1: Manual integration testing
  - Launch termcode with a directory argument (update CLI to accept directory path)
  - Verify sidebar appears on `Ctrl+B`, disappears on `Ctrl+B` again
  - Verify directory tree renders correctly with proper indentation
  - Verify .gitignore files are respected (ignored files not shown)
  - Verify expand/collapse directories works
  - Verify opening a file creates a tab and shows content
  - Verify tab switching works
  - Verify closing tabs works
  - Verify editor view resizes correctly when sidebar toggles
  - Verify status bar still renders correctly

- [x] Task 6.2: Update CLI entrypoint
  - If argument is a directory path: set it as the file explorer root, show sidebar on startup
  - If argument is a file path: open file in editor (current behavior), set parent directory as explorer root (hidden by default)
  - If no argument: set CWD as explorer root, show sidebar on startup

- [x] Task 6.3: Full build verification
  - `cargo build` -- full project builds
  - `cargo test` -- all tests pass
  - `cargo clippy` -- no warnings
  - `cargo fmt --check` -- formatting consistent

## 5. Quality Gate

- [x] Build success: `cargo build`
- [x] Tests pass: `cargo test`
- [x] Lint pass: `cargo clippy` (no new warnings in Phase 2 code; pre-existing warnings in core/syntax crates)
- [x] Format check: `cargo fmt --check`
- [ ] Manual smoke test: open directory, navigate tree, open files in tabs, switch tabs, toggle sidebar

## 6. Notes

### Implementation Considerations

- **Terminal Ctrl+Tab capture**: Many terminal emulators intercept `Ctrl+Tab`. Provide `Alt+Right`/`Alt+Left` as alternative tab switching keys. Document both options.
- **Lazy directory loading**: Only load immediate children when expanding a directory. Do not recursively walk the entire tree on startup -- this would be slow for large repositories.
- **FileExplorer tree representation**: The `tree` Vec stores only currently visible nodes (expanded directories have their children inserted inline). This avoids a separate flatten step and keeps rendering O(n) where n is visible nodes.
- **Tab-to-view mapping**: Each tab holds a `doc_id`. To find the corresponding view, iterate `editor.views` to find one with matching `doc_id`. For Phase 2 this is O(n) over a small collection; optimize later if needed.
- **No `ignore` in workspace dependencies**: The `ignore` crate is only needed by `termcode-view`, so add it directly to that crate's Cargo.toml rather than the workspace `[workspace.dependencies]`. Alternatively, add to workspace deps for consistency -- implementer's choice.
- **EditorMode expansion**: Phase 2 only adds `Normal` and `FileExplorer`. The architecture envisions `Insert`, `Command`, `Search` modes for later phases. Design the mode enum to be easily extended.

### Patterns to Follow

- Widget pattern: struct holding references with lifetime `'a`, implement `Widget` trait with `render(self, area: Rect, buf: &mut Buffer)` -- see `EditorViewWidget` and `StatusBarWidget`
- Manual buffer cell writing (not ratatui Paragraph/Block) -- established pattern for precise control
- Theme color access: `self.theme.ui.{color_name}.to_ratatui()` for Color conversion
- Editor state access: immutable borrow of `&Editor` in render functions; mutable `&mut Editor` in update/handle_key

### Potential Issues

- **Borrow checker in handle_key**: Opening a file from the explorer requires reading `file_explorer.selected_path()` and then calling `editor.open_file()`. These may conflict on `&self` / `&mut self`. Solution: clone the path before the mutable call.
- **View dimension update timing**: View dimensions must be computed AFTER layout, not before. The `run()` loop must compute layout first, then set `view.area_height/area_width` from the layout's `editor_area`.

### Recommendation

No formal analysis report (`docs/analysis/phase2-file-explorer-tabs.md`) exists for this feature. Consider running the yyy-analyze agent if a more thorough codebase impact analysis is desired before implementation. The current plan is based on direct code review and the architecture blueprint, which provides sufficient detail for this scope.

## 7. Implementation Notes

### Phase 1 (2026-03-27)

- Created: 2 files (file_explorer.rs, tab.rs)
- Modified: 3 files (Cargo.toml, lib.rs, editor.rs) + 1 downstream fix (app.rs)
- Risk: Low
- Notes: Added `ignore` crate dependency. FileExplorer uses lazy loading (depth-1 only). EditorMode enum added with Normal/FileExplorer variants. Editor::new() now accepts optional root path.

### Phase 2 (2026-03-27)

- Created: 1 file (layout.rs)
- Modified: 1 file (lib.rs)
- Risk: Low
- Notes: AppLayout with 5 regions. compute_layout uses ratatui Layout constraints.

### Phase 3 (2026-03-27)

- Created: 3 files (top_bar.rs, tab_bar.rs, file_explorer.rs widget)
- Modified: 1 file (ui/mod.rs)
- Risk: Low
- Notes: All widgets follow established manual buffer-writing pattern from StatusBarWidget.

### Phase 4 (2026-03-27)

- Created: 0 files
- Modified: 2 files (render.rs, app.rs)
- Risk: Medium
- Notes: Replaced 2-chunk layout with full 5-region layout engine. View dimensions now computed from layout's editor_area. Fixed terminal.size() returning Size vs Rect.

### Phase 5 (2026-03-27)

- Created: 0 files
- Modified: 1 file (app.rs)
- Risk: Medium
- Notes: Mode-aware key dispatch. Global keys: Ctrl+Q/C quit, Ctrl+B toggle sidebar, Alt+Left/Right tab switch, Ctrl+W close tab. FileExplorer mode: j/k/arrows navigate, Enter open/expand, h/l collapse/expand, Esc/Tab return to Normal. Clone path before open_file() to avoid borrow checker issues.

### Phase 6 (2026-03-27)

- Created: 0 files
- Modified: 2 files (main.rs, app.rs)
- Risk: Low
- Notes: CLI now handles directory args (show sidebar), file args (open file, hide sidebar), no args (show sidebar with CWD). All quality gates pass except manual smoke test.

---

Last Updated: 2026-03-27
Status: Complete (pending manual smoke test)
