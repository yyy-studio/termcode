use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::Widget;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use termcode_theme::theme::Theme;
use termcode_view::file_explorer::FileExplorer;

/// A button in the file explorer header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolbarAction {
    NewFile,
    NewFolder,
    Refresh,
    CopyPath,
}

impl ToolbarAction {
    /// Drawing order, left to right.
    pub const ALL: [ToolbarAction; 4] = [
        ToolbarAction::NewFile,
        ToolbarAction::NewFolder,
        ToolbarAction::Refresh,
        ToolbarAction::CopyPath,
    ];

    /// The explorer command the button runs, without the `explorer.` prefix.
    pub fn command(self) -> &'static str {
        match self {
            ToolbarAction::NewFile => "new_file",
            ToolbarAction::NewFolder => "new_folder",
            ToolbarAction::Refresh => "refresh_all",
            ToolbarAction::CopyPath => "copy_path",
        }
    }
}

/// The glyphs the buttons are drawn with, resolved once so that the widget and
/// the mouse handler measure the same columns.
///
/// The emoji are all East Asian Wide -- two columns everywhere -- unlike the
/// more obvious symbol glyphs (⚙ ⟳ ⎘), which are Ambiguous and would shift the
/// header from one terminal to the next. The ASCII fallback exists because a
/// terminal that cannot draw the file tree's icons cannot draw these either.
pub struct ToolbarLabels {
    new_file: String,
    new_folder: String,
    refresh: String,
    copy_path: String,
}

impl ToolbarLabels {
    /// `use_emoji` is the file tree's own `show_file_type_emoji` setting: the
    /// header follows the tree it sits on rather than adding a second switch.
    pub fn resolve(theme: &Theme, use_emoji: bool) -> Self {
        if !use_emoji {
            return Self {
                new_file: " +F ".to_string(),
                new_folder: " +D ".to_string(),
                refresh: " R ".to_string(),
                copy_path: " C ".to_string(),
            };
        }
        let icons = &theme.icons;
        Self {
            new_file: format!("{} ", icons.new_file),
            new_folder: format!("{} ", icons.new_folder),
            refresh: format!("{} ", icons.refresh),
            copy_path: format!("{} ", icons.copy_path),
        }
    }

    pub fn label(&self, action: ToolbarAction) -> &str {
        match action {
            ToolbarAction::NewFile => &self.new_file,
            ToolbarAction::NewFolder => &self.new_folder,
            ToolbarAction::Refresh => &self.refresh,
            ToolbarAction::CopyPath => &self.copy_path,
        }
    }

    fn width(&self, action: ToolbarAction) -> u16 {
        self.label(action).width() as u16
    }
}

/// Columns kept for the project name, so a narrow sidebar still says which
/// directory is open.
const MIN_TITLE_WIDTH: u16 = 4;

/// Where each button sits, so the widget and the mouse handler cannot disagree
/// about what was clicked. Buttons are dropped from the left as room runs out.
pub fn buttons(area: Rect, labels: &ToolbarLabels) -> Vec<(ToolbarAction, u16)> {
    let mut actions: &[ToolbarAction] = &ToolbarAction::ALL;
    while !actions.is_empty() {
        let total: u16 = actions.iter().map(|a| labels.width(*a)).sum();
        if total + MIN_TITLE_WIDTH <= area.width {
            let mut x = area.x + area.width - total;
            return actions
                .iter()
                .map(|action| {
                    let start = x;
                    x += labels.width(*action);
                    (*action, start)
                })
                .collect();
        }
        actions = &actions[1..];
    }
    Vec::new()
}

/// The button at column `x`, if any.
pub fn action_at(area: Rect, labels: &ToolbarLabels, x: u16) -> Option<ToolbarAction> {
    buttons(area, labels)
        .into_iter()
        .find(|(action, start)| x >= *start && x < *start + labels.width(*action))
        .map(|(action, _)| action)
}

pub struct ExplorerToolbarWidget<'a> {
    explorer: &'a FileExplorer,
    theme: &'a Theme,
    focused: bool,
    labels: ToolbarLabels,
}

impl<'a> ExplorerToolbarWidget<'a> {
    pub fn new(
        explorer: &'a FileExplorer,
        theme: &'a Theme,
        focused: bool,
        use_emoji: bool,
    ) -> Self {
        Self {
            explorer,
            theme,
            focused,
            labels: ToolbarLabels::resolve(theme, use_emoji),
        }
    }

    /// The project name shown at the left, as the root directory is called.
    fn title(&self) -> String {
        let root = &self.explorer.root;
        let name = root
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .or_else(|| {
                std::fs::canonicalize(root)
                    .ok()
                    .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
            })
            .unwrap_or_else(|| "EXPLORER".to_string());
        format!(" {}", name.to_uppercase())
    }
}

impl Widget for ExplorerToolbarWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        let (fg, bg) = if self.focused {
            (
                self.theme.ui.foreground.to_ratatui(),
                self.theme.ui.tab_active_bg.to_ratatui(),
            )
        } else {
            (
                self.theme.ui.pane_inactive_fg.to_ratatui(),
                self.theme.ui.pane_inactive_bg.to_ratatui(),
            )
        };
        let style = Style::default().fg(fg).bg(bg);
        let button_fg = if self.focused {
            self.theme.ui.info.to_ratatui()
        } else {
            self.theme.ui.pane_inactive_fg.to_ratatui()
        };
        let button_style = Style::default().fg(button_fg).bg(bg);

        for x in area.x..area.x + area.width {
            buf[(x, area.y)].set_char(' ').set_style(style);
        }

        let buttons = buttons(area, &self.labels);
        // One blank column before the first button, so a long project name
        // does not run straight into it.
        let title_end = buttons
            .first()
            .map(|(_, start)| start.saturating_sub(1))
            .unwrap_or(area.x + area.width);

        // The project name is whatever the directory is called, so it may be
        // CJK: step by the glyph's columns and blank the cell a wide one
        // covers, as the buttons below do. A column per character would put
        // the next character in a cell ratatui's diff skips, dropping every
        // second one -- "한글폴더명" came out as "한폴명".
        let mut x = area.x;
        for ch in self.title().chars() {
            let w = ch.width().unwrap_or(0) as u16;
            if w == 0 || x + w > title_end {
                break;
            }
            buf[(x, area.y)].set_char(ch).set_style(style);
            for i in 1..w {
                buf[(x + i, area.y)].set_char(' ').set_style(style);
            }
            x += w;
        }

        for (action, start) in buttons {
            let mut x = start;
            let mut glyph_x: Option<u16> = None;
            for ch in self.labels.label(action).chars() {
                let w = ch.width().unwrap_or(0) as u16;
                if w == 0 {
                    // A variation selector belongs to the glyph before it, not
                    // to a cell of its own.
                    if let Some(gx) = glyph_x {
                        let mut symbol = buf[(gx, area.y)].symbol().to_string();
                        symbol.push(ch);
                        buf[(gx, area.y)].set_symbol(&symbol);
                    }
                    continue;
                }
                if x >= area.x + area.width {
                    break;
                }
                buf[(x, area.y)].set_char(ch).set_style(button_style);
                // A wide glyph owns the cells it covers; leaving them as they
                // were would show half of whatever was drawn underneath.
                for i in 1..w {
                    if x + i < area.x + area.width {
                        buf[(x + i, area.y)].set_char(' ').set_style(button_style);
                    }
                }
                glyph_x = Some(x);
                x += w;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(width: u16) -> Rect {
        Rect::new(0, 0, width, 1)
    }

    fn emoji() -> ToolbarLabels {
        ToolbarLabels::resolve(&Theme::default(), true)
    }

    fn ascii() -> ToolbarLabels {
        ToolbarLabels::resolve(&Theme::default(), false)
    }

    #[test]
    fn all_buttons_fit_a_default_sidebar() {
        for labels in [emoji(), ascii()] {
            let placed = buttons(area(30), &labels);
            assert_eq!(placed.len(), ToolbarAction::ALL.len());
            // Right-aligned, in order, without overlapping.
            let mut expected_x = 30 - placed.iter().map(|(a, _)| labels.width(*a)).sum::<u16>();
            for (action, start) in placed {
                assert_eq!(start, expected_x);
                expected_x += labels.width(action);
            }
            assert_eq!(expected_x, 30);
        }
    }

    #[test]
    fn every_emoji_button_is_two_columns_wide_plus_a_gap() {
        // The whole reason these glyphs were picked: East Asian Wide, so the
        // header lines up the same way in every terminal.
        let labels = emoji();
        for action in ToolbarAction::ALL {
            assert_eq!(labels.width(action), 3, "{action:?}");
        }
    }

    #[test]
    fn a_narrow_sidebar_drops_buttons_from_the_left() {
        let labels = emoji();
        let placed = buttons(area(10), &labels);
        assert!(placed.len() < ToolbarAction::ALL.len(), "{placed:?}");
        // Whatever survives keeps room for the project name.
        assert!(placed.first().map(|(_, x)| *x >= MIN_TITLE_WIDTH).unwrap());
        // Copy is the last button, so it is the one that survives longest.
        assert_eq!(placed.last().unwrap().0, ToolbarAction::CopyPath);
    }

    #[test]
    fn a_sidebar_with_no_room_shows_no_buttons() {
        let labels = emoji();
        assert!(buttons(area(5), &labels).is_empty());
        assert!(buttons(area(0), &labels).is_empty());
    }

    fn render_row(width: u16, root: &str, use_emoji: bool) -> String {
        let theme = Theme::default();
        let mut explorer = FileExplorer::open_with_gitignore(std::path::PathBuf::from(root), false)
            .expect("scratch root");
        explorer.visible = true;
        let a = area(width);
        let mut buf = Buffer::empty(a);
        ExplorerToolbarWidget::new(&explorer, &theme, true, use_emoji).render(a, &mut buf);

        // Read the row the way the terminal does: ratatui's diff skips the
        // cells a wide glyph covers, whatever those cells hold.
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
    fn the_header_shows_the_project_name_and_the_buttons() {
        let dir = std::env::temp_dir().join("termcode-tb").join("acme-app");
        std::fs::create_dir_all(&dir).unwrap();
        let row = render_row(30, dir.to_str().unwrap(), true);
        assert!(row.starts_with(" ACME-APP"), "got: {row:?}");
        assert!(row.ends_with("📄 📁 🔄 📋 "), "got: {row:?}");
        // Each wide glyph owns two cells, the second blanked by the widget.
        assert_eq!(row.width(), 30, "got: {row:?}");

        let ascii_row = render_row(30, dir.to_str().unwrap(), false);
        assert!(ascii_row.ends_with(" +F  +D  R  C "), "got: {ascii_row:?}");
        let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    }

    #[test]
    fn a_cjk_project_name_keeps_all_its_characters() {
        let dir = std::env::temp_dir()
            .join("termcode-tb-cjk")
            .join("한글폴더명");
        std::fs::create_dir_all(&dir).unwrap();
        let row = render_row(30, dir.to_str().unwrap(), true);
        // Read the way the terminal does, every second character used to be
        // written into a cell the diff skips.
        assert!(row.starts_with(" 한글폴더명"), "got: {row:?}");
        assert_eq!(row.width(), 30, "got: {row:?}");
        let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    }

    #[test]
    fn a_long_project_name_stops_before_the_buttons() {
        let dir = std::env::temp_dir().join("termcode-toolbar-a-very-long-project-name");
        std::fs::create_dir_all(&dir).unwrap();
        let row = render_row(24, dir.to_str().unwrap(), true);
        assert!(row.ends_with("📄 📁 🔄 📋 "), "got: {row:?}");
        // The name is cut with a blank column before the first button.
        assert!(row.starts_with(" TERMCODE-T "), "got: {row:?}");
        assert_eq!(row.width(), 24, "got: {row:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn clicks_map_to_the_button_under_them() {
        let a = area(30);
        for labels in [emoji(), ascii()] {
            for (action, start) in buttons(a, &labels) {
                // Including the second cell of a wide glyph, which is part of
                // the same button as far as a click is concerned.
                for offset in 0..labels.width(action) {
                    assert_eq!(action_at(a, &labels, start + offset), Some(action));
                }
            }
            assert_eq!(action_at(a, &labels, 0), None);
        }
    }
}
