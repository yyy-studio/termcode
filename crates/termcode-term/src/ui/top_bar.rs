use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::Widget;

use termcode_theme::theme::Theme;

pub struct TopBarWidget<'a> {
    theme: &'a Theme,
}

impl<'a> TopBarWidget<'a> {
    pub fn new(theme: &'a Theme) -> Self {
        Self { theme }
    }
}

/// Width of the " ? Help " button (including padding).
pub const HELP_BUTTON_TEXT: &str = " ? Help ";
pub const HELP_BUTTON_WIDTH: u16 = 8;

impl Widget for TopBarWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 {
            return;
        }

        let bg = self.theme.ui.tab_active_bg.to_ratatui();
        let fg = self.theme.ui.foreground.to_ratatui();
        let style = Style::default().fg(fg).bg(bg);

        // Fill background
        for x in area.x..area.x + area.width {
            buf[(x, area.y)].set_char(' ').set_style(style);
        }

        // Left: app name
        let left_text = " termcode";
        let mut x_offset = area.x;
        for ch in left_text.chars() {
            if x_offset < area.x + area.width {
                buf[(x_offset, area.y)].set_char(ch).set_style(style);
            }
            x_offset += 1;
        }

        // Right: Help button
        let btn_style = Style::default()
            .fg(Color::Rgb(200, 204, 212))
            .bg(Color::Rgb(62, 68, 81));

        let btn_start = (area.x + area.width).saturating_sub(HELP_BUTTON_WIDTH);
        for (i, ch) in HELP_BUTTON_TEXT.chars().enumerate() {
            let x = btn_start + i as u16;
            if x >= area.x && x < area.x + area.width {
                buf[(x, area.y)].set_char(ch).set_style(btn_style);
            }
        }
    }
}
