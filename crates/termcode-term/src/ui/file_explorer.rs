use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::Widget;

use termcode_theme::theme::Theme;
use termcode_view::file_explorer::{FileExplorer, FileNodeKind};

pub struct FileExplorerWidget<'a> {
    explorer: &'a FileExplorer,
    theme: &'a Theme,
}

impl<'a> FileExplorerWidget<'a> {
    pub fn new(explorer: &'a FileExplorer, theme: &'a Theme) -> Self {
        Self { explorer, theme }
    }
}

impl Widget for FileExplorerWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let bg = self.theme.ui.sidebar_bg.to_ratatui();
        let fg = self.theme.ui.sidebar_fg.to_ratatui();
        let sel_bg = self.theme.ui.selection.to_ratatui();
        let normal_style = Style::default().fg(fg).bg(bg);

        // Fill background
        for y in area.y..area.y + area.height {
            for x in area.x..area.x + area.width {
                buf[(x, y)].set_char(' ').set_style(normal_style);
            }
        }

        let nodes = self.explorer.flatten_visible();
        let offset = self.explorer.scroll_offset;

        for (vi, node) in nodes.iter().enumerate().skip(offset) {
            let row = (vi - offset) as u16;
            let y = area.y + row;
            if y >= area.y + area.height {
                break;
            }

            let style = if vi == self.explorer.selected {
                Style::default().fg(fg).bg(sel_bg)
            } else {
                normal_style
            };

            // Fill row background for selected item
            if vi == self.explorer.selected {
                for x in area.x..area.x + area.width {
                    buf[(x, y)].set_char(' ').set_style(style);
                }
            }

            // Indent
            let indent = node.depth * 2;
            let mut x = area.x + indent as u16;

            // Directory/file icon
            let icon = match node.kind {
                FileNodeKind::Directory if node.expanded => "v ",
                FileNodeKind::Directory => "> ",
                _ => "  ",
            };

            for ch in icon.chars() {
                if x < area.x + area.width {
                    buf[(x, y)].set_char(ch).set_style(style);
                    x += 1;
                }
            }

            // Name (clipped to sidebar width)
            for ch in node.name.chars() {
                if x >= area.x + area.width {
                    break;
                }
                buf[(x, y)].set_char(ch).set_style(style);
                x += 1;
            }
        }
    }
}
