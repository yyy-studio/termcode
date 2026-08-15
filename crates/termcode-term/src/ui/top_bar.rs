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

/// The Settings button, sitting just left of Help. ASCII only: a gear glyph is
/// East Asian Ambiguous, so terminals disagree on whether it is one column or
/// two and the whole bar would shift under it.
pub const SETTINGS_BUTTON_TEXT: &str = " F2 Settings ";
pub const SETTINGS_BUTTON_WIDTH: u16 = 13;

/// Where the top bar's buttons sit, so the widget and the mouse handler cannot
/// disagree about what was clicked.
pub struct TopBarButtons {
    pub help_start: u16,
    /// `None` when the bar is too narrow to fit Settings as well; Help is the
    /// one that stays, since it is how a lost user finds everything else.
    pub settings_start: Option<u16>,
}

impl TopBarButtons {
    /// The leftmost column any button occupies -- where the title has to stop.
    pub fn left_edge(&self) -> u16 {
        self.settings_start.unwrap_or(self.help_start)
    }
}

pub fn buttons(area: Rect) -> TopBarButtons {
    let help_start = (area.x + area.width).saturating_sub(HELP_BUTTON_WIDTH);
    // Keep a column of title visible rather than letting the buttons take the
    // whole bar; below that the Settings button is dropped entirely.
    let settings_start = help_start
        .checked_sub(SETTINGS_BUTTON_WIDTH)
        .filter(|start| *start > area.x);
    TopBarButtons {
        help_start,
        settings_start,
    }
}

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

        let buttons = buttons(area);

        // Left: app name and version, truncated before the first button
        let left_text = title_text();
        for (x_offset, ch) in (area.x..).zip(left_text.chars()) {
            if x_offset >= buttons.left_edge() {
                break;
            }
            buf[(x_offset, area.y)].set_char(ch).set_style(style);
        }

        // Right: Settings, then Help
        let btn_style = Style::default()
            .fg(Color::Rgb(200, 204, 212))
            .bg(Color::Rgb(62, 68, 81));

        let mut draw_button = |start: u16, text: &str| {
            for (i, ch) in text.chars().enumerate() {
                let x = start + i as u16;
                if x >= area.x && x < area.x + area.width {
                    buf[(x, area.y)].set_char(ch).set_style(btn_style);
                }
            }
        };

        if let Some(start) = buttons.settings_start {
            draw_button(start, SETTINGS_BUTTON_TEXT);
        }
        draw_button(buttons.help_start, HELP_BUTTON_TEXT);
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

    #[test]
    fn top_bar_puts_settings_left_of_help() {
        let row = render_row(80);
        let expected = format!("{SETTINGS_BUTTON_TEXT}{HELP_BUTTON_TEXT}");
        assert!(row.ends_with(&expected), "got: {row:?}");
    }

    #[test]
    fn a_narrow_bar_keeps_help_and_drops_settings() {
        // Room for both buttons would leave no title at all, so Settings goes.
        let row = render_row(20);
        assert!(row.ends_with(HELP_BUTTON_TEXT), "got: {row:?}");
        assert!(!row.contains("Settings"), "got: {row:?}");
        assert_eq!(row.len(), 20);
    }

    #[test]
    fn the_title_stops_before_the_leftmost_button() {
        // 30 columns: both buttons fit (21), leaving 9 for the title, which is
        // shorter than " termcode vX.Y.Z" -- so it must be cut, not overlap.
        let row = render_row(30);
        let buttons = super::buttons(Rect::new(0, 0, 30, 1));
        let start = buttons.settings_start.expect("both buttons should fit");
        let title: String = row.chars().take(start as usize).collect();
        assert!(title.starts_with(" termcode"), "got: {title:?}");
        assert!(
            !title.contains("Settings") && !title.contains('?'),
            "got: {title:?}"
        );
    }

    #[test]
    fn button_positions_stay_inside_the_bar() {
        for width in [1, 8, 9, 20, 21, 22, 40, 200] {
            let area = Rect::new(0, 0, width, 1);
            let buttons = super::buttons(area);
            assert!(buttons.help_start < width.max(1), "width {width}");
            if let Some(start) = buttons.settings_start {
                assert!(start < buttons.help_start, "width {width}");
                assert!(start > area.x, "width {width}");
            }
            // Rendering must not panic at any of these widths either.
            render_row(width);
        }
    }
}
