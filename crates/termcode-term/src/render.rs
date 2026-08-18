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
            editor.config.line_numbers,
            is_editor_active,
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
        EditorMode::Settings => {
            let settings_widget = SettingsWidget::new(&editor.settings, &editor.theme);
            frame.render_widget(settings_widget, app_layout.editor_area);
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
        crate::display_width::char_index_to_display_col(line_text, view.cursor.column);

    // Clamp to the code area: long (minified) lines can exceed u16 display columns.
    let code_width = app_layout
        .editor_area
        .width
        .saturating_sub(gutter_width + 1);
    if code_width == 0 {
        return None;
    }
    let col_offset = display_col
        .saturating_sub(view.scroll.left_col)
        .min(code_width as usize - 1) as u16;

    let row = view.cursor.line.saturating_sub(view.scroll.top_line);
    if row >= app_layout.editor_area.height as usize {
        return None;
    }

    let cursor_x = app_layout.editor_area.x + gutter_width + 1 + col_offset;
    let cursor_y = app_layout.editor_area.y + row as u16;
    Some((cursor_x, cursor_y))
}
