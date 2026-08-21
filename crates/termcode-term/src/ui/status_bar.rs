use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::Widget;

use termcode_core::diagnostic::DiagnosticSeverity;
use termcode_theme::theme::Theme;
use termcode_view::document::Document;
use termcode_view::editor::EditorMode;
use termcode_view::image::ImageEntry;
use termcode_view::view::View;

/// Status bar widget displaying cursor position, file info, etc.
pub struct StatusBarWidget<'a> {
    doc: Option<&'a Document>,
    view: Option<&'a View>,
    theme: &'a Theme,
    status_message: Option<&'a str>,
    mode: EditorMode,
    image: Option<&'a ImageEntry>,
    pending_keys: &'a str,
}

impl<'a> StatusBarWidget<'a> {
    pub fn new(
        doc: Option<&'a Document>,
        view: Option<&'a View>,
        theme: &'a Theme,
        status_message: Option<&'a str>,
        mode: EditorMode,
        image: Option<&'a ImageEntry>,
        pending_keys: &'a str,
    ) -> Self {
        Self {
            doc,
            view,
            theme,
            status_message,
            mode,
            image,
            pending_keys,
        }
    }
}

impl Widget for StatusBarWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let bg = self.theme.ui.status_bar_bg.to_ratatui();
        let fg = self.theme.ui.status_bar_fg.to_ratatui();
        let style = Style::default().fg(fg).bg(bg);

        // Fill background
        for x in area.x..area.x + area.width {
            buf[(x, area.y)].set_char(' ').set_style(style);
        }

        // Mode indicator
        let (mode_label, mode_bg) = match self.mode {
            EditorMode::Normal => (" NORMAL ", Color::Rgb(97, 175, 239)),
            EditorMode::Insert => (" EDIT ", Color::Rgb(152, 195, 121)),
            EditorMode::FileExplorer => (" EXPLORER ", Color::Rgb(209, 154, 102)),
            EditorMode::Search => (" SEARCH ", Color::Rgb(229, 192, 123)),
            EditorMode::FuzzyFinder => (" FINDER ", Color::Rgb(198, 120, 221)),
            EditorMode::CommandPalette => (" COMMAND ", Color::Rgb(198, 120, 221)),
            EditorMode::Settings => (" SETTINGS ", Color::Rgb(86, 182, 194)),
        };
        let mode_style = Style::default().fg(Color::Rgb(40, 44, 52)).bg(mode_bg);

        let mut x_offset = area.x;
        for ch in mode_label.chars() {
            if x_offset < area.x + area.width {
                buf[(x_offset, area.y)].set_char(ch).set_style(mode_style);
            }
            x_offset += 1;
        }

        // Separator after mode
        if x_offset < area.x + area.width {
            buf[(x_offset, area.y)].set_char(' ').set_style(style);
            x_offset += 1;
        }

        // Diagnostic counts
        if let Some(doc) = self.doc {
            let errors = doc
                .diagnostics
                .iter()
                .filter(|d| d.severity == DiagnosticSeverity::Error)
                .count();
            let warnings = doc
                .diagnostics
                .iter()
                .filter(|d| d.severity == DiagnosticSeverity::Warning)
                .count();

            if errors > 0 {
                let error_style = Style::default().fg(self.theme.ui.error.to_ratatui()).bg(bg);
                let text = format!("E:{errors} ");
                for ch in text.chars() {
                    if x_offset < area.x + area.width {
                        buf[(x_offset, area.y)].set_char(ch).set_style(error_style);
                    }
                    x_offset += 1;
                }
            }
            if warnings > 0 {
                let warn_style = Style::default()
                    .fg(self.theme.ui.warning.to_ratatui())
                    .bg(bg);
                let text = format!("W:{warnings} ");
                for ch in text.chars() {
                    if x_offset < area.x + area.width {
                        buf[(x_offset, area.y)].set_char(ch).set_style(warn_style);
                    }
                    x_offset += 1;
                }
            }
        }

        // Left side: status message or file name
        let left_text = if let Some(msg) = self.status_message {
            msg.to_string()
        } else if let Some(doc) = self.doc {
            let modified = if doc.is_modified() { " [+]" } else { "" };
            let name = doc.display_name();
            format!("{name}{modified}")
        } else {
            "termcode".to_string()
        };

        // A file name may be CJK, so advance by columns rather than by
        // characters and blank the cell a wide glyph covers -- ratatui's diff
        // skips that cell, so a character written into it never appears.
        for ch in left_text.chars() {
            let w = crate::display_width::ui_char_width(ch) as u16;
            if w == 0 {
                continue;
            }
            if x_offset + w > area.x + area.width {
                break;
            }
            buf[(x_offset, area.y)].set_char(ch).set_style(style);
            for i in 1..w {
                buf[(x_offset + i, area.y)].set_char(' ').set_style(style);
            }
            x_offset += w;
        }

        // Right side: cursor position, encoding, language (or image info)
        let right_text = if let Some(img) = self.image {
            let size = format_file_size(img.file_size);
            let dims = img
                .dimensions
                .map(|(w, h)| format!("{w} x {h} px  "))
                .unwrap_or_default();
            format!("{}{}  {} ", dims, img.format.to_uppercase(), size)
        } else if let (Some(doc), Some(view)) = (self.doc, self.view) {
            let line = view.cursor.line + 1;
            let col = view.cursor.column + 1;
            let encoding = doc.buffer.encoding();
            let lang = doc
                .language_id
                .as_ref()
                .map(|id| id.as_ref())
                .unwrap_or("text");
            let total_lines = doc.buffer.line_count();
            format!("Ln {line}, Col {col}  {encoding}  {lang}  {total_lines}L ")
        } else {
            String::new()
        };

        if !right_text.is_empty() {
            let right_start = (area.x + area.width).saturating_sub(right_text.len() as u16);
            for (i, ch) in right_text.chars().enumerate() {
                let x = right_start + i as u16;
                if x >= area.x && x < area.x + area.width {
                    buf[(x, area.y)].set_char(ch).set_style(style);
                }
            }
        }

        // Pending chord, drawn left of the cursor info so a half-typed sequence
        // like `g` does not look like an ignored key press.
        if !self.pending_keys.is_empty() {
            let hint = format!(" {} ", self.pending_keys);
            // A keymap may bind non-ASCII keys, so measure in columns rather
            // than bytes or the hint drifts out of place.
            let hint_width = crate::display_width::ui_str_width(&hint) as u16;
            let hint_style = Style::default()
                .fg(self.theme.ui.status_bar_bg.to_ratatui())
                .bg(self.theme.ui.warning.to_ratatui());
            let right_edge = (area.x + area.width).saturating_sub(right_text.len() as u16);
            let hint_start = right_edge.saturating_sub(hint_width + 1);
            let mut x = hint_start;
            for ch in hint.chars() {
                if x >= x_offset && x < right_edge && x < area.x + area.width {
                    buf[(x, area.y)].set_char(ch).set_style(hint_style);
                }
                x += crate::display_width::ui_char_width(ch) as u16;
            }
        }
    }
}

fn format_file_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use unicode_width::UnicodeWidthStr;

    /// Read a rendered row the way the terminal does: the cell a wide glyph
    /// covers is skipped by ratatui's diff, whatever it holds.
    fn row_of(buf: &Buffer, width: u16) -> String {
        let mut row = String::new();
        let mut covered = 0usize;
        for x in 0..width {
            if covered > 0 {
                covered -= 1;
                continue;
            }
            let symbol = buf[(x, 0)].symbol();
            covered = symbol.width().saturating_sub(1);
            row.push_str(symbol);
        }
        row
    }

    #[test]
    fn a_cjk_status_message_keeps_all_its_characters() {
        let theme = Theme::default();
        let area = Rect::new(0, 0, 40, 1);
        let mut buf = Buffer::empty(area);
        StatusBarWidget::new(
            None,
            None,
            &theme,
            Some("한글폴더명"),
            EditorMode::Normal,
            None,
            "",
        )
        .render(area, &mut buf);

        let row = row_of(&buf, area.width);
        assert!(row.contains("한글폴더명"), "got: {row:?}");
        assert_eq!(row.width(), 40, "got: {row:?}");
    }
}
