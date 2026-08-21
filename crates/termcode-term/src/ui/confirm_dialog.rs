use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::Widget;

use crate::display_width::{ui_char_width, ui_str_width};
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

const BUTTON_BRACKET_WIDTH: usize = 4; // "[ " + " ]"
const BUTTON_SPACING: usize = 2;
const BORDER_AND_PADDING: u16 = 4; // 2 border + 2 padding
const POPUP_HEIGHT: u16 = 7; // border + blank + message + blank + buttons + blank + border
const MIN_AREA_WIDTH: u16 = 10;

/// Where the dialog lands inside `area` and where its buttons sit.
///
/// The single source of the geometry, shared by the widget below and
/// `mouse.rs`, so a click cannot land on a button the widget drew elsewhere.
pub struct DialogLayout {
    pub popup: Rect,
    pub button_y: u16,
    /// Screen columns `[start, end)` of each button *including* its brackets,
    /// in the dialog's own button order.
    pub buttons: Vec<(u16, u16)>,
}

impl DialogLayout {
    /// Index of the button at `(x, y)`, if any.
    pub fn button_at(&self, x: u16, y: u16) -> Option<usize> {
        if y != self.button_y {
            return None;
        }
        self.buttons
            .iter()
            .position(|(start, end)| x >= *start && x < *end)
    }
}

/// `None` when `area` is too small for the dialog, which is then not drawn.
pub fn layout(dialog: &ConfirmDialog, area: Rect) -> Option<DialogLayout> {
    if area.width < MIN_AREA_WIDTH || area.height < POPUP_HEIGHT {
        return None;
    }

    let button_widths: Vec<usize> = dialog
        .buttons
        .iter()
        .map(|b| ui_str_width(b) + BUTTON_BRACKET_WIDTH)
        .collect();
    let buttons_width: usize = button_widths.iter().sum::<usize>()
        + dialog.buttons.len().saturating_sub(1) * BUTTON_SPACING;

    let content_width = ui_str_width(&dialog.message).max(buttons_width);
    let popup_width = (content_width as u16 + BORDER_AND_PADDING)
        .min(area.width.saturating_sub(BORDER_AND_PADDING));

    let popup = Rect::new(
        area.x + (area.width.saturating_sub(popup_width)) / 2,
        area.y + (area.height.saturating_sub(POPUP_HEIGHT)) / 2,
        popup_width,
        POPUP_HEIGHT,
    );

    let inner_x = popup.x + 1;
    let inner_width = popup.width.saturating_sub(2) as usize;
    let offset = (inner_width.saturating_sub(buttons_width)) / 2;

    let mut buttons = Vec::with_capacity(button_widths.len());
    let mut col = offset;
    for width in button_widths {
        let start = inner_x + col as u16;
        buttons.push((start, start + width as u16));
        col += width + BUTTON_SPACING;
    }

    Some(DialogLayout {
        popup,
        button_y: popup.y + 4,
        buttons,
    })
}

impl Widget for ConfirmDialogWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let Some(placed) = layout(self.dialog, area) else {
            return;
        };

        let bg = self.theme.ui.background.to_ratatui();
        let fg = self.theme.ui.foreground.to_ratatui();
        let border_color = self.theme.ui.border.to_ratatui();
        let selection_bg = self.theme.ui.selection.to_ratatui();

        let bg_style = Style::default().fg(fg).bg(bg);
        let border_style = Style::default().fg(border_color).bg(bg);
        let message_style = Style::default()
            .fg(self.theme.ui.line_number_active.to_ratatui())
            .bg(bg);

        let popup_rect = placed.popup;
        crate::ui::overlay::render_shadow(popup_rect, buf, self.theme);

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
            let msg_display_width = ui_str_width(msg);
            let msg_offset = (inner_width.saturating_sub(msg_display_width)) / 2;
            let mut col = 0usize;
            for ch in msg.chars() {
                let w = ui_char_width(ch);
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

        let btn_y = placed.button_y;
        if btn_y < buf.area().height {
            for (i, (start, _end)) in placed.buttons.iter().enumerate() {
                let selected = i == self.dialog.selected_button;
                let bracket_style = if selected {
                    Style::default().fg(fg).bg(selection_bg)
                } else {
                    Style::default().fg(border_color).bg(bg)
                };
                let label_style = if selected {
                    Style::default().fg(fg).bg(selection_bg)
                } else {
                    Style::default().fg(fg).bg(bg)
                };

                let mut col = 0usize;
                for (text, style) in [
                    ("[ ", bracket_style),
                    (self.dialog.buttons[i].as_str(), label_style),
                    (" ]", bracket_style),
                ] {
                    for ch in text.chars() {
                        let x = start + col as u16;
                        if x < buf.area().width {
                            buf[(x, btn_y)].set_char(ch).set_style(style);
                        }
                        col += ui_char_width(ch);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use termcode_view::confirm::{ConfirmAction, ConfirmDialog};

    fn dialog() -> ConfirmDialog {
        ConfirmDialog::new(
            ConfirmAction::QuitAll,
            "You have 2 unsaved file(s).".to_string(),
            vec![
                "Save All & Quit".to_string(),
                "Quit Without Saving".to_string(),
                "Cancel".to_string(),
            ],
        )
    }

    /// The buttons the widget draws are the buttons `button_at` reports, which
    /// is what keeps a click from landing one column off.
    #[test]
    fn every_button_is_where_it_is_drawn() {
        let area = Rect::new(0, 0, 80, 24);
        let dialog = dialog();
        let placed = layout(&dialog, area).expect("80x24 fits the dialog");

        let mut buf = Buffer::empty(area);
        ConfirmDialogWidget::new(&dialog, &Theme::default()).render(area, &mut buf);
        let row: String = (0..area.width)
            .map(|x| buf[(x, placed.button_y)].symbol().to_string())
            .collect();

        for (i, label) in dialog.buttons.iter().enumerate() {
            let (start, end) = placed.buttons[i];
            let drawn: String = row
                .chars()
                .skip(start as usize)
                .take((end - start) as usize)
                .collect();
            assert_eq!(drawn, format!("[ {label} ]"));
            assert_eq!(placed.button_at(start, placed.button_y), Some(i));
            assert_eq!(placed.button_at(end - 1, placed.button_y), Some(i));
        }

        // The gap between two buttons belongs to neither.
        let gap = placed.buttons[0].1;
        assert_eq!(placed.button_at(gap, placed.button_y), None);
        // Nor does any other row.
        assert_eq!(
            placed.button_at(placed.buttons[0].0, placed.button_y - 1),
            None
        );
    }

    #[test]
    fn a_small_area_has_no_layout_and_draws_nothing() {
        assert!(layout(&dialog(), Rect::new(0, 0, 9, 24)).is_none());
        assert!(layout(&dialog(), Rect::new(0, 0, 80, 6)).is_none());
    }
}
