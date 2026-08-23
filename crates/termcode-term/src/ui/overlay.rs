use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

use termcode_theme::theme::Theme;

use crate::display_width::{ui_char_width, ui_col_at_char};

#[derive(Debug, Clone, Copy)]
pub enum OverlayPosition {
    Top,
    Center,
}

#[derive(Debug, Clone)]
pub struct OverlayConfig {
    pub width_percent: u16,
    pub max_height: u16,
    pub position: OverlayPosition,
}

pub fn compute_overlay_rect(parent: Rect, config: &OverlayConfig) -> Rect {
    match config.position {
        OverlayPosition::Top => {
            let height = config.max_height.min(parent.height);
            Rect::new(parent.x, parent.y, parent.width, height)
        }
        OverlayPosition::Center => {
            let width = (parent.width as u32 * config.width_percent as u32 / 100) as u16;
            let width = width.min(parent.width);
            let height = config.max_height.min(parent.height);
            let x = parent.x + (parent.width.saturating_sub(width)) / 2;
            let y = parent.y + (parent.height.saturating_sub(height)) / 2;
            Rect::new(x, y, width, height)
        }
    }
}

/// Columns and rows the shadow is offset by. Two to one, because a terminal
/// cell is about twice as tall as it is wide -- an even offset reads as a
/// shadow thrown from off to the side rather than from above.
const SHADOW_OFFSET_X: u16 = 2;
const SHADOW_OFFSET_Y: u16 = 1;

/// How much of what is underneath survives, in percent. Enough to lift the
/// popup off the page without blacking out the text it falls across.
const SHADOW_LEVEL: u16 = 50;

fn shade_channel(value: u8) -> u8 {
    (value as u16 * SHADOW_LEVEL / 100) as u8
}

/// Darken one colour toward black. Indexed and terminal-default colours carry
/// no channels to scale, so they take the theme's own shade instead of being
/// left at full brightness.
fn shade(color: ratatui::style::Color, fallback: ratatui::style::Color) -> ratatui::style::Color {
    match color {
        ratatui::style::Color::Rgb(r, g, b) => {
            ratatui::style::Color::Rgb(shade_channel(r), shade_channel(g), shade_channel(b))
        }
        _ => fallback,
    }
}

/// Cast a shadow from `area` onto whatever is drawn behind it.
///
/// The band is the popup's own rectangle shifted down and to the right, minus
/// the popup itself. The cells there keep their contents and are only dimmed,
/// so the shadow falls *across* the editor rather than punching a hole in it.
///
/// Call this before drawing the popup: it writes only outside `area`, but the
/// text it dims has to already be there.
pub fn render_shadow(area: Rect, buf: &mut Buffer, theme: &Theme) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let fallback = shade(
        theme.ui.background.to_ratatui(),
        ratatui::style::Color::Black,
    );
    let clip = *buf.area();

    for y in area.y + SHADOW_OFFSET_Y..area.bottom() + SHADOW_OFFSET_Y {
        for x in area.x + SHADOW_OFFSET_X..area.right() + SHADOW_OFFSET_X {
            // The popup covers its own shadow.
            if x < area.right() && y < area.bottom() {
                continue;
            }
            if x < clip.x || y < clip.y || x >= clip.right() || y >= clip.bottom() {
                continue;
            }
            let cell = &mut buf[(x, y)];
            let fg = shade(cell.fg, fallback);
            let bg = shade(cell.bg, fallback);
            cell.set_fg(fg).set_bg(bg);
        }
    }
}

pub fn render_overlay_frame(area: Rect, buf: &mut Buffer, theme: &Theme) {
    render_shadow(area, buf, theme);

    let bg = theme.ui.sidebar_bg.to_ratatui();
    let border_color = theme.ui.border.to_ratatui();
    let border_style = Style::default().fg(border_color).bg(bg);

    // Fill background (reset to clear inherited modifiers like REVERSED cursor)
    for y in area.y..area.y + area.height {
        for x in area.x..area.x + area.width {
            buf[(x, y)].reset();
            buf[(x, y)].set_char(' ').set_bg(bg);
        }
    }

    // Top border
    if area.height > 0 {
        let y = area.y;
        if area.width >= 2 {
            buf[(area.x, y)]
                .set_char('\u{250c}')
                .set_style(border_style);
            buf[(area.x + area.width - 1, y)]
                .set_char('\u{2510}')
                .set_style(border_style);
            for x in (area.x + 1)..(area.x + area.width - 1) {
                buf[(x, y)].set_char('\u{2500}').set_style(border_style);
            }
        }
    }

    // Bottom border
    if area.height > 1 {
        let y = area.y + area.height - 1;
        if area.width >= 2 {
            buf[(area.x, y)]
                .set_char('\u{2514}')
                .set_style(border_style);
            buf[(area.x + area.width - 1, y)]
                .set_char('\u{2518}')
                .set_style(border_style);
            for x in (area.x + 1)..(area.x + area.width - 1) {
                buf[(x, y)].set_char('\u{2500}').set_style(border_style);
            }
        }
    }

    // Side borders
    for y in (area.y + 1)..(area.y + area.height.saturating_sub(1)) {
        if area.width >= 2 {
            buf[(area.x, y)]
                .set_char('\u{2502}')
                .set_style(border_style);
            buf[(area.x + area.width - 1, y)]
                .set_char('\u{2502}')
                .set_style(border_style);
        }
    }
}

pub fn render_input_line(
    area: Rect,
    buf: &mut Buffer,
    prompt: &str,
    text: &str,
    cursor_pos: Option<usize>,
    theme: &Theme,
) {
    if area.width < 4 || area.height == 0 {
        return;
    }

    let fg = theme.ui.foreground.to_ratatui();
    let prompt_fg = theme.ui.info.to_ratatui();
    let bg = theme.ui.sidebar_bg.to_ratatui();
    let style = Style::default().fg(fg).bg(bg);
    let prompt_style = Style::default().fg(prompt_fg).bg(bg);

    let mut x = area.x;
    let max_x = area.x + area.width;

    // Render prompt
    for ch in prompt.chars() {
        if x >= max_x {
            break;
        }
        buf[(x, area.y)].set_char(ch).set_style(prompt_style);
        x += 1;
    }

    // Scroll the text in display columns, not in characters: `cursor_pos` is a
    // character index, and a CJK character is two columns wide, so counting
    // characters would leave the cursor sitting inside the text it follows.
    let available_width = (max_x.saturating_sub(x)) as usize;
    let cursor_col = cursor_pos.map(|c| ui_col_at_char(text, c));
    // The first character drawn, and the column it starts at. A wide character
    // straddling the scroll point is skipped whole rather than drawn as half.
    let (text_offset, offset_col) = match cursor_col {
        Some(col) if col > available_width.saturating_sub(1) => {
            let skip = col - available_width.saturating_sub(1);
            let mut idx = 0usize;
            let mut start = 0usize;
            for ch in text.chars() {
                if start >= skip {
                    break;
                }
                start += ui_char_width(ch);
                idx += 1;
            }
            (idx, start)
        }
        _ => (0, 0),
    };

    // Render text
    let input_start_x = x;
    for (i, ch) in text.chars().enumerate() {
        if i < text_offset {
            continue;
        }
        if x >= max_x {
            break;
        }
        let ch_width = ui_char_width(ch) as u16;
        if x + ch_width > max_x {
            break;
        }
        buf[(x, area.y)].set_char(ch).set_style(style);
        for offset in 1..ch_width {
            buf[(x + offset, area.y)].set_char(' ').set_style(style);
        }
        x += ch_width;
    }

    // Render cursor only when focused (Some)
    if let Some(col) = cursor_col {
        let cursor_x = input_start_x + col.saturating_sub(offset_col) as u16;
        if cursor_x < max_x {
            let cell = &mut buf[(cursor_x, area.y)];
            cell.set_style(Style::default().fg(theme.ui.background.to_ratatui()).bg(fg));
        }
    }
}

#[derive(Debug)]
pub struct ListItem {
    pub text: String,
    pub secondary: Option<String>,
    pub highlights: Vec<usize>,
}

pub fn render_result_list(
    area: Rect,
    buf: &mut Buffer,
    items: &[ListItem],
    selected: usize,
    scroll_offset: usize,
    theme: &Theme,
) {
    if area.width < 2 || area.height == 0 {
        return;
    }

    let fg = theme.ui.foreground.to_ratatui();
    let bg = theme.ui.sidebar_bg.to_ratatui();
    let sel_bg = theme.ui.selection.to_ratatui();
    let highlight_fg = theme.ui.info.to_ratatui();
    let dim_fg = theme.ui.line_number.to_ratatui();

    for (row, item_idx) in (scroll_offset..).enumerate() {
        if row >= area.height as usize {
            break;
        }
        if item_idx >= items.len() {
            break;
        }

        let item = &items[item_idx];
        let y = area.y + row as u16;
        let is_selected = item_idx == selected;
        let row_bg = if is_selected { sel_bg } else { bg };

        // Fill row background
        for x in area.x..area.x + area.width {
            buf[(x, y)].set_char(' ').set_bg(row_bg);
        }

        // Render primary text with character highlights
        let mut x = area.x + 1;
        let max_x = area.x + area.width - 1;

        for (ci, ch) in item.text.chars().enumerate() {
            let ch_width = ui_char_width(ch) as u16;
            if x + ch_width > max_x {
                break;
            }
            let char_fg = if item.highlights.contains(&ci) {
                highlight_fg
            } else {
                fg
            };
            let ch_style = Style::default().fg(char_fg).bg(row_bg);
            buf[(x, y)].set_char(ch).set_style(ch_style);
            for offset in 1..ch_width {
                buf[(x + offset, y)].set_char(' ').set_style(ch_style);
            }
            x += ch_width;
        }

        // Render secondary text (dimmer)
        if let Some(ref sec) = item.secondary {
            x += 1;
            for ch in sec.chars() {
                let ch_width = ui_char_width(ch) as u16;
                if x + ch_width > max_x {
                    break;
                }
                let dim_style = Style::default().fg(dim_fg).bg(row_bg);
                buf[(x, y)].set_char(ch).set_style(dim_style);
                for offset in 1..ch_width {
                    buf[(x + offset, y)].set_char(' ').set_style(dim_style);
                }
                x += ch_width;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Color as RatColor;

    fn buffer_with_a_lit_background(w: u16, h: u16) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, w, h));
        for y in 0..h {
            for x in 0..w {
                buf[(x, y)]
                    .set_char('x')
                    .set_fg(RatColor::Rgb(200, 200, 200))
                    .set_bg(RatColor::Rgb(40, 44, 52));
            }
        }
        buf
    }

    fn is_shadowed(buf: &Buffer, x: u16, y: u16) -> bool {
        buf[(x, y)].bg == RatColor::Rgb(20, 22, 26)
    }

    #[test]
    fn the_shadow_falls_below_and_right_of_the_popup_and_nowhere_else() {
        let mut buf = buffer_with_a_lit_background(40, 20);
        let popup = Rect::new(10, 5, 20, 8);
        render_shadow(popup, &mut buf, &Theme::default());

        // Down the right, starting one row below the popup's top.
        assert!(is_shadowed(&buf, 30, 6) && is_shadowed(&buf, 31, 6));
        assert!(is_shadowed(&buf, 30, 12) && is_shadowed(&buf, 31, 12));
        // Along the bottom, starting two columns right of its left edge.
        assert!(is_shadowed(&buf, 12, 13) && is_shadowed(&buf, 31, 13));

        // Not on the popup itself -- it covers its own shadow.
        assert!(!is_shadowed(&buf, 20, 8));
        // Not above it, not left of it, and not past the band.
        assert!(!is_shadowed(&buf, 30, 5), "the corner the offset skips");
        assert!(
            !is_shadowed(&buf, 11, 13),
            "left of where the bottom starts"
        );
        assert!(!is_shadowed(&buf, 32, 8), "past the two columns");
        assert!(!is_shadowed(&buf, 20, 14), "below the one row");
    }

    #[test]
    fn the_shadow_dims_what_is_there_rather_than_covering_it() {
        let mut buf = buffer_with_a_lit_background(40, 20);
        render_shadow(Rect::new(10, 5, 20, 8), &mut buf, &Theme::default());

        let cell = &buf[(30, 6)];
        assert_eq!(cell.symbol(), "x", "the text behind survives");
        assert_eq!(cell.fg, RatColor::Rgb(100, 100, 100), "dimmed with it");
    }

    #[test]
    fn a_shadow_running_off_the_screen_is_clipped_not_panicking() {
        let mut buf = buffer_with_a_lit_background(20, 10);
        // Flush with the bottom-right corner: the whole band is off-screen.
        render_shadow(Rect::new(10, 5, 10, 5), &mut buf, &Theme::default());
        assert!(!is_shadowed(&buf, 19, 9));

        // And a popup larger than the buffer it is asked to shade.
        render_shadow(Rect::new(0, 0, 100, 100), &mut buf, &Theme::default());
    }

    #[test]
    fn colours_with_no_channels_to_scale_fall_back_to_the_themes_shade() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 10));
        buf[(11, 5)].set_bg(RatColor::Indexed(4));
        render_shadow(Rect::new(0, 0, 10, 5), &mut buf, &Theme::default());

        // The theme's own background, shaded -- not left at full brightness.
        assert_eq!(buf[(11, 5)].bg, RatColor::Rgb(20, 22, 26));
    }

    /// The column the cursor cell sits at, found by its reversed colours.
    fn cursor_column(buf: &Buffer, width: u16, theme: &Theme) -> Option<u16> {
        let cursor_bg = theme.ui.foreground.to_ratatui();
        (0..width).find(|x| buf[(*x, 0)].bg == cursor_bg)
    }

    #[test]
    fn the_cursor_follows_cjk_text_by_columns_not_by_characters() {
        let theme = Theme::default();
        let area = Rect::new(0, 0, 30, 1);
        let mut buf = Buffer::empty(area);
        // Three Hangul syllables: six columns after the eight-column prompt.
        render_input_line(area, &mut buf, "Search: ", "한글값", Some(3), &theme);
        assert_eq!(cursor_column(&buf, area.width, &theme), Some(8 + 6));

        // And mid-string, after the first syllable.
        let mut buf = Buffer::empty(area);
        render_input_line(area, &mut buf, "Search: ", "한글값", Some(1), &theme);
        assert_eq!(cursor_column(&buf, area.width, &theme), Some(8 + 2));
    }

    #[test]
    fn cjk_text_wider_than_the_line_scrolls_to_keep_the_cursor_in_view() {
        let theme = Theme::default();
        // Eight columns of prompt and six of text leave the cursor at the edge.
        let area = Rect::new(0, 0, 15, 1);
        let mut buf = Buffer::empty(area);
        render_input_line(area, &mut buf, "Search: ", "한글값어치", Some(5), &theme);

        let cursor = cursor_column(&buf, area.width, &theme).expect("a cursor");
        assert!(
            cursor < area.width,
            "the cursor stays on the line: {cursor}"
        );
        // Whole syllables only -- no half of a wide character at the seam.
        assert_eq!(buf[(8, 0)].symbol(), "값");
    }
}
