use std::collections::HashMap;
use std::sync::Mutex;

use ratatui::Frame;
use ratatui::style::Style as RatStyle;
use ratatui::widgets::{Block, Borders};

use ratatui_image::protocol::StatefulProtocol;
use termcode_view::editor::{Editor, EditorMode};
use termcode_view::image::{ImageId, TabContent};

use termcode_theme::theme::PaneFocusStyle;

use crate::layout::{self, AppLayout};
use crate::ui::command_palette::CommandPaletteWidget;
use crate::ui::completion::CompletionWidget;
use crate::ui::confirm_dialog::ConfirmDialogWidget;
use crate::ui::editor_view::EditorViewWidget;
use crate::ui::explorer_toolbar::ExplorerToolbarWidget;
use crate::ui::file_explorer::FileExplorerWidget;
use crate::ui::fuzzy_finder::FuzzyFinderWidget;
use crate::ui::help_popup::HelpPopupWidget;
use crate::ui::hover::HoverWidget;
use crate::ui::image_view::{ImagePlaceholderWidget, ImageViewWidget};
use crate::ui::pane_focus::{PaneAccentLineWidget, PaneBorderWidget, PaneTitleWidget};
use crate::ui::scrollbar::{self, HScrollbarWidget, ScrollbarWidget};
use crate::ui::search::SearchOverlayWidget;
use crate::ui::settings::SettingsWidget;
use crate::ui::status_bar::StatusBarWidget;
use crate::ui::tab_bar::TabBarWidget;
use crate::ui::top_bar::TopBarWidget;

pub fn render(
    frame: &mut Frame,
    editor: &Editor,
    image_cache: &HashMap<ImageId, Mutex<StatefulProtocol>>,
    input_mapper: &crate::input::InputMapper,
) {
    let area = frame.area();
    let app_layout = layout::compute_layout(
        area,
        editor.file_explorer.visible,
        editor.file_explorer.width,
        editor.theme.ui.pane_focus_style,
        editor.theme.ui.panel_borders,
    );

    // Render panel borders
    let border_style = RatStyle::default().fg(editor.theme.ui.border.to_ratatui());
    if let Some(panel_rect) = app_layout.sidebar_panel {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style);
        frame.render_widget(block, panel_rect);
    }
    if let Some(panel_rect) = app_layout.editor_panel {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style);
        frame.render_widget(block, panel_rect);
    }

    let top_bar_widget = TopBarWidget::new(&editor.theme);
    frame.render_widget(top_bar_widget, app_layout.top_bar);

    let is_sidebar_active = editor.mode == EditorMode::FileExplorer;
    let is_editor_active = !is_sidebar_active;

    if let Some(sidebar_area) = app_layout.sidebar {
        let explorer_widget = FileExplorerWidget::new(
            &editor.file_explorer,
            &editor.theme,
            is_sidebar_active,
            editor.file_tree_style,
        );
        frame.render_widget(explorer_widget, sidebar_area);
    }

    if let Some(toolbar_area) = app_layout.sidebar_toolbar {
        let toolbar = ExplorerToolbarWidget::new(
            &editor.file_explorer,
            &editor.theme,
            is_sidebar_active,
            editor.file_tree_style.show_file_type_emoji,
        );
        frame.render_widget(toolbar, toolbar_area);
    }

    if let Some(title_area) = app_layout.sidebar_title {
        match editor.theme.ui.pane_focus_style {
            PaneFocusStyle::TitleBar => {
                let w = PaneTitleWidget::new(&editor.theme, is_sidebar_active);
                frame.render_widget(w, title_area);
            }
            PaneFocusStyle::AccentLine => {
                let w = PaneAccentLineWidget::new(&editor.theme);
                frame.render_widget(w, title_area);
            }
            _ => {}
        }
    }
    if let Some(border_area) = app_layout.sidebar_border {
        let w = PaneBorderWidget::new(&editor.theme);
        frame.render_widget(w, border_area);
    }

    let tab_bar_widget = TabBarWidget::new(&editor.tabs, &editor.theme);
    frame.render_widget(tab_bar_widget, app_layout.tab_bar);

    let active_tab_content = editor.tabs.active_tab().map(|t| &t.content);
    let is_image_tab = matches!(active_tab_content, Some(TabContent::Image(_)));

    if is_image_tab {
        if let Some(TabContent::Image(image_id)) = active_tab_content {
            if let Some(mutex_proto) = image_cache.get(image_id) {
                if let Ok(mut protocol) = mutex_proto.lock() {
                    let image_widget = ImageViewWidget::new(&editor.theme);
                    image_widget.render_stateful(
                        app_layout.editor_area,
                        frame.buffer_mut(),
                        &mut protocol,
                    );
                }
            } else {
                let placeholder = ImagePlaceholderWidget::new("Image not available", &editor.theme);
                frame.render_widget(placeholder, app_layout.editor_area);
            }
        }
    } else if let (Some(view), Some(doc)) = (editor.active_view(), editor.active_document()) {
        let search = if editor.mode == EditorMode::Search {
            Some(&editor.search)
        } else {
            None
        };
        let editor_widget = EditorViewWidget::new(
            doc,
            view,
            &editor.theme,
            editor.mode,
            search,
            &editor.config,
            is_editor_active,
        );
        frame.render_widget(editor_widget, app_layout.editor_area);
    }

    // The column and the row are reserved whatever the tab holds, so the
    // branches with no thumb to draw -- an image tab, or no view at all --
    // still have to paint them. Not for staleness: ratatui resets the back
    // buffer before every draw, so nothing from the last frame survives. It is
    // that `Cell::reset` leaves `bg: Color::Reset`, which the backend emits as
    // the *terminal's* default background -- an unpainted column would show as
    // a vertical stripe beside the editor's own background.
    if let Some(scrollbar_area) = app_layout.editor_scrollbar {
        match (is_image_tab, editor.active_view(), editor.active_document()) {
            (false, Some(view), Some(doc)) => {
                let scrollbar = ScrollbarWidget::new(
                    &editor.theme,
                    doc.buffer.line_count(),
                    view.scroll.top_line,
                );
                frame.render_widget(scrollbar, scrollbar_area);
            }
            _ => scrollbar::blank(&editor.theme, scrollbar_area, frame.buffer_mut()),
        }
    }

    if let Some(row) = app_layout.editor_hscrollbar {
        // The whole row first, so the gutter columns -- which have no track,
        // because the gutter does not scroll -- get the editor's background
        // whether or not there is a thumb to draw beside them.
        scrollbar::blank(&editor.theme, row, frame.buffer_mut());

        if let (false, Some(view), Some(doc)) =
            (is_image_tab, editor.active_view(), editor.active_document())
        {
            let gutter_width = crate::ui::editor_view::line_number_width_styled(
                doc.buffer.line_count(),
                editor.config.line_numbers,
            );
            if let Some(track) = scrollbar::h_track(row, gutter_width) {
                // `hscroll_total` and not `content_width` spelled out again:
                // `mouse.rs` maps the pointer through the very same function,
                // so the thumb drawn is the thumb being held. The number does
                // not depend on `left_col`, which is the only thing a drag
                // writes, so this frame agrees with the one before it and with
                // the one after the button comes up.
                let total = scrollbar::hscroll_total(editor, track.width as usize);
                let hscrollbar = HScrollbarWidget::new(&editor.theme, total, view.scroll.left_col);
                frame.render_widget(hscrollbar, track);
            }
        }
    }

    // The one cell where the row meets the column belongs to neither track --
    // and for the same reason as above, it has to be painted rather than left
    // alone, or it shows as a notch of the terminal's own background in the
    // corner of the editor.
    if let Some(corner) = app_layout.editor_scrollbar_corner() {
        scrollbar::blank(&editor.theme, corner, frame.buffer_mut());
    }

    match editor.mode {
        EditorMode::Search => {
            let search_widget = SearchOverlayWidget::new(&editor.search, &editor.theme);
            frame.render_widget(search_widget, app_layout.editor_area);
        }
        EditorMode::FuzzyFinder => {
            let finder_widget = FuzzyFinderWidget::new(&editor.fuzzy_finder, &editor.theme);
            frame.render_widget(finder_widget, app_layout.editor_area);
        }
        EditorMode::CommandPalette => {
            let palette_widget = CommandPaletteWidget::new(&editor.command_palette, &editor.theme);
            frame.render_widget(palette_widget, app_layout.editor_area);
        }
        EditorMode::Settings => {
            let settings_widget = SettingsWidget::new(&editor.settings, &editor.theme);
            frame.render_widget(settings_widget, app_layout.frame);
        }
        _ => {}
    }

    if !is_image_tab && editor.completion.visible {
        if let Some((cursor_x, cursor_y)) = cursor_screen_position(editor, &app_layout) {
            let completion_widget = CompletionWidget::new(
                &editor.completion,
                &editor.theme,
                cursor_x,
                cursor_y,
                app_layout.editor_area,
            );
            frame.render_widget(completion_widget, frame.area());
        }
    }

    if !is_image_tab && editor.hover.visible {
        if let Some((cursor_x, cursor_y)) = cursor_screen_position(editor, &app_layout) {
            let hover_widget = HoverWidget::new(
                &editor.hover,
                &editor.theme,
                cursor_x,
                cursor_y,
                app_layout.editor_area,
            );
            frame.render_widget(hover_widget, frame.area());
        }
    }

    // Read the pending chord straight from the mapper: mirroring it into
    // `Editor` would go stale on every key path that bypasses the mapper.
    let pending_keys = input_mapper.pending_display();
    let status_widget = StatusBarWidget::new(
        editor.active_document(),
        editor.active_view(),
        &editor.theme,
        editor.status_message.as_deref(),
        editor.mode,
        editor.active_image(),
        &pending_keys,
    );
    frame.render_widget(status_widget, app_layout.status_bar);

    // Help popup overlay (rendered last, on top of everything)
    if editor.help_visible {
        let help_widget = HelpPopupWidget::new(&editor.theme, input_mapper, editor.mode);
        frame.render_widget(help_widget, area);
    }

    // Confirm dialog overlay (highest z-order)
    if let Some(ref dialog) = editor.confirm_dialog {
        let confirm_widget = ConfirmDialogWidget::new(dialog, &editor.theme);
        frame.render_widget(confirm_widget, area);
    }
}

/// The cell the cursor is drawn in, which is what the completion and hover
/// popups anchor themselves to. Despite the name it does **not** place the
/// terminal's own cursor: the block the user sees is the REVERSED cell the
/// widget paints. `None` means the cursor is outside the code area on either
/// axis, and a popup with no anchor is not drawn at all.
fn cursor_screen_position(editor: &Editor, app_layout: &AppLayout) -> Option<(u16, u16)> {
    let view = editor.active_view()?;
    let doc = editor.active_document()?;
    let gutter_width = crate::ui::editor_view::line_number_width_styled(
        doc.buffer.line_count(),
        editor.config.line_numbers,
    );

    let line_text: String = doc.buffer.line(view.cursor.line).chars().collect();
    let line_text = line_text.trim_end_matches('\n').trim_end_matches('\r');
    let display_col = crate::display_width::TabStops::from_config(&editor.config)
        .col_at_char(line_text, view.cursor.column);

    // Out of the code area is *no* cell, not the nearest one. Clamping used to
    // pin a scrolled-away cursor to the edge column, which the widget does not
    // reverse -- the anchor named a cell the user sees nothing in. The bounds
    // are the widget's own (`left_col..left_col + code_width`), so wherever
    // this answers a cell, that is the cell the widget reversed, and where the
    // widget draws no cursor this answers nothing. A `code_width` of 0 falls
    // out of the same test.
    let code_width = app_layout
        .editor_area
        .width
        .saturating_sub(gutter_width + 1) as usize;
    let col_offset = display_col.checked_sub(view.scroll.left_col)?;
    if col_offset >= code_width {
        return None;
    }
    let col_offset = col_offset as u16;

    // The same rule vertically, and for the same reason: a cursor line above
    // `top_line` used to saturate to 0 and name the first visible row, which
    // the widget does not reverse either. The wheel reaches this one without
    // moving the cursor at all.
    let row = view.cursor.line.checked_sub(view.scroll.top_line)?;
    if row >= app_layout.editor_area.height as usize {
        return None;
    }

    let cursor_x = app_layout.editor_area.x + gutter_width + 1 + col_offset;
    let cursor_y = app_layout.editor_area.y + row as u16;
    Some((cursor_x, cursor_y))
}

#[cfg(test)]
mod tests {
    use super::*;

    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use ratatui::style::Modifier;
    use ratatui::widgets::Widget;

    use termcode_core::config_types::EditorConfig;
    use termcode_syntax::language::LanguageRegistry;
    use termcode_theme::theme::Theme;

    /// A line mixing every shape a column can take: a leading tab, consecutive
    /// tabs, a CJK character, a combining mark and a trailing tab.
    const LINE: &str = "\tab\t\t한글\te\u{0301}x\ty";

    /// The frame the fixtures stand in for, built by `compute_layout` rather
    /// than written out: a literal `AppLayout` that drifts from what production
    /// computes tests a screen that never happens.
    fn layout() -> AppLayout {
        layout::compute_layout(
            Rect::new(0, 0, 100, 24),
            true,
            20,
            PaneFocusStyle::TitleBar,
            false,
        )
    }

    fn editor_with_the_line(name: &str, tab_size: usize) -> (Editor, std::path::PathBuf) {
        editor_with_text(name, tab_size, LINE)
    }

    /// The same line, `count` times over, for the tests that scroll vertically.
    fn editor_with_lines(
        name: &str,
        tab_size: usize,
        count: usize,
    ) -> (Editor, std::path::PathBuf) {
        let text = vec![LINE; count].join("\n");
        editor_with_text(name, tab_size, &text)
    }

    fn editor_with_text(name: &str, tab_size: usize, text: &str) -> (Editor, std::path::PathBuf) {
        let dir = std::env::temp_dir().join("termcode-cursor-tests");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("{name}-{tab_size}.txt"));
        std::fs::write(&path, format!("{text}\n")).unwrap();

        let config = EditorConfig {
            tab_size,
            ..EditorConfig::default()
        };
        let mut editor = Editor::new(Theme::default(), config, LanguageRegistry::new(), None);
        editor.open_file(&path).unwrap();
        let area = layout().editor_area;
        let view = editor.active_view_mut().unwrap();
        view.area_width = area.width;
        view.area_height = area.height;
        (editor, path)
    }

    /// The cell the *widget* reverses, which is the block the user sees.
    fn reversed_cell(editor: &Editor, app_layout: &AppLayout) -> Option<(u16, u16)> {
        let area = app_layout.editor_area;
        let mut buf = Buffer::empty(area);
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

        for y in area.y..area.y + area.height {
            for x in area.x..area.x + area.width {
                if buf[(x, y)]
                    .style()
                    .add_modifier
                    .contains(Modifier::REVERSED)
                {
                    return Some((x, y));
                }
            }
        }
        None
    }

    #[test]
    fn the_popup_anchor_sits_on_the_cell_the_widget_reverses() {
        // FR-TAB-005. Two independent paths compute the cursor's column --
        // `cursor_screen_position` for the popup anchor, the widget for the
        // reversed block -- and a tab is where they used to disagree: the
        // widget expanded it to four columns while the measurement counted
        // zero.
        //
        // `left_col` is part of the claim, so it is part of the loop: at 0 the
        // anchor's horizontal bounds are never tested, since no column of the
        // line can fall to the left of the viewport.
        for tab_size in [4usize, 8, 2] {
            let (mut editor, _p) = editor_with_the_line("cursor-agreement", tab_size);
            let app_layout = layout();
            for left_col in [0usize, 1, 4, 20, 200] {
                editor.active_view_mut().unwrap().scroll.left_col = left_col;
                for column in 0..LINE.chars().count() {
                    editor.active_view_mut().unwrap().cursor.column = column;
                    assert_eq!(
                        cursor_screen_position(&editor, &app_layout),
                        reversed_cell(&editor, &app_layout),
                        "tab_size={tab_size} left_col={left_col} column={column}: the \
                         popup anchor and the drawn block are in different cells"
                    );
                }
            }
        }
    }

    #[test]
    fn the_popup_anchor_sits_on_the_cell_the_widget_reverses_at_every_top_line() {
        // The claim is about both axes, so the vertical one is looped too: a
        // cursor line above `top_line` is the case the horizontal test cannot
        // reach with a one-line fixture, and the one a wheel scroll produces
        // without touching the cursor.
        let app_layout = layout();
        let rows = app_layout.editor_area.height as usize;
        let (mut editor, _p) = editor_with_lines("cursor-agreement-rows", 4, rows * 3);

        for cursor_line in [0usize, 1, rows, rows * 2] {
            for top_line in [0usize, 1, rows - 1, rows, rows * 2] {
                {
                    let view = editor.active_view_mut().unwrap();
                    view.cursor.line = cursor_line;
                    view.cursor.column = 3;
                    view.scroll.top_line = top_line;
                }
                assert_eq!(
                    cursor_screen_position(&editor, &app_layout),
                    reversed_cell(&editor, &app_layout),
                    "cursor_line={cursor_line} top_line={top_line}: the popup anchor \
                     and the drawn block are in different cells"
                );
            }
        }
    }

    #[test]
    fn a_cursor_scrolled_out_of_the_code_area_has_no_anchor() {
        // The two directions the horizontal test above covers by construction,
        // written out: a cursor to the left of `left_col` and one past the
        // right edge. Both draw no block, so both must anchor nothing --
        // pinning a popup to the edge column would point it at a cell the
        // cursor is not in. Reached by dragging the horizontal scrollbar or
        // turning the wheel sideways, neither of which moves the cursor.
        let app_layout = layout();
        // Long enough that a column of it can be past the right edge of the
        // code area with the viewport still at the start of the line.
        let long_line = "x".repeat(400);
        let (mut editor, _p) = editor_with_text("cursor-off-screen", 4, &long_line);

        // Off to the left: column 0, with the viewport scrolled past it.
        {
            let view = editor.active_view_mut().unwrap();
            view.cursor.column = 0;
            view.scroll.left_col = 20;
        }
        assert_eq!(
            cursor_screen_position(&editor, &app_layout),
            None,
            "a cursor left of `left_col` anchors nothing"
        );
        assert_eq!(reversed_cell(&editor, &app_layout), None);

        // Off to the right: the end of the line, with the viewport at column 0.
        {
            let view = editor.active_view_mut().unwrap();
            view.cursor.column = long_line.chars().count() - 1;
            view.scroll.left_col = 0;
        }
        assert_eq!(
            cursor_screen_position(&editor, &app_layout),
            None,
            "a cursor past the right edge anchors nothing"
        );
        assert_eq!(reversed_cell(&editor, &app_layout), None);

        // And the same cursor *is* anchored once the viewport reaches it, so
        // the two assertions above are about the viewport and not about the
        // fixture being unrenderable.
        editor.active_view_mut().unwrap().scroll.left_col = 380;
        assert!(
            cursor_screen_position(&editor, &app_layout).is_some(),
            "scrolled onto the cursor, the anchor comes back"
        );
        assert_eq!(
            cursor_screen_position(&editor, &app_layout),
            reversed_cell(&editor, &app_layout)
        );
    }

    #[test]
    fn the_cursor_on_a_tab_sits_at_the_tabs_first_column() {
        // The deliberate half of FR-TAB-005: a tab occupies several columns and
        // the cursor takes the one the renderer starts painting it at. The
        // alternative (the last column) would put the terminal cursor in the
        // *next* character's neighbourhood and make a click on the cursor move
        // it.
        for tab_size in [4usize, 8, 2] {
            let (mut editor, _p) = editor_with_the_line("cursor-on-a-tab", tab_size);
            let app_layout = layout();
            // Character 0 is the leading tab, which starts at column 0.
            editor.active_view_mut().unwrap().cursor.column = 0;
            let (x, _) = cursor_screen_position(&editor, &app_layout).expect("a cursor");
            assert_eq!(
                x,
                app_layout.editor_area.x + 3 + 1,
                "tab_size={tab_size}: the cursor left the tab's first column"
            );
        }
    }
}
