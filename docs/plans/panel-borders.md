# Panel Borders & Lazygit Theme Implementation Plan

**Created**: 2026-03-29
**Analysis Report**: None (inline analysis performed; recommend running yyy-analyze for future features)
**Status**: Pending

## 1. Requirements Summary

### Functional Requirements

- [FR-001] Add `panel_borders` boolean setting to theme TOML `[ui]` section
- [FR-002] When `panel_borders = true`, draw box borders around the sidebar panel and the editor panel (tab bar + editor area)
- [FR-003] When `panel_borders = false` (default), preserve current behavior exactly
- [FR-004] Top bar and status bar are NOT bordered (they span full width)
- [FR-005] Create a lazygit-style theme TOML file that enables panel_borders
- [FR-006] Any theme can opt into panel_borders; it is not tied to a specific theme

### UI Layout When panel_borders = true

```
+---------------------------------------------+
| TOP_BAR (no border)                         |
+------------------+--------------------------+
| +==============+ | +======================+ |
| | SIDEBAR_TITLE| | | TAB_BAR              | |
| |              | | |                      | |
| | SIDEBAR      | | | EDITOR_AREA          | |
| |              | | |                      | |
| +==============+ | +======================+ |
+------------------+--------------------------+
| STATUS_BAR (no border)                      |
+---------------------------------------------+
```

Each bordered panel uses ratatui `Block::default().borders(Borders::ALL)` rendered on the panel Rect, with content rendered inside `block.inner(area)`.

## 2. Analysis Report Reference

### Reference Documents

- No formal analysis report. Analysis performed inline from source code inspection.

### Applied Recommendations

- Follow existing pattern: `panel_borders` is a UiColors field (like `pane_focus_style`), parsed as optional in UiDef, defaulting to `false`
- Use ratatui `Block` widget for borders -- renders inside the allocated Rect, so outer layout dimensions do not change
- Border color uses existing `ui.border` color slot (already defined in all themes)

### Reusable Code

| Code               | Location                                    | Purpose                                 |
| ------------------ | ------------------------------------------- | --------------------------------------- |
| `PaneBorderWidget` | `crates/termcode-term/src/ui/pane_focus.rs` | Reference for border rendering patterns |
| `compute_layout()` | `crates/termcode-term/src/layout.rs`        | Layout computation to be extended       |
| `UiDef` struct     | `crates/termcode-theme/src/loader.rs`       | TOML deserialization pattern to follow  |
| `one-dark.toml`    | `runtime/themes/one-dark.toml`              | Template for new theme file             |

### Constraints

- `termcode-theme` is Layer 0: no ratatui dependency. `panel_borders` is just a `bool` in `UiColors`.
- `termcode-view` is frontend-agnostic: border rendering logic lives in `termcode-term` only.
- When `panel_borders = true`, the inner content areas shrink by 1 cell on each side (2 cols narrower, 2 rows shorter). `AppLayout` fields must reflect the _content_ areas so all downstream widgets (file explorer, editor, overlays, cursor position) work correctly without modification.
- Existing `pane_focus_style` behavior should be preserved. When both `panel_borders = true` and a `pane_focus_style` is active, they should coexist (the pane focus indicator renders within the bordered panel content area, or the border style itself could serve as the focus indicator).

## 3. Impact Analysis (Dry-Run)

### Overall Risk: Medium

### Files to Create

| File                          | Risk | Description                  |
| ----------------------------- | ---- | ---------------------------- |
| `runtime/themes/lazygit.toml` | Low  | New lazygit-style theme file |

### Files to Modify

| File                                  | Risk   | Description                                                                          |
| ------------------------------------- | ------ | ------------------------------------------------------------------------------------ |
| `crates/termcode-theme/src/theme.rs`  | Low    | Add `panel_borders: bool` to `UiColors`                                              |
| `crates/termcode-theme/src/loader.rs` | Low    | Add `panel_borders` to `UiDef`, parse as bool                                        |
| `crates/termcode-term/src/layout.rs`  | Medium | Extend `compute_layout` to accept & handle panel_borders; return panel wrapper Rects |
| `crates/termcode-term/src/render.rs`  | Medium | Render Block borders around panels when enabled                                      |

### Files to Delete

| File | Risk | Description |
| ---- | ---- | ----------- |
| None | -    | -           |

### Destructive Operations

- None

### Rollback Plan

- Full rollback via branch deletion: `git checkout main && git branch -D feature/panel-borders`
- No database or persistent state changes involved

## 4. Implementation Order

### Phase 1: Theme Data Model -- Add panel_borders Field

**Goal**: Add the `panel_borders` boolean to the theme system so themes can declare it
**Risk**: Low
**Status**: Pending

- [ ] Task 1.1: Add `panel_borders: bool` field to `UiColors` in `crates/termcode-theme/src/theme.rs`
  - Add after `pane_inactive_bg` field
  - Default to `false` in `impl Default for UiColors`

- [ ] Task 1.2: Add `panel_borders: Option<bool>` to `UiDef` in `crates/termcode-theme/src/loader.rs`

- [ ] Task 1.3: In `parse_theme()` in `loader.rs`, set `panel_borders` on the constructed `UiColors`:

  ```rust
  panel_borders: file.ui.panel_borders.unwrap_or(false),
  ```

- [ ] Task 1.4: Add test in `loader.rs` to verify `panel_borders` parsing:
  - Test that `panel_borders = true` in TOML parses correctly
  - Test that missing `panel_borders` defaults to `false`

- [ ] Task 1.5: Verify all existing theme TOML files still parse (`cargo test -p termcode-theme`)

### Phase 2: Layout -- Support Bordered Panels

**Goal**: Extend layout computation to provide panel wrapper Rects when panel_borders is enabled
**Risk**: Medium
**Status**: Pending

- [ ] Task 2.1: Add fields to `AppLayout` in `crates/termcode-term/src/layout.rs`:

  ```rust
  pub sidebar_panel: Option<Rect>,   // full sidebar panel area (for Block border)
  pub editor_panel: Option<Rect>,    // full editor panel area (for Block border)
  ```

  These are `Some(rect)` only when `panel_borders = true`.

- [ ] Task 2.2: Extend `compute_layout()` signature to accept `panel_borders: bool`

- [ ] Task 2.3: Implement panel border layout logic:
  - When `panel_borders = true` and sidebar is visible:
    - The middle area is split into left (sidebar_width) and right as before
    - Set `sidebar_panel = Some(left_rect)` and `editor_panel = Some(right_rect)`
    - Compute `Block::default().borders(Borders::ALL)` inner area for each
    - The sidebar content Rect (and sidebar_title if applicable) comes from the inner area of the left Block
    - The tab_bar and editor_area come from splitting the inner area of the right Block
  - When `panel_borders = true` and sidebar is NOT visible:
    - Set `editor_panel = Some(middle)` and `sidebar_panel = None`
    - The tab_bar and editor_area come from the inner area of the editor Block
  - When `panel_borders = false`: existing behavior unchanged, `sidebar_panel = None`, `editor_panel = None`

- [ ] Task 2.4: Ensure existing `pane_focus_style` handling still works within the bordered content area. The sidebar_title/sidebar_border fields operate within the inner area of the bordered panel. When `panel_borders = true`, the `PaneFocusStyle::Border` vertical separator is redundant (the box border already separates panels), so skip the sidebar_border in that case.

- [ ] Task 2.5: Add tests for `compute_layout` with `panel_borders = true`:
  - Test that sidebar_panel and editor_panel are Some with correct dimensions
  - Test that sidebar content area is 2 cols narrower and 2 rows shorter than the panel Rect
  - Test that editor content area is similarly inset
  - Test sidebar hidden case
  - Test interaction with each PaneFocusStyle variant

### Phase 3: Rendering -- Draw Box Borders

**Goal**: Render Block borders around panels in render.rs
**Risk**: Medium
**Status**: Pending

- [ ] Task 3.1: Update `render()` in `crates/termcode-term/src/render.rs`:
  - Pass `editor.theme.ui.panel_borders` to `compute_layout()`
  - After computing layout, if `app_layout.sidebar_panel` is Some, render a `Block` with `Borders::ALL` and border color from `editor.theme.ui.border` on that Rect
  - If `app_layout.editor_panel` is Some, render a `Block` with `Borders::ALL` on that Rect
  - All existing content widgets continue to render on the (already inset) content Rects in AppLayout

- [ ] Task 3.2: Verify that overlay widgets (search, fuzzy finder, command palette, completion, hover) still position correctly since they use `app_layout.editor_area` which is now the inner content area

- [ ] Task 3.3: Verify cursor position calculation in `cursor_screen_position()` still works (it uses `app_layout.editor_area` which is already the content area)

### Phase 4: Lazygit Theme

**Goal**: Create a lazygit-inspired theme file
**Risk**: Low
**Status**: Pending

- [ ] Task 4.1: Create `runtime/themes/lazygit.toml` with:
  - `[meta]` name = "Lazygit"
  - `[palette]` with lazygit-inspired dark colors (dark background, green/cyan/magenta accents reminiscent of lazygit's UI)
  - `[ui]` section with `panel_borders = true`
  - `[scopes]` section with syntax highlighting colors
  - Use `pane_focus_style = "title_bar"` for visible panel headers within borders

- [ ] Task 4.2: Add a parse test in `loader.rs` for the new theme:
  ```rust
  #[test]
  fn parse_lazygit_theme() {
      let toml = include_str!("../../../runtime/themes/lazygit.toml");
      let theme = parse_theme(toml).expect("lazygit should parse");
      assert_eq!(theme.name, "Lazygit");
      assert!(theme.ui.panel_borders);
  }
  ```

### Phase 5: Integration Verification

**Goal**: End-to-end verification
**Risk**: Low
**Status**: Pending

- [ ] Task 5.1: Run `cargo build` -- ensure clean compilation
- [ ] Task 5.2: Run `cargo test --workspace` -- all tests pass
- [ ] Task 5.3: Run `cargo clippy --workspace` -- zero warnings
- [ ] Task 5.4: Run `cargo fmt --check` -- formatting clean
- [ ] Task 5.5: Manual smoke test: `cargo run -- .` then switch to lazygit theme via command palette, verify borders appear around sidebar and editor panels
- [ ] Task 5.6: Verify existing themes still render without borders (no visual regression)

## 5. Quality Gate

- [ ] Build success: `cargo build`
- [ ] Tests pass: `cargo test --workspace`
- [ ] Lint pass: `cargo clippy --workspace`
- [ ] Format check: `cargo fmt --check`

## 6. Notes

### Design Decisions

1. **panel_borders as bool, not enum**: A simple boolean is sufficient. If future needs arise (e.g., rounded borders, partial borders), this can be evolved into an enum later.

2. **Block renders inside existing Rect**: The key insight is that `ratatui::Block` draws borders within its given Rect and `block.inner()` returns the content area. Layout computes the outer panel Rects at the same dimensions as today's middle split, then uses `block.inner()` to derive the content Rects. This means the overall screen layout (top_bar, status_bar heights, sidebar width) does not change.

3. **PaneFocusStyle interaction**: When `panel_borders = true`:
   - `TitleBar` mode: title row renders inside the bordered panel content area (works naturally)
   - `AccentLine` mode: accent line renders inside the bordered panel content area (works naturally)
   - `Border` mode: the vertical separator between sidebar and editor is redundant since the box borders already create visual separation. The code should skip rendering `sidebar_border` when `panel_borders = true` to avoid a double border artifact.

4. **Content area reduction**: With borders enabled, each panel loses 2 columns and 2 rows of usable space. For very narrow terminals or small sidebar widths, this could be tight. The layout should handle edge cases (e.g., if inner area would be zero-sized, skip rendering the panel content).

### Potential Issues

- If sidebar width is set to a very small value (e.g., 3 or less), the bordered panel inner area could be 1 column or zero. The layout code should guard against this.
- Overlay widgets that use `app_layout.editor_area` for positioning should work correctly since that field already represents the content area, but manual verification is needed.

### Patterns to Avoid

- Do NOT put ratatui types in `termcode-theme` -- the `panel_borders` field is a plain `bool`
- Do NOT change the outer layout dimensions based on `panel_borders` -- Block renders within the existing allocation
- Do NOT duplicate border color -- reuse the existing `ui.border` color slot

## 7. Implementation Notes

(yyy-implement agent records while working)

---

Last Updated: 2026-03-29
Status: Pending
