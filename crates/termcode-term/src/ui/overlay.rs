use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

use termcode_theme::theme::Theme;

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

pub fn render_overlay_frame(area: Rect, buf: &mut Buffer, theme: &Theme) {
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

    // Calculate text scroll offset if text is too long
    let available_width = (max_x.saturating_sub(x)) as usize;
    let effective_cursor = cursor_pos.unwrap_or(0);
    let text_offset =
        if cursor_pos.is_some() && effective_cursor > available_width.saturating_sub(1) {
            effective_cursor - available_width.saturating_sub(1)
        } else {
            0
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
        let ch_width = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0) as u16;
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
    if let Some(cpos) = cursor_pos {
        let cursor_x = input_start_x + (cpos - text_offset) as u16;
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
            let ch_width = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0) as u16;
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
                let ch_width = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0) as u16;
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
