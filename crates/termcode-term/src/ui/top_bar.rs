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

/// Hotkey hints displayed in the top bar.
const HOTKEY_HINTS: &[(&str, &str)] = &[
    ("^B", "Sidebar"),
    ("i", "Insert"),
    ("Esc", "Normal"),
    ("^S", "Save"),
    ("^Q", "Quit"),
    ("^P", "Find"),
    ("^F", "Search"),
];

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

        // Right: hotkey hints
        let key_style = Style::default()
            .fg(Color::Rgb(200, 204, 212))
            .bg(Color::Rgb(62, 68, 81));
        let label_style = Style::default().fg(Color::Rgb(171, 178, 191)).bg(bg);
        let sep_style = style;

        // Build the hints string to calculate total width
        let mut hint_parts: Vec<(String, String)> = Vec::new();
        let mut total_width: u16 = 1; // trailing space
        for (key, label) in HOTKEY_HINTS {
            let key_part = format!(" {key} ");
            let label_part = format!("{label}");
            total_width += key_part.len() as u16 + label_part.len() as u16 + 1; // +1 for separator
            hint_parts.push((key_part, label_part));
        }

        let right_start = (area.x + area.width).saturating_sub(total_width);
        let mut rx = right_start;

        for (key_part, label_part) in &hint_parts {
            // Key badge
            for ch in key_part.chars() {
                if rx >= area.x && rx < area.x + area.width {
                    buf[(rx, area.y)].set_char(ch).set_style(key_style);
                }
                rx += 1;
            }
            // Label
            for ch in label_part.chars() {
                if rx >= area.x && rx < area.x + area.width {
                    buf[(rx, area.y)].set_char(ch).set_style(label_style);
                }
                rx += 1;
            }
            // Separator space
            if rx < area.x + area.width {
                buf[(rx, area.y)].set_char(' ').set_style(sep_style);
            }
            rx += 1;
        }
    }
}
