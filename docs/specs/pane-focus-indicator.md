# Pane Focus Indicator

Theme-configurable visual indicator showing which pane (sidebar or editor) is currently active.

## Background

Currently there is no visual cue distinguishing the active pane from the inactive one. The `EditorMode` enum already tracks focus (`FileExplorer` = sidebar, `Normal`/`Insert` = editor), but the rendering does not reflect this distinction.

## Code Reference Checklist

| Item                    | Result                                                                                                                                            |
| ----------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------- |
| Similar feature exists? | No. No pane focus logic exists anywhere in rendering.                                                                                             |
| Reference pattern       | `TabBarWidget` fills background with inactive style, then paints active tab -- same "fill + highlight" pattern applies here.                      |
| Technical constraints   | `termcode-view` is frontend-agnostic (no ratatui). New enum/colors go in `termcode-theme` (Layer 0). Layout changes in `termcode-term` (Layer 3). |

### Key files

| File                                           | Role                                                                        |
| ---------------------------------------------- | --------------------------------------------------------------------------- |
| `crates/termcode-theme/src/theme.rs`           | `UiColors` struct -- add new color fields                                   |
| `crates/termcode-theme/src/loader.rs`          | `UiDef` struct + `parse_theme()` -- add TOML deserialization                |
| `crates/termcode-term/src/layout.rs`           | `AppLayout` + `compute_layout()` -- adjust for title_bar/border/accent_line |
| `crates/termcode-term/src/render.rs`           | Orchestration -- pass `is_sidebar_active` to widgets                        |
| `crates/termcode-term/src/ui/tab_bar.rs`       | `TabBarWidget` -- empty area uses pane active/inactive colors               |
| `crates/termcode-term/src/ui/file_explorer.rs` | `FileExplorerWidget` -- render title bar / border / accent line             |
| `runtime/themes/*.toml`                        | Theme files -- add optional pane fields                                     |

---

## FR-PANE-001: Pane Focus Style Enum

- **Priority**: High
- **Status**: Draft

### Description

Add a `PaneFocusStyle` enum to `termcode-theme` (Layer 0):

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PaneFocusStyle {
    #[default]
    TitleBar,
    Border,
    AccentLine,
}
```

TOML string mapping:

| TOML value              | Variant                         |
| ----------------------- | ------------------------------- |
| `"title_bar"` (default) | `TitleBar`                      |
| `"border"`              | `Border`                        |
| `"accent_line"`         | `AccentLine`                    |
| unknown value           | `TitleBar` (fallback, no error) |

---

## FR-PANE-002: Theme UI Color Fields

- **Priority**: High
- **Status**: Draft

### Description

Add the following optional fields to `UiColors` (theme.rs) and `UiDef` (loader.rs):

| Field              | Type             | TOML key           | Default                   |
| ------------------ | ---------------- | ------------------ | ------------------------- |
| `pane_focus_style` | `PaneFocusStyle` | `pane_focus_style` | `TitleBar`                |
| `pane_active_fg`   | `Color`          | `pane_active_fg`   | `ui.background`           |
| `pane_active_bg`   | `Color`          | `pane_active_bg`   | `ui.info` (blue)          |
| `pane_inactive_fg` | `Color`          | `pane_inactive_fg` | `ui.line_number` (gutter) |
| `pane_inactive_bg` | `Color`          | `pane_inactive_bg` | `ui.sidebar_bg`           |

### Defaults rationale

- Active: white-on-blue (bg on info) provides strong contrast, matches common editor conventions.
- Inactive: dim gutter text on sidebar background -- blends with surroundings.
- Defaults produce a usable indicator without any theme TOML changes.

### Backward compatibility

All fields are `Option<String>` in `UiDef`. Missing fields resolve to the defaults above. Existing themes continue to work unchanged.

---

## FR-PANE-003: Layout Adaptation

- **Priority**: High
- **Status**: Draft

### Description

`compute_layout()` must accept `PaneFocusStyle` and adjust areas accordingly.

**Current layout** (middle section):

```
[ sidebar (N cols) ][ tab_bar (1 row) + editor ]
```

**Adapted layouts by style**:

### title_bar (default)

Sidebar gains a 1-row title bar at the top. The editor's existing tab bar serves as its title bar.

```
[ sidebar_title (1 row) ][ tab_bar (1 row)      ]
[ sidebar_content       ][ editor_area           ]
```

- `sidebar` area is split vertically: 1-row title + remaining content.
- `sidebar_title` and `tab_bar` are at the same vertical position.

### border

Sidebar loses 1 column on its right edge for a vertical border.

```
[ sidebar_content | border (1 col) ][ tab_bar (1 row)  ]
                                    [ editor_area       ]
```

- Sidebar area width is reduced by 1. Border column is drawn by the widget.

### accent_line

Sidebar gains a 1-row horizontal accent line at the top (same as title_bar but no text).

```
[ sidebar_accent (1 row) ][ tab_bar (1 row)      ]
[ sidebar_content        ][ editor_area           ]
```

### AppLayout changes

Add fields to `AppLayout`:

```rust
pub struct AppLayout {
    pub top_bar: Rect,
    pub sidebar: Option<Rect>,
    pub sidebar_title: Option<Rect>,    // NEW: title_bar / accent_line style
    pub sidebar_border: Option<Rect>,   // NEW: border style (1-col vertical)
    pub tab_bar: Rect,
    pub editor_area: Rect,
    pub status_bar: Rect,
}
```

`compute_layout()` signature change:

```rust
pub fn compute_layout(
    area: Rect,
    sidebar_visible: bool,
    sidebar_width: u16,
    pane_focus_style: PaneFocusStyle,  // NEW
) -> AppLayout
```

---

## FR-PANE-004: Sidebar Rendering

- **Priority**: High
- **Status**: Draft

### Description

Update `FileExplorerWidget` and `render()` to render the focus indicator based on style.

### Active pane detection

The sidebar is active when `editor.mode == EditorMode::FileExplorer`. This boolean is passed to widgets.

### title_bar style

Render a 1-row bar in `sidebar_title` area:

- Text: `" EXPLORER"` (left-aligned, uppercase)
- Active: `pane_active_fg` on `pane_active_bg`
- Inactive: `pane_inactive_fg` on `pane_inactive_bg`

### border style

Render a 1-column vertical line on the right edge (`sidebar_border` area):

- Character: `'│'` (U+2502 BOX DRAWINGS LIGHT VERTICAL)
- Active: `pane_active_bg` foreground (border drawn in accent color)
- Inactive: `pane_inactive_bg` foreground

### accent_line style

Render a 1-row line in `sidebar_title` area:

- Fill with `'▔'` (U+2594 UPPER ONE EIGHTH BLOCK) or `'━'` (U+2501 BOX DRAWINGS HEAVY HORIZONTAL) -- use `'━'` for better terminal compatibility
- Active: `pane_active_bg` foreground
- Inactive: `pane_inactive_bg` foreground

---

## FR-PANE-005: Tab Bar Empty Area

- **Priority**: Medium
- **Status**: Draft

### Description

The tab bar currently fills its entire background with `tab_inactive_bg`. Change the **empty area after the last tab** to reflect pane focus state:

- Editor pane is active: empty area uses `pane_active_bg`
- Sidebar is active: empty area uses `pane_inactive_bg`

Individual tab colors (`tab_active_bg`, `tab_inactive_bg`) remain unchanged. This avoids conflicting "tab-selected" vs "pane-active" semantics.

### title_bar style special case

When `pane_focus_style == TitleBar`, the tab bar doubles as the editor's title bar. The empty area coloring described above visually links it to the pane focus system.

### border / accent_line styles

Same behavior -- the empty area still reflects pane focus. The coloring is subtle but consistent.

---

## FR-PANE-006: Theme TOML Configuration

- **Priority**: High
- **Status**: Draft

### Description

New optional fields in `[ui]` section of theme TOML files:

```toml
[ui]
# ... existing fields ...

# Pane focus indicator
pane_focus_style = "title_bar"     # "title_bar" | "border" | "accent_line"
pane_active_fg = "bg"              # palette ref or hex
pane_active_bg = "blue"            # palette ref or hex
pane_inactive_fg = "gutter"        # palette ref or hex
pane_inactive_bg = "#21252b"       # palette ref or hex
```

### Parsing rules

- All fields are optional. Absent = use defaults (FR-PANE-002).
- `pane_focus_style` is a string parsed case-insensitively. Unknown values fall back to `"title_bar"`.
- Color fields follow the same resolution as existing UI fields: palette name lookup, then hex, then default.

### Example: One Dark with title_bar (explicit)

```toml
pane_focus_style = "title_bar"
pane_active_fg = "bg"
pane_active_bg = "blue"
pane_inactive_fg = "gutter"
pane_inactive_bg = "#21252b"
```

### Example: Gruvbox Dark with border

```toml
pane_focus_style = "border"
pane_active_bg = "yellow"
pane_inactive_bg = "#3c3836"
```

---

## FR-PANE-007: Default Theme Updates

- **Priority**: Low
- **Status**: Draft

### Description

Update the three bundled themes with explicit pane focus values. These are optional (defaults work), but explicit values ensure intentional design.

| Theme            | Style     | Active fg | Active bg | Inactive fg  | Inactive bg |
| ---------------- | --------- | --------- | --------- | ------------ | ----------- |
| one-dark         | title_bar | `"bg"`    | `"blue"`  | `"gutter"`   | `"#21252b"` |
| gruvbox-dark     | title_bar | `"bg0"`   | `"blue"`  | `"gray"`     | `"bg0_h"`   |
| catppuccin-mocha | title_bar | `"base"`  | `"blue"`  | `"overlay0"` | `"mantle"`  |

Exact palette names depend on each theme's `[palette]` section. Values above are indicative.

---

## Implementation Notes

### Crate boundaries

| Change                             | Crate             | Layer |
| ---------------------------------- | ----------------- | ----- |
| `PaneFocusStyle` enum              | `termcode-theme`  | 0     |
| `UiColors` new fields              | `termcode-theme`  | 0     |
| `UiDef` + parsing                  | `termcode-theme`  | 0     |
| `AppLayout` + `compute_layout()`   | `termcode-term`   | 3     |
| Sidebar title/border/accent widget | `termcode-term`   | 3     |
| `TabBarWidget` empty area          | `termcode-term`   | 3     |
| `render()` orchestration           | `termcode-term`   | 3     |
| Theme TOML files                   | `runtime/themes/` | N/A   |

### No changes needed in

- `termcode-core` (Layer 0)
- `termcode-view` (Layer 2) -- `EditorMode` already provides focus info; no new state needed
- `termcode-syntax`, `termcode-config`, `termcode-lsp`, `termcode-plugin`

### Edge cases

- **Sidebar hidden**: No indicator rendered. Layout is unchanged.
- **Overlays active** (Search, FuzzyFinder, CommandPalette): These modes logically focus the editor pane. Sidebar indicator shows inactive.
- **Terminal too narrow for sidebar**: If sidebar_width < 3, indicator may be clipped. Acceptable -- same behavior as current file tree clipping.
- **Title bar text overflow**: `"EXPLORER"` is 8 chars + 1 space = 9 chars. If sidebar is narrower, truncate. Never wrap.

---

## Assigned IDs

| ID          | Description                           |
| ----------- | ------------------------------------- |
| FR-PANE-001 | PaneFocusStyle enum in termcode-theme |
| FR-PANE-002 | UiColors pane focus color fields      |
| FR-PANE-003 | Layout adaptation per style           |
| FR-PANE-004 | Sidebar focus indicator rendering     |
| FR-PANE-005 | Tab bar empty area pane coloring      |
| FR-PANE-006 | Theme TOML configuration              |
| FR-PANE-007 | Default theme updates                 |
