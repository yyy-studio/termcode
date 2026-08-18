use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

use termcode_core::selection::Selection;
use termcode_view::editor::{Editor, EditorMode};

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
}

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
            sidebar_panel: None,
            editor_panel: None,
            tab_bar: Rect::new(20, 1, 60, 1),
            editor_area: Rect::new(20, 2, 60, 21),
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
            sidebar_panel: None,
            editor_panel: None,
            tab_bar: Rect::new(20, 1, 60, 1),
            editor_area: Rect::new(20, 2, 60, 21),
            status_bar: Rect::new(0, 23, 80, 1),
        }
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
}
