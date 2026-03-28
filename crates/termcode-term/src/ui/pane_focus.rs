use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::Widget;

use termcode_theme::theme::Theme;

pub struct PaneTitleWidget<'a> {
    theme: &'a Theme,
}

impl<'a> PaneTitleWidget<'a> {
    pub fn new(theme: &'a Theme) -> Self {
        Self { theme }
    }
}

impl Widget for PaneTitleWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        let fg = self.theme.ui.pane_inactive_fg.to_ratatui();
        let bg = self.theme.ui.pane_inactive_bg.to_ratatui();
        let style = Style::default().fg(fg).bg(bg);

        for x in area.x..area.x + area.width {
            buf[(x, area.y)].set_char(' ').set_style(style);
        }

        let title = " EXPLORER";
        let mut x = area.x;
        for ch in title.chars() {
            if x >= area.x + area.width {
                break;
            }
            buf[(x, area.y)].set_char(ch).set_style(style);
            x += 1;
        }
    }
}

pub struct PaneBorderWidget<'a> {
    theme: &'a Theme,
}

impl<'a> PaneBorderWidget<'a> {
    pub fn new(theme: &'a Theme) -> Self {
        Self { theme }
    }
}

impl Widget for PaneBorderWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let fg = self.theme.ui.pane_inactive_bg.to_ratatui();
        let style = Style::default().fg(fg);

        for y in area.y..area.y + area.height {
            buf[(area.x, y)].set_char('\u{2502}').set_style(style);
        }
    }
}

pub struct PaneAccentLineWidget<'a> {
    theme: &'a Theme,
}

impl<'a> PaneAccentLineWidget<'a> {
    pub fn new(theme: &'a Theme) -> Self {
        Self { theme }
    }
}

impl Widget for PaneAccentLineWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        let fg = self.theme.ui.pane_inactive_bg.to_ratatui();
        let style = Style::default().fg(fg);

        for x in area.x..area.x + area.width {
            buf[(x, area.y)].set_char('\u{2501}').set_style(style);
        }
    }
}
