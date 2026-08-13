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

/// App name and version shown at the left of the top bar.
fn title_text() -> String {
    format!(" termcode v{}", env!("CARGO_PKG_VERSION"))
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

        let btn_start = (area.x + area.width).saturating_sub(HELP_BUTTON_WIDTH);

        // Left: app name and version, truncated before the Help button
        let left_text = title_text();
        for (x_offset, ch) in (area.x..).zip(left_text.chars()) {
            if x_offset >= btn_start {
                break;
            }
            buf[(x_offset, area.y)].set_char(ch).set_style(style);
        }

        // Right: Help button
        let btn_style = Style::default()
            .fg(Color::Rgb(200, 204, 212))
            .bg(Color::Rgb(62, 68, 81));

        for (i, ch) in HELP_BUTTON_TEXT.chars().enumerate() {
            let x = btn_start + i as u16;
            if x >= area.x && x < area.x + area.width {
                buf[(x, area.y)].set_char(ch).set_style(btn_style);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render_row(width: u16) -> String {
        let theme = Theme::default();
        let area = Rect::new(0, 0, width, 1);
        let mut buf = Buffer::empty(area);
        TopBarWidget::new(&theme).render(area, &mut buf);
        (0..width)
            .map(|x| buf[(x, 0)].symbol().to_string())
            .collect()
    }

    #[test]
    fn top_bar_shows_app_version() {
        let row = render_row(80);
        assert!(
            row.contains(&format!("termcode v{}", env!("CARGO_PKG_VERSION"))),
            "top bar should show the app version, got: {row:?}"
        );
    }

    #[test]
    fn top_bar_keeps_help_button() {
        let row = render_row(80);
        assert!(row.ends_with(HELP_BUTTON_TEXT), "got: {row:?}");
    }

    #[test]
    fn top_bar_title_does_not_overrun_help_button() {
        // Narrow enough that the title would collide with the button.
        let row = render_row(12);
        assert!(row.ends_with(HELP_BUTTON_TEXT), "got: {row:?}");
        assert_eq!(row.len(), 12);
    }
}
