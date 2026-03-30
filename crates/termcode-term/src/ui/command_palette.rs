use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

use termcode_theme::theme::Theme;
use termcode_view::palette::CommandPaletteState;

use super::overlay::{
    ListItem, OverlayConfig, OverlayPosition, compute_overlay_rect, render_input_line,
    render_overlay_frame, render_result_list,
};

pub struct CommandPaletteWidget<'a> {
    state: &'a CommandPaletteState,
    theme: &'a Theme,
}

impl<'a> CommandPaletteWidget<'a> {
    pub fn new(state: &'a CommandPaletteState, theme: &'a Theme) -> Self {
        Self { state, theme }
    }
}

impl Widget for CommandPaletteWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let config = OverlayConfig {
            width_percent: 50,
            max_height: 15,
            position: OverlayPosition::Center,
        };
        let overlay_area = compute_overlay_rect(area, &config);
        if overlay_area.height < 4 || overlay_area.width < 10 {
            return;
        }

        render_overlay_frame(overlay_area, buf, self.theme);

        // Input line
        let inner_x = overlay_area.x + 1;
        let inner_width = overlay_area.width.saturating_sub(2);
        let input_y = overlay_area.y + 1;

        let input_area = Rect::new(inner_x, input_y, inner_width, 1);
        render_input_line(
            input_area,
            buf,
            "> ",
            &self.state.query,
            Some(self.state.cursor_pos),
            self.theme,
        );

        // Result list
        let list_y = overlay_area.y + 2;
        let list_height = overlay_area.height.saturating_sub(3);
        if list_height == 0 {
            return;
        }

        let list_area = Rect::new(inner_x, list_y, inner_width, list_height);
        let bg = self.theme.ui.sidebar_bg.to_ratatui();

        if self.state.filtered.is_empty() {
            let msg = "No matching commands";
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
            .map(|item| ListItem {
                text: item.name.clone(),
                secondary: Some(item.id.clone()),
                highlights: Vec::new(),
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
