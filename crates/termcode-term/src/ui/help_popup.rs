use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::Widget;

use termcode_theme::theme::Theme;

pub struct HelpPopupWidget<'a> {
    theme: &'a Theme,
}

impl<'a> HelpPopupWidget<'a> {
    pub fn new(theme: &'a Theme) -> Self {
        Self { theme }
    }
}

const SECTIONS: &[(&str, &[(&str, &str)])] = &[
    (
        "General",
        &[
            ("Ctrl+Q", "Quit"),
            ("Ctrl+S", "Save"),
            ("Ctrl+B", "Toggle Sidebar"),
            ("Ctrl+Z", "Undo"),
            ("Ctrl+Y", "Redo"),
            ("F1 / ?", "Toggle Help"),
        ],
    ),
    (
        "Navigation",
        &[
            ("Ctrl+P", "Find File"),
            ("Ctrl+F", "Search"),
            ("Ctrl+H", "Search & Replace"),
            ("Ctrl+Shift+P / :", "Command Palette"),
            ("Alt+Left/Right", "Switch Tab"),
            ("Ctrl+W", "Close Tab"),
        ],
    ),
    (
        "Editing",
        &[
            ("i", "Insert Mode"),
            ("Esc", "Normal Mode"),
            ("Ctrl+C", "Copy"),
            ("Ctrl+X", "Cut"),
            ("Ctrl+V", "Paste"),
        ],
    ),
    (
        "Code",
        &[
            ("Ctrl+D / F12", "Go to Definition"),
            ("Shift+K", "Hover Info"),
            ("] / [", "Next/Prev Diagnostic"),
        ],
    ),
];

impl Widget for HelpPopupWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Calculate content size
        let mut content_lines: Vec<(Style, String)> = Vec::new();
        let title = "Keyboard Shortcuts";
        let max_key_width = SECTIONS
            .iter()
            .flat_map(|(_, items)| items.iter())
            .map(|(k, _)| k.len())
            .max()
            .unwrap_or(0);

        let content_width = max_key_width + 4 + 20; // key + separator + description
        let popup_width = (content_width + 4) as u16; // padding
        let popup_width = popup_width.min(area.width.saturating_sub(4));

        let bg = self.theme.ui.background.to_ratatui();
        let fg = self.theme.ui.foreground.to_ratatui();
        let border_color = self.theme.ui.border.to_ratatui();
        let title_style = Style::default()
            .fg(self.theme.ui.line_number_active.to_ratatui())
            .bg(bg);
        let section_style = Style::default().fg(self.theme.ui.info.to_ratatui()).bg(bg);
        let key_style = Style::default()
            .fg(self.theme.ui.line_number_active.to_ratatui())
            .bg(bg);
        let desc_style = Style::default().fg(fg).bg(bg);
        let border_style = Style::default().fg(border_color).bg(bg);
        let bg_style = Style::default().fg(fg).bg(bg);

        // Build content lines
        content_lines.push((title_style, title.to_string()));
        content_lines.push((bg_style, String::new())); // blank line

        for (section_name, items) in SECTIONS {
            content_lines.push((section_style, format!("  {section_name}")));
            for (key, desc) in *items {
                content_lines.push((
                    bg_style,
                    format!("    {key:<width$}  {desc}", width = max_key_width),
                ));
            }
            content_lines.push((bg_style, String::new())); // blank after section
        }

        // Add footer
        content_lines.push((desc_style, "  Press any key to close".to_string()));

        let popup_height = (content_lines.len() as u16 + 2).min(area.height.saturating_sub(2)); // +2 for border
        let popup_x = area.x + (area.width.saturating_sub(popup_width)) / 2;
        let popup_y = area.y + (area.height.saturating_sub(popup_height)) / 2;

        let popup_rect = Rect::new(popup_x, popup_y, popup_width, popup_height);

        // Fill background (reset to clear any inherited modifiers like REVERSED cursor)
        for y in popup_rect.y..popup_rect.y + popup_rect.height {
            for x in popup_rect.x..popup_rect.x + popup_rect.width {
                if x < buf.area().width && y < buf.area().height {
                    buf[(x, y)].reset();
                    buf[(x, y)].set_char(' ').set_style(bg_style);
                }
            }
        }

        // Draw border
        let right = popup_rect.x + popup_rect.width - 1;
        let bottom = popup_rect.y + popup_rect.height - 1;

        // Top and bottom borders
        for x in popup_rect.x..=right {
            if x < buf.area().width {
                if popup_rect.y < buf.area().height {
                    buf[(x, popup_rect.y)]
                        .set_char(if x == popup_rect.x {
                            '╭'
                        } else if x == right {
                            '╮'
                        } else {
                            '─'
                        })
                        .set_style(border_style);
                }
                if bottom < buf.area().height {
                    buf[(x, bottom)]
                        .set_char(if x == popup_rect.x {
                            '╰'
                        } else if x == right {
                            '╯'
                        } else {
                            '─'
                        })
                        .set_style(border_style);
                }
            }
        }

        // Left and right borders
        for y in (popup_rect.y + 1)..bottom {
            if y < buf.area().height {
                if popup_rect.x < buf.area().width {
                    buf[(popup_rect.x, y)].set_char('│').set_style(border_style);
                }
                if right < buf.area().width {
                    buf[(right, y)].set_char('│').set_style(border_style);
                }
            }
        }

        // Render content lines
        let inner_x = popup_rect.x + 1;
        let inner_width = popup_rect.width.saturating_sub(2) as usize;
        for (i, (style, line)) in content_lines.iter().enumerate() {
            let y = popup_rect.y + 1 + i as u16;
            if y >= bottom {
                break;
            }
            if y >= buf.area().height {
                break;
            }

            // Determine style for each character based on position
            let is_shortcut_line = line.starts_with("    ") && !line.trim().is_empty();

            for (j, ch) in line.chars().enumerate() {
                if j >= inner_width {
                    break;
                }
                let x = inner_x + j as u16;
                if x < buf.area().width {
                    if is_shortcut_line {
                        // Key portion vs description
                        let trimmed_start = 4; // "    " prefix
                        let key_end = trimmed_start + max_key_width;
                        let char_style = if j < key_end { key_style } else { desc_style };
                        buf[(x, y)].set_char(ch).set_style(char_style);
                    } else {
                        buf[(x, y)].set_char(ch).set_style(*style);
                    }
                }
            }
        }
    }
}
