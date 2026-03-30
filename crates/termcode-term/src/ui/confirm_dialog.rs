use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::Widget;
use unicode_width::UnicodeWidthStr;

use crate::display_width::char_display_width;
use termcode_theme::theme::Theme;
use termcode_view::confirm::ConfirmDialog;

pub struct ConfirmDialogWidget<'a> {
    dialog: &'a ConfirmDialog,
    theme: &'a Theme,
}

impl<'a> ConfirmDialogWidget<'a> {
    pub fn new(dialog: &'a ConfirmDialog, theme: &'a Theme) -> Self {
        Self { dialog, theme }
    }
}

impl Widget for ConfirmDialogWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        const BUTTON_BRACKET_WIDTH: usize = 4; // "[ " + " ]"
        const BUTTON_SPACING: usize = 2;
        const BORDER_AND_PADDING: u16 = 4; // 2 border + 2 padding
        const POPUP_HEIGHT: u16 = 7; // border + blank + message + blank + buttons + blank + border
        const MIN_AREA_WIDTH: u16 = 10;

        let bg = self.theme.ui.background.to_ratatui();
        let fg = self.theme.ui.foreground.to_ratatui();
        let border_color = self.theme.ui.border.to_ratatui();
        let selection_bg = self.theme.ui.selection.to_ratatui();

        let bg_style = Style::default().fg(fg).bg(bg);
        let border_style = Style::default().fg(border_color).bg(bg);
        let message_style = Style::default()
            .fg(self.theme.ui.line_number_active.to_ratatui())
            .bg(bg);

        let message_width = self.dialog.message.width();
        let buttons_width: usize = self
            .dialog
            .buttons
            .iter()
            .map(|b| b.width() + BUTTON_BRACKET_WIDTH)
            .sum::<usize>()
            + self.dialog.buttons.len().saturating_sub(1) * BUTTON_SPACING;

        let content_width = message_width.max(buttons_width);
        let popup_width = (content_width as u16 + BORDER_AND_PADDING)
            .min(area.width.saturating_sub(BORDER_AND_PADDING));

        if area.width < MIN_AREA_WIDTH || area.height < POPUP_HEIGHT {
            return;
        }

        let popup_x = area.x + (area.width.saturating_sub(popup_width)) / 2;
        let popup_y = area.y + (area.height.saturating_sub(POPUP_HEIGHT)) / 2;
        let popup_rect = Rect::new(popup_x, popup_y, popup_width, POPUP_HEIGHT);

        for y in popup_rect.y..popup_rect.y + popup_rect.height {
            for x in popup_rect.x..popup_rect.x + popup_rect.width {
                if x < buf.area().width && y < buf.area().height {
                    buf[(x, y)].reset();
                    buf[(x, y)].set_char(' ').set_style(bg_style);
                }
            }
        }

        let right = popup_rect.x + popup_rect.width - 1;
        let bottom = popup_rect.y + popup_rect.height - 1;

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

        let inner_x = popup_rect.x + 1;
        let inner_width = popup_rect.width.saturating_sub(2) as usize;

        let msg_y = popup_rect.y + 2;
        if msg_y < buf.area().height {
            let msg = &self.dialog.message;
            let msg_display_width = msg.width();
            let msg_offset = (inner_width.saturating_sub(msg_display_width)) / 2;
            let mut col = 0usize;
            for ch in msg.chars() {
                let w = char_display_width(ch);
                if col + w > inner_width {
                    break;
                }
                let x = inner_x + (msg_offset + col) as u16;
                if x < buf.area().width {
                    buf[(x, msg_y)].set_char(ch).set_style(message_style);
                }
                col += w;
            }
        }

        let btn_y = popup_rect.y + 4;
        if btn_y < buf.area().height {
            struct BtnSegment {
                text: String,
                is_bracket: bool,
                selected: bool,
            }

            let mut segments: Vec<BtnSegment> = Vec::new();
            for (i, label) in self.dialog.buttons.iter().enumerate() {
                if i > 0 {
                    segments.push(BtnSegment {
                        text: "  ".to_string(),
                        is_bracket: false,
                        selected: false,
                    });
                }
                let sel = i == self.dialog.selected_button;
                segments.push(BtnSegment {
                    text: "[ ".to_string(),
                    is_bracket: true,
                    selected: sel,
                });
                segments.push(BtnSegment {
                    text: label.clone(),
                    is_bracket: false,
                    selected: sel,
                });
                segments.push(BtnSegment {
                    text: " ]".to_string(),
                    is_bracket: true,
                    selected: sel,
                });
            }

            let total_len: usize = segments.iter().map(|s| s.text.width()).sum();
            let btn_offset = (inner_width.saturating_sub(total_len)) / 2;

            let mut col = 0;
            for seg in &segments {
                let style = if seg.selected {
                    Style::default().fg(fg).bg(selection_bg)
                } else if seg.is_bracket {
                    Style::default().fg(border_color).bg(bg)
                } else {
                    Style::default().fg(fg).bg(bg)
                };
                for ch in seg.text.chars() {
                    let x = inner_x + (btn_offset + col) as u16;
                    if x < buf.area().width {
                        buf[(x, btn_y)].set_char(ch).set_style(style);
                    }
                    col += char_display_width(ch);
                }
            }
        }
    }
}
