use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

use termcode_theme::theme::Theme;
use termcode_view::fuzzy::FuzzyFinderState;

use super::overlay::{
    ListItem, OverlayConfig, OverlayPosition, compute_overlay_rect, render_input_line,
    render_overlay_frame, render_result_list,
};

pub struct FuzzyFinderWidget<'a> {
    state: &'a FuzzyFinderState,
    theme: &'a Theme,
}

impl<'a> FuzzyFinderWidget<'a> {
    pub fn new(state: &'a FuzzyFinderState, theme: &'a Theme) -> Self {
        Self { state, theme }
    }
}

impl Widget for FuzzyFinderWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let config = OverlayConfig {
            width_percent: 60,
            max_height: 20,
            position: OverlayPosition::Center,
        };
        let overlay_area = compute_overlay_rect(area, &config);
        if overlay_area.height < 4 || overlay_area.width < 10 {
            return;
        }

        render_overlay_frame(overlay_area, buf, self.theme);

        // Input line (row 1 inside border)
        let inner_x = overlay_area.x + 1;
        let inner_width = overlay_area.width.saturating_sub(2);
        let input_y = overlay_area.y + 1;

        // File count
        let count_text = format!("{} files", self.state.filtered.len());
        let count_width = count_text.len() as u16;
        let input_width = inner_width.saturating_sub(count_width + 1);

        let input_area = Rect::new(inner_x, input_y, input_width, 1);
        render_input_line(
            input_area,
            buf,
            "Open: ",
            &self.state.query,
            Some(self.state.cursor_pos),
            self.theme,
        );

        // Count on right side
        let count_x = inner_x + inner_width - count_width;
        let dim_fg = self.theme.ui.line_number.to_ratatui();
        let bg = self.theme.ui.sidebar_bg.to_ratatui();
        let count_style = ratatui::style::Style::default().fg(dim_fg).bg(bg);
        for (i, ch) in count_text.chars().enumerate() {
            let x = count_x + i as u16;
            if x < overlay_area.x + overlay_area.width - 1 {
                buf[(x, input_y)].set_char(ch).set_style(count_style);
            }
        }

        // Result list (rows 2.. inside border)
        let list_y = overlay_area.y + 2;
        let list_height = overlay_area.height.saturating_sub(3);
        if list_height == 0 {
            return;
        }

        let list_area = Rect::new(inner_x, list_y, inner_width, list_height);

        if self.state.filtered.is_empty() {
            let msg = if self.state.query.is_empty() {
                "Type to search..."
            } else {
                "No files found"
            };
            let fg = self.theme.ui.line_number.to_ratatui();
            let style = ratatui::style::Style::default().fg(fg).bg(bg);
            for (i, ch) in msg.chars().enumerate() {
                let x = inner_x + 1 + i as u16;
                if x < overlay_area.x + overlay_area.width - 1 {
                    buf[(x, list_y)].set_char(ch).set_style(style);
                }
            }
            return;
        }

        let items: Vec<ListItem> = self
            .state
            .filtered
            .iter()
            .map(|m| ListItem {
                text: m.path.clone(),
                secondary: None,
                highlights: m.indices.clone(),
            })
            .collect();

        render_result_list(
            list_area,
            buf,
            &items,
            self.state.selected,
            self.state.scroll_offset,
            self.theme,
        );
    }
}
