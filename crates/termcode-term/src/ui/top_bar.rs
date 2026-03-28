use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::Widget;

use termcode_theme::theme::Theme;

pub struct TopBarWidget<'a> {
    path: Option<&'a str>,
    theme: &'a Theme,
}

impl<'a> TopBarWidget<'a> {
    pub fn new(path: Option<&'a str>, theme: &'a Theme) -> Self {
        Self { path, theme }
    }
}

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
        for (i, ch) in left_text.chars().enumerate() {
            let x = area.x + i as u16;
            if x < area.x + area.width {
                buf[(x, area.y)].set_char(ch).set_style(style);
            }
        }

        // Right: current file path
        if let Some(path) = self.path {
            let right_text = format!("{path} ");
            let right_start = (area.x + area.width).saturating_sub(right_text.len() as u16);
            for (i, ch) in right_text.chars().enumerate() {
                let x = right_start + i as u16;
                if x >= area.x && x < area.x + area.width {
                    buf[(x, area.y)].set_char(ch).set_style(style);
                }
            }
        }
    }
}
