use ratatui::Frame;

use termcode_view::editor::{Editor, EditorMode};

use crate::layout::{self, AppLayout};
use crate::ui::command_palette::CommandPaletteWidget;
use crate::ui::completion::CompletionWidget;
use crate::ui::editor_view::EditorViewWidget;
use crate::ui::file_explorer::FileExplorerWidget;
use crate::ui::fuzzy_finder::FuzzyFinderWidget;
use crate::ui::hover::HoverWidget;
use crate::ui::search::SearchOverlayWidget;
use crate::ui::status_bar::StatusBarWidget;
use crate::ui::tab_bar::TabBarWidget;
use crate::ui::top_bar::TopBarWidget;

pub fn render(frame: &mut Frame, editor: &Editor) {
    let area = frame.area();
    let app_layout = layout::compute_layout(
        area,
        editor.file_explorer.visible,
        editor.file_explorer.width,
    );

    let current_path = editor
        .active_document()
        .and_then(|d| d.path.as_ref())
        .and_then(|p| p.to_str());
    let top_bar_widget = TopBarWidget::new(current_path, &editor.theme);
    frame.render_widget(top_bar_widget, app_layout.top_bar);

    if let Some(sidebar_area) = app_layout.sidebar {
        let explorer_widget = FileExplorerWidget::new(&editor.file_explorer, &editor.theme);
        frame.render_widget(explorer_widget, sidebar_area);
    }

    let tab_bar_widget = TabBarWidget::new(&editor.tabs, &editor.theme);
    frame.render_widget(tab_bar_widget, app_layout.tab_bar);

    if let (Some(view), Some(doc)) = (editor.active_view(), editor.active_document()) {
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
            editor.config.line_numbers,
        );
        frame.render_widget(editor_widget, app_layout.editor_area);
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
        _ => {}
    }

    if editor.completion.visible {
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

    if editor.hover.visible {
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

    let status_widget = StatusBarWidget::new(
        editor.active_document(),
        editor.active_view(),
        &editor.theme,
        editor.status_message.as_deref(),
        editor.mode,
    );
    frame.render_widget(status_widget, app_layout.status_bar);
}

fn cursor_screen_position(editor: &Editor, app_layout: &AppLayout) -> Option<(u16, u16)> {
    let view = editor.active_view()?;
    let doc = editor.active_document()?;
    let gutter_width = crate::ui::editor_view::line_number_width_styled(
        doc.buffer.line_count(),
        editor.config.line_numbers,
    );

    let line_text: String = doc.buffer.line(view.cursor.line).chars().collect();
    let line_text = line_text.trim_end_matches('\n').trim_end_matches('\r');
    let display_col =
        crate::display_width::char_index_to_display_col(line_text, view.cursor.column) as u16;

    let cursor_x = app_layout.editor_area.x
        + gutter_width
        + 1
        + display_col.saturating_sub(view.scroll.left_col as u16);
    let cursor_y =
        app_layout.editor_area.y + (view.cursor.line.saturating_sub(view.scroll.top_line)) as u16;
    Some((cursor_x, cursor_y))
}
