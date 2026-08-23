use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

use termcode_core::selection::Selection;
use termcode_view::editor::{Editor, EditorMode, ScrollbarDrag};

use crate::command::sync_selection_from_cursor;
use crate::layout::AppLayout;

/// Result of mouse handling that requires App-level action.
pub enum MouseAction {
    None,
    OpenExplorerItem(usize),
    /// Expand or collapse the tree row, from a click on its chevron.
    ToggleExplorerExpand(usize),
    SwitchTab(usize),
    OpenSettings,
    /// An explorer toolbar button, named by its command without the
    /// `explorer.` prefix.
    ExplorerCommand(&'static str),
    /// The top bar's Exit button. Quitting is `App`'s to do: unsaved documents
    /// have to be confirmed first.
    Quit,
    /// A click on a confirm dialog button, which is already selected by the
    /// time this is returned. Running the action is `App`'s job.
    ConfirmSelected,
    /// The sidebar divider was released after a drag that changed the width.
    /// Writing it to the config file is `App`'s job.
    SidebarResized(u16),
    /// One wheel notch on the settings screen, `-1` up and `1` down. The
    /// screen has a category pane, a value picker and live preview hanging off
    /// it, so the move goes back to `App` and down the same path the keyboard
    /// uses rather than being applied here.
    ScrollSettings(i32),
}

/// Rows one wheel notch moves, matching what the editor scrolls by.
const WHEEL_LINES: i32 = 3;

/// Handle a mouse event, dispatching based on which layout region was clicked.
pub fn handle_mouse(editor: &mut Editor, event: MouseEvent, layout: &AppLayout) -> MouseAction {
    // The confirm dialog is modal: nothing behind it can be clicked or
    // scrolled, exactly as no key reaches past it.
    if editor.confirm_dialog.is_some() {
        return match event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                handle_confirm_click(editor, event.column, event.row, layout.frame)
            }
            _ => MouseAction::None,
        };
    }

    match event.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            // A press starts from a clean slate: an `Up` lost outside the
            // terminal would otherwise leave the divider or the scrollbar thumb
            // stuck to the pointer.
            editor.file_explorer.resizing = None;
            editor.scrollbar_drag = None;
            handle_left_click(editor, event.column, event.row, layout)
        }
        MouseEventKind::ScrollUp => handle_wheel(editor, -1, layout),
        MouseEventKind::ScrollDown => handle_wheel(editor, 1, layout),
        MouseEventKind::Drag(MouseButton::Left) => {
            handle_drag(editor, event.column, event.row, layout);
            MouseAction::None
        }
        MouseEventKind::Up(MouseButton::Left) => {
            editor.scrollbar_drag = None;
            end_sidebar_resize(editor)
        }
        _ => MouseAction::None,
    }
}

/// Release the divider. Reports the new width only when the drag actually moved
/// it, so a press that never turned into a drag does not rewrite the config.
fn end_sidebar_resize(editor: &mut Editor) -> MouseAction {
    match editor.file_explorer.resizing.take() {
        Some(original) if original != editor.file_explorer.width => {
            MouseAction::SidebarResized(editor.file_explorer.width)
        }
        _ => MouseAction::None,
    }
}

/// A click while the confirm dialog is open. Only its buttons do anything;
/// dismissing it is `Esc`'s job, so a stray click cannot discard the choice.
///
/// The first click moves the focus, the second runs the focused button -- the
/// same select-then-act the tree uses. Discarding unsaved work is not something
/// a single misplaced click should be able to do.
fn handle_confirm_click(
    editor: &mut Editor,
    x: u16,
    y: u16,
    frame: ratatui::layout::Rect,
) -> MouseAction {
    let button = editor
        .confirm_dialog
        .as_ref()
        .and_then(|dialog| crate::ui::confirm_dialog::layout(dialog, frame))
        .and_then(|placed| placed.button_at(x, y));

    let Some(index) = button else {
        return MouseAction::None;
    };
    let Some(dialog) = editor.confirm_dialog.as_mut() else {
        return MouseAction::None;
    };
    let already_focused = dialog.selected_button == index;
    dialog.selected_button = index;
    if already_focused {
        MouseAction::ConfirmSelected
    } else {
        MouseAction::None
    }
}

fn handle_left_click(editor: &mut Editor, x: u16, y: u16, layout: &AppLayout) -> MouseAction {
    // Help popup: any click dismisses it
    if editor.help_visible {
        editor.help_visible = false;
        return MouseAction::None;
    }

    // Buttons in the top bar (right-aligned)
    if rect_contains(&layout.top_bar, x, y) {
        let buttons = crate::ui::top_bar::buttons(layout.top_bar);
        if x >= buttons.exit_start {
            return MouseAction::Quit;
        }
        if buttons.settings_start.is_some_and(|start| x >= start) {
            return MouseAction::OpenSettings;
        }
    }

    if let Some(toolbar) = layout.sidebar_toolbar {
        if rect_contains(&toolbar, x, y) {
            // The buttons act on the tree, so the tree is what the click
            // focuses -- including when it lands on the project name.
            editor.switch_mode(EditorMode::FileExplorer);
            let labels = crate::ui::explorer_toolbar::ToolbarLabels::resolve(
                &editor.theme,
                editor.file_tree_style.show_file_type_emoji,
            );
            if let Some(action) = crate::ui::explorer_toolbar::action_at(toolbar, &labels, x) {
                return MouseAction::ExplorerCommand(action.command());
            }
            return MouseAction::None;
        }
    }

    // The divider is the sidebar's last column, so it is tested before the
    // regions that also contain it. A press only arms the drag; a click that
    // never moves still focuses the tree, which is what that column did before.
    if let Some(divider) = layout.sidebar_divider {
        if rect_contains(&divider, x, y) {
            editor.file_explorer.resizing = Some(editor.file_explorer.width);
            editor.switch_mode(EditorMode::FileExplorer);
            return MouseAction::None;
        }
    }

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

    if let Some(sidebar) = layout.sidebar {
        if rect_contains(&sidebar, x, y) {
            return handle_sidebar_click(editor, x, y, &sidebar);
        }
    }

    if rect_contains(&layout.tab_bar, x, y) {
        return handle_tab_bar_click(editor, x, y, &layout.tab_bar);
    }

    // The scrollbar is inside the editor region's columns, so it is tested
    // before it -- otherwise a press on the thumb would place the cursor on the
    // last visible character of the line instead.
    //
    // A popup owns the column while it is up, exactly as it owns the wheel: the
    // press is swallowed rather than scrolling the buffer behind it. It is not
    // allowed to close the popup either -- the scrollbar never changes the mode.
    if let Some(track) = layout.editor_scrollbar {
        if rect_contains(&track, x, y) {
            if !popup_is_up(editor) {
                handle_scrollbar_press(editor, y, &track);
            }
            return MouseAction::None;
        }
    }

    // The reserved row does not overlap `editor_area` -- `compute_layout` cut it
    // out of the same rows -- so testing it before the editor area is symmetry
    // with the column above and insurance against a future re-carve, not a
    // precedence the present code depends on.
    if let Some(row) = layout.editor_hscrollbar {
        if rect_contains(&row, x, y) {
            if !popup_is_up(editor) {
                handle_hscrollbar_press(editor, x, &row);
            }
            return MouseAction::None;
        }
    }

    if rect_contains(&layout.editor_area, x, y) {
        handle_editor_click(editor, x, y, &layout.editor_area);
    }

    MouseAction::None
}

/// A press on the scrollbar column. It is the wheel with a handle: the cursor,
/// the selection and the mode are all left exactly as they were.
///
/// A press on the thumb grabs it where it was pressed; a press anywhere else on
/// the track centres the thumb under the pointer and carries on under the same
/// rule, so the press and the drag that follows it cannot disagree.
///
/// No `MouseAction` comes back: scrolling is pure `Editor` state and `App` has
/// nothing to decide, unlike a sidebar resize (which rewrites the config).
fn handle_scrollbar_press(editor: &mut Editor, y: u16, track: &ratatui::layout::Rect) {
    let line_count = scrollbar_line_count(editor);
    let top_line = editor.active_view().map(|v| v.scroll.top_line).unwrap_or(0);
    // Nothing to scroll: the click is swallowed rather than falling through to
    // the text behind the column.
    let Some((offset, length)) = crate::ui::scrollbar::thumb(track.height, line_count, top_line)
    else {
        return;
    };

    let row = y.saturating_sub(track.y);
    if row >= offset && row < offset + length {
        editor.scrollbar_drag = Some(ScrollbarDrag::Vertical { grab: row - offset });
    } else {
        let grab = length / 2;
        editor.scrollbar_drag = Some(ScrollbarDrag::Vertical { grab });
        scroll_to_thumb(editor, y, track, grab);
    }
}

/// Put the thumb where the pointer holds it and scroll to match.
///
/// The pointer routinely runs past the track's ends, so the offset is clamped
/// rather than the event being dropped -- above the track pins to the first
/// line, below it to the last screen.
fn scroll_to_thumb(editor: &mut Editor, y: u16, track: &ratatui::layout::Rect, grab: u16) {
    let line_count = scrollbar_line_count(editor);
    let offset = y.saturating_sub(track.y).saturating_sub(grab);
    let top_line = crate::ui::scrollbar::offset_for_thumb(track.height, line_count, offset);
    if let Some(view) = editor.active_view_mut() {
        view.scroll.top_line = top_line;
    }
}

fn scrollbar_line_count(editor: &Editor) -> usize {
    editor
        .active_document()
        .map(|d| d.buffer.line_count())
        .unwrap_or(0)
}

/// A press on the reserved horizontal row, under exactly the vertical bar's
/// rules: the cursor, the selection and the mode are all left as they were, and
/// no `MouseAction` comes back because scrolling is pure `Editor` state.
///
/// A press on the gutter part of the row is swallowed rather than doing
/// anything: the gutter does not scroll, so it has no track (FR-HSCROLL-004).
///
/// An **empty** track -- no thumb, because everything on screen fits -- is the
/// one place the two bars differ, and deliberately: a press there returns the
/// view to column 0. The vertical bar has no such state to be in, since a
/// `top_line` past the document is not reachable, but `left_col` survives the
/// line it was scrolled along, so a long line scrolling off the top can leave
/// the viewport parked to the right of everything still on screen -- showing
/// blank columns, with the bar reporting (honestly) that this screen has
/// nothing to scroll. Pressing the empty track is what brings the content back.
/// The alternative, keeping a thumb on screen by flooring the total at
/// `left_col + code_width`, is what made the total depend on the field a drag
/// writes; see `ui::scrollbar::content_width`.
///
/// No drag is armed in that case: with no thumb there is nothing to take hold
/// of, and after the press there is nothing left to scroll to either.
fn handle_hscrollbar_press(editor: &mut Editor, x: u16, row: &ratatui::layout::Rect) {
    let Some(track) = hscrollbar_track(editor, row) else {
        return;
    };
    if !rect_contains(&track, x, track.y) {
        return;
    }
    debug_assert!(
        editor.scrollbar_drag.is_none(),
        "a press with a live drag: `handle_mouse` clears it on every `Down`"
    );

    let left_col = editor.active_view().map(|v| v.scroll.left_col).unwrap_or(0);
    let total = hscrollbar_total(editor, &track);
    let Some((offset, length)) = crate::ui::scrollbar::thumb(track.width, total, left_col) else {
        // Nothing to scroll on this screen. The press is still swallowed rather
        // than falling through to the text above the row -- it just has one
        // thing left to do first.
        if let Some(view) = editor.active_view_mut() {
            view.scroll.left_col = 0;
        }
        return;
    };

    let col = x.saturating_sub(track.x);
    if col >= offset && col < offset + length {
        editor.scrollbar_drag = Some(ScrollbarDrag::Horizontal { grab: col - offset });
    } else {
        let grab = length / 2;
        editor.scrollbar_drag = Some(ScrollbarDrag::Horizontal { grab });
        hscroll_to_thumb(editor, x, &track, grab);
    }
}

/// Put the horizontal thumb where the pointer holds it and scroll to match.
///
/// The pointer routinely runs past the track's ends, so the offset is clamped
/// rather than the event being dropped -- left of the track pins to column 0,
/// right of it to `max_left`.
///
/// The total is measured here, on every event, from the same function
/// `render.rs` draws through. That is safe -- and is the whole design -- because
/// it does not depend on `left_col`: this writes `left_col` and nothing else,
/// so the press, every drag event and the frame after the release all get the
/// same number, and the mapping from pointer column to position is one fixed
/// linear function for the life of the gesture.
fn hscroll_to_thumb(editor: &mut Editor, x: u16, track: &ratatui::layout::Rect, grab: u16) {
    let total = hscrollbar_total(editor, track);
    let offset = x.saturating_sub(track.x).saturating_sub(grab);
    let left_col = crate::ui::scrollbar::offset_for_thumb(track.width, total, offset);
    if let Some(view) = editor.active_view_mut() {
        view.scroll.left_col = left_col;
    }
}

/// The track inside the reserved row, or `None` where the gutter fills it or
/// there is no document. `ui::scrollbar::h_track` is the single source of its
/// columns, shared with `render.rs`.
fn hscrollbar_track(editor: &Editor, row: &ratatui::layout::Rect) -> Option<ratatui::layout::Rect> {
    let doc = editor.active_document()?;
    let gutter_width = crate::ui::editor_view::line_number_width_styled(
        doc.buffer.line_count(),
        editor.config.line_numbers,
    );
    crate::ui::scrollbar::h_track(*row, gutter_width)
}

/// The horizontal scroll total for the current view, from the one function
/// `render.rs` draws the thumb through -- so the thumb grabbed is the thumb
/// that was drawn, and neither side can drift.
fn hscrollbar_total(editor: &Editor, track: &ratatui::layout::Rect) -> usize {
    crate::ui::scrollbar::hscroll_total(editor, track.width as usize)
}

/// Put the divider under the cursor: the column dragged to becomes the
/// sidebar's last one, so the divider tracks the pointer without drifting.
///
/// The drag is not confined to the sidebar -- the pointer routinely runs past
/// the bounds, and the clamp is what stops it rather than the event being
/// dropped.
fn resize_sidebar(editor: &mut Editor, x: u16, layout: &AppLayout) {
    let width = x.saturating_sub(layout.frame.x).saturating_add(1);
    editor.file_explorer.width = crate::layout::clamp_sidebar_width(width, layout.frame.width);
}

fn handle_editor_click(editor: &mut Editor, x: u16, y: u16, editor_area: &ratatui::layout::Rect) {
    let line_count = editor
        .active_document()
        .map(|d| d.buffer.line_count())
        .unwrap_or(0);
    let gutter_width =
        crate::ui::editor_view::line_number_width_styled(line_count, editor.config.line_numbers);
    let code_start = editor_area.x + gutter_width + 1;

    if x < code_start {
        handle_line_number_click(editor, y, editor_area);
        return;
    }

    let view_top = editor.active_view().map(|v| v.scroll.top_line).unwrap_or(0);
    let left_col = editor.active_view().map(|v| v.scroll.left_col).unwrap_or(0);
    let row_offset = (y - editor_area.y) as usize;
    let target_line = view_top + row_offset;

    if target_line >= line_count {
        return;
    }

    let display_col = (x - code_start) as usize + left_col;
    // A click anywhere inside a character's span names that character: the
    // second cell of a CJK glyph, and every column of a tab's expansion.
    let tabs = crate::display_width::TabStops::from_config(&editor.config);
    let target_col = editor
        .active_document()
        .map(|d| {
            let line_text: String = d.buffer.line(target_line).chars().collect();
            let line_text = line_text.trim_end_matches(&['\n', '\r'][..]);
            tabs.char_at_col(line_text, display_col)
        })
        .unwrap_or(0);

    if editor.mode != EditorMode::Insert && editor.mode != EditorMode::Normal {
        editor.switch_to_default_mode();
    }

    if let Some(view) = editor.active_view_mut() {
        view.cursor.line = target_line;
        view.cursor.column = target_col;
    }
    sync_selection_from_cursor(editor);
}

fn handle_line_number_click(editor: &mut Editor, y: u16, editor_area: &ratatui::layout::Rect) {
    let line_count = editor
        .active_document()
        .map(|d| d.buffer.line_count())
        .unwrap_or(0);
    let view_top = editor.active_view().map(|v| v.scroll.top_line).unwrap_or(0);
    let row_offset = (y - editor_area.y) as usize;
    let target_line = view_top + row_offset;

    if target_line >= line_count {
        return;
    }

    let doc = match editor.active_document() {
        Some(d) => d,
        None => return,
    };
    let line_byte_start = doc.buffer.text().line_to_byte(target_line);
    let line_byte_end = if target_line + 1 < line_count {
        doc.buffer.text().line_to_byte(target_line + 1)
    } else {
        doc.buffer.text().len_bytes()
    };

    let doc_id = editor.active_view().map(|v| v.doc_id);
    if let Some(doc_id) = doc_id {
        if let Some(doc) = editor.documents.get_mut(&doc_id) {
            doc.selection = Selection::single(line_byte_start, line_byte_end);
        }
    }

    if let Some(view) = editor.active_view_mut() {
        view.cursor.line = target_line;
        view.cursor.column = 0;
    }
}

fn handle_sidebar_click(
    editor: &mut Editor,
    x: u16,
    y: u16,
    sidebar: &ratatui::layout::Rect,
) -> MouseAction {
    // A click picks a row, and the pending new-entry row would shift every
    // index below it. Clicking away from a half-typed name drops it.
    editor.file_explorer.cancel_new_entry();
    let row_offset = (y - sidebar.y) as usize;
    let target_index = editor.file_explorer.scroll_offset + row_offset;

    if target_index >= editor.file_explorer.tree.len() {
        return MouseAction::None;
    }

    let already_selected = editor.file_explorer.selected == target_index;
    editor.file_explorer.selected = target_index;
    editor.switch_mode(EditorMode::FileExplorer);

    // The chevron is a control rather than part of the entry: one click on it
    // expands the directory in place, where a double click on the name would
    // re-root the tree there.
    let logical_x = (x - sidebar.x) + editor.file_explorer.scroll_left;
    let span = crate::ui::file_explorer::chevron_span(
        &editor.file_explorer.tree[target_index],
        &editor.file_tree_style,
        &editor.theme,
    );
    if span.is_some_and(|(start, end)| logical_x >= start && logical_x < end) {
        return MouseAction::ToggleExplorerExpand(target_index);
    }

    // First click selects, second click on same item opens
    if already_selected {
        MouseAction::OpenExplorerItem(target_index)
    } else {
        MouseAction::None
    }
}

fn handle_tab_bar_click(
    editor: &mut Editor,
    x: u16,
    _y: u16,
    tab_bar: &ratatui::layout::Rect,
) -> MouseAction {
    let positions = tab_positions(&editor.tabs);
    let click_x = (x - tab_bar.x) as usize;

    for (i, (start, end)) in positions.iter().enumerate() {
        if click_x >= *start && click_x < *end {
            return MouseAction::SwitchTab(i);
        }
    }
    MouseAction::None
}

/// Compute tab label positions (start_x, end_x) for mouse hit-testing.
pub fn tab_positions(tabs: &termcode_view::tab::TabManager) -> Vec<(usize, usize)> {
    let mut positions = Vec::new();
    let mut x: usize = 0;
    for (i, tab) in tabs.tabs.iter().enumerate() {
        if i > 0 {
            x += 1; // separator '|'
        }
        let label_width = crate::display_width::ui_str_width(&tab.label);
        let label_len = if tab.modified {
            3 + label_width + 1
        } else {
            1 + label_width + 1
        };
        positions.push((x, x + label_len));
        x += label_len;
    }
    positions
}

/// True while something is drawn over the editor that the wheel belongs to.
fn popup_is_up(editor: &Editor) -> bool {
    editor.help_visible
        || matches!(
            editor.mode,
            EditorMode::Search
                | EditorMode::FuzzyFinder
                | EditorMode::CommandPalette
                | EditorMode::Settings
        )
}

/// One wheel notch, `-1` up and `1` down.
///
/// A popup owns the wheel while it is up: it moves the popup's own list where
/// there is one and is swallowed where there is not, rather than scrolling the
/// buffer behind it. Text sliding around underneath a dialog reads as the
/// input having gone through to the editor, which is exactly what has not
/// happened -- these popups are modal for the keyboard already.
///
/// Where the pointer is does not come into it, for the same reason: a wheel
/// that fell through to the editor whenever the pointer happened to be outside
/// the popup would be the leak this exists to close.
fn handle_wheel(editor: &mut Editor, direction: i32, layout: &AppLayout) -> MouseAction {
    if popup_is_up(editor) {
        return match editor.mode {
            EditorMode::CommandPalette => {
                editor
                    .command_palette
                    .move_selection(direction * WHEEL_LINES);
                MouseAction::None
            }
            EditorMode::FuzzyFinder => {
                editor.fuzzy_finder.move_selection(direction * WHEEL_LINES);
                MouseAction::None
            }
            EditorMode::Settings => MouseAction::ScrollSettings(direction),
            // The search overlay is an input line and the help popup is a
            // single page: nothing to move, but still nothing to leak.
            _ => MouseAction::None,
        };
    }

    if direction < 0 {
        handle_scroll_up(editor, 0, 0, layout);
    } else {
        handle_scroll_down(editor, 0, 0, layout);
    }
    MouseAction::None
}

fn handle_scroll_up(editor: &mut Editor, _x: u16, _y: u16, _layout: &AppLayout) {
    if let Some(view) = editor.active_view_mut() {
        view.scroll_up(3);
    }
}

fn handle_scroll_down(editor: &mut Editor, _x: u16, _y: u16, _layout: &AppLayout) {
    let line_count = editor
        .active_document()
        .map(|d| d.buffer.line_count())
        .unwrap_or(0);
    if let Some(view) = editor.active_view_mut() {
        view.scroll_down(3, line_count);
    }
}

fn handle_drag(editor: &mut Editor, x: u16, y: u16, layout: &AppLayout) {
    // A drag belongs to whichever bar the press started on. `ScrollbarDrag`
    // carries the axis, so there is no order to pick between two bars.
    //
    // Same swallow rule as the press on both arms. A popup cannot normally come
    // up mid-drag, but the swallow belongs on the drag too: falling through to
    // the text selection below would be a worse leak than doing nothing.
    match editor.scrollbar_drag {
        Some(ScrollbarDrag::Vertical { grab }) => {
            if let Some(track) = layout.editor_scrollbar {
                if !popup_is_up(editor) {
                    scroll_to_thumb(editor, y, &track, grab);
                }
            }
            return;
        }
        Some(ScrollbarDrag::Horizontal { grab }) => {
            if let (Some(row), false) = (layout.editor_hscrollbar, popup_is_up(editor)) {
                if let Some(track) = hscrollbar_track(editor, &row) {
                    hscroll_to_thumb(editor, x, &track, grab);
                }
            }
            return;
        }
        None => {}
    }

    if editor.file_explorer.resizing.is_some() {
        resize_sidebar(editor, x, layout);
        return;
    }

    if !rect_contains(&layout.editor_area, x, y) {
        return;
    }

    let line_count = editor
        .active_document()
        .map(|d| d.buffer.line_count())
        .unwrap_or(0);
    let gutter_width =
        crate::ui::editor_view::line_number_width_styled(line_count, editor.config.line_numbers);
    let code_start = layout.editor_area.x + gutter_width + 1;

    if x < code_start {
        return;
    }

    let view_top = editor.active_view().map(|v| v.scroll.top_line).unwrap_or(0);
    let left_col = editor.active_view().map(|v| v.scroll.left_col).unwrap_or(0);
    let row_offset = (y - layout.editor_area.y) as usize;
    let target_line = (view_top + row_offset).min(line_count.saturating_sub(1));
    let display_col = (x - code_start) as usize + left_col;

    let tabs = crate::display_width::TabStops::from_config(&editor.config);
    let target_col = editor
        .active_document()
        .map(|d| {
            if target_line < d.buffer.line_count() {
                let line_text: String = d.buffer.line(target_line).chars().collect();
                let line_text = line_text.trim_end_matches(&['\n', '\r'][..]);
                tabs.char_at_col(line_text, display_col)
            } else {
                0
            }
        })
        .unwrap_or(0);

    if let Some(view) = editor.active_view_mut() {
        view.cursor.line = target_line;
        view.cursor.column = target_col;
    }

    let sel_data = editor.active_view().and_then(|view| {
        let cursor = view.cursor;
        let doc_id = view.doc_id;
        let doc = editor.documents.get(&doc_id)?;
        let head_byte = doc.buffer.pos_to_byte(&cursor);
        let anchor = doc.selection.primary().anchor;
        Some((doc_id, anchor, head_byte))
    });
    if let Some((doc_id, anchor, head_byte)) = sel_data {
        if let Some(doc) = editor.documents.get_mut(&doc_id) {
            doc.selection = Selection::single(anchor, head_byte);
        }
    }
}

fn rect_contains(rect: &ratatui::layout::Rect, x: u16, y: u16) -> bool {
    x >= rect.x && x < rect.x + rect.width && y >= rect.y && y < rect.y + rect.height
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_positions_empty() {
        let tabs = termcode_view::tab::TabManager::new();
        assert!(tab_positions(&tabs).is_empty());
    }

    #[test]
    fn tab_positions_single() {
        let mut tabs = termcode_view::tab::TabManager::new();
        tabs.add(
            "main.rs".to_string(),
            termcode_view::image::TabContent::Document(termcode_view::document::DocumentId(0)),
        );
        let positions = tab_positions(&tabs);
        assert_eq!(positions.len(), 1);
        // " main.rs " = 1 + 7 + 1 = 9
        assert_eq!(positions[0], (0, 9));
    }

    #[test]
    fn tab_positions_multiple() {
        let mut tabs = termcode_view::tab::TabManager::new();
        tabs.add(
            "a.rs".to_string(),
            termcode_view::image::TabContent::Document(termcode_view::document::DocumentId(0)),
        );
        tabs.add(
            "b.rs".to_string(),
            termcode_view::image::TabContent::Document(termcode_view::document::DocumentId(1)),
        );
        let positions = tab_positions(&tabs);
        assert_eq!(positions.len(), 2);
        // " a.rs " = 6, then separator (1), " b.rs " = 6
        assert_eq!(positions[0], (0, 6));
        assert_eq!(positions[1], (7, 13));
    }

    #[test]
    fn rect_contains_basic() {
        let rect = ratatui::layout::Rect::new(10, 10, 20, 20);
        assert!(rect_contains(&rect, 10, 10));
        assert!(rect_contains(&rect, 29, 29));
        assert!(!rect_contains(&rect, 30, 30));
        assert!(!rect_contains(&rect, 9, 10));
    }

    use ratatui::layout::Rect;

    fn make_editor() -> Editor {
        use termcode_core::config_types::EditorConfig;
        use termcode_syntax::language::LanguageRegistry;
        use termcode_theme::theme::Theme;
        Editor::new(
            Theme::default(),
            EditorConfig::default(),
            LanguageRegistry::new(),
            None,
        )
    }

    fn layout_with_title() -> AppLayout {
        AppLayout {
            frame: Rect::new(0, 0, 80, 24),
            top_bar: Rect::new(0, 0, 80, 1),
            sidebar: Some(Rect::new(0, 2, 20, 21)),
            sidebar_toolbar: None,
            sidebar_title: Some(Rect::new(0, 1, 20, 1)),
            sidebar_border: None,
            sidebar_divider: Some(Rect::new(19, 1, 1, 22)),
            sidebar_panel: None,
            editor_panel: None,
            tab_bar: Rect::new(20, 1, 60, 1),
            // The text rows, less the scrollbar column and the scrollbar row.
            // These four must agree with `compute_layout(frame, true, 20, ..)`
            // -- `the_fixtures_agree_with_compute_layout` is what holds them to
            // it, because a fixture that drifts tests nothing real.
            editor_area: Rect::new(20, 2, 59, 20),
            editor_scrollbar: Some(Rect::new(79, 2, 1, 20)),
            editor_hscrollbar: Some(Rect::new(20, 22, 59, 1)),
            status_bar: Rect::new(0, 23, 80, 1),
        }
    }

    fn layout_with_border() -> AppLayout {
        AppLayout {
            frame: Rect::new(0, 0, 80, 24),
            top_bar: Rect::new(0, 0, 80, 1),
            sidebar: Some(Rect::new(0, 2, 19, 21)),
            sidebar_toolbar: Some(Rect::new(0, 1, 19, 1)),
            sidebar_title: None,
            sidebar_border: Some(Rect::new(19, 1, 1, 22)),
            sidebar_divider: Some(Rect::new(19, 1, 1, 22)),
            sidebar_panel: None,
            editor_panel: None,
            tab_bar: Rect::new(20, 1, 60, 1),
            editor_area: Rect::new(20, 2, 59, 20),
            editor_scrollbar: Some(Rect::new(79, 2, 1, 20)),
            editor_hscrollbar: Some(Rect::new(20, 22, 59, 1)),
            status_bar: Rect::new(0, 23, 80, 1),
        }
    }

    fn mouse_at(kind: MouseEventKind, x: u16, y: u16) -> MouseEvent {
        MouseEvent {
            kind,
            column: x,
            row: y,
            modifiers: crossterm::event::KeyModifiers::NONE,
        }
    }

    fn press(x: u16, y: u16) -> MouseEvent {
        mouse_at(MouseEventKind::Down(MouseButton::Left), x, y)
    }

    fn drag(x: u16, y: u16) -> MouseEvent {
        mouse_at(MouseEventKind::Drag(MouseButton::Left), x, y)
    }

    fn release(x: u16, y: u16) -> MouseEvent {
        mouse_at(MouseEventKind::Up(MouseButton::Left), x, y)
    }

    #[test]
    fn dragging_the_divider_resizes_the_sidebar_and_reports_the_new_width() {
        let mut editor = make_editor();
        let layout = layout_with_title();
        editor.file_explorer.width = 20;

        handle_mouse(&mut editor, press(19, 5), &layout);
        assert_eq!(editor.file_explorer.resizing, Some(20), "the drag is armed");

        // The column dragged to becomes the sidebar's last one, so the divider
        // sits under the pointer rather than drifting away from it.
        handle_mouse(&mut editor, drag(34, 5), &layout);
        assert_eq!(editor.file_explorer.width, 35);

        let action = handle_mouse(&mut editor, release(34, 5), &layout);
        assert!(matches!(action, MouseAction::SidebarResized(35)));
        assert!(editor.file_explorer.resizing.is_none(), "the drag ended");
    }

    #[test]
    fn a_press_on_the_divider_that_never_moves_writes_nothing() {
        let mut editor = make_editor();
        let layout = layout_with_title();
        editor.file_explorer.width = 20;

        handle_mouse(&mut editor, press(19, 5), &layout);
        let action = handle_mouse(&mut editor, release(19, 5), &layout);

        assert!(matches!(action, MouseAction::None), "nothing to save");
        assert_eq!(editor.file_explorer.width, 20);
        // The column focuses the tree as it did before it was also a handle.
        assert_eq!(editor.mode, EditorMode::FileExplorer);
    }

    #[test]
    fn the_drag_is_clamped_rather_than_dropped_when_it_leaves_the_sidebar() {
        let mut editor = make_editor();
        let layout = layout_with_title();
        editor.file_explorer.width = 20;

        handle_mouse(&mut editor, press(19, 5), &layout);
        // Far right, past the frame: the editor keeps its columns.
        handle_mouse(&mut editor, drag(79, 5), &layout);
        assert_eq!(
            editor.file_explorer.width,
            crate::layout::clamp_sidebar_width(80, 80)
        );
        // Far left, into the first column: the sidebar keeps its minimum.
        handle_mouse(&mut editor, drag(0, 5), &layout);
        assert_eq!(editor.file_explorer.width, crate::layout::MIN_SIDEBAR_WIDTH);
    }

    #[test]
    fn a_drag_that_did_not_start_on_the_divider_still_selects_text() {
        let mut editor = make_editor();
        let layout = layout_with_title();
        editor.file_explorer.width = 20;

        handle_mouse(&mut editor, press(40, 5), &layout);
        handle_mouse(&mut editor, drag(50, 5), &layout);

        assert_eq!(editor.file_explorer.width, 20, "the sidebar is untouched");
    }

    #[test]
    fn a_new_press_clears_a_drag_whose_release_was_lost() {
        let mut editor = make_editor();
        let layout = layout_with_title();
        editor.file_explorer.width = 20;
        // As if the button had come up outside the terminal.
        editor.file_explorer.resizing = Some(20);

        handle_mouse(&mut editor, press(40, 5), &layout);
        assert!(editor.file_explorer.resizing.is_none());

        handle_mouse(&mut editor, drag(50, 5), &layout);
        assert_eq!(editor.file_explorer.width, 20, "not stuck to the pointer");
    }

    fn wheel(down: bool, x: u16, y: u16) -> MouseEvent {
        mouse_at(
            if down {
                MouseEventKind::ScrollDown
            } else {
                MouseEventKind::ScrollUp
            },
            x,
            y,
        )
    }

    /// Size a view the way `App` does every frame: from the fixture layout's
    /// `editor_area`, never from a track's own height or a hand-written number.
    ///
    /// In production the two are the same number, because `compute_layout`
    /// carves both tracks out of the rows and columns `editor_area` gives up. A
    /// fixture where they differ would let the wheel's `max_top` and the
    /// thumb's disagree here and nowhere else -- and horizontally, would let
    /// `content_width`'s `code_width` (taken from the track) and
    /// `ensure_h_scroll`'s (taken from `area_width`) describe different
    /// viewports.
    fn size_view_from_layout(editor: &mut Editor, layout: &AppLayout) {
        let area = layout.editor_area;
        let view = editor.active_view_mut().unwrap();
        view.area_height = area.height;
        view.area_width = area.width;
    }

    /// An editor with a document long enough to scroll, parked partway down it.
    fn editor_with_a_scrolled_document(name: &str) -> (Editor, std::path::PathBuf) {
        let dir = std::env::temp_dir().join("termcode-wheel-tests");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("{name}.txt"));
        std::fs::write(&path, "line\n".repeat(500)).unwrap();

        let mut editor = make_editor();
        editor.open_file(&path).unwrap();
        size_view_from_layout(&mut editor, &layout_with_title());
        editor.active_view_mut().unwrap().scroll.top_line = 100;
        (editor, path)
    }

    #[test]
    fn a_popup_swallows_the_wheel_instead_of_scrolling_the_buffer_behind_it() {
        let layout = layout_with_title();
        for mode in [
            EditorMode::Search,
            EditorMode::FuzzyFinder,
            EditorMode::CommandPalette,
            EditorMode::Settings,
        ] {
            let (mut editor, _p) = editor_with_a_scrolled_document("popup-modes");
            editor.mode = mode;

            handle_mouse(&mut editor, wheel(true, 60, 10), &layout);
            handle_mouse(&mut editor, wheel(false, 60, 10), &layout);

            assert_eq!(
                editor.active_view().unwrap().scroll.top_line,
                100,
                "{mode:?} let the wheel through to the buffer"
            );
        }
    }

    #[test]
    fn a_popup_swallows_a_scrollbar_press_and_drag_the_same_way_it_swallows_the_wheel() {
        let layout = layout_with_title();
        let track = scrollbar_track();
        for mode in [
            EditorMode::Search,
            EditorMode::FuzzyFinder,
            EditorMode::CommandPalette,
            EditorMode::Settings,
        ] {
            let (mut editor, _p) = editor_with_a_scrolled_document("popup-scrollbar");
            editor.mode = mode;

            let action = handle_mouse(&mut editor, press(track.x, track.y + 15), &layout);
            assert!(matches!(action, MouseAction::None));
            assert_eq!(
                editor.scrollbar_drag, None,
                "{mode:?} let the press grab the thumb"
            );

            // The drag that follows a *refused* press: `scrollbar_drag` is
            // `None`, so this goes down `handle_drag`'s `None` arm and must not
            // reach the text selection underneath either. The guard on the
            // `Some` arms is a different case, and is pinned by
            // `a_popup_that_opens_mid_drag_freezes_the_held_vertical_thumb`.
            handle_mouse(&mut editor, drag(track.x, track.y + 18), &layout);
            handle_mouse(&mut editor, release(track.x, track.y + 18), &layout);

            assert_eq!(
                editor.active_view().unwrap().scroll.top_line,
                100,
                "{mode:?} let the scrollbar through to the buffer"
            );
            // Swallowed, not dismissed: the scrollbar never changes the mode.
            assert_eq!(editor.mode, mode);
        }
    }

    /// A popup that comes up *mid-drag* -- the case `handle_drag`'s guard is
    /// the only thing standing in the way of.
    ///
    /// The press has to land before the popup, or the press guard turns the
    /// drag away and `scrollbar_drag` is never armed, leaving `handle_drag` to
    /// take its `None` arm and the guard on its `Some` arms unexecuted. The way
    /// this happens for real is the keyboard: the thumb is held with the mouse
    /// and `Ctrl+P` opens the palette, and crossterm's key and mouse streams
    /// are independent, so the next `Drag` arrives with the popup already up.
    #[test]
    fn a_popup_that_opens_mid_drag_freezes_the_held_vertical_thumb() {
        let layout = layout_with_title();
        let track = scrollbar_track();

        // The same press and drag with nothing in the way, so the assertions
        // below are about the guard rather than about a drag going nowhere.
        let (mut editor, _p) = editor_with_a_scrolled_document("mid-drag-vertical-control");
        handle_mouse(&mut editor, press(track.x, track.y + 15), &layout);
        let armed = editor.active_view().unwrap().scroll.top_line;
        handle_mouse(
            &mut editor,
            drag(track.x, track.y + track.height + 40),
            &layout,
        );
        let moved = editor.active_view().unwrap().scroll.top_line;
        assert_ne!(moved, armed, "the control drag moved nothing to guard");

        for mode in [
            EditorMode::Search,
            EditorMode::FuzzyFinder,
            EditorMode::CommandPalette,
            EditorMode::Settings,
        ] {
            let (mut editor, _p) = editor_with_a_scrolled_document("mid-drag-vertical");
            handle_mouse(&mut editor, press(track.x, track.y + 15), &layout);
            assert!(
                editor.scrollbar_drag.is_some(),
                "the press armed no drag to interrupt"
            );
            let held = editor.active_view().unwrap().scroll.top_line;

            editor.mode = mode;
            handle_mouse(
                &mut editor,
                drag(track.x, track.y + track.height + 40),
                &layout,
            );

            assert_eq!(
                editor.active_view().unwrap().scroll.top_line,
                held,
                "{mode:?} let the held thumb go on dragging the buffer behind it"
            );
            // Swallowed, not dismissed: the scrollbar never changes the mode.
            assert_eq!(editor.mode, mode);
        }
    }

    #[test]
    fn the_help_popup_swallows_the_wheel_too() {
        let (mut editor, _p) = editor_with_a_scrolled_document("popup-help");
        editor.help_visible = true;

        handle_mouse(&mut editor, wheel(true, 60, 10), &layout_with_title());

        assert_eq!(editor.active_view().unwrap().scroll.top_line, 100);
    }

    #[test]
    fn the_wheel_moves_the_list_of_the_popup_that_took_it() {
        use termcode_view::palette::PaletteItem;

        let (mut editor, _p) = editor_with_a_scrolled_document("popup-list");
        editor.mode = EditorMode::CommandPalette;
        editor.command_palette.load_commands(
            (0..20)
                .map(|i| PaletteItem {
                    id: format!("cmd.{i}"),
                    name: format!("Command {i}"),
                })
                .collect(),
        );

        handle_mouse(&mut editor, wheel(true, 60, 10), &layout_with_title());
        assert_eq!(
            editor.command_palette.selected, 3,
            "one notch is three rows"
        );

        handle_mouse(&mut editor, wheel(false, 60, 10), &layout_with_title());
        assert_eq!(editor.command_palette.selected, 0);
    }

    #[test]
    fn the_settings_wheel_goes_back_to_app() {
        let (mut editor, _p) = editor_with_a_scrolled_document("popup-settings");
        editor.mode = EditorMode::Settings;

        // The screen has a category pane and a value picker hanging off it, so
        // the move is `App`'s to make.
        let action = handle_mouse(&mut editor, wheel(true, 60, 10), &layout_with_title());
        assert!(matches!(action, MouseAction::ScrollSettings(1)));
        let action = handle_mouse(&mut editor, wheel(false, 60, 10), &layout_with_title());
        assert!(matches!(action, MouseAction::ScrollSettings(-1)));
    }

    #[test]
    fn the_wheel_still_scrolls_the_buffer_with_nothing_over_it() {
        let (mut editor, _p) = editor_with_a_scrolled_document("no-popup");

        handle_mouse(&mut editor, wheel(true, 60, 10), &layout_with_title());
        assert_eq!(editor.active_view().unwrap().scroll.top_line, 103);

        handle_mouse(&mut editor, wheel(false, 60, 10), &layout_with_title());
        assert_eq!(editor.active_view().unwrap().scroll.top_line, 100);
    }

    #[test]
    fn click_on_the_top_bar_buttons_hits_the_right_one() {
        let layout = layout_with_title();
        let buttons = crate::ui::top_bar::buttons(layout.top_bar);
        let settings_start = buttons.settings_start.expect("80 columns fits both");

        // Exit only asks; App decides, since unsaved files need confirming.
        let mut editor = make_editor();
        assert!(matches!(
            handle_left_click(&mut editor, buttons.exit_start, 0, &layout),
            MouseAction::Quit
        ));

        // The click lands on the Settings button, but App is what opens the
        // screen -- the editor is untouched here.
        let mut editor = make_editor();
        assert!(matches!(
            handle_left_click(&mut editor, settings_start, 0, &layout),
            MouseAction::OpenSettings
        ));
        assert!(matches!(
            handle_left_click(&mut editor, buttons.exit_start - 1, 0, &layout),
            MouseAction::OpenSettings
        ));

        // One column left of Settings is the title, which does nothing.
        let mut editor = make_editor();
        assert!(matches!(
            handle_left_click(&mut editor, settings_start - 1, 0, &layout),
            MouseAction::None
        ));
        assert!(!editor.help_visible);
    }

    #[test]
    fn click_on_a_toolbar_button_runs_its_explorer_command() {
        let layout = layout_with_border();
        let toolbar = layout.sidebar_toolbar.unwrap();
        let labels = crate::ui::explorer_toolbar::ToolbarLabels::resolve(
            &make_editor().theme,
            termcode_core::config_types::FileTreeStyle::default().show_file_type_emoji,
        );
        let placed = crate::ui::explorer_toolbar::buttons(toolbar, &labels);
        assert!(!placed.is_empty(), "19 columns should fit some buttons");

        for (action, start) in placed {
            let mut editor = make_editor();
            editor.switch_mode(EditorMode::Normal);
            let result = handle_left_click(&mut editor, start, toolbar.y, &layout);
            match result {
                MouseAction::ExplorerCommand(cmd) => assert_eq!(cmd, action.command()),
                _ => panic!("{action:?} did not dispatch"),
            }
            assert_eq!(editor.mode, EditorMode::FileExplorer);
        }
    }

    #[test]
    fn click_on_the_toolbar_beside_the_buttons_only_focuses_the_explorer() {
        let layout = layout_with_border();
        let toolbar = layout.sidebar_toolbar.unwrap();
        let mut editor = make_editor();
        editor.switch_mode(EditorMode::Normal);
        assert!(matches!(
            handle_left_click(&mut editor, toolbar.x, toolbar.y, &layout),
            MouseAction::None
        ));
        assert_eq!(editor.mode, EditorMode::FileExplorer);
    }

    #[test]
    fn click_on_the_chevron_expands_and_a_click_on_the_name_does_not() {
        use termcode_view::file_explorer::FileNodeKind;

        let layout = layout_with_border();
        let sidebar = layout.sidebar.unwrap();
        let mut editor = make_editor();
        let index = editor
            .file_explorer
            .tree
            .iter()
            .position(|n| n.kind == FileNodeKind::Directory && !n.is_parent)
            .expect("the crate directory has subdirectories");
        let (start, end) = crate::ui::file_explorer::chevron_span(
            &editor.file_explorer.tree[index],
            &editor.file_tree_style,
            &editor.theme,
        )
        .expect("icons are on by default");
        let row = sidebar.y + index as u16;

        // On the chevron: expand, on the first click, without opening anything.
        assert!(matches!(
            handle_left_click(&mut editor, sidebar.x + start, row, &layout),
            MouseAction::ToggleExplorerExpand(i) if i == index
        ));

        // On the name: the usual select-then-open, which for a directory
        // re-roots the tree.
        let name_x = sidebar.x + end + 1;
        let mut editor = make_editor();
        assert!(matches!(
            handle_left_click(&mut editor, name_x, row, &layout),
            MouseAction::None
        ));
        assert_eq!(editor.file_explorer.selected, index);
        assert!(matches!(
            handle_left_click(&mut editor, name_x, row, &layout),
            MouseAction::OpenExplorerItem(i) if i == index
        ));
    }

    #[test]
    fn the_parent_row_has_no_chevron_to_click() {
        let layout = layout_with_border();
        let sidebar = layout.sidebar.unwrap();
        let mut editor = make_editor();
        assert!(
            editor.file_explorer.tree[0].is_parent,
            "the crate directory has a parent"
        );

        // Every column of the `..` row selects it, then opens it -- there is no
        // chevron to swallow the click.
        for _ in 0..2 {
            let action = handle_left_click(&mut editor, sidebar.x, sidebar.y, &layout);
            assert!(matches!(
                action,
                MouseAction::None | MouseAction::OpenExplorerItem(0)
            ));
        }
        assert_eq!(editor.file_explorer.selected, 0);
    }

    #[test]
    fn a_confirm_button_takes_the_focus_first_and_runs_on_the_second_click() {
        use termcode_view::confirm::{ConfirmAction, ConfirmDialog};

        let layout = layout_with_title();
        let mut editor = make_editor();
        let dialog = ConfirmDialog::new(
            ConfirmAction::QuitAll,
            "You have 1 unsaved file(s).".to_string(),
            vec!["Save".to_string(), "Discard".to_string()],
        );
        let placed = crate::ui::confirm_dialog::layout(&dialog, layout.frame).unwrap();
        editor.confirm_dialog = Some(dialog);

        let (start, _) = placed.buttons[1];
        let event = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: start,
            row: placed.button_y,
            modifiers: crossterm::event::KeyModifiers::NONE,
        };

        // Button 1 is not the focused one, so the first click only moves focus.
        assert!(matches!(
            handle_mouse(&mut editor, event, &layout),
            MouseAction::None
        ));
        assert_eq!(editor.confirm_dialog.as_ref().unwrap().selected_button, 1);

        assert!(matches!(
            handle_mouse(&mut editor, event, &layout),
            MouseAction::ConfirmSelected
        ));
        assert_eq!(
            editor.confirm_dialog.as_ref().unwrap().selected_button,
            1,
            "the clicked button is the one that runs"
        );
    }

    #[test]
    fn the_confirm_dialog_swallows_clicks_that_miss_its_buttons() {
        use termcode_view::confirm::{ConfirmAction, ConfirmDialog};

        let layout = layout_with_title();
        let mut editor = make_editor();
        editor.confirm_dialog = Some(ConfirmDialog::new(
            ConfirmAction::QuitAll,
            "You have 1 unsaved file(s).".to_string(),
            vec!["Save".to_string(), "Cancel".to_string()],
        ));

        // The Exit button is behind the dialog and must not be reachable.
        let buttons = crate::ui::top_bar::buttons(layout.top_bar);
        let event = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: buttons.exit_start,
            row: 0,
            modifiers: crossterm::event::KeyModifiers::NONE,
        };
        assert!(matches!(
            handle_mouse(&mut editor, event, &layout),
            MouseAction::None
        ));
        assert!(
            editor.confirm_dialog.is_some(),
            "a stray click must not dismiss the dialog"
        );
    }

    #[test]
    fn click_sidebar_title_switches_to_file_explorer() {
        let mut editor = make_editor();
        editor.switch_mode(EditorMode::Normal);
        let layout = layout_with_title();
        let action = handle_left_click(&mut editor, 5, 1, &layout);
        assert!(matches!(action, MouseAction::None));
        assert_eq!(editor.mode, EditorMode::FileExplorer);
    }

    #[test]
    fn click_sidebar_border_switches_to_file_explorer() {
        let mut editor = make_editor();
        editor.switch_mode(EditorMode::Normal);
        let layout = layout_with_border();
        let action = handle_left_click(&mut editor, 19, 5, &layout);
        assert!(matches!(action, MouseAction::None));
        assert_eq!(editor.mode, EditorMode::FileExplorer);
    }

    #[test]
    fn click_sidebar_content_dispatches_to_sidebar_handler() {
        let mut editor = make_editor();
        editor.switch_mode(EditorMode::Normal);
        let layout = layout_with_title();
        let action = handle_left_click(&mut editor, 5, 3, &layout);
        // sidebar content click goes to handle_sidebar_click -> OpenExplorerItem or None
        // (tree is empty so returns None, but mode switches to FileExplorer)
        assert!(matches!(
            action,
            MouseAction::None | MouseAction::OpenExplorerItem(_)
        ));
        assert_eq!(editor.mode, EditorMode::FileExplorer);
    }

    /// The reserved column of `layout_with_title()`.
    fn scrollbar_track() -> Rect {
        layout_with_title().editor_scrollbar.expect("a scrollbar")
    }

    /// A line mixing every shape a column can take, for the click round trip:
    /// a leading tab, consecutive tabs, a CJK character, a combining mark and a
    /// trailing tab.
    const TAB_LINE: &str = "\tab\t\t한글\te\u{0301}x\ty";

    fn editor_with_a_tab_mixed_line(name: &str, tab_size: usize) -> (Editor, std::path::PathBuf) {
        use termcode_core::config_types::EditorConfig;
        use termcode_syntax::language::LanguageRegistry;
        use termcode_theme::theme::Theme;

        let dir = std::env::temp_dir().join("termcode-tab-click-tests");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("{name}-{tab_size}.txt"));
        std::fs::write(&path, format!("{TAB_LINE}\n")).unwrap();

        let config = EditorConfig {
            tab_size,
            ..EditorConfig::default()
        };
        let mut editor = Editor::new(Theme::default(), config, LanguageRegistry::new(), None);
        editor.open_file(&path).unwrap();
        size_view_from_layout(&mut editor, &layout_with_title());
        (editor, path)
    }

    /// The code area's cells, one entry per column, as the widget draws them.
    fn drawn_columns(editor: &Editor, layout: &AppLayout) -> Vec<String> {
        use ratatui::widgets::Widget;
        let area = layout.editor_area;
        let mut buf = ratatui::buffer::Buffer::empty(area);
        let doc = editor.active_document().unwrap();
        let view = editor.active_view().unwrap();
        crate::ui::editor_view::EditorViewWidget::new(
            doc,
            view,
            &editor.theme,
            editor.mode,
            None,
            &editor.config,
            true,
        )
        .render(area, &mut buf);
        // Three gutter columns and a separator, then the code.
        (area.x + 4..area.x + area.width)
            .map(|x| buf[(x, area.y)].symbol().to_string())
            .collect()
    }

    #[test]
    fn a_click_anywhere_in_a_characters_span_names_that_character() {
        // FR-TAB-006. Every column of a tab's expansion resolves to the tab,
        // the round trip `col -> char index -> col` lands on the start column
        // of the character covering the click, and the character drawn at the
        // clicked column is the one the click named.
        let layout = layout_with_title();
        let code_start = layout.editor_area.x + 4;
        for tab_size in [4usize, 8, 2] {
            let (mut editor, _p) = editor_with_a_tab_mixed_line("round-trip", tab_size);
            let tabs = crate::display_width::TabStops::from_config(&editor.config);
            let cells = drawn_columns(&editor, &layout);
            let total = tabs.col_at_char(TAB_LINE, TAB_LINE.chars().count());
            assert!(
                total <= cells.len(),
                "the fixture line must fit the code area"
            );

            for (col, cell) in cells.iter().enumerate().take(total) {
                handle_left_click(
                    &mut editor,
                    code_start + col as u16,
                    layout.editor_area.y,
                    &layout,
                );
                let index = editor.active_view().unwrap().cursor.column;
                assert_eq!(
                    index,
                    tabs.char_at_col(TAB_LINE, col),
                    "tab_size={tab_size} col={col}"
                );

                let ch = TAB_LINE.chars().nth(index).expect("a character");
                let start = tabs.col_at_char(TAB_LINE, index);
                assert!(
                    start <= col && tabs.next_col(start, ch) > col,
                    "tab_size={tab_size} col={col}: char {index} does not cover the clicked column"
                );
                // Clicking the character's own start column names it again, so
                // clicking where the cursor already is leaves it alone.
                handle_left_click(
                    &mut editor,
                    code_start + start as u16,
                    layout.editor_area.y,
                    &layout,
                );
                assert_eq!(
                    editor.active_view().unwrap().cursor.column,
                    index,
                    "tab_size={tab_size} col={col}: the round trip moved the cursor"
                );

                // And the frame agrees: what is painted at the clicked column
                // belongs to the character the click named.
                if ch == '\t' {
                    assert_eq!(cell, " ", "tab_size={tab_size} col={col}");
                } else if col == start && tabs.next_col(start, ch) > start {
                    assert_eq!(cell, &ch.to_string(), "tab_size={tab_size} col={col}");
                }
            }
        }
    }

    #[test]
    fn a_click_inside_a_tab_moves_the_cursor_to_the_tabs_first_column() {
        // Visible behaviour, and the contract rather than a glitch: a character
        // occupying several columns is selected whole, the same answer already
        // given for the second cell of a CJK glyph.
        let layout = layout_with_title();
        let code_start = layout.editor_area.x + 4;
        for tab_size in [4usize, 8, 2] {
            let (mut editor, _p) = editor_with_a_tab_mixed_line("inside-a-tab", tab_size);
            // The leading tab spans columns 0..tab_size; click its last one.
            handle_left_click(
                &mut editor,
                code_start + (tab_size - 1) as u16,
                layout.editor_area.y,
                &layout,
            );
            let view = editor.active_view().unwrap();
            assert_eq!(
                view.cursor.column, 0,
                "tab_size={tab_size}: the click landed off the tab"
            );
            let tabs = crate::display_width::TabStops::from_config(&editor.config);
            assert_eq!(
                tabs.col_at_char(TAB_LINE, view.cursor.column),
                0,
                "tab_size={tab_size}: the cursor is not at the tab's first column"
            );
        }
    }

    #[test]
    fn a_click_past_the_end_of_a_line_lands_on_its_last_column() {
        let layout = layout_with_title();
        let code_start = layout.editor_area.x + 4;
        for tab_size in [4usize, 8, 2] {
            let (mut editor, _p) = editor_with_a_tab_mixed_line("past-the-end", tab_size);
            let tabs = crate::display_width::TabStops::from_config(&editor.config);
            let total = tabs.col_at_char(TAB_LINE, TAB_LINE.chars().count());
            handle_left_click(
                &mut editor,
                code_start + (total + 5) as u16,
                layout.editor_area.y,
                &layout,
            );
            assert_eq!(
                editor.active_view().unwrap().cursor.column,
                TAB_LINE.chars().count(),
                "tab_size={tab_size}"
            );
        }
    }

    #[test]
    fn the_fixtures_agree_with_compute_layout() {
        // Both fixtures stand in for an 80x24 frame with a 20-column sidebar.
        // The rects the scrollbars are hit-tested against here have to be the
        // ones production computes, or every test below measures a layout that
        // never happens.
        let real = crate::layout::compute_layout(
            Rect::new(0, 0, 80, 24),
            true,
            20,
            termcode_theme::theme::PaneFocusStyle::TitleBar,
            false,
        );
        for fixture in [layout_with_title(), layout_with_border()] {
            assert_eq!(fixture.tab_bar, real.tab_bar);
            assert_eq!(fixture.editor_area, real.editor_area);
            assert_eq!(fixture.editor_scrollbar, real.editor_scrollbar);
            assert_eq!(fixture.editor_hscrollbar, real.editor_hscrollbar);
            assert_eq!(
                fixture.editor_scrollbar_corner(),
                real.editor_scrollbar_corner()
            );
        }
    }

    #[test]
    fn the_thumbs_bottom_and_the_wheels_bottom_are_the_same_line() {
        // The vertical track gave up a row to the horizontal bar. Had it kept
        // it, `ui::scrollbar::thumb`'s `max_top` (from the track's height) and
        // `View::scroll_down`'s (from `area_height`) would differ by one, and a
        // drag to the bottom of the track would stop one line short of where
        // the wheel gets to -- the classic "the drag will not quite reach the
        // last line" bug, arriving by the back door. Both numbers below come
        // from the layout, neither from a literal.
        let layout = layout_with_title();
        let track = layout.editor_scrollbar.unwrap();

        let (mut wheeled, _p) = editor_with_a_scrolled_document("wheel-bottom");
        for _ in 0..500 {
            handle_mouse(&mut wheeled, wheel(true, 60, 10), &layout);
        }
        let wheel_bottom = wheeled.active_view().unwrap().scroll.top_line;

        let (mut dragged, _q) = editor_with_a_scrolled_document("drag-bottom");
        handle_mouse(
            &mut dragged,
            press(track.x, track.y + track.height - 1),
            &layout,
        );
        handle_mouse(
            &mut dragged,
            drag(track.x, track.y + track.height + 50),
            &layout,
        );
        let drag_bottom = dragged.active_view().unwrap().scroll.top_line;

        assert_eq!(
            drag_bottom, wheel_bottom,
            "the thumb's bottom and the wheel's must be the same line"
        );
        let lines = wheeled.active_document().unwrap().buffer.line_count();
        assert_eq!(wheel_bottom, lines - track.height as usize);
    }

    /// An editor whose document is shorter than the viewport: nothing to scroll.
    fn editor_with_a_short_document(name: &str) -> (Editor, std::path::PathBuf) {
        let dir = std::env::temp_dir().join("termcode-scrollbar-tests");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("{name}.txt"));
        std::fs::write(&path, "line\n".repeat(5)).unwrap();

        let mut editor = make_editor();
        editor.open_file(&path).unwrap();
        size_view_from_layout(&mut editor, &layout_with_title());
        (editor, path)
    }

    #[test]
    fn grabbing_the_thumb_and_dragging_it_scrolls_the_document() {
        let (mut editor, _p) = editor_with_a_scrolled_document("scrollbar-grab");
        let layout = layout_with_title();
        let track = scrollbar_track();
        let lines = editor.active_document().unwrap().buffer.line_count();
        let (offset, _) = crate::ui::scrollbar::thumb(track.height, lines, 100).unwrap();

        // Pressing the thumb where it already is does not move the document.
        handle_mouse(&mut editor, press(track.x, track.y + offset), &layout);
        assert_eq!(
            editor.scrollbar_drag,
            Some(ScrollbarDrag::Vertical { grab: 0 }),
            "the grab point is remembered, and which bar it belongs to"
        );
        assert_eq!(editor.active_view().unwrap().scroll.top_line, 100);

        handle_mouse(&mut editor, drag(track.x, track.y + offset + 5), &layout);
        let expected = crate::ui::scrollbar::offset_for_thumb(track.height, lines, offset + 5);
        assert_eq!(editor.active_view().unwrap().scroll.top_line, expected);
        assert!(expected > 100, "dragging down scrolls down");

        // The drag is not confined to the column: the pointer routinely leaves
        // it, and the row is what the scroll follows. Dragging back to the row
        // the thumb started on lands on that thumb offset's own top line --
        // one thumb row covers many lines, so it is not necessarily line 100.
        handle_mouse(&mut editor, drag(track.x - 30, track.y + offset), &layout);
        let back = crate::ui::scrollbar::offset_for_thumb(track.height, lines, offset);
        assert_eq!(editor.active_view().unwrap().scroll.top_line, back);
        assert_eq!(
            crate::ui::scrollbar::thumb(track.height, lines, back)
                .unwrap()
                .0,
            offset,
            "the thumb is back where it was grabbed"
        );
    }

    #[test]
    fn a_press_off_the_thumb_centres_it_under_the_pointer() {
        let (mut editor, _p) = editor_with_a_scrolled_document("scrollbar-jump");
        let layout = layout_with_title();
        let track = scrollbar_track();
        let lines = editor.active_document().unwrap().buffer.line_count();

        handle_mouse(&mut editor, press(track.x, track.y + 15), &layout);

        let top = editor.active_view().unwrap().scroll.top_line;
        let (_, length) = crate::ui::scrollbar::thumb(track.height, lines, top).unwrap();
        assert_eq!(
            editor.scrollbar_drag,
            Some(ScrollbarDrag::Vertical { grab: length / 2 })
        );
        let expected = crate::ui::scrollbar::offset_for_thumb(track.height, lines, 15 - length / 2);
        assert_eq!(top, expected, "the thumb jumped to the pointer");

        // The drag carries on under the same rule the press established.
        handle_mouse(&mut editor, drag(track.x, track.y + 16), &layout);
        let expected = crate::ui::scrollbar::offset_for_thumb(track.height, lines, 16 - length / 2);
        assert_eq!(editor.active_view().unwrap().scroll.top_line, expected);
    }

    #[test]
    fn dragging_past_the_track_pins_to_the_first_line_and_the_last_screen() {
        let (mut editor, _p) = editor_with_a_scrolled_document("scrollbar-ends");
        let layout = layout_with_title();
        let track = scrollbar_track();
        let lines = editor.active_document().unwrap().buffer.line_count();
        let max_top = lines - track.height as usize;

        handle_mouse(&mut editor, press(track.x, track.y + 10), &layout);

        // Below the track: the last screen, exactly -- a drag that cannot quite
        // reach the end is the classic scrollbar bug.
        handle_mouse(
            &mut editor,
            drag(track.x, track.y + track.height + 40),
            &layout,
        );
        assert_eq!(editor.active_view().unwrap().scroll.top_line, max_top);

        // The wheel agrees that this is the bottom. `View::scroll_down` clamps
        // to `line_count - area_height` and the thumb to `line_count -
        // track_height`; the two are the same rows in production, so a document
        // dragged to the end must have nowhere left for the wheel to go.
        editor.active_view_mut().unwrap().scroll_down(3, lines);
        assert_eq!(
            editor.active_view().unwrap().scroll.top_line,
            max_top,
            "the thumb's bottom and the wheel's bottom must be the same line"
        );

        // Above it, including above the whole frame.
        handle_mouse(&mut editor, drag(track.x, 0), &layout);
        assert_eq!(editor.active_view().unwrap().scroll.top_line, 0);
    }

    #[test]
    fn a_press_with_nothing_to_scroll_is_swallowed() {
        let (mut editor, _p) = editor_with_a_short_document("scrollbar-short");
        let layout = layout_with_title();
        let track = scrollbar_track();
        editor.switch_mode(EditorMode::Normal);

        let action = handle_left_click(&mut editor, track.x, track.y + 8, &layout);
        assert!(matches!(action, MouseAction::None));
        assert_eq!(editor.scrollbar_drag, None, "there is no thumb to grab");
        assert_eq!(editor.active_view().unwrap().scroll.top_line, 0);
        // Swallowed, not passed on to the text behind the column.
        assert_eq!(editor.active_view().unwrap().cursor.line, 0);
        assert_eq!(editor.active_view().unwrap().cursor.column, 0);
    }

    #[test]
    fn the_scrollbar_never_moves_the_cursor_the_selection_or_the_mode() {
        let (mut editor, _p) = editor_with_a_scrolled_document("scrollbar-quiet");
        let layout = layout_with_title();
        let track = scrollbar_track();
        editor.switch_mode(EditorMode::Normal);
        {
            let view = editor.active_view_mut().unwrap();
            view.cursor.line = 7;
            view.cursor.column = 3;
        }
        let doc_id = editor.active_view().unwrap().doc_id;
        let selection = editor.documents.get(&doc_id).unwrap().selection.clone();

        handle_mouse(&mut editor, press(track.x, track.y + 15), &layout);
        handle_mouse(&mut editor, drag(track.x, track.y + 18), &layout);
        handle_mouse(&mut editor, release(track.x, track.y + 18), &layout);

        assert_eq!(
            editor.mode,
            EditorMode::Normal,
            "it is the wheel with a handle"
        );
        let view = editor.active_view().unwrap();
        assert_eq!((view.cursor.line, view.cursor.column), (7, 3));
        assert_eq!(
            editor.documents.get(&doc_id).unwrap().selection.primary(),
            selection.primary()
        );
        assert_eq!(editor.scrollbar_drag, None, "the release let go");
    }

    #[test]
    fn a_new_press_clears_a_scrollbar_drag_whose_release_was_lost() {
        let (mut editor, _p) = editor_with_a_scrolled_document("scrollbar-lost-up");
        let layout = layout_with_title();
        // As if the button had come up outside the terminal.
        editor.scrollbar_drag = Some(ScrollbarDrag::Vertical { grab: 0 });

        handle_mouse(&mut editor, press(40, 5), &layout);
        assert!(editor.scrollbar_drag.is_none());

        // The drag now selects text, as a drag from the editor area should.
        handle_mouse(&mut editor, drag(50, 6), &layout);
        assert_eq!(
            editor.active_view().unwrap().scroll.top_line,
            100,
            "not stuck to the pointer"
        );
    }

    fn hscrollbar_row() -> Rect {
        layout_with_title()
            .editor_hscrollbar
            .expect("an h-scrollbar")
    }

    /// The track inside the row, from the same function `render.rs` draws with.
    fn htrack(editor: &Editor) -> Rect {
        hscrollbar_track(editor, &hscrollbar_row()).expect("a track")
    }

    fn htotal(editor: &Editor) -> usize {
        hscrollbar_total(editor, &htrack(editor))
    }

    /// An editor whose first line is far wider than the code area, with short
    /// lines under it -- the horizontal counterpart of a document long enough
    /// to scroll. `area_width`/`area_height` come from the fixture layout, so
    /// the track's `code_width` and `ensure_h_scroll`'s describe one viewport.
    fn editor_with_a_long_line(name: &str) -> (Editor, std::path::PathBuf) {
        let dir = std::env::temp_dir().join("termcode-hscrollbar-tests");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("{name}.txt"));
        let text = format!("{}\n{}", "x".repeat(400), "short\n".repeat(29));
        std::fs::write(&path, &text).unwrap();

        let mut editor = make_editor();
        editor.open_file(&path).unwrap();
        size_view_from_layout(&mut editor, &layout_with_title());
        (editor, path)
    }

    /// A line far wider than `ui::scrollbar::SCAN_BUDGET`, so the scroll total
    /// is the budget rather than the line. That is the regime the horizontal
    /// drag used to break in: while the scan cap was measured from `left_col`,
    /// the total grew with every event, and the fixtures above -- 400 columns,
    /// comfortably inside any cap -- could not see it.
    fn editor_with_a_line_past_the_scan_budget(name: &str) -> (Editor, std::path::PathBuf) {
        let dir = std::env::temp_dir().join("termcode-hscrollbar-tests");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("{name}.txt"));
        let text = format!("{}\n{}", "x".repeat(100_000), "short\n".repeat(29));
        std::fs::write(&path, &text).unwrap();

        let mut editor = make_editor();
        editor.open_file(&path).unwrap();
        size_view_from_layout(&mut editor, &layout_with_title());
        (editor, path)
    }

    /// An editor whose *visible* lines are all short while `left_col` is parked
    /// far to the right of them -- reached by scrolling right along a long line
    /// and then scrolling it off the top of the screen.
    ///
    /// Nothing on this screen overflows the code area, so there is no thumb:
    /// the regime where the bar is empty and a press on it is the way back to
    /// the content.
    const PARKED_LEFT_COL: usize = 500;

    fn editor_parked_right_of_every_visible_line(name: &str) -> (Editor, std::path::PathBuf) {
        let dir = std::env::temp_dir().join("termcode-hscrollbar-tests");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("{name}.txt"));
        std::fs::write(&path, "shortish line\n".repeat(40)).unwrap();

        let mut editor = make_editor();
        editor.open_file(&path).unwrap();
        size_view_from_layout(&mut editor, &layout_with_title());
        editor.active_view_mut().unwrap().scroll.left_col = PARKED_LEFT_COL;
        (editor, path)
    }

    /// Hold the pointer at one column of the track and assert the view is
    /// placed there by the *first* drag event and stays put.
    ///
    /// A stationary pointer still produces a drag event per mouse report while
    /// the button is down, so "one event" is not something a real drag ever
    /// gets to rely on: a mapping that only converges is a mapping that creeps.
    fn assert_a_held_pointer_settles_at(editor: &mut Editor, col: u16) -> usize {
        let layout = layout_with_title();
        let track = htrack(editor);

        handle_mouse(editor, press(track.x, track.y), &layout);
        let x = track.x + col;
        handle_mouse(editor, drag(x, track.y), &layout);
        let settled = editor.active_view().unwrap().scroll.left_col;

        for event in 1..10 {
            handle_mouse(editor, drag(x, track.y), &layout);
            assert_eq!(
                editor.active_view().unwrap().scroll.left_col,
                settled,
                "column {col}, event {event}: the view crept instead of settling"
            );
        }
        settled
    }

    #[test]
    fn a_held_pointer_settles_on_the_first_drag_event_where_the_budget_bounds_the_total() {
        let (editor, _p) = editor_with_a_line_past_the_scan_budget("hscrollbar-budget-hold");
        let track = htrack(&editor);
        let total = htotal(&editor);
        assert_eq!(
            total,
            crate::ui::scrollbar::SCAN_BUDGET,
            "the fixture must be in the regime where the budget bounds the total"
        );
        drop(editor);

        // The far end of the track, and a column well short of it. The end
        // alone proves less than it looks: `offset == max_offset` maps to
        // itself whatever the total is, so it settles even under a mapping that
        // creeps everywhere else. A fresh editor per column, because a settled
        // drag is not a starting position for the next one.
        for col in [track.width - 1, track.width / 3, 7] {
            let (mut editor, _p) =
                editor_with_a_line_past_the_scan_budget("hscrollbar-budget-hold");
            let settled = assert_a_held_pointer_settles_at(&mut editor, col);
            if col == track.width - 1 {
                assert_eq!(
                    settled,
                    total - track.width as usize,
                    "the far end of the track is the last screen, reached at once"
                );
            } else {
                assert!(
                    settled > 0 && settled < total - track.width as usize,
                    "column {col} settled at an end instead of where it points"
                );
            }
        }
    }

    #[test]
    fn a_press_on_an_empty_track_brings_the_view_back_to_the_content() {
        // The state the floor used to paper over: a long line scrolled off the
        // top, `left_col` left behind it, and nothing on screen wide enough to
        // scroll -- so the code area is blank and the bar, honestly, has no
        // thumb. Something must lead back, and it is the empty track itself.
        let (mut editor, _p) = editor_parked_right_of_every_visible_line("hscrollbar-stranded");
        let layout = layout_with_title();
        let track = htrack(&editor);
        editor.switch_mode(EditorMode::Normal);
        assert_eq!(
            crate::ui::scrollbar::thumb(track.width, htotal(&editor), PARKED_LEFT_COL),
            None,
            "the fixture must be in the regime where the bar is empty"
        );

        for col in [0u16, 7, track.width / 2, track.width - 1] {
            editor.active_view_mut().unwrap().scroll.left_col = PARKED_LEFT_COL;
            let action = handle_left_click(&mut editor, track.x + col, track.y, &layout);

            assert!(matches!(action, MouseAction::None));
            assert_eq!(
                editor.active_view().unwrap().scroll.left_col,
                0,
                "column {col} left the view stranded"
            );
            // Swallowed all the same: it is still a scrollbar, so it does not
            // reach the text above the row or move the cursor.
            assert_eq!(
                editor.scrollbar_drag, None,
                "column {col} armed a drag with no thumb to hold"
            );
            let view = editor.active_view().unwrap();
            assert_eq!((view.cursor.line, view.cursor.column), (0, 0));
        }

        // And a drag afterwards drags nothing: there is no thumb, so the
        // gesture has nowhere to go and the view stays where the press put it.
        handle_mouse(&mut editor, drag(track.x + 40, track.y), &layout);
        assert_eq!(editor.active_view().unwrap().scroll.left_col, 0);
    }

    /// A drag that starts past the scan horizon: the line on screen is wider
    /// than `SCAN_BUDGET` and `left_col` sits out beyond what the bar can
    /// measure, having been carried there by the cursor rather than by the bar.
    /// The creep was at its worst here -- three events to cross from 73,636 to
    /// 24,517 -- and the first event must now place the view instead.
    #[test]
    fn a_drag_beginning_past_the_scan_horizon_settles_on_the_first_event_too() {
        let (mut editor, _p) = editor_with_a_line_past_the_scan_budget("hscrollbar-horizon-hold");
        let track = htrack(&editor);
        editor.active_view_mut().unwrap().scroll.left_col = 150_000;
        let total = htotal(&editor);
        assert!(
            editor.active_view().unwrap().scroll.left_col > total,
            "the fixture must start past what the bar measures"
        );
        // The thumb pins to the right end of the track rather than overflowing
        // it, which is what makes it grabbable from out here at all.
        let (offset, length) =
            crate::ui::scrollbar::thumb(track.width, total, 150_000).expect("a thumb");
        assert_eq!(offset + length, track.width);

        let settled = assert_a_held_pointer_settles_at(&mut editor, track.width / 2);
        assert!(
            settled > 0 && settled < total - track.width as usize,
            "the pointer landed mid-track, at once: {settled}"
        );
    }

    #[test]
    fn a_drag_across_a_line_past_the_budget_maps_the_pointer_to_a_position() {
        // Every column of the track means one place, and the same column means
        // the same place whichever direction it was arrived from. Under the old
        // `left_col`-relative cap the pointer set a *speed*: the same column
        // read differently depending on where the view already was.
        let (mut editor, _p) = editor_with_a_line_past_the_scan_budget("hscrollbar-budget-map");
        let layout = layout_with_title();
        let track = htrack(&editor);
        let total = htotal(&editor);

        handle_mouse(&mut editor, press(track.x, track.y), &layout);

        let mut seen = Vec::new();
        for col in [0u16, 7, 20, 40, track.width - 1] {
            handle_mouse(&mut editor, drag(track.x + col, track.y), &layout);
            seen.push((col, editor.active_view().unwrap().scroll.left_col));
        }
        // Walking back down the same columns must retrace the same positions.
        for &(col, left_col) in seen.iter().rev() {
            handle_mouse(&mut editor, drag(track.x + col, track.y), &layout);
            assert_eq!(
                editor.active_view().unwrap().scroll.left_col,
                left_col,
                "column {col} means one position, not a direction"
            );
        }
        assert!(
            seen.windows(2).all(|w| w[0].1 < w[1].1),
            "rightwards on the track is rightwards in the document: {seen:?}"
        );
        // And measured again after all of it, the total is the number every
        // one of those events was mapped through: the drag wrote `left_col`
        // eight times and the scale it was written on never moved.
        assert_eq!(htotal(&editor), total, "the total moved under the drag");
        assert_eq!(
            editor.scrollbar_drag,
            Some(ScrollbarDrag::Horizontal { grab: 0 })
        );
    }

    /// The columns of the track carrying a thumb, drawn exactly as `render.rs`
    /// draws them, into a frame-sized buffer.
    fn drawn_thumb(editor: &Editor, layout: &AppLayout, track: ratatui::layout::Rect) -> Vec<u16> {
        let mut buf = ratatui::buffer::Buffer::empty(layout.frame);
        let total = crate::ui::scrollbar::hscroll_total(editor, track.width as usize);
        let left_col = editor.active_view().unwrap().scroll.left_col;
        ratatui::widgets::Widget::render(
            crate::ui::scrollbar::HScrollbarWidget::new(&editor.theme, total, left_col),
            track,
            &mut buf,
        );
        (track.x..track.x + track.width)
            .filter(|&x| buf[(x, track.y)].symbol() != " ")
            .collect()
    }

    #[test]
    fn the_thumb_drawn_during_a_drag_is_the_thumb_under_the_pointer() {
        // `render.rs` draws through `scrollbar::hscroll_total`, the same
        // function `hscroll_to_thumb` maps the pointer through. Nothing is
        // remembered between them: what makes them agree is that the number
        // does not depend on the `left_col` the drag has just written.
        let (mut editor, _p) = editor_with_a_line_past_the_scan_budget("hscrollbar-draw");
        let layout = layout_with_title();
        let track = htrack(&editor);
        let col = track.width / 3;

        handle_mouse(&mut editor, press(track.x, track.y), &layout);
        let Some(ScrollbarDrag::Horizontal { grab }) = editor.scrollbar_drag else {
            panic!("the press did not arm a horizontal drag");
        };
        handle_mouse(&mut editor, drag(track.x + col, track.y), &layout);

        let drawn = drawn_thumb(&editor, &layout, track);
        assert!(!drawn.is_empty(), "no thumb was drawn");
        assert_eq!(
            drawn[0],
            track.x + col - grab,
            "the thumb was drawn at {drawn:?}, not under the pointer at {}",
            track.x + col
        );
        assert!(
            drawn.contains(&(track.x + col)),
            "the pointer is not on the thumb it is holding: {drawn:?}"
        );
    }

    #[test]
    fn letting_go_of_the_thumb_does_not_move_it() {
        // The release is not an event the bar acts on -- it only ends the drag
        // -- so the frame after it must draw the identical thumb. It did not
        // while the total was latched: the drag was drawn through the latched
        // number and the next frame through a fresh one, and where the two
        // differed the thumb jumped as the button came up. Now there is one
        // number, so there is nothing to jump between.
        let layout = layout_with_title();
        for col in [0u16, 7, 20, 40] {
            let (mut editor, _p) = editor_with_a_line_past_the_scan_budget("hscrollbar-release");
            let track = htrack(&editor);

            handle_mouse(&mut editor, press(track.x, track.y), &layout);
            handle_mouse(&mut editor, drag(track.x + col, track.y), &layout);
            let during = drawn_thumb(&editor, &layout, track);
            let held = editor.active_view().unwrap().scroll.left_col;

            handle_mouse(&mut editor, release(track.x + col, track.y), &layout);
            assert_eq!(editor.scrollbar_drag, None, "the release let go");
            assert_eq!(
                editor.active_view().unwrap().scroll.left_col,
                held,
                "column {col}: the release moved the view"
            );
            assert_eq!(
                drawn_thumb(&editor, &layout, track),
                during,
                "column {col}: the thumb jumped when the button came up"
            );
        }
    }

    #[test]
    fn a_drag_whose_release_was_lost_leaves_no_thumb_behind() {
        // A latched total outlived the gesture that latched it: with the `Up`
        // lost outside the terminal it stayed in `scrollbar_drag`, and every
        // later frame was drawn through it. Scrolling the long line off the
        // screen, or switching to a document that has none, then drew a thumb
        // for content that is not there and cannot be scrolled to.
        let (mut editor, _p) = editor_with_a_long_line("hscrollbar-ghost");
        let layout = layout_with_title();
        let track = htrack(&editor);

        handle_mouse(&mut editor, press(track.x, track.y), &layout);
        handle_mouse(&mut editor, drag(track.x + 20, track.y), &layout);
        assert!(
            !drawn_thumb(&editor, &layout, track).is_empty(),
            "the long line is on screen, so there is a thumb to lose"
        );
        // No `Up`: as if the button had come up outside the terminal.
        assert!(editor.scrollbar_drag.is_some(), "the drag is still armed");

        // The wheel takes the long line off the top of the screen.
        for _ in 0..4 {
            handle_mouse(&mut editor, wheel(true, 60, 10), &layout);
        }
        assert!(
            editor.active_view().unwrap().scroll.top_line > 0,
            "the wheel must have moved the screen"
        );
        assert!(
            drawn_thumb(&editor, &layout, track).is_empty(),
            "a thumb was drawn for a screen with nothing on it to scroll"
        );

        // And a tab switch, the other way of changing what is on screen.
        let dir = std::env::temp_dir().join("termcode-hscrollbar-tests");
        let other = dir.join("hscrollbar-ghost-short.txt");
        std::fs::write(
            &other,
            "short
"
            .repeat(10),
        )
        .unwrap();
        editor.open_file(&other).unwrap();
        size_view_from_layout(&mut editor, &layout);
        assert!(
            drawn_thumb(&editor, &layout, htrack(&editor)).is_empty(),
            "the drag it was dropped in followed it into another tab"
        );
    }

    #[test]
    fn grabbing_the_horizontal_thumb_and_dragging_it_scrolls_the_view() {
        let (mut editor, _p) = editor_with_a_long_line("hscrollbar-grab");
        let layout = layout_with_title();
        let track = htrack(&editor);
        editor.active_view_mut().unwrap().scroll.left_col = 100;
        let total = htotal(&editor);
        let (offset, _) = crate::ui::scrollbar::thumb(track.width, total, 100).unwrap();

        // Pressing the thumb where it already is does not move the view.
        handle_mouse(&mut editor, press(track.x + offset, track.y), &layout);
        assert_eq!(
            editor.scrollbar_drag,
            Some(ScrollbarDrag::Horizontal { grab: 0 }),
            "the grab point is remembered, and which bar it belongs to"
        );
        assert_eq!(editor.active_view().unwrap().scroll.left_col, 100);

        // Right, then back left again.
        handle_mouse(&mut editor, drag(track.x + offset + 5, track.y), &layout);
        let expected = crate::ui::scrollbar::offset_for_thumb(track.width, total, offset + 5);
        assert_eq!(editor.active_view().unwrap().scroll.left_col, expected);
        assert!(expected > 100, "dragging right scrolls right");

        handle_mouse(&mut editor, drag(track.x + offset, track.y), &layout);
        assert_eq!(
            editor.active_view().unwrap().scroll.left_col,
            crate::ui::scrollbar::offset_for_thumb(track.width, total, offset)
        );
    }

    #[test]
    fn a_press_off_the_horizontal_thumb_centres_it_under_the_pointer() {
        let (mut editor, _p) = editor_with_a_long_line("hscrollbar-jump");
        let layout = layout_with_title();
        let track = htrack(&editor);
        let total = htotal(&editor);

        handle_mouse(&mut editor, press(track.x + 30, track.y), &layout);

        let (_, length) = crate::ui::scrollbar::thumb(
            track.width,
            total,
            editor.active_view().unwrap().scroll.left_col,
        )
        .unwrap();
        assert_eq!(
            editor.scrollbar_drag,
            Some(ScrollbarDrag::Horizontal { grab: length / 2 })
        );
        let expected = crate::ui::scrollbar::offset_for_thumb(track.width, total, 30 - length / 2);
        assert_eq!(
            editor.active_view().unwrap().scroll.left_col,
            expected,
            "the thumb jumped to the pointer"
        );

        // The drag carries on under the same rule the press established.
        handle_mouse(&mut editor, drag(track.x + 31, track.y), &layout);
        let expected = crate::ui::scrollbar::offset_for_thumb(track.width, total, 31 - length / 2);
        assert_eq!(editor.active_view().unwrap().scroll.left_col, expected);
    }

    #[test]
    fn dragging_past_either_end_of_the_row_pins_to_the_first_column_and_the_last_screen() {
        let (mut editor, _p) = editor_with_a_long_line("hscrollbar-ends");
        let layout = layout_with_title();
        let row = hscrollbar_row();
        let track = htrack(&editor);
        editor.active_view_mut().unwrap().scroll.left_col = 100;

        // The end to reach, read *before* the drag. Reading it afterwards asked
        // the question of the state the drag had just produced, so a drag that
        // dragged the goalpost with it agreed with itself and the assertion
        // passed on a view that had gone nowhere near the end.
        let max_left = htotal(&editor) - track.width as usize;

        handle_mouse(&mut editor, press(track.x + 10, track.y), &layout);

        // Far right of the track -- and past the row entirely. Reaching the end
        // matters: a thumb that will not quite get there is the classic bug.
        handle_mouse(&mut editor, drag(row.x + row.width + 40, track.y), &layout);
        assert_eq!(editor.active_view().unwrap().scroll.left_col, max_left);

        // Far left, including left of the row's own origin.
        handle_mouse(&mut editor, drag(0, track.y), &layout);
        assert_eq!(editor.active_view().unwrap().scroll.left_col, 0);
    }

    #[test]
    fn a_press_on_the_gutter_part_of_the_row_is_swallowed() {
        let (mut editor, _p) = editor_with_a_long_line("hscrollbar-gutter");
        let layout = layout_with_title();
        let row = hscrollbar_row();
        let track = htrack(&editor);
        editor.switch_mode(EditorMode::Normal);
        editor.active_view_mut().unwrap().scroll.left_col = 100;

        for x in row.x..track.x {
            let action = handle_left_click(&mut editor, x, row.y, &layout);
            assert!(matches!(action, MouseAction::None));
            assert_eq!(editor.scrollbar_drag, None, "column {x} has no track");
            assert_eq!(editor.active_view().unwrap().scroll.left_col, 100);
            assert_eq!(editor.active_view().unwrap().cursor.line, 0);
            assert_eq!(editor.active_view().unwrap().cursor.column, 0);
        }
    }

    #[test]
    fn a_press_with_nothing_to_scroll_horizontally_is_swallowed() {
        // The common case of an empty track: a document that fits, already at
        // column 0. The return-to-content rule has nothing to do here, and must
        // not be visible as a jump.
        let (mut editor, _p) = editor_with_a_short_document("hscrollbar-short");
        let layout = layout_with_title();
        let track = htrack(&editor);
        editor.switch_mode(EditorMode::Normal);

        let action = handle_left_click(&mut editor, track.x + 8, track.y, &layout);
        assert!(matches!(action, MouseAction::None));
        assert_eq!(editor.scrollbar_drag, None, "there is no thumb to grab");
        assert_eq!(editor.active_view().unwrap().scroll.left_col, 0);
        // Swallowed, not passed on to the text above the row.
        assert_eq!(editor.active_view().unwrap().cursor.line, 0);
        assert_eq!(editor.active_view().unwrap().cursor.column, 0);
    }

    #[test]
    fn a_popup_swallows_a_horizontal_press_and_drag_the_same_way_it_swallows_the_wheel() {
        let layout = layout_with_title();
        for mode in [
            EditorMode::Search,
            EditorMode::FuzzyFinder,
            EditorMode::CommandPalette,
            EditorMode::Settings,
        ] {
            let (mut editor, _p) = editor_with_a_long_line("popup-hscrollbar");
            let track = htrack(&editor);
            editor.mode = mode;
            editor.active_view_mut().unwrap().scroll.left_col = 100;

            handle_mouse(&mut editor, press(track.x + 30, track.y), &layout);
            assert_eq!(
                editor.scrollbar_drag, None,
                "{mode:?} armed a drag behind the popup"
            );
            // As above: the press was refused, so this exercises the `None`
            // arm. The mid-drag case has its own test.
            handle_mouse(&mut editor, drag(track.x + 40, track.y), &layout);

            assert_eq!(
                editor.active_view().unwrap().scroll.left_col,
                100,
                "{mode:?} let the press through to the buffer"
            );
            // Swallowed, not dismissed: the scrollbar never changes the mode.
            assert_eq!(editor.mode, mode);
        }
    }

    /// The horizontal half of `a_popup_that_opens_mid_drag_freezes_the_held_vertical_thumb`
    /// -- the two arms of `handle_drag` carry the guard separately, so one of
    /// them passing says nothing about the other.
    #[test]
    fn a_popup_that_opens_mid_drag_freezes_the_held_horizontal_thumb() {
        let layout = layout_with_title();
        let row = hscrollbar_row();

        let (mut editor, _p) = editor_with_a_long_line("mid-drag-horizontal-control");
        let track = htrack(&editor);
        editor.switch_mode(EditorMode::Normal);
        editor.active_view_mut().unwrap().scroll.left_col = 100;
        handle_mouse(&mut editor, press(track.x + 10, track.y), &layout);
        let armed = editor.active_view().unwrap().scroll.left_col;
        handle_mouse(&mut editor, drag(row.x + row.width + 40, track.y), &layout);
        let moved = editor.active_view().unwrap().scroll.left_col;
        assert_ne!(moved, armed, "the control drag moved nothing to guard");

        for mode in [
            EditorMode::Search,
            EditorMode::FuzzyFinder,
            EditorMode::CommandPalette,
            EditorMode::Settings,
        ] {
            let (mut editor, _p) = editor_with_a_long_line("mid-drag-horizontal");
            editor.switch_mode(EditorMode::Normal);
            editor.active_view_mut().unwrap().scroll.left_col = 100;

            handle_mouse(&mut editor, press(track.x + 10, track.y), &layout);
            assert!(
                editor.scrollbar_drag.is_some(),
                "the press armed no drag to interrupt"
            );
            let held = editor.active_view().unwrap().scroll.left_col;

            editor.mode = mode;
            handle_mouse(&mut editor, drag(row.x + row.width + 40, track.y), &layout);

            assert_eq!(
                editor.active_view().unwrap().scroll.left_col,
                held,
                "{mode:?} let the held thumb go on dragging the buffer behind it"
            );
            assert_eq!(editor.mode, mode);
        }
    }

    #[test]
    fn the_horizontal_scrollbar_never_moves_the_cursor_the_selection_or_the_mode() {
        let (mut editor, _p) = editor_with_a_long_line("hscrollbar-quiet");
        let layout = layout_with_title();
        let track = htrack(&editor);
        editor.switch_mode(EditorMode::Normal);
        {
            let view = editor.active_view_mut().unwrap();
            view.cursor.line = 7;
            view.cursor.column = 3;
        }
        let doc_id = editor.active_view().unwrap().doc_id;
        let selection = editor.documents.get(&doc_id).unwrap().selection.clone();

        handle_mouse(&mut editor, press(track.x + 20, track.y), &layout);
        handle_mouse(&mut editor, drag(track.x + 25, track.y), &layout);
        handle_mouse(&mut editor, release(track.x + 25, track.y), &layout);

        assert!(
            editor.active_view().unwrap().scroll.left_col > 0,
            "it did scroll -- otherwise this asserts nothing"
        );
        assert_eq!(
            editor.mode,
            EditorMode::Normal,
            "it is the wheel with a handle"
        );
        let view = editor.active_view().unwrap();
        assert_eq!((view.cursor.line, view.cursor.column), (7, 3));
        assert_eq!(
            editor.documents.get(&doc_id).unwrap().selection.primary(),
            selection.primary()
        );
        assert_eq!(editor.scrollbar_drag, None, "the release let go");
    }

    #[test]
    fn a_new_press_clears_a_horizontal_drag_whose_release_was_lost() {
        let (mut editor, _p) = editor_with_a_long_line("hscrollbar-lost-up");
        let layout = layout_with_title();
        // As if the button had come up outside the terminal.
        editor.scrollbar_drag = Some(ScrollbarDrag::Horizontal { grab: 0 });
        editor.active_view_mut().unwrap().scroll.left_col = 100;

        handle_mouse(&mut editor, press(40, 5), &layout);
        assert!(editor.scrollbar_drag.is_none());
        // The click placed the cursor, and `ensure_h_scroll` brought the view
        // to it -- that is the editor's own horizontal scroll, not the bar's.
        let after_press = editor.active_view().unwrap().scroll.left_col;

        // The drag now selects text, as a drag from the editor area should.
        handle_mouse(&mut editor, drag(50, 6), &layout);
        assert_eq!(
            editor.active_view().unwrap().scroll.left_col,
            after_press,
            "not stuck to the pointer"
        );
        let view = editor.active_view().unwrap();
        assert_eq!(view.cursor.line, 4, "the drag selected text instead");
    }
}
