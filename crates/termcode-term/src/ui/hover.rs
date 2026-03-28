use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::Widget;

use termcode_theme::theme::Theme;
use termcode_view::editor::HoverState;

use crate::display_width::str_display_width;

const MAX_WIDTH: u16 = 60;
const MAX_LINES: usize = 10;

pub struct HoverWidget<'a> {
    hover: &'a HoverState,
    theme: &'a Theme,
    cursor_x: u16,
    cursor_y: u16,
    parent_area: Rect,
}

impl<'a> HoverWidget<'a> {
    pub fn new(
        hover: &'a HoverState,
        theme: &'a Theme,
        cursor_x: u16,
        cursor_y: u16,
        parent_area: Rect,
    ) -> Self {
        Self {
            hover,
            theme,
            cursor_x,
            cursor_y,
            parent_area,
        }
    }
}

impl Widget for HoverWidget<'_> {
    fn render(self, _area: Rect, buf: &mut Buffer) {
        if !self.hover.visible || self.hover.content.is_empty() {
            return;
        }

        // Wrap content into lines.
        let max_content_width = (MAX_WIDTH - 2) as usize;
        let mut lines: Vec<String> = Vec::new();
        for raw_line in self.hover.content.lines() {
            if str_display_width(raw_line) <= max_content_width {
                lines.push(raw_line.to_string());
            } else {
                let mut current = String::new();
                let mut current_width = 0usize;
                for word in raw_line.split_whitespace() {
                    let word_width = str_display_width(word);
                    if current.is_empty() {
                        current = word.to_string();
                        current_width = word_width;
                    } else if current_width + 1 + word_width <= max_content_width {
                        current.push(' ');
                        current.push_str(word);
                        current_width += 1 + word_width;
                    } else {
                        lines.push(current);
                        current = word.to_string();
                        current_width = word_width;
                    }
                }
                if !current.is_empty() {
                    lines.push(current);
                }
            }
            if lines.len() >= MAX_LINES {
                break;
            }
        }
        lines.truncate(MAX_LINES);

        let content_width = lines
            .iter()
            .map(|l| str_display_width(l))
            .max()
            .unwrap_or(10) as u16;
        let width = (content_width + 2).clamp(10, MAX_WIDTH);
        let height = lines.len() as u16 + 2; // +2 for border

        // Position above cursor.
        let x = self
            .cursor_x
            .min(self.parent_area.x + self.parent_area.width.saturating_sub(width));
        let y = if self.cursor_y >= height {
            self.cursor_y - height
        } else {
            self.cursor_y + 1
        };

        let popup_rect = Rect::new(x, y, width, height);

        let bg = self.theme.ui.sidebar_bg.to_ratatui();
        let fg = self.theme.ui.foreground.to_ratatui();
        let border_color = self.theme.ui.border.to_ratatui();
        let border_style = Style::default().fg(border_color).bg(bg);
        let text_style = Style::default().fg(fg).bg(bg);

        // Fill background.
        for py in popup_rect.y..popup_rect.y + popup_rect.height {
            for px in popup_rect.x..popup_rect.x + popup_rect.width {
                if px < buf.area.width && py < buf.area.height {
                    buf[(px, py)].set_char(' ').set_bg(bg);
                }
            }
        }

        // Top border
        if popup_rect.width >= 2 {
            let by = popup_rect.y;
            buf[(popup_rect.x, by)]
                .set_char('\u{250c}')
                .set_style(border_style);
            buf[(popup_rect.x + popup_rect.width - 1, by)]
                .set_char('\u{2510}')
                .set_style(border_style);
            for bx in (popup_rect.x + 1)..(popup_rect.x + popup_rect.width - 1) {
                buf[(bx, by)].set_char('\u{2500}').set_style(border_style);
            }
        }

        // Bottom border
        if popup_rect.width >= 2 && popup_rect.height > 1 {
            let by = popup_rect.y + popup_rect.height - 1;
            buf[(popup_rect.x, by)]
                .set_char('\u{2514}')
                .set_style(border_style);
            buf[(popup_rect.x + popup_rect.width - 1, by)]
                .set_char('\u{2518}')
                .set_style(border_style);
            for bx in (popup_rect.x + 1)..(popup_rect.x + popup_rect.width - 1) {
                buf[(bx, by)].set_char('\u{2500}').set_style(border_style);
            }
        }

        // Side borders
        for py in (popup_rect.y + 1)..(popup_rect.y + popup_rect.height.saturating_sub(1)) {
            if popup_rect.width >= 2 {
                buf[(popup_rect.x, py)]
                    .set_char('\u{2502}')
                    .set_style(border_style);
                buf[(popup_rect.x + popup_rect.width - 1, py)]
                    .set_char('\u{2502}')
                    .set_style(border_style);
            }
        }

        // Render content lines.
        for (i, line) in lines.iter().enumerate() {
            let ly = popup_rect.y + 1 + i as u16;
            if ly >= popup_rect.y + popup_rect.height.saturating_sub(1) {
                break;
            }
            let mut lx = popup_rect.x + 1;
            let max_lx = popup_rect.x + popup_rect.width.saturating_sub(1);
            for ch in line.chars() {
                let ch_width = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0) as u16;
                if lx + ch_width > max_lx {
                    break;
                }
                buf[(lx, ly)].set_char(ch).set_style(text_style);
                for offset in 1..ch_width {
                    buf[(lx + offset, ly)].set_char(' ').set_style(text_style);
                }
                lx += ch_width;
            }
        }
    }
}
