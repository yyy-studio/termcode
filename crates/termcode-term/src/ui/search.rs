use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

use termcode_theme::theme::Theme;
use termcode_view::search::SearchState;

use super::overlay::{
    compute_overlay_rect, render_input_line, render_overlay_frame, OverlayConfig, OverlayPosition,
};

pub struct SearchOverlayWidget<'a> {
    search: &'a SearchState,
    theme: &'a Theme,
}

impl<'a> SearchOverlayWidget<'a> {
    pub fn new(search: &'a SearchState, theme: &'a Theme) -> Self {
        Self { search, theme }
    }
}

impl Widget for SearchOverlayWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let height = if self.search.replace_mode { 4 } else { 3 };
        let config = OverlayConfig {
            width_percent: 100,
            max_height: height,
            position: OverlayPosition::Top,
        };
        let overlay_area = compute_overlay_rect(area, &config);
        if overlay_area.height == 0 || overlay_area.width < 10 {
            return;
        }

        render_overlay_frame(overlay_area, buf, self.theme);

        // Search input line (inside border: row 1)
        let input_y = overlay_area.y + 1;
        let inner_x = overlay_area.x + 1;
        let inner_width = overlay_area.width.saturating_sub(2);
        if inner_width < 5 {
            return;
        }

        // Match count indicator
        let count_text = if self.search.query.is_empty() {
            String::new()
        } else if self.search.matches.is_empty() {
            "No results".to_string()
        } else {
            let current = self.search.current_match.map(|i| i + 1).unwrap_or(0);
            format!("{} of {}", current, self.search.match_count())
        };

        let count_width = count_text.len() as u16;
        let input_width = inner_width.saturating_sub(count_width + 1);

        // Show cursor only on focused field
        let search_cursor = if self.search.replace_focused {
            None
        } else {
            Some(self.search.cursor_pos)
        };

        let input_area = Rect::new(inner_x, input_y, input_width, 1);
        render_input_line(
            input_area,
            buf,
            "Search: ",
            &self.search.query,
            search_cursor,
            self.theme,
        );

        // Render count indicator on the right
        if !count_text.is_empty() {
            let count_x = inner_x + inner_width - count_width;
            let fg = self.theme.ui.line_number.to_ratatui();
            let bg = self.theme.ui.sidebar_bg.to_ratatui();
            let style = ratatui::style::Style::default().fg(fg).bg(bg);
            for (i, ch) in count_text.chars().enumerate() {
                let x = count_x + i as u16;
                if x < overlay_area.x + overlay_area.width - 1 {
                    buf[(x, input_y)].set_char(ch).set_style(style);
                }
            }
        }

        // Replace input line (if in replace mode)
        if self.search.replace_mode && overlay_area.height > 3 {
            let replace_y = overlay_area.y + 2;
            let replace_area = Rect::new(inner_x, replace_y, inner_width, 1);
            let replace_cursor = if self.search.replace_focused {
                Some(self.search.replace_cursor_pos)
            } else {
                None
            };
            render_input_line(
                replace_area,
                buf,
                "Replace: ",
                &self.search.replace_text,
                replace_cursor,
                self.theme,
            );
        }
    }
}
