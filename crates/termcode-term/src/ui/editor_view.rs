use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::Widget;

use crate::display_width::TabStops;
use termcode_core::config_types::{EditorConfig, LineNumberStyle};
use termcode_core::diagnostic::DiagnosticSeverity;
use termcode_theme::theme::Theme;
use termcode_view::document::Document;
use termcode_view::editor::EditorMode;
use termcode_view::search::SearchState;
use termcode_view::view::View;

/// Widget that renders the code editor area with line numbers and syntax highlighting.
pub struct EditorViewWidget<'a> {
    doc: &'a Document,
    view: &'a View,
    theme: &'a Theme,
    mode: EditorMode,
    search: Option<&'a SearchState>,
    /// The whole editor config rather than a `LineNumberStyle`: the widget needs
    /// both `line_numbers` and `tab_size`, and one borrow carries every display
    /// setting the next one will need too.
    config: &'a EditorConfig,
    is_active: bool,
}

/// Blend a color over a background at the given opacity (0.0–1.0).
fn blend_color(
    fg: ratatui::style::Color,
    bg: ratatui::style::Color,
    alpha: f32,
) -> ratatui::style::Color {
    match (fg, bg) {
        (ratatui::style::Color::Rgb(fr, fg_g, fb), ratatui::style::Color::Rgb(br, bg_g, bb)) => {
            ratatui::style::Color::Rgb(
                (fr as f32 * alpha + br as f32 * (1.0 - alpha)) as u8,
                (fg_g as f32 * alpha + bg_g as f32 * (1.0 - alpha)) as u8,
                (fb as f32 * alpha + bb as f32 * (1.0 - alpha)) as u8,
            )
        }
        _ => fg,
    }
}

/// The columns of a character that fall inside the viewport: the one clipping
/// rule the main render loop, the search highlight and the selection highlight
/// all use, so no cell is painted by one of them and left behind by another.
///
/// `col` is where the character starts and `next` is `TabStops::next_col` of
/// it -- measured from **column 0 of the line**, never from `left_col`, so
/// scrolling horizontally cannot move where a tab's stops fall.
///
/// A tab is clipped **per column**: it is measured whole and whichever of its
/// columns are on screen are painted, which is what draws one straddling either
/// edge of the viewport. Every other character is all-or-nothing -- half a CJK
/// glyph is not a glyph, so one straddling an edge is dropped rather than drawn
/// as a stray blank.
fn visible_span(
    col: usize,
    next: usize,
    ch: char,
    left_col: usize,
    visible_end: usize,
) -> std::ops::Range<usize> {
    if ch == '\t' {
        // `Range` is empty rather than panicking when the start passes the end,
        // which is what a tab entirely off either side of the viewport gives.
        col.max(left_col)..next.min(visible_end)
    } else if next > col && col >= left_col && next <= visible_end {
        col..next
    } else {
        col..col
    }
}

impl<'a> EditorViewWidget<'a> {
    pub fn new(
        doc: &'a Document,
        view: &'a View,
        theme: &'a Theme,
        mode: EditorMode,
        search: Option<&'a SearchState>,
        config: &'a EditorConfig,
        is_active: bool,
    ) -> Self {
        Self {
            doc,
            view,
            theme,
            mode,
            search,
            config,
            is_active,
        }
    }
}

impl Widget for EditorViewWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let line_count = self.doc.buffer.line_count();
        let gutter_width = line_number_width_styled(line_count, self.config.line_numbers);
        // Built once for the whole frame: every column this widget paints or
        // measures is a fold over `tabs.next_col`, so there is exactly one
        // place a tab's width is decided.
        let tabs = TabStops::from_config(self.config);
        let top_line = self.view.scroll.top_line;
        let visible_lines = area.height as usize;

        let gutter_style = Style::default().fg(self.theme.ui.line_number.to_ratatui());
        let gutter_active_style =
            Style::default().fg(self.theme.ui.line_number_active.to_ratatui());
        let bg_color = self.theme.ui.background.to_ratatui();
        let separator_style = Style::default().fg(self.theme.ui.border.to_ratatui());

        for y in area.y..area.y + area.height {
            for x in area.x..area.x + area.width {
                buf[(x, y)].set_char(' ').set_bg(bg_color);
            }
        }

        let end_line = (top_line + visible_lines).min(line_count);
        let all_spans = self
            .doc
            .syntax
            .as_ref()
            .map(|s| s.highlight_lines(self.doc.buffer.text(), top_line..end_line))
            .unwrap_or_default();

        for i in 0..visible_lines {
            let line_idx = top_line + i;
            let y = area.y + i as u16;

            if line_idx >= line_count {
                let tilde_x = area.x;
                if tilde_x < area.x + area.width {
                    buf[(tilde_x, y)].set_char('~').set_style(gutter_style);
                }
                continue;
            }

            let is_cursor_line = line_idx == self.view.cursor.line;
            if self.config.line_numbers != LineNumberStyle::None {
                let display_num = match self.config.line_numbers {
                    LineNumberStyle::Absolute => line_idx + 1,
                    LineNumberStyle::Relative => {
                        if is_cursor_line {
                            0
                        } else {
                            let cursor_line = self.view.cursor.line;
                            line_idx.abs_diff(cursor_line)
                        }
                    }
                    LineNumberStyle::RelativeAbsolute => {
                        if is_cursor_line {
                            line_idx + 1
                        } else {
                            let cursor_line = self.view.cursor.line;
                            line_idx.abs_diff(cursor_line)
                        }
                    }
                    LineNumberStyle::None => unreachable!(),
                };
                let line_num = format!("{:>width$}", display_num, width = gutter_width as usize);
                let num_style = if is_cursor_line {
                    gutter_active_style
                } else {
                    gutter_style
                };

                for (j, ch) in line_num.chars().enumerate() {
                    let x = area.x + j as u16;
                    if x < area.x + area.width {
                        buf[(x, y)].set_char(ch).set_style(num_style);
                    }
                }
            }

            let sep_x = area.x + gutter_width;
            if sep_x < area.x + area.width {
                let line_severity = self
                    .doc
                    .diagnostics
                    .iter()
                    .filter(|d| d.range.0.line <= line_idx && d.range.1.line >= line_idx)
                    .map(|d| d.severity)
                    .min_by_key(|s| match s {
                        DiagnosticSeverity::Error => 0,
                        DiagnosticSeverity::Warning => 1,
                        DiagnosticSeverity::Info => 2,
                        DiagnosticSeverity::Hint => 3,
                    });

                match line_severity {
                    Some(DiagnosticSeverity::Error) => {
                        let color = self.theme.ui.error.to_ratatui();
                        buf[(sep_x, y)]
                            .set_char('E')
                            .set_style(Style::default().fg(color));
                    }
                    Some(DiagnosticSeverity::Warning) => {
                        let color = self.theme.ui.warning.to_ratatui();
                        buf[(sep_x, y)]
                            .set_char('W')
                            .set_style(Style::default().fg(color));
                    }
                    Some(DiagnosticSeverity::Info) => {
                        let color = self.theme.ui.info.to_ratatui();
                        buf[(sep_x, y)]
                            .set_char('I')
                            .set_style(Style::default().fg(color));
                    }
                    Some(DiagnosticSeverity::Hint) => {
                        let color = self.theme.ui.hint.to_ratatui();
                        buf[(sep_x, y)]
                            .set_char('H')
                            .set_style(Style::default().fg(color));
                    }
                    None => {
                        buf[(sep_x, y)].set_char(' ').set_style(separator_style);
                    }
                }
            }

            let code_start = area.x + gutter_width + 1;
            let code_width = area.width.saturating_sub(gutter_width + 1);

            let rope_line = self.doc.buffer.line(line_idx);
            let line_text: String = rope_line.chars().collect();
            let line_text = line_text.trim_end_matches('\n').trim_end_matches('\r');

            let spans = if i < all_spans.len() {
                &all_spans[i]
            } else {
                &[] as &[termcode_syntax::highlighter::HighlightSpan]
            };

            let line_bytes = line_text.as_bytes();
            let mut char_styles: Vec<Style> =
                vec![Style::default().fg(self.theme.ui.foreground.to_ratatui()); line_bytes.len()];

            for span in spans {
                let resolved = self.theme.resolve(&span.scope);
                let style = resolved.to_ratatui();
                for cs in &mut char_styles[span.byte_start..span.byte_end.min(line_bytes.len())] {
                    *cs = style;
                }
            }

            if is_cursor_line {
                let raw_cursor_line_bg = self.theme.ui.cursor_line_bg.to_ratatui();
                let editor_bg = self.theme.ui.background.to_ratatui();
                let cursor_line_bg = if self.is_active {
                    raw_cursor_line_bg
                } else {
                    blend_color(raw_cursor_line_bg, editor_bg, 0.2)
                };
                for cs in &mut char_styles[..line_bytes.len()] {
                    *cs = cs.bg(cursor_line_bg);
                }
                for x in code_start..(code_start + code_width) {
                    buf[(x, y)].set_bg(cursor_line_bg);
                }
            }

            let left_col = self.view.scroll.left_col;
            // Display columns are tracked as usize: minified files routinely have
            // lines wider than u16::MAX, which would wrap and index outside the buffer.
            let visible_end = left_col.saturating_add(code_width as usize);
            let mut col = 0usize;
            for (byte_idx, ch) in line_text.char_indices() {
                if col >= visible_end {
                    break;
                }
                // Exactly one place a column advances, whatever the character
                // is: a tab, a CJK glyph and a combining mark are all folded
                // through `next_col`, so the drawn column and every measured
                // one come from the same arithmetic.
                let next = tabs.next_col(col, ch);
                let span = visible_span(col, next, ch, left_col, visible_end);
                for (offset, c) in span.enumerate() {
                    let x = code_start + (c - left_col) as u16;
                    // The character is drawn in its first column; its
                    // remaining cells are blanked so a wide glyph does not
                    // leave the old background showing beside it. A tab is
                    // blank the whole way across -- the terminal never sees the
                    // `\t` itself, only the columns it expanded to.
                    let symbol = if offset == 0 && ch != '\t' { ch } else { ' ' };
                    buf[(x, y)]
                        .set_char(symbol)
                        .set_style(char_styles[byte_idx]);
                }
                col = next;
            }

            if is_cursor_line && self.is_active {
                // The cursor sits at the **first** column of its character --
                // on a tab, the column the loop above starts painting it at.
                // That is what makes this REVERSED cell and the popup anchor
                // `render::cursor_screen_position` returns name the same cell,
                // and what keeps the click round trip idempotent: clicking
                // where the cursor already is leaves it alone. The bounds below
                // are that anchor's too, so where nothing is drawn nothing is
                // anchored either.
                let cursor_display_col = tabs.col_at_char(line_text, self.view.cursor.column);
                if cursor_display_col >= left_col && cursor_display_col < visible_end {
                    let cursor_x = code_start + (cursor_display_col - left_col) as u16;
                    let cell = &mut buf[(cursor_x, y)];
                    // The same filled block in every mode, Insert included: the
                    // status bar already names the mode, and an underline is
                    // easy to lose against the cursor line.
                    cell.set_style(cell.style().add_modifier(Modifier::REVERSED));
                }
            }

            for diag in &self.doc.diagnostics {
                if diag.range.0.line > line_idx || diag.range.1.line < line_idx {
                    continue;
                }
                let diag_color = match diag.severity {
                    DiagnosticSeverity::Error => self.theme.ui.error.to_ratatui(),
                    DiagnosticSeverity::Warning => self.theme.ui.warning.to_ratatui(),
                    DiagnosticSeverity::Info => self.theme.ui.info.to_ratatui(),
                    DiagnosticSeverity::Hint => self.theme.ui.hint.to_ratatui(),
                };
                // Both ends through `col_at_char`, so a diagnostic spanning a
                // tab underlines the tab's whole expansion rather than the zero
                // columns a per-character width gave it.
                let start_col = if diag.range.0.line == line_idx {
                    tabs.col_at_char(line_text, diag.range.0.column)
                } else {
                    0
                };
                let end_col = if diag.range.1.line == line_idx {
                    tabs.col_at_char(line_text, diag.range.1.column)
                } else {
                    // A diagnostic that runs past this line is underlined to
                    // the line's end, capped at the last visible column so a
                    // minified line is not walked to draw it. The cap bounds
                    // characters as well as columns, so on a line whose first
                    // `visible_end` characters carry no width -- combining
                    // marks, and only those now that a tab is measured as the
                    // columns it is drawn in -- this returns short and the
                    // underline stops early or vanishes. Bounding the scan is
                    // worth more than an exact underline on a line of that
                    // shape.
                    tabs.width_capped(line_text, visible_end)
                };
                for c in start_col.max(left_col)..end_col.min(visible_end) {
                    let x = code_start + (c - left_col) as u16;
                    let cell = &mut buf[(x, y)];
                    cell.set_style(
                        cell.style()
                            .fg(diag_color)
                            .add_modifier(Modifier::UNDERLINED),
                    );
                }
            }
        }

        if let Some(search) = self.search {
            if self.mode == EditorMode::Search && !search.matches.is_empty() {
                let match_bg = self.theme.ui.search_match.to_ratatui();
                let active_bg = self.theme.ui.search_match_active.to_ratatui();
                let search_code_start = area.x + gutter_width + 1;
                let search_code_width = area.width.saturating_sub(gutter_width + 1);
                let search_left_col = self.view.scroll.left_col;
                let search_visible_end = search_left_col.saturating_add(search_code_width as usize);

                for i in 0..visible_lines {
                    let line_idx = top_line + i;
                    if line_idx >= line_count {
                        break;
                    }
                    let y = area.y + i as u16;
                    let line_byte_start = self.doc.buffer.text().line_to_byte(line_idx);
                    let line_byte_end = if line_idx + 1 < line_count {
                        self.doc.buffer.text().line_to_byte(line_idx + 1)
                    } else {
                        self.doc.buffer.text().len_bytes()
                    };

                    let rope_line = self.doc.buffer.line(line_idx);
                    let line_text: String = rope_line.chars().collect();
                    let line_text = line_text.trim_end_matches('\n').trim_end_matches('\r');

                    for (mi, m) in search.matches.iter().enumerate() {
                        if m.end <= line_byte_start || m.start >= line_byte_end {
                            continue;
                        }

                        let start_in_line = m.start.max(line_byte_start) - line_byte_start;
                        let end_in_line = m.end.min(line_byte_end) - line_byte_start;
                        let is_active = search.current_match == Some(mi);
                        let bg = if is_active { active_bg } else { match_bg };

                        let mut byte_cursor = 0;
                        let mut display_col = 0usize;
                        for ch in line_text.chars() {
                            let ch_len = ch.len_utf8();
                            if byte_cursor >= end_in_line || display_col >= search_visible_end {
                                break;
                            }
                            // The same fold and the same clipping as the render
                            // loop above: a match spanning a tab is highlighted
                            // across the tab's whole expansion, which measuring
                            // it as zero columns painted as nothing at all.
                            let next = tabs.next_col(display_col, ch);
                            if byte_cursor >= start_in_line {
                                for c in visible_span(
                                    display_col,
                                    next,
                                    ch,
                                    search_left_col,
                                    search_visible_end,
                                ) {
                                    let x = search_code_start + (c - search_left_col) as u16;
                                    buf[(x, y)].set_bg(bg);
                                }
                            }
                            display_col = next;
                            byte_cursor += ch_len;
                        }
                    }
                }
            }
        }

        let primary = self.doc.selection.primary();
        if !primary.is_empty() {
            let sel_from = primary.from();
            let sel_to = primary.to();
            let sel_bg = self.theme.ui.selection.to_ratatui();
            let sel_code_start = area.x + gutter_width + 1;
            let sel_code_width = area.width.saturating_sub(gutter_width + 1);
            let sel_left_col = self.view.scroll.left_col;
            let sel_visible_end = sel_left_col.saturating_add(sel_code_width as usize);

            for i in 0..visible_lines {
                let line_idx = top_line + i;
                if line_idx >= line_count {
                    break;
                }
                let y = area.y + i as u16;
                let line_byte_start = self.doc.buffer.text().line_to_byte(line_idx);
                let line_byte_end = if line_idx + 1 < line_count {
                    self.doc.buffer.text().line_to_byte(line_idx + 1)
                } else {
                    self.doc.buffer.text().len_bytes()
                };

                if line_byte_end <= sel_from || line_byte_start >= sel_to {
                    continue;
                }

                let start_in_line = sel_from.max(line_byte_start) - line_byte_start;
                let end_in_line = sel_to.min(line_byte_end) - line_byte_start;

                let rope_line = self.doc.buffer.line(line_idx);
                let line_text: String = rope_line.chars().collect();
                let line_text = line_text.trim_end_matches('\n').trim_end_matches('\r');

                let mut byte_cursor = 0;
                let mut display_col = 0usize;
                for ch in line_text.chars() {
                    let ch_len = ch.len_utf8();
                    if byte_cursor >= end_in_line || display_col >= sel_visible_end {
                        break;
                    }
                    // As in the render loop and the search highlight: a
                    // selection over a tab-indented line covers the indent,
                    // which a zero-column tab left blank.
                    let next = tabs.next_col(display_col, ch);
                    if byte_cursor >= start_in_line {
                        for c in visible_span(display_col, next, ch, sel_left_col, sel_visible_end)
                        {
                            let x = sel_code_start + (c - sel_left_col) as u16;
                            buf[(x, y)].set_bg(sel_bg);
                        }
                    }
                    display_col = next;
                    byte_cursor += ch_len;
                }
            }
        }
    }
}

pub fn line_number_width_styled(line_count: usize, style: LineNumberStyle) -> u16 {
    if style == LineNumberStyle::None {
        return 0;
    }
    let digits = if line_count == 0 {
        1
    } else {
        (line_count as f64).log10().floor() as u16 + 1
    };
    digits.max(3) // minimum 3 chars wide
}

#[cfg(test)]
mod tests {
    use super::*;

    // The capped-scan tests moved with the scan itself, onto `TabStops` in
    // `crate::display_width`.

    #[test]
    fn line_number_width_none_returns_zero() {
        assert_eq!(line_number_width_styled(100, LineNumberStyle::None), 0);
    }

    #[test]
    fn line_number_width_absolute_minimum_three() {
        assert_eq!(line_number_width_styled(1, LineNumberStyle::Absolute), 3);
        assert_eq!(line_number_width_styled(99, LineNumberStyle::Absolute), 3);
    }

    #[test]
    fn line_number_width_grows_with_lines() {
        assert_eq!(line_number_width_styled(1000, LineNumberStyle::Absolute), 4);
        assert_eq!(
            line_number_width_styled(10000, LineNumberStyle::Absolute),
            5
        );
    }

    #[test]
    fn line_number_width_relative_uses_same_logic() {
        assert_eq!(line_number_width_styled(100, LineNumberStyle::Relative), 3);
    }
}
