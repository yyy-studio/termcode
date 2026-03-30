use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::{StatefulWidget, Widget};
use ratatui_image::StatefulImage;
use ratatui_image::protocol::StatefulProtocol;

use termcode_theme::theme::Theme;

/// Widget that renders an image in the editor area using ratatui-image.
pub struct ImageViewWidget<'a> {
    theme: &'a Theme,
}

impl<'a> ImageViewWidget<'a> {
    pub fn new(theme: &'a Theme) -> Self {
        Self { theme }
    }
}

impl<'a> ImageViewWidget<'a> {
    pub fn render_stateful(self, area: Rect, buf: &mut Buffer, protocol: &mut StatefulProtocol) {
        let bg = self.theme.ui.background.to_ratatui();
        let style = Style::default().bg(bg);
        for y in area.y..area.y + area.height {
            for x in area.x..area.x + area.width {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_style(style);
                    cell.set_char(' ');
                }
            }
        }

        let image_widget = StatefulImage::default();
        image_widget.render(area, buf, protocol);
    }
}

/// Placeholder widget shown when no image protocol is available or image failed to decode.
pub struct ImagePlaceholderWidget<'a> {
    message: &'a str,
    theme: &'a Theme,
}

impl<'a> ImagePlaceholderWidget<'a> {
    pub fn new(message: &'a str, theme: &'a Theme) -> Self {
        Self { message, theme }
    }
}

impl<'a> Widget for ImagePlaceholderWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let bg = self.theme.ui.background.to_ratatui();
        let fg = self.theme.ui.foreground.to_ratatui();
        let style = Style::default().fg(fg).bg(bg);

        for y in area.y..area.y + area.height {
            for x in area.x..area.x + area.width {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_style(style);
                    cell.set_char(' ');
                }
            }
        }

        if area.height > 0 && area.width > 0 {
            let msg = self.message;
            let y = area.y + area.height / 2;
            let x = area.x + area.width.saturating_sub(msg.len() as u16) / 2;
            buf.set_string(x, y, msg, style);
        }
    }
}
