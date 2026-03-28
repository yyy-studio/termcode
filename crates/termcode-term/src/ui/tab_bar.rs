use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::Widget;

use termcode_theme::theme::Theme;
use termcode_view::tab::TabManager;

pub struct TabBarWidget<'a> {
    tabs: &'a TabManager,
    theme: &'a Theme,
    is_editor_active: bool,
}

impl<'a> TabBarWidget<'a> {
    pub fn new(tabs: &'a TabManager, theme: &'a Theme, is_editor_active: bool) -> Self {
        Self {
            tabs,
            theme,
            is_editor_active,
        }
    }
}

impl Widget for TabBarWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 {
            return;
        }

        let inactive_bg = self.theme.ui.tab_inactive_bg.to_ratatui();
        let inactive_fg = self.theme.ui.line_number.to_ratatui();
        let inactive_style = Style::default().fg(inactive_fg).bg(inactive_bg);

        let empty_bg = if self.is_editor_active {
            self.theme.ui.pane_active_bg.to_ratatui()
        } else {
            self.theme.ui.pane_inactive_bg.to_ratatui()
        };
        let empty_style = Style::default().bg(empty_bg);
        for x in area.x..area.x + area.width {
            buf[(x, area.y)].set_char(' ').set_style(empty_style);
        }

        if self.tabs.tabs.is_empty() {
            return;
        }

        let active_bg = self.theme.ui.tab_active_bg.to_ratatui();
        let active_fg = self.theme.ui.foreground.to_ratatui();
        let active_style = Style::default().fg(active_fg).bg(active_bg);
        let sep_fg = self.theme.ui.border.to_ratatui();
        let sep_style = Style::default().fg(sep_fg).bg(inactive_bg);

        let mut x = area.x;

        for (i, tab) in self.tabs.tabs.iter().enumerate() {
            if x >= area.x + area.width {
                break;
            }

            if i > 0 && x < area.x + area.width {
                buf[(x, area.y)].set_char('|').set_style(sep_style);
                x += 1;
            }

            let style = if i == self.tabs.active {
                active_style
            } else {
                inactive_style
            };

            let label = if tab.modified {
                format!(" \u{2022} {} ", tab.label)
            } else {
                format!(" {} ", tab.label)
            };

            for ch in label.chars() {
                let ch_width = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0) as u16;
                if x + ch_width > area.x + area.width {
                    break;
                }
                buf[(x, area.y)].set_char(ch).set_style(style);
                for offset in 1..ch_width {
                    buf[(x + offset, area.y)].set_char(' ').set_style(style);
                }
                x += ch_width;
            }
        }
    }
}
