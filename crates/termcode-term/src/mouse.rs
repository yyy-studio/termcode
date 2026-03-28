use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

use termcode_core::selection::Selection;
use termcode_view::editor::{Editor, EditorMode};

use crate::command::sync_selection_from_cursor;
use crate::layout::AppLayout;

/// Result of mouse handling that requires App-level action.
pub enum MouseAction {
    None,
    OpenExplorerItem(usize),
    SwitchTab(usize),
}

/// Handle a mouse event, dispatching based on which layout region was clicked.
pub fn handle_mouse(editor: &mut Editor, event: MouseEvent, layout: &AppLayout) -> MouseAction {
    match event.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            handle_left_click(editor, event.column, event.row, layout)
        }
        MouseEventKind::ScrollUp => {
            handle_scroll_up(editor, event.column, event.row, layout);
            MouseAction::None
        }
        MouseEventKind::ScrollDown => {
            handle_scroll_down(editor, event.column, event.row, layout);
            MouseAction::None
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            handle_drag(editor, event.column, event.row, layout);
            MouseAction::None
        }
        _ => MouseAction::None,
    }
}

fn handle_left_click(editor: &mut Editor, x: u16, y: u16, layout: &AppLayout) -> MouseAction {
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

    if rect_contains(&layout.editor_area, x, y) {
        handle_editor_click(editor, x, y, &layout.editor_area);
    }

    MouseAction::None
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
    let target_col = editor
        .active_document()
        .map(|d| {
            let line_text: String = d.buffer.line(target_line).chars().collect();
            let line_text = line_text.trim_end_matches(&['\n', '\r'][..]);
            crate::display_width::display_col_to_char_index(line_text, display_col)
        })
        .unwrap_or(0);

    if editor.mode != EditorMode::Insert && editor.mode != EditorMode::Normal {
        editor.switch_mode(EditorMode::Normal);
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
    _x: u16,
    y: u16,
    sidebar: &ratatui::layout::Rect,
) -> MouseAction {
    let row_offset = (y - sidebar.y) as usize;
    let target_index = editor.file_explorer.scroll_offset + row_offset;

    if target_index >= editor.file_explorer.tree.len() {
        return MouseAction::None;
    }

    let already_selected = editor.file_explorer.selected == target_index;
    editor.file_explorer.selected = target_index;
    editor.switch_mode(EditorMode::FileExplorer);

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
        let label_width = crate::display_width::str_display_width(&tab.label);
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

    let target_col = editor
        .active_document()
        .map(|d| {
            if target_line < d.buffer.line_count() {
                let line_text: String = d.buffer.line(target_line).chars().collect();
                let line_text = line_text.trim_end_matches(&['\n', '\r'][..]);
                crate::display_width::display_col_to_char_index(line_text, display_col)
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
            termcode_view::document::DocumentId(0),
        );
        let positions = tab_positions(&tabs);
        assert_eq!(positions.len(), 1);
        // " main.rs " = 1 + 7 + 1 = 9
        assert_eq!(positions[0], (0, 9));
    }

    #[test]
    fn tab_positions_multiple() {
        let mut tabs = termcode_view::tab::TabManager::new();
        tabs.add("a.rs".to_string(), termcode_view::document::DocumentId(0));
        tabs.add("b.rs".to_string(), termcode_view::document::DocumentId(1));
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
            top_bar: Rect::new(0, 0, 80, 1),
            sidebar: Some(Rect::new(0, 2, 20, 21)),
            sidebar_title: Some(Rect::new(0, 1, 20, 1)),
            sidebar_border: None,
            tab_bar: Rect::new(20, 1, 60, 1),
            editor_area: Rect::new(20, 2, 60, 21),
            status_bar: Rect::new(0, 23, 80, 1),
        }
    }

    fn layout_with_border() -> AppLayout {
        AppLayout {
            top_bar: Rect::new(0, 0, 80, 1),
            sidebar: Some(Rect::new(0, 1, 19, 22)),
            sidebar_title: None,
            sidebar_border: Some(Rect::new(19, 1, 1, 22)),
            tab_bar: Rect::new(20, 1, 60, 1),
            editor_area: Rect::new(20, 2, 60, 21),
            status_bar: Rect::new(0, 23, 80, 1),
        }
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
}
