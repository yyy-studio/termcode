use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::Widget;

use termcode_theme::theme::Theme;
use termcode_view::editor::CompletionState;

use super::overlay::{ListItem, render_result_list};

const MAX_VISIBLE_ITEMS: usize = 10;
const MAX_WIDTH: u16 = 40;

pub struct CompletionWidget<'a> {
    completion: &'a CompletionState,
    theme: &'a Theme,
    cursor_x: u16,
    cursor_y: u16,
    parent_area: Rect,
}

impl<'a> CompletionWidget<'a> {
    pub fn new(
        completion: &'a CompletionState,
        theme: &'a Theme,
        cursor_x: u16,
        cursor_y: u16,
        parent_area: Rect,
    ) -> Self {
        Self {
            completion,
            theme,
            cursor_x,
            cursor_y,
            parent_area,
        }
    }
}

impl Widget for CompletionWidget<'_> {
    fn render(self, _area: Rect, buf: &mut Buffer) {
        if !self.completion.visible || self.completion.items.is_empty() {
            return;
        }

        let item_count = self.completion.items.len();
        let visible_count = item_count.min(MAX_VISIBLE_ITEMS);
        let height = visible_count as u16 + 2; // +2 for top and bottom border

        // Calculate width from longest label.
        let content_width = self
            .completion
            .items
            .iter()
            .map(|i| i.label.len())
            .max()
            .unwrap_or(10) as u16
            + 2; // padding
        let width = content_width.clamp(15, MAX_WIDTH);

        // Position popup below cursor, or above if not enough space below.
        let x = self
            .cursor_x
            .min(self.parent_area.x + self.parent_area.width.saturating_sub(width));
        let below_space = self
            .parent_area
            .y
            .saturating_add(self.parent_area.height)
            .saturating_sub(self.cursor_y + 1);
        let y = if below_space >= height {
            self.cursor_y + 1
        } else {
            self.cursor_y.saturating_sub(height)
        };

        let popup_rect = Rect::new(x, y, width, height);

        // Fill background and draw border.
        let bg = self.theme.ui.sidebar_bg.to_ratatui();
        let border_color = self.theme.ui.border.to_ratatui();
        let border_style = Style::default().fg(border_color).bg(bg);

        for py in popup_rect.y..popup_rect.y + popup_rect.height {
            for px in popup_rect.x..popup_rect.x + popup_rect.width {
                if px < buf.area.width && py < buf.area.height {
                    buf[(px, py)].set_char(' ').set_bg(bg);
                }
            }
        }

        // Top border
        if popup_rect.width >= 2 && popup_rect.height > 0 {
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

        // Render items using render_result_list.
        let items: Vec<ListItem> = self
            .completion
            .items
            .iter()
            .map(|i| ListItem {
                text: i.label.clone(),
                secondary: i.detail.clone(),
                highlights: Vec::new(),
            })
            .collect();

        let content_rect = Rect::new(
            popup_rect.x + 1,
            popup_rect.y + 1,
            popup_rect.width.saturating_sub(2),
            visible_count as u16,
        );

        let scroll_offset = if self.completion.selected >= visible_count {
            self.completion.selected - visible_count + 1
        } else {
            0
        };

        render_result_list(
            content_rect,
            buf,
            &items,
            self.completion.selected,
            scroll_offset,
            self.theme,
        );
    }
}
