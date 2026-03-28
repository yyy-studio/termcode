# Phase 4: Search, Fuzzy Finder, and Command Palette Implementation Plan

**Created**: 2026-03-27
**Analysis Report**: N/A (no formal analysis report; planning based on architecture blueprint, codebase review, and team lead specifications)
**Status**: Complete

## 1. Requirements Summary

### Functional Requirements

- [FR-P4-01] Search overlay: open with `Ctrl+F`, text input for query, highlight all matches in editor
- [FR-P4-02] Search navigation: `Enter`/`n` for next match, `Shift+Enter`/`N` for previous match
- [FR-P4-03] Replace overlay: open with `Ctrl+H`, replace current match, replace all matches
- [FR-P4-04] Search overlay UI: floating panel at top of editor area (VS Code style)
- [FR-P4-05] Fuzzy file finder: open with `Ctrl+P`, walk project files respecting .gitignore
- [FR-P4-06] Fuzzy matching: simple scoring algorithm on file paths (no nucleo for MVP)
- [FR-P4-07] Fuzzy finder UI: centered overlay popup, text input, scrollable results list
- [FR-P4-08] Fuzzy finder actions: type to filter, `Enter` to open file, `Esc` to cancel
- [FR-P4-09] Command palette: open with `Ctrl+Shift+P`, list all CommandRegistry commands
- [FR-P4-10] Command palette: fuzzy filter as user types, `Enter` to execute, `Esc` to cancel
- [FR-P4-11] Command palette UI: centered overlay popup (same layout as fuzzy finder)
- [FR-P4-12] All three features use `EditorMode` variants for mode-based input dispatch
- [FR-P4-13] `Esc` dismisses any overlay and returns to previous mode (Normal)

**Deferred to future phase**:

- Regex search mode (would require adding `regex` crate dependency; not needed for MVP literal search)

### Architecture Constraints (from docs/architecture/termcode.md)

- TEA pattern: Event -> Update -> Render (all state changes through this cycle)
- Command Pattern: user actions map to named commands through CommandRegistry
- Crate boundaries: view has no UI awareness; term owns widgets and rendering
- Search state lives in `termcode-view` (Editor/Document level); UI widgets in `termcode-term`
- Overlay rendering: widgets render on top of editor area in render.rs after base widgets

## 2. Analysis Report Reference

### Reference Documents

- Architecture Blueprint: `docs/architecture/termcode.md`
- Project Plan: `/Users/hankyung/.claude/plans/cosmic-prancing-whisper.md`
- Phase 3 Plan (pattern reference): `docs/plans/phase3-editing.md`

### Applied Recommendations (from architecture)

- Architecture specifies `command_palette.rs`, `fuzzy_finder.rs`, `search.rs` under `termcode-term/src/ui/`
- `EditorMode` enum is the mode dispatch mechanism -- add Search, FuzzyFinder, CommandPalette variants
- `InputMapper` provides per-mode keybinding resolution -- add mode-specific bindings for each overlay
- `CommandRegistry` already stores `name: &'static str` per command -- use for palette display
- `ignore` crate already in termcode-view for .gitignore-aware file walking -- reuse for fuzzy finder
- `Transaction::replace` exists in termcode-core -- use for search/replace operations

### Reusable Code

| Code                                              | Location                                        | Purpose                                    |
| ------------------------------------------------- | ----------------------------------------------- | ------------------------------------------ |
| `CommandRegistry` with `commands` HashMap         | `crates/termcode-term/src/command.rs`           | Enumerate all commands for palette         |
| `CommandEntry.name` field                         | `crates/termcode-term/src/command.rs:12`        | Display name in command palette            |
| `InputMapper` per-mode resolution                 | `crates/termcode-term/src/input.rs`             | Add new mode keybindings                   |
| `EditorMode` enum                                 | `crates/termcode-view/src/editor.rs:14-19`      | Add new mode variants                      |
| `Editor.switch_mode()`                            | `crates/termcode-view/src/editor.rs:104`        | Mode transitions for overlays              |
| `FileExplorer::load_children` / `WalkBuilder`     | `crates/termcode-view/src/file_explorer.rs:128` | Pattern for .gitignore-aware file walking  |
| `Transaction::replace()`                          | `crates/termcode-core/src/transaction.rs:331`   | Replace text for search/replace            |
| `Buffer.text()` -> `Rope`                         | `crates/termcode-core/src/buffer.rs:84`         | Text search via Rope API                   |
| `render::render()`                                | `crates/termcode-term/src/render.rs:12`         | Add overlay rendering after base widgets   |
| `App::handle_key()`                               | `crates/termcode-term/src/app.rs:110`           | Add mode dispatch for overlay key handling |
| `UiColors` (selection, border, status_bar colors) | `crates/termcode-theme/src/theme.rs:8`          | Styling for overlay panels                 |

### Constraints

- No formal analysis report; specifications derived from architecture blueprint and team lead input
- Current `EditorMode` has only `Normal`, `Insert`, `FileExplorer` -- must add three new variants
- Current `InputMapper::resolve()` match on line 97-101 must handle new modes
- `App::handle_key()` dispatches by mode -- needs overlay-specific branches
- No overlay/popup infrastructure exists -- must create base overlay widget
- Fuzzy matching should use a simple built-in algorithm (no nucleo dependency for MVP)
- Regex search deferred to future phase (avoids adding `regex` crate dependency for MVP)
- `UiColors` may need `search_match` and `search_match_active` colors for match highlighting
- Search state must live in termcode-view (Editor level) so rendering can access match positions
- File walking for fuzzy finder should be cached (not re-walked on every keystroke)
- `CommandRegistry` is owned by `App`, not `Editor` -- command palette needs access pattern
- `SearchState::find_matches` takes `&str` (not `&ropey::Rope`) to avoid cross-crate type exposure; caller converts Rope to String before calling

## 3. Impact Analysis (Dry-Run)

### Overall Risk: Medium

This phase adds three new overlay features. All changes are additive -- no existing functionality is deleted or broken. The main risk areas are: modifying the mode dispatch (app.rs, input.rs) which is the central input handling path, and modifying EditorMode which is used across many files. However, the pattern is well-established from Phase 3 (adding Insert mode, FileExplorer mode).

### Files to Create

| File                                             | Risk | Description                                                                |
| ------------------------------------------------ | ---- | -------------------------------------------------------------------------- |
| `crates/termcode-view/src/search.rs`             | Low  | SearchState: query, matches, current index, replace string                 |
| `crates/termcode-view/src/fuzzy.rs`              | Low  | FuzzyFinderState: file list cache, query, filtered results, selected index |
| `crates/termcode-view/src/palette.rs`            | Low  | CommandPaletteState: query string, filtered commands, selected index       |
| `crates/termcode-term/src/ui/overlay.rs`         | Low  | Shared overlay widget: border, text input, scrollable list rendering       |
| `crates/termcode-term/src/ui/search.rs`          | Low  | SearchOverlayWidget: search/replace UI at top of editor                    |
| `crates/termcode-term/src/ui/fuzzy_finder.rs`    | Low  | FuzzyFinderWidget: centered popup with file list                           |
| `crates/termcode-term/src/ui/command_palette.rs` | Low  | CommandPaletteWidget: centered popup with command list                     |

### Files to Modify

| File                                         | Risk   | Description                                                                           |
| -------------------------------------------- | ------ | ------------------------------------------------------------------------------------- |
| `Cargo.toml` (workspace root)                | Low    | No new workspace dependencies needed for MVP (regex deferred)                         |
| `crates/termcode-view/Cargo.toml`            | Low    | No new dependencies needed (already has `ignore` for file walking)                    |
| `crates/termcode-view/src/editor.rs`         | Medium | Add 3 EditorMode variants, search/fuzzy/palette state fields to Editor                |
| `crates/termcode-view/src/lib.rs`            | Low    | Add `pub mod search; pub mod fuzzy; pub mod palette;`                                 |
| `crates/termcode-term/src/input.rs`          | Medium | Add mode keybindings for Search, FuzzyFinder, CommandPalette; update resolve()        |
| `crates/termcode-term/src/command.rs`        | Medium | Register search/fuzzy/palette commands; add methods to list all commands              |
| `crates/termcode-term/src/app.rs`            | High   | Add overlay mode handling in handle_key(); manage overlay lifecycle                   |
| `crates/termcode-term/src/render.rs`         | Medium | Render overlay widgets on top of editor area based on mode                            |
| `crates/termcode-term/src/ui/mod.rs`         | Low    | Add `pub mod overlay; pub mod search; pub mod fuzzy_finder; pub mod command_palette;` |
| `crates/termcode-term/src/ui/editor_view.rs` | Medium | Highlight search matches in editor rendering                                          |
| `crates/termcode-theme/src/theme.rs`         | Low    | Add search_match, search_match_active to UiColors                                     |
| `crates/termcode-theme/src/loader.rs`        | Low    | Parse new theme keys                                                                  |
| `runtime/themes/one-dark.toml`               | Low    | Add search match colors                                                               |
| `docs/architecture/termcode.md`              | Low    | Document new EditorMode variants, overlay infrastructure, state models                |

### Files to Delete

| File | Risk | Description |
| ---- | ---- | ----------- |
| None | -    | -           |

### Destructive Operations

- None. All changes are additive or extend existing code. Existing viewer/editor functionality is fully preserved.

### Rollback Plan

- All work on a feature branch; full rollback via `git checkout main && git branch -D feature/phase4-search`
- No database changes; no external system modifications
- Cargo.lock changes are auto-resolved by reverting Cargo.toml

## 4. Implementation Order

### Phase 1: State Models (termcode-view)

**Goal**: Define the state structures for search, fuzzy finder, and command palette. These are pure data types with no UI awareness, following the crate boundary rules.
**Risk**: Low
**Status**: Complete

- [x] Task 1.1: Create `crates/termcode-view/src/search.rs`
  - Define `SearchState` struct:
    - `pub query: String` -- current search query text
    - `pub matches: Vec<SearchMatch>` -- list of all match positions in current document
    - `pub current_match: Option<usize>` -- index into matches for "active" match
    - `pub replace_text: String` -- replacement string (empty when not in replace mode)
    - `pub replace_mode: bool` -- whether replace input is visible
    - `pub cursor_pos: usize` -- cursor position within query input field
  - Define `SearchMatch` struct:
    - `pub start: usize` -- byte offset start in document
    - `pub end: usize` -- byte offset end in document
  - Implement `SearchState::new() -> Self` -- all fields empty/default
  - Implement `SearchState::clear(&mut self)` -- reset all state
  - Implement `SearchState::find_matches(&mut self, text: &str)` -- perform literal case-insensitive substring search:
    - Takes `&str` (caller converts Rope to String to avoid cross-crate ropey type exposure)
    - Walk through text, collect all (start, end) byte offset pairs into `self.matches`
    - Set `self.current_match` to first match at or after cursor position (if any)
  - Implement `SearchState::next_match(&mut self)` -- advance current_match, wrap around
  - Implement `SearchState::prev_match(&mut self)` -- go to previous match, wrap around
  - Implement `SearchState::current(&self) -> Option<&SearchMatch>` -- get active match
  - Implement `SearchState::match_count(&self) -> usize`
  - Add `#[derive(Debug, Clone)]` on SearchMatch, `#[derive(Debug)]` on SearchState

- [x] Task 1.2: Create `crates/termcode-view/src/fuzzy.rs`
  - Define `FuzzyFinderState` struct:
    - `pub query: String` -- current filter text
    - `pub all_files: Vec<String>` -- all project file paths (relative to root), cached
    - `pub filtered: Vec<FuzzyMatch>` -- filtered + scored results
    - `pub selected: usize` -- index into filtered results
    - `pub scroll_offset: usize` -- for scrolling the results list
    - `pub cursor_pos: usize` -- cursor position within query input
  - Define `FuzzyMatch` struct:
    - `pub path: String` -- relative file path
    - `pub score: i64` -- match score (higher is better)
    - `pub indices: Vec<usize>` -- matched character indices (for highlight rendering)
  - Implement `FuzzyFinderState::new() -> Self`
  - Implement `FuzzyFinderState::load_files(&mut self, root: &Path)` -- walk project using `ignore::WalkBuilder`:
    - Walk recursively (no max_depth limit unlike file explorer)
    - Collect file paths only (skip directories)
    - Store relative paths (strip root prefix)
    - Sort alphabetically
  - Implement `FuzzyFinderState::update_filter(&mut self)` -- re-score and sort all_files against query:
    - For each file path, compute fuzzy score using simple algorithm
    - Filter out zero-score entries
    - Sort by score descending, then alphabetically for ties
    - Reset selected to 0, scroll_offset to 0
  - Implement `fuzzy_score(query: &str, target: &str) -> Option<(i64, Vec<usize>)>` -- simple scoring:
    - Case-insensitive matching
    - Each query character must appear in target in order (subsequence match)
    - Score bonuses: consecutive matches, start-of-word matches, path separator proximity
    - Return None if not a subsequence match
  - Implement `FuzzyFinderState::move_selection(&mut self, delta: i32)` -- navigate results
  - Implement `FuzzyFinderState::selected_path(&self) -> Option<&str>` -- get currently selected file
  - Add `#[derive(Debug, Clone)]` on FuzzyMatch

- [x] Task 1.3: Create `crates/termcode-view/src/palette.rs`
  - Define `PaletteItem` struct:
    - `pub id: String` -- command ID (e.g. "file.save")
    - `pub name: String` -- display name (e.g. "Save File")
  - Define `CommandPaletteState` struct:
    - `pub query: String` -- current filter text
    - `pub all_commands: Vec<PaletteItem>` -- all registered commands
    - `pub filtered: Vec<PaletteItem>` -- filtered results
    - `pub selected: usize` -- index into filtered
    - `pub scroll_offset: usize` -- for scrolling
    - `pub cursor_pos: usize` -- cursor position within query input
  - Implement `CommandPaletteState::new() -> Self`
  - Implement `CommandPaletteState::load_commands(&mut self, commands: Vec<PaletteItem>)` -- populate from registry
  - Implement `CommandPaletteState::update_filter(&mut self)` -- filter commands by fuzzy match on name:
    - Reuse the same `fuzzy_score` function from fuzzy.rs (or a shared utility)
    - Filter and sort by score
  - Implement `CommandPaletteState::move_selection(&mut self, delta: i32)`
  - Implement `CommandPaletteState::selected_command(&self) -> Option<&PaletteItem>`
  - Add `#[derive(Debug, Clone)]` on PaletteItem

- [x] Task 1.4: Update `crates/termcode-view/src/editor.rs`
  - Add `use crate::search::SearchState;`
  - Add `use crate::fuzzy::FuzzyFinderState;`
  - Add `use crate::palette::CommandPaletteState;`
  - Add `EditorMode::Search` variant
  - Add `EditorMode::FuzzyFinder` variant
  - Add `EditorMode::CommandPalette` variant
  - Add fields to `Editor`:
    - `pub search: SearchState` -- initialized to `SearchState::new()`
    - `pub fuzzy_finder: FuzzyFinderState` -- initialized to `FuzzyFinderState::new()`
    - `pub command_palette: CommandPaletteState` -- initialized to `CommandPaletteState::new()`
  - Update `Editor::new()` to initialize these fields

- [x] Task 1.5: Update `crates/termcode-view/src/lib.rs`
  - Add `pub mod search;`
  - Add `pub mod fuzzy;`
  - Add `pub mod palette;`

- [x] Task 1.6: Verify Phase 1 builds
  - Run `cargo check -p termcode-view`
  - Run `cargo test -p termcode-view`
  - Run `cargo clippy -p termcode-view`

### Phase 2: Theme and Search Highlighting (termcode-theme + editor_view)

**Goal**: Add search match highlight colors to the theme system and integrate match highlighting into the editor view rendering.
**Risk**: Medium
**Status**: Complete
**Depends on**: Phase 1

- [x] Task 2.1: Add search match colors to `crates/termcode-theme/src/theme.rs`
  - Add to `UiColors`:
    - `pub search_match: Color` -- background for all search matches (e.g. dark yellow/amber)
    - `pub search_match_active: Color` -- background for current active match (e.g. brighter orange)
  - Update `Default for UiColors` with reasonable defaults:
    - `search_match: Color::Rgb(229, 192, 123)` (One Dark yellow, semi-transparent effect)
    - `search_match_active: Color::Rgb(209, 154, 102)` (One Dark orange, stands out more)

- [x] Task 2.2: Update theme loader `crates/termcode-theme/src/loader.rs`
  - Parse `ui.search_match` and `ui.search_match_active` from theme TOML
  - Fall back to defaults if not specified

- [x] Task 2.3: Update `runtime/themes/one-dark.toml`
  - Add `search_match` and `search_match_active` color definitions under `[ui]`

- [x] Task 2.4: Add search match highlighting to `crates/termcode-term/src/ui/editor_view.rs`
  - After base rendering (syntax + cursor), render search match highlights:
    - Access `editor.search.matches` (passed via widget constructor or via Editor reference)
    - For each visible line, check if any SearchMatch overlaps
    - Apply `search_match` background color to matching ranges
    - Apply `search_match_active` background to the current match
  - The widget needs access to SearchState -- update constructor to accept `Option<&SearchState>`
  - Only highlight when search has active matches (not when overlay is closed but matches are stale)

- [x] Task 2.5: Verify Phase 2 builds
  - Run `cargo check --workspace`
  - Run `cargo clippy --workspace`

### Phase 3: Overlay Widget Infrastructure (termcode-term)

**Goal**: Create the shared overlay rendering primitives used by all three features. This establishes the visual pattern: a floating panel with text input and scrollable list.
**Risk**: Low
**Status**: Complete
**Depends on**: Phase 1

- [x] Task 3.1: Create `crates/termcode-term/src/ui/overlay.rs`
  - Define `OverlayConfig` struct:
    - `pub width_percent: u16` -- overlay width as percentage of parent area (e.g. 60)
    - `pub max_height: u16` -- maximum height in lines
    - `pub position: OverlayPosition` -- Top (for search) or Center (for fuzzy/palette)
  - Define `OverlayPosition` enum: `Top`, `Center`
  - Implement `compute_overlay_rect(parent: Rect, config: &OverlayConfig) -> Rect` -- calculate overlay position:
    - `Top`: full width of editor area, fixed height at top of editor area
    - `Center`: centered horizontally and vertically within parent, respecting width_percent and max_height
  - Implement `render_overlay_frame(area: Rect, buf: &mut Buffer, theme: &Theme)` -- draw border and background:
    - Fill area with `background` color
    - Draw border using `border` theme color (single-line box characters)
    - This is a helper function, not a standalone widget
  - Implement `render_input_line(area: Rect, buf: &mut Buffer, prompt: &str, text: &str, cursor_pos: usize, theme: &Theme)` -- render the text input field:
    - Render prompt label (e.g. "Search: ", "Open: ", "> ")
    - Render input text with cursor indicator
    - Handle text overflow (scroll input text if longer than area)
  - Implement `render_result_list(area: Rect, buf: &mut Buffer, items: &[ListItem], selected: usize, scroll_offset: usize, theme: &Theme)` -- render scrollable list:
    - Define `ListItem` struct: `text: String`, `secondary: Option<String>`, `highlights: Vec<usize>`
    - Render visible items with selection highlight
    - Highlight matched characters (using indices from fuzzy matching)
    - Show scroll indicator if items overflow

- [x] Task 3.2: Update `crates/termcode-term/src/ui/mod.rs`
  - Add `pub mod overlay;`
  - Add `pub mod search;`
  - Add `pub mod fuzzy_finder;`
  - Add `pub mod command_palette;`

- [x] Task 3.3: Verify Phase 3 builds
  - Run `cargo check -p termcode-term`
  - Run `cargo clippy -p termcode-term`

### Phase 4: Search/Replace Feature (full stack)

**Goal**: Implement the complete search/replace feature: commands, keybindings, overlay widget, and integration with the app loop. Search is literal case-insensitive only (regex deferred).
**Risk**: Medium-High
**Status**: Complete
**Depends on**: Phase 2, Phase 3

- [x] Task 4.1: Create `crates/termcode-term/src/ui/search.rs` -- SearchOverlayWidget
  - Define `SearchOverlayWidget` struct holding references to `SearchState` and `Theme`
  - Implement `Widget for SearchOverlayWidget`:
    - Position at top of editor area (use `OverlayConfig { position: Top, ... }`)
    - Height: 1 line for search-only, 2 lines when replace is visible
    - Render search input line: "Search: {query}" with cursor
    - Render match count indicator: "N of M" or "No results"
    - Render navigation hints: icons/text for prev/next
    - If replace_mode, render second line: "Replace: {replace_text}"
  - Keep the widget compact -- this is a toolbar-style overlay, not a full popup

- [x] Task 4.2: Register search/replace commands in `crates/termcode-term/src/command.rs`
  - Register: `"search.open"` / "Find" -- open search overlay (`Ctrl+F`)
  - Register: `"search.open_replace"` / "Find and Replace" -- open search+replace overlay (`Ctrl+H`)
  - Register: `"search.next"` / "Find Next" -- go to next match
  - Register: `"search.prev"` / "Find Previous" -- go to previous match
  - Register: `"search.replace_current"` / "Replace" -- replace active match
  - Register: `"search.replace_all"` / "Replace All" -- replace all matches
  - Register: `"search.close"` / "Close Search" -- dismiss overlay
  - Command handlers:
    - `cmd_search_open`: set mode to Search, clear query, focus search input
    - `cmd_search_open_replace`: set mode to Search, set replace_mode = true
    - `cmd_search_next`: call `editor.search.next_match()`, scroll to match
    - `cmd_search_prev`: call `editor.search.prev_match()`, scroll to match
    - `cmd_search_replace_current`: apply `Transaction::replace` for current match, re-run search
    - `cmd_search_replace_all`: iterate all matches in reverse order, apply Transaction::replace for each, re-run search
    - `cmd_search_close`: switch mode back to Normal, clear matches
  - Important for replace: apply replacements in reverse byte-offset order to avoid invalidating subsequent match positions
  - After replace operations, re-run `find_matches` to update match list

- [x] Task 4.3: Add search keybindings to `crates/termcode-term/src/input.rs`
  - Add `search: Vec<(KeyEvent, CommandId)>` field to InputMapper
  - Define Search mode keybindings:
    - `Esc` -> `"search.close"`
    - `Enter` -> `"search.next"`
    - `Shift+Enter` -> `"search.prev"`
    - `Ctrl+H` -> toggle replace mode (when already in search)
  - Add global keybindings:
    - `Ctrl+F` -> `"search.open"` (add to global list)
    - `Ctrl+H` -> `"search.open_replace"` (add to global list)
  - Update `InputMapper::resolve()` match to handle `EditorMode::Search`

- [x] Task 4.4: Integrate search into `crates/termcode-term/src/app.rs`
  - In `handle_key()`, add `EditorMode::Search` branch:
    - Check search-mode keybindings via InputMapper
    - Handle printable character input: append to `editor.search.query`, re-run `find_matches`
    - Handle `Backspace`: remove from query, re-run `find_matches`
    - Handle `Tab`: switch focus between search and replace input fields
    - When `search.close` executes, return to Normal mode
  - For `find_matches` calls: convert active document Rope to String via `doc.buffer.text().to_string()`, then pass `&str` to `SearchState::find_matches`
  - When search.next/prev executes, scroll editor view to the match position:
    - Convert match byte offset to Position using `doc.buffer.byte_to_pos()`
    - Update view cursor and call `view.ensure_cursor_visible()`
  - Handle replace operations: execute transaction, sync cursor

- [x] Task 4.5: Integrate search overlay rendering in `crates/termcode-term/src/render.rs`
  - After base editor rendering, check `editor.mode`:
    - If `EditorMode::Search`, render `SearchOverlayWidget` on top of editor area
  - Pass `&editor.search` to `EditorViewWidget` for match highlighting (Task 2.4)

- [x] Task 4.6: Write tests for search
  - Test `SearchState::find_matches` with literal query on sample text
  - Test `SearchState::next_match` / `prev_match` wrapping behavior
  - Test replace operation: single replace, replace all
  - Test case-insensitive matching

- [x] Task 4.7: Verify Phase 4 builds and tests pass
  - Run `cargo build --workspace`
  - Run `cargo test --workspace`
  - Run `cargo clippy --workspace`
  - Run `cargo fmt --check`

### Phase 5: Fuzzy File Finder Feature (full stack)

**Goal**: Implement the fuzzy file finder: file walking, fuzzy matching, overlay widget, file opening.
**Risk**: Medium
**Status**: Complete
**Depends on**: Phase 3

- [x] Task 5.1: Create `crates/termcode-term/src/ui/fuzzy_finder.rs` -- FuzzyFinderWidget
  - Define `FuzzyFinderWidget` struct holding references to `FuzzyFinderState` and `Theme`
  - Implement `Widget for FuzzyFinderWidget`:
    - Use `OverlayConfig { position: Center, width_percent: 60, max_height: 20 }`
    - Render overlay frame with border
    - Render input line with prompt: "Open: {query}"
    - Render result count: "N files" in top-right or input line
    - Render result list using `render_result_list`:
      - Each item shows relative file path
      - Matched characters highlighted (using `FuzzyMatch.indices`)
      - Selected item highlighted with distinct background
    - Handle empty state: show "No files found" or "Type to search..."

- [x] Task 5.2: Register fuzzy finder commands in `crates/termcode-term/src/command.rs`
  - Register: `"fuzzy.open"` / "Open File" -- open fuzzy finder (`Ctrl+P`)
  - Register: `"fuzzy.close"` / "Close Finder" -- dismiss overlay
  - Command handlers:
    - `cmd_fuzzy_open`: set mode to FuzzyFinder, load files if cache empty, clear query
    - `cmd_fuzzy_close`: switch mode back to Normal

- [x] Task 5.3: Add fuzzy finder keybindings to `crates/termcode-term/src/input.rs`
  - Add `fuzzy_finder: Vec<(KeyEvent, CommandId)>` field to InputMapper
  - Define FuzzyFinder mode keybindings:
    - `Esc` -> `"fuzzy.close"`
    - `Enter` -> open selected file (handled directly in app.rs, not via command)
    - `Up` / `Ctrl+K` -> move selection up
    - `Down` / `Ctrl+J` -> move selection down
  - Add global keybinding:
    - `Ctrl+P` -> `"fuzzy.open"` (add to global list)
  - Update `InputMapper::resolve()` match to handle `EditorMode::FuzzyFinder`

- [x] Task 5.4: Integrate fuzzy finder into `crates/termcode-term/src/app.rs`
  - In `handle_key()`, add `EditorMode::FuzzyFinder` branch:
    - Check fuzzy finder keybindings via InputMapper
    - Handle printable character input: append to query, call `update_filter()`
    - Handle `Backspace`: remove from query, call `update_filter()`
    - Handle `Enter`: get selected file path, call `editor.open_file()`, switch to Normal mode
    - Handle `Up`/`Down`: call `move_selection()`
  - When `fuzzy.open` command executes:
    - Call `editor.fuzzy_finder.load_files(&editor.file_explorer.root)` if files not loaded
    - Clear query, switch mode
  - When file is opened from fuzzy finder, handle same as explorer (check for existing tab):
    - Reuse existing tab if file already open
    - Otherwise call `editor.open_file()`

- [x] Task 5.5: Integrate fuzzy finder overlay rendering in `crates/termcode-term/src/render.rs`
  - If `editor.mode == EditorMode::FuzzyFinder`, render `FuzzyFinderWidget`
  - Render centered over editor area (may span sidebar too for wider visibility)

- [x] Task 5.6: Write tests for fuzzy finder
  - Test `fuzzy_score` function: exact match, subsequence match, no match, case insensitivity
  - Test `FuzzyFinderState::update_filter` with sample file lists
  - Test `FuzzyFinderState::move_selection` bounds checking
  - Test `FuzzyFinderState::load_files` (using a temp directory fixture)

- [x] Task 5.7: Verify Phase 5 builds and tests pass
  - Run `cargo build --workspace`
  - Run `cargo test --workspace`
  - Run `cargo clippy --workspace`
  - Run `cargo fmt --check`

### Phase 6: Command Palette Feature (full stack)

**Goal**: Implement the command palette: command enumeration, fuzzy filtering, overlay widget, command execution.
**Risk**: Medium
**Status**: Complete
**Depends on**: Phase 3, Phase 5 (reuses fuzzy_score and overlay patterns)

- [x] Task 6.1: Create `crates/termcode-term/src/ui/command_palette.rs` -- CommandPaletteWidget
  - Define `CommandPaletteWidget` struct holding references to `CommandPaletteState` and `Theme`
  - Implement `Widget for CommandPaletteWidget`:
    - Use `OverlayConfig { position: Center, width_percent: 50, max_height: 15 }`
    - Render overlay frame with border
    - Render input line with prompt: "> {query}"
    - Render result list:
      - Each item shows command name (e.g. "Save File")
      - Secondary text shows command ID (e.g. "file.save") in dimmer color
      - Matched characters highlighted
      - Selected item highlighted
    - Handle empty state: "No matching commands"

- [x] Task 6.2: Add command enumeration to `crates/termcode-term/src/command.rs`
  - Add `CommandRegistry::list_commands(&self) -> Vec<(&str, &str)>` -- returns (id, name) pairs
  - This provides the data source for command palette population

- [x] Task 6.3: Register command palette commands in `crates/termcode-term/src/command.rs`
  - Register: `"palette.open"` / "Command Palette" -- open command palette (`Ctrl+Shift+P`)
  - Register: `"palette.close"` / "Close Palette" -- dismiss overlay
  - Command handlers:
    - `cmd_palette_open`: populate palette from registry, set mode to CommandPalette
    - `cmd_palette_close`: switch mode back to Normal

- [x] Task 6.4: Add command palette keybindings to `crates/termcode-term/src/input.rs`
  - Add `command_palette: Vec<(KeyEvent, CommandId)>` field to InputMapper
  - Define CommandPalette mode keybindings:
    - `Esc` -> `"palette.close"`
    - `Enter` -> execute selected command (handled in app.rs)
    - `Up` / `Ctrl+K` -> move selection up
    - `Down` / `Ctrl+J` -> move selection down
  - Add global keybinding:
    - `Ctrl+Shift+P` -> `"palette.open"` (add to global list)
  - Update `InputMapper::resolve()` match to handle `EditorMode::CommandPalette`
  - Note: `Ctrl+Shift+P` in crossterm is `KeyModifiers::CONTROL | KeyModifiers::SHIFT`, `KeyCode::Char('P')` (uppercase due to Shift)

- [x] Task 6.5: Integrate command palette into `crates/termcode-term/src/app.rs`
  - In `handle_key()`, add `EditorMode::CommandPalette` branch:
    - Check command palette keybindings via InputMapper
    - Handle printable character input: append to query, call `update_filter()`
    - Handle `Backspace`: remove from query, call `update_filter()`
    - Handle `Enter`: get selected command ID, switch to Normal mode, execute command via CommandRegistry
    - Handle `Up`/`Down`: call `move_selection()`
  - When `palette.open` executes:
    - Collect commands from `self.command_registry.list_commands()`
    - Convert to `Vec<PaletteItem>` and load into `editor.command_palette`
    - Switch mode
  - Note: command execution after palette selection needs care -- switch to Normal mode first, then execute, so the command runs in Normal mode context

- [x] Task 6.6: Integrate command palette overlay rendering in `crates/termcode-term/src/render.rs`
  - If `editor.mode == EditorMode::CommandPalette`, render `CommandPaletteWidget`
  - Render centered over editor area

- [x] Task 6.7: Write tests for command palette
  - Test `CommandPaletteState::update_filter` with sample command list
  - Test fuzzy matching on command names
  - Test `move_selection` bounds checking
  - Test `list_commands` returns all registered commands

- [x] Task 6.8: Verify Phase 6 builds and tests pass
  - Run `cargo build --workspace`
  - Run `cargo test --workspace`
  - Run `cargo clippy --workspace`
  - Run `cargo fmt --check`

### Phase 7: Integration, Polish, and Quality Gate

**Goal**: Final integration testing, edge case handling, quality verification, and documentation update.
**Risk**: Low
**Status**: Complete
**Depends on**: Phase 4, Phase 5, Phase 6

- [x] Task 7.1: Edge case handling
  - Search on empty document (no active document)
  - Search with empty query (should clear matches)
  - Fuzzy finder with no files in project
  - Command palette when no commands match filter
  - Opening overlay while another is active (should close previous)
  - Overlay behavior when terminal is very small (graceful degradation)
  - Esc from any overlay returns to Normal mode cleanly

- [x] Task 7.2: Mode transition integrity
  - Verify: Normal -> Search -> Normal (via Esc)
  - Verify: Normal -> FuzzyFinder -> Normal (via Esc or file open)
  - Verify: Normal -> CommandPalette -> Normal (via Esc or command execute)
  - Verify: Insert -> Ctrl+F -> Search -> Esc -> Normal (not Insert)
  - Verify: FileExplorer -> Ctrl+P -> FuzzyFinder -> Esc -> Normal
  - Verify: overlays work regardless of sidebar visibility

- [x] Task 7.3: Search/replace integrity
  - Verify replace does not corrupt document (undo after replace restores original)
  - Verify replace all with overlapping matches
  - Verify search highlights clear after closing search overlay
  - Verify search across multi-byte UTF-8 characters

- [x] Task 7.4: Final quality gate
  - `cargo build --workspace` -- builds successfully
  - `cargo test --workspace` -- all tests pass
  - `cargo clippy --workspace -- -D warnings` -- zero warnings
  - `cargo fmt --check` -- formatting clean
  - Manual smoke test: open project, Ctrl+F search, Ctrl+P open file, Ctrl+Shift+P run command

- [x] Task 7.5: Update architecture documentation
  - Update `docs/architecture/termcode.md` with:
    - New `EditorMode` variants: `Search`, `FuzzyFinder`, `CommandPalette`
    - Overlay infrastructure: `OverlayConfig`, `OverlayPosition`, shared rendering helpers
    - State models: `SearchState` in termcode-view, `FuzzyFinderState` in termcode-view, `CommandPaletteState` in termcode-view
    - New UI widgets: `SearchOverlayWidget`, `FuzzyFinderWidget`, `CommandPaletteWidget` in termcode-term
    - New commands registered in `CommandRegistry` (search._, fuzzy._, palette.\*)
    - Updated `InputMapper` with per-mode keybinding vectors for overlay modes
    - Note that regex search is planned for a future phase

## 5. Quality Gate

- [x] Build success: `cargo build --workspace`
- [x] Tests pass: `cargo test --workspace`
- [x] Lint pass: `cargo clippy --workspace -- -D warnings`
- [x] Format pass: `cargo fmt --check`
- [x] Manual verification: all three overlays functional

## 6. Notes

### Design Decisions

- **Regex deferred**: Regex search mode is deferred to a future phase. This avoids adding the `regex` crate dependency for MVP. Literal case-insensitive substring search covers the most common use case. When regex is added later, it will require: adding `regex = "1"` to workspace dependencies (root Cargo.toml), adding `regex.workspace = true` to `crates/termcode-view/Cargo.toml`, and re-adding `regex_mode: bool` to SearchState.

- **SearchState::find_matches takes &str**: The method takes `&str` rather than `&ropey::Rope` to avoid exposing the `ropey` crate type across the API boundary. The caller (in termcode-term) converts the Rope to a String before calling. This adds a temporary allocation but keeps the interface clean and testable without ropey.

- **Shared fuzzy_score function**: The fuzzy scoring algorithm is used by both the fuzzy file finder and command palette. It should be defined once in `termcode-view/src/fuzzy.rs` and re-exported or extracted to a shared location. Since both `FuzzyFinderState` and `CommandPaletteState` live in `termcode-view`, placing the function in `fuzzy.rs` and importing it from `palette.rs` is the simplest approach.

- **Search state persistence**: When search overlay is closed, `SearchState.matches` is cleared but `SearchState.query` is preserved. This lets the user re-open search and see their last query (common UX pattern). Match highlights are only rendered when `editor.mode == EditorMode::Search` to avoid stale highlights.

- **File cache for fuzzy finder**: `FuzzyFinderState.all_files` is loaded once when the finder is first opened and reused. It is not refreshed on every open. A future enhancement could add a refresh command or trigger refresh when files change.

- **CommandRegistry access pattern**: `CommandRegistry` lives in `App`, not `Editor`. For the command palette, when opening, `App` extracts the command list from `self.command_registry.list_commands()` and pushes it into `editor.command_palette.all_commands`. This avoids passing `CommandRegistry` through Editor or breaking crate boundaries.

- **Overlay rendering order**: Overlays render last in `render.rs`, on top of the editor area. Only one overlay is active at a time (enforced by EditorMode). The overlay writes directly to the frame buffer, overwriting the editor content underneath.

- **Replace safety**: Replace operations must be applied in reverse byte-offset order (highest match first) to avoid invalidating subsequent match positions. After replace, re-run `find_matches` to get updated positions.

### Patterns to Avoid

- Do not add `nucleo` dependency for MVP -- use simple built-in fuzzy scoring
- Do not add `regex` dependency for MVP -- use literal case-insensitive search
- Do not add popup/overlay state to `termcode-core` -- overlays are view/term concern
- Do not pass `CommandRegistry` into `Editor` -- breaks crate boundary (term depends on view, not reverse)
- Do not render overlays before base widgets -- they must overwrite on top
- Do not pass `&ropey::Rope` across public API boundaries in termcode-view -- use `&str`

### Potential Issues

- **Ctrl+Shift+P detection**: Crossterm reports `Ctrl+Shift+P` as `KeyModifiers::CONTROL | KeyModifiers::SHIFT` with `KeyCode::Char('P')` (uppercase P). This needs to be handled correctly in InputMapper to avoid conflicting with `Ctrl+P` (which is `Char('p')` lowercase). Verify key event representation on macOS terminal.
- **Search performance on large files**: Searching a large file on every keystroke could be slow. The Rope-to-String conversion adds overhead. For MVP, this is acceptable. Future optimization: debounce search, or search only visible area with background full-document search.
- **No new Cargo.toml dependencies for MVP**: Since regex is deferred, no workspace dependency changes are needed. The existing `ignore` crate in termcode-view handles file walking. If regex is added in a future phase, both root `Cargo.toml` and `crates/termcode-view/Cargo.toml` will need updates.

## 7. Implementation Notes

### Phase 1 (2026-03-27)

- Created: 3 files (search.rs, fuzzy.rs, palette.rs in termcode-view)
- Modified: 2 files (editor.rs, lib.rs)
- Risk: Low
- Notes: 21 new tests, all passing

### Phase 2 (2026-03-27)

- Modified: 5 files (theme.rs, loader.rs, one-dark.toml, editor_view.rs, status_bar.rs, input.rs)
- Risk: Medium
- Notes: Added search_match colors, match highlighting in editor, mode labels for status bar

### Phase 3 (2026-03-27)

- Created: 4 files (overlay.rs, search.rs, fuzzy_finder.rs, command_palette.rs in termcode-term/ui)
- Modified: 1 file (ui/mod.rs)
- Risk: Low
- Notes: Shared overlay infrastructure with frame, input line, result list rendering

### Phase 4-6 (2026-03-27)

- Modified: 4 files (command.rs, input.rs, app.rs, render.rs)
- Risk: Medium-High
- Notes: All three features (search/replace, fuzzy finder, command palette) integrated in a single pass since they share the same patterns. 11 new commands registered. Replace in reverse byte-offset order for correctness.

### Phase 7 (2026-03-27)

- Modified: 1 file (docs/architecture/termcode.md)
- Risk: Low
- Notes: All quality gates pass. 52 total tests (31 core + 21 view). 0 clippy warnings. Format clean.

---

Last Updated: 2026-03-27
Status: Complete (Phase 7/7)
