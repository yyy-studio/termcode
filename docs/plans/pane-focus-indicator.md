# Pane Focus Indicator Implementation Plan

**Created**: 2026-03-28
**Analysis Report**: Inline analysis synthesis (no docs/analysis/ file)
**Spec**: docs/specs/pane-focus-indicator.md
**Status**: ✅ Complete

## 1. Requirements Summary

### Functional Requirements

- [FR-PANE-001] PaneFocusStyle enum (`TitleBar`, `Border`, `AccentLine`) in termcode-theme
- [FR-PANE-002] 5 new UiColors fields: `pane_focus_style`, `pane_active_fg/bg`, `pane_inactive_fg/bg`
- [FR-PANE-003] Layout adaptation per style (sidebar_title, sidebar_border areas)
- [FR-PANE-004] Sidebar focus indicator rendering (title bar / border / accent line widgets)
- [FR-PANE-005] Tab bar empty area pane coloring
- [FR-PANE-006] Theme TOML configuration (all fields optional with defaults)
- [FR-PANE-007] Default theme updates for 3 bundled themes

### Database

N/A

### API

N/A

### UI

- Sidebar title bar / border / accent line (3 styles)
- Tab bar empty area color change based on active pane

## 2. Analysis Report Reference

### Reference Documents

- Spec: `docs/specs/pane-focus-indicator.md`
- Analysis synthesis provided by team lead (3 analysts)

### Applied Recommendations

- Widget struct pattern: `struct FooWidget<'a> { state, theme }` + `impl Widget`
- Color resolution: `resolve()` closure in loader.rs:87-91
- Layout split: `Layout::default().direction().constraints().split()`
- Background fill: cell-by-cell `buf[(x, y)].set_char(' ').set_style(style)`
- Tab bar "fill + highlight" pattern for empty area coloring

### Reusable Code

| Code                     | Location                | Purpose                        |
| ------------------------ | ----------------------- | ------------------------------ |
| `resolve()` closure      | `loader.rs:87-91`       | Color resolution from palette  |
| `UiColors::default()`    | `theme.rs:31-58`        | Default color fallbacks        |
| `compute_layout()`       | `layout.rs:11-47`       | Layout structure to extend     |
| `TabBarWidget::render()` | `tab_bar.rs:21-83`      | Fill + paint pattern reference |
| `FileExplorerWidget`     | `file_explorer.rs:9-85` | Sidebar widget pattern         |
| `rect_contains()`        | `mouse.rs:265-267`      | Hit-test helper                |

### Constraints

- `termcode-theme` is Layer 0: no internal deps, no ratatui
- `termcode-view` is frontend-agnostic: no changes needed (EditorMode already tracks focus)
- TEA pattern: widgets never mutate state during rendering
- `PaneFocusStyle` must derive `Default` (TitleBar)
- All new TOML fields must be optional for backward compatibility

## 3. Impact Analysis (Dry-Run)

### Overall Risk: Medium

### Files to Create

| File                                        | Risk | Description                                             |
| ------------------------------------------- | ---- | ------------------------------------------------------- |
| `crates/termcode-term/src/ui/pane_focus.rs` | Low  | PaneTitleWidget, PaneBorderWidget, PaneAccentLineWidget |

### Files to Modify

| File                                           | Risk   | Description                                           |
| ---------------------------------------------- | ------ | ----------------------------------------------------- |
| `crates/termcode-theme/src/theme.rs`           | Medium | Add PaneFocusStyle enum + 5 fields to UiColors        |
| `crates/termcode-theme/src/loader.rs`          | Medium | Add 5 fields to UiDef + parsing (4 resolve + 1 enum)  |
| `crates/termcode-term/src/layout.rs`           | High   | AppLayout new fields, compute_layout signature change |
| `crates/termcode-term/src/render.rs`           | Medium | Pass pane_focus_style to layout, render new widgets   |
| `crates/termcode-term/src/app.rs`              | High   | Update 2 compute_layout call sites (lines 129, 238)   |
| `crates/termcode-term/src/ui/tab_bar.rs`       | Medium | Add is_editor_active, change empty area bg color      |
| `crates/termcode-term/src/ui/file_explorer.rs` | Low    | No structural change (title rendered by new widget)   |
| `crates/termcode-term/src/ui/mod.rs`           | Low    | Add `pub mod pane_focus;`                             |
| `crates/termcode-term/src/mouse.rs`            | High   | Handle sidebar_title/sidebar_border click areas       |
| `runtime/themes/one-dark.toml`                 | Low    | Add pane focus fields                                 |
| `runtime/themes/gruvbox-dark.toml`             | Low    | Add pane focus fields                                 |
| `runtime/themes/catppuccin-mocha.toml`         | Low    | Add pane focus fields                                 |

### Files to Delete

None

### Destructive Operations

None

### Rollback Plan

- Working on branch, full rollback via `git checkout main && git branch -D feature/pane-focus-indicator`
- No DB changes, no breaking API changes

## 4. Implementation Order

### Phase 1: Theme Types + Parsing (termcode-theme, Layer 0)

**Goal**: Add PaneFocusStyle enum, new UiColors fields, TOML parsing
**Risk**: Medium
**Status**: ✅ Complete

#### Task 1.1: Add PaneFocusStyle enum to theme.rs

- Add `PaneFocusStyle` enum with `TitleBar`, `Border`, `AccentLine` variants
- Derive `Debug, Clone, Copy, PartialEq, Eq, Default`
- `#[default] TitleBar`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PaneFocusStyle {
    #[default]
    TitleBar,
    Border,
    AccentLine,
}
```

#### Task 1.2: Add 5 new fields to UiColors

Add to `UiColors` struct (after `search_match_active`):

| Field              | Type             | Default value                             |
| ------------------ | ---------------- | ----------------------------------------- |
| `pane_focus_style` | `PaneFocusStyle` | `PaneFocusStyle::TitleBar`                |
| `pane_active_fg`   | `Color`          | `defaults.background` (bg = #282c34)      |
| `pane_active_bg`   | `Color`          | `defaults.info` (blue = #61afef)          |
| `pane_inactive_fg` | `Color`          | `defaults.line_number` (gutter = #4b5263) |
| `pane_inactive_bg` | `Color`          | `defaults.sidebar_bg` (#21252b)           |

Update `Default::default()` impl accordingly.

#### Task 1.3: Add fields to UiDef + parsing in loader.rs

- Add 5 `Option<String>` fields to `UiDef` struct
- In `parse_theme()`:
  - 4 color fields use existing `resolve()` closure with `UiColors::default()` fallbacks (consistent with existing pattern)
  - `pane_focus_style` uses a separate string match (not `resolve()`):

```rust
let pane_focus_style = file.ui.pane_focus_style
    .as_deref()
    .map(|s| match s.to_lowercase().as_str() {
        "border" => PaneFocusStyle::Border,
        "accent_line" => PaneFocusStyle::AccentLine,
        _ => PaneFocusStyle::TitleBar,
    })
    .unwrap_or_default();
```

- Add pane fields to the `UiColors` struct construction in the same single pass as existing fields:

```rust
let ui = UiColors {
    // ... existing 20 fields ...
    pane_focus_style,
    pane_active_fg: resolve(&file.ui.pane_active_fg, defaults.background),
    pane_active_bg: resolve(&file.ui.pane_active_bg, defaults.info),
    pane_inactive_fg: resolve(&file.ui.pane_inactive_fg, defaults.line_number),
    pane_inactive_bg: resolve(&file.ui.pane_inactive_bg, defaults.sidebar_bg),
};
```

#### Task 1.4: Add tests for pane focus parsing

- Test that existing themes still parse (already covered by existing tests -- verify they pass)
- Add test: parse theme with explicit pane_focus_style values
- Add test: parse theme with missing pane fields (defaults)
- Add test: unknown pane_focus_style falls back to TitleBar
- Add test: case-insensitive pane_focus_style parsing (`"Title_Bar"`, `"BORDER"`, `"Accent_Line"` all parse correctly)

#### Quality Gate (Phase 1)

```bash
cargo test -p termcode-theme
cargo clippy -p termcode-theme
```

---

### Phase 2: Layout Changes (termcode-term, Layer 3)

**Goal**: Extend AppLayout and compute_layout to support pane focus areas
**Risk**: High (3 call sites must update simultaneously)
**Status**: ✅ Complete

#### Task 2.1: Add new fields to AppLayout

```rust
pub struct AppLayout {
    pub top_bar: Rect,
    pub sidebar: Option<Rect>,
    pub sidebar_title: Option<Rect>,    // NEW: TitleBar/AccentLine style
    pub sidebar_border: Option<Rect>,   // NEW: Border style (1-col vertical)
    pub tab_bar: Rect,
    pub editor_area: Rect,
    pub status_bar: Rect,
}
```

#### Task 2.2: Update compute_layout signature and logic

New signature:

```rust
pub fn compute_layout(
    area: Rect,
    sidebar_visible: bool,
    sidebar_width: u16,
    pane_focus_style: PaneFocusStyle,
) -> AppLayout
```

Logic changes when sidebar is visible:

- **TitleBar / AccentLine**: Split sidebar area vertically into 1-row title + remaining content. Set `sidebar_title = Some(title_row)`, `sidebar = Some(content_area)`. The `sidebar` field is now **content-only** (excludes title).
- **Border**: Split sidebar area horizontally into (width-1) content + 1-col border. Set `sidebar_border = Some(border_col)`, `sidebar = Some(content_area)`.
- When sidebar is hidden: all sidebar fields are `None`.

**CRITICAL**: `sidebar` must always be content-only. This ensures `file_explorer.viewport_height = sidebar.height` (app.rs:139) uses the correct height after title bar subtraction.

#### Task 2.3: Update 3 compute_layout call sites

1. **render.rs:19** -- Pass `editor.theme.ui.pane_focus_style`
2. **app.rs:129** -- Pass `self.editor.theme.ui.pane_focus_style`
3. **app.rs:238** -- Pass `self.editor.theme.ui.pane_focus_style`

All 3 sites must be updated simultaneously since the signature changes.

#### Task 2.4: Update mouse.rs for new layout areas

In `handle_left_click()`:

- Before the existing `sidebar` check, add checks for `sidebar_title` and `sidebar_border`
- Clicking `sidebar_title` should switch to FileExplorer mode (no file selection)
- Clicking `sidebar_border` should switch to FileExplorer mode (no file selection)
- These clicks should NOT dispatch to `handle_sidebar_click` (which opens files)

```rust
// In handle_left_click:
if let Some(sidebar_title) = layout.sidebar_title {
    if rect_contains(&sidebar_title, x, y) {
        editor.switch_mode(EditorMode::FileExplorer);
        return MouseAction::None;
    }
}
if let Some(sidebar_border) = layout.sidebar_border {
    if rect_contains(&sidebar_border, x, y) {
        editor.switch_mode(EditorMode::FileExplorer);
        return MouseAction::None;
    }
}
```

#### Task 2.5: Add layout and mouse tests

**Layout tests:**

- Test TitleBar style: sidebar splits into title (1 row) + content
- Test Border style: sidebar splits into content + border (1 col)
- Test AccentLine style: same as TitleBar layout
- Test sidebar hidden: all sidebar fields None
- Test sidebar.height is content-only (not including title)

**Mouse click tests (high-risk area):**

- Test click on `sidebar_title` area switches to FileExplorer mode, returns `MouseAction::None` (not OpenExplorerItem)
- Test click on `sidebar_border` area switches to FileExplorer mode, returns `MouseAction::None` (not OpenExplorerItem)
- Test click on `sidebar` content area still dispatches to `handle_sidebar_click` as before (regression guard)

#### Quality Gate (Phase 2)

```bash
cargo test -p termcode-term
cargo clippy -p termcode-term
```

---

### Phase 3: Widgets + Rendering (termcode-term, Layer 3)

**Goal**: Create pane focus widgets and integrate into render pipeline
**Risk**: Medium
**Status**: ✅ Complete

#### Task 3.1: Create ui/pane_focus.rs

Add 3 widget structs following existing widget pattern:

**PaneTitleWidget** (for TitleBar style):

- Fields: `is_active: bool`, `theme: &Theme`
- Render: Fill area with active/inactive style. Text: `" EXPLORER"` left-aligned, truncated if wider than area.
- Active style: `pane_active_fg` on `pane_active_bg`
- Inactive style: `pane_inactive_fg` on `pane_inactive_bg`

**PaneBorderWidget** (for Border style):

- Fields: `is_active: bool`, `theme: &Theme`
- Render: Fill 1-col area with `'\u{2502}'` (U+2502 BOX DRAWINGS LIGHT VERTICAL) in each row
- Active: `pane_active_bg` as foreground color (border drawn in accent)
- Inactive: `pane_inactive_bg` as foreground color

**PaneAccentLineWidget** (for AccentLine style):

- Fields: `is_active: bool`, `theme: &Theme`
- Render: Fill 1-row area with `'\u{2501}'` (U+2501 BOX DRAWINGS HEAVY HORIZONTAL)
- Active: `pane_active_bg` as foreground color
- Inactive: `pane_inactive_bg` as foreground color

#### Task 3.2: Register module in ui/mod.rs

Add `pub mod pane_focus;` to `crates/termcode-term/src/ui/mod.rs`.

#### Task 3.3: Update tab_bar.rs

Modify `TabBarWidget`:

- Add `is_editor_active: bool` field
- Update constructor: `pub fn new(tabs, theme, is_editor_active) -> Self`
- In `render()`: Change empty area fill (after all tabs) to use:
  - `pane_active_bg` when `is_editor_active` is true
  - `pane_inactive_bg` when false
- Individual tab colors remain unchanged

#### Task 3.4: Update render.rs

In `render()` function:

1. Determine `is_sidebar_active`:

   ```rust
   let is_sidebar_active = editor.mode == EditorMode::FileExplorer;
   ```

2. Pass `editor.theme.ui.pane_focus_style` to `compute_layout()`

3. Render pane focus widgets based on style:

   ```rust
   if let Some(title_area) = app_layout.sidebar_title {
       match editor.theme.ui.pane_focus_style {
           PaneFocusStyle::TitleBar => {
               let w = PaneTitleWidget::new(is_sidebar_active, &editor.theme);
               frame.render_widget(w, title_area);
           }
           PaneFocusStyle::AccentLine => {
               let w = PaneAccentLineWidget::new(is_sidebar_active, &editor.theme);
               frame.render_widget(w, title_area);
           }
           _ => {}
       }
   }
   if let Some(border_area) = app_layout.sidebar_border {
       let w = PaneBorderWidget::new(is_sidebar_active, &editor.theme);
       frame.render_widget(w, border_area);
   }
   ```

4. Pass `is_editor_active = !is_sidebar_active` to `TabBarWidget::new()`

#### Quality Gate (Phase 3)

```bash
cargo test -p termcode-term
cargo clippy -p termcode-term
```

---

### Phase 4: Theme Files

**Goal**: Add explicit pane focus values to bundled themes
**Risk**: Low
**Status**: ✅ Complete

#### Task 4.1: Update one-dark.toml

```toml
# Pane focus indicator
pane_focus_style = "title_bar"
pane_active_fg = "bg"
pane_active_bg = "blue"
pane_inactive_fg = "gutter"
pane_inactive_bg = "#21252b"
```

#### Task 4.2: Update gruvbox-dark.toml

```toml
# Pane focus indicator
pane_focus_style = "title_bar"
pane_active_fg = "bg"
pane_active_bg = "bright_blue"
pane_inactive_fg = "gray"
pane_inactive_bg = "bg0_h"
```

#### Task 4.3: Update catppuccin-mocha.toml

```toml
# Pane focus indicator
pane_focus_style = "title_bar"
pane_active_fg = "base"
pane_active_bg = "blue"
pane_inactive_fg = "overlay0"
pane_inactive_bg = "mantle"
```

#### Quality Gate (Phase 4)

```bash
cargo test --workspace
cargo clippy --workspace
cargo fmt --check
```

## 5. Quality Gate

- [ ] Build success: `cargo build`
- [ ] Tests pass: `cargo test --workspace`
- [ ] Lint pass: `cargo clippy --workspace` (0 warnings)
- [ ] Format check: `cargo fmt --check`

## 6. Notes

### Patterns to follow

- Widget struct pattern with lifetime: `struct FooWidget<'a> { state, theme: &'a Theme }`
- Fill background cell-by-cell before painting content (same as TabBarWidget, FileExplorerWidget)
- Use `to_ratatui()` to convert `termcode_theme::Color` to `ratatui::style::Color`

### Potential issues

- **viewport_height off-by-one**: Phase 2 addresses this by making `sidebar` content-only. The existing line `self.editor.file_explorer.viewport_height = sidebar.height as usize` (app.rs:139) automatically picks up the correct height.
- **Mouse click on title bar**: Phase 2 Task 2.4 adds hit-test guards before sidebar dispatch.
- **Backward compatibility**: All new TOML fields are optional. Existing themes work without changes. Phase 4 adds explicit values for design consistency.

### Edge cases (from spec)

- Sidebar hidden: No indicator rendered (all sidebar layout fields None)
- Overlays active (Search/FuzzyFinder/CommandPalette): These modes are not FileExplorer, so sidebar shows inactive -- correct behavior
- Terminal too narrow: Indicator may clip, acceptable (matches current file tree clipping)
- Title text overflow: `"EXPLORER"` (9 chars) truncated if sidebar narrower

## 7. Implementation Notes

### Phase 1 (2026-03-28)

- Created: 0 files
- Modified: 2 files (theme.rs, loader.rs)
- Risk: Low
- Notes: PaneFocusStyle enum, 5 UiColors fields, two-pass parsing, 5 new tests

### Phase 2 (2026-03-28)

- Created: 0 files
- Modified: 4 files (layout.rs, render.rs, app.rs, mouse.rs)
- Risk: Medium
- Notes: AppLayout new fields, compute_layout with PaneFocusStyle param, mouse click handling, 8 new tests

### Phase 3 (2026-03-28)

- Created: 1 file (ui/pane_focus.rs)
- Modified: 4 files (mod.rs, tab_bar.rs, render.rs)
- Risk: Medium
- Notes: 3 widget structs, tab bar pane-aware empty area, render pipeline integration

### Phase 4 (2026-03-28)

- Created: 0 files
- Modified: 3 files (one-dark.toml, gruvbox-dark.toml, catppuccin-mocha.toml)
- Risk: Low
- Notes: Explicit pane focus values for all bundled themes

---

Last Updated: 2026-03-28
Status: ✅ Complete
