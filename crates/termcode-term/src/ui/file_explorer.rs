use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::Widget;

use termcode_core::config_types::FileTreeStyle;
use termcode_theme::theme::Theme;
use termcode_view::file_explorer::{FileExplorer, FileNodeKind};

pub struct FileExplorerWidget<'a> {
    explorer: &'a FileExplorer,
    theme: &'a Theme,
    is_active: bool,
    file_tree_style: FileTreeStyle,
}

impl<'a> FileExplorerWidget<'a> {
    pub fn new(
        explorer: &'a FileExplorer,
        theme: &'a Theme,
        is_active: bool,
        file_tree_style: FileTreeStyle,
    ) -> Self {
        Self {
            explorer,
            theme,
            is_active,
            file_tree_style,
        }
    }
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

/// Check if node at `index` is the last sibling at its depth level.
fn is_last_sibling(nodes: &[termcode_view::file_explorer::FileNode], index: usize) -> bool {
    let depth = nodes[index].depth;
    for node in &nodes[index + 1..] {
        if node.depth < depth {
            return true;
        }
        if node.depth == depth {
            return false;
        }
    }
    true
}

/// Build the tree-line prefix for a node (e.g., "│   ├── ").
/// Returns the prefix string and the set of ancestor depths that have continuing lines.
fn build_tree_prefix(nodes: &[termcode_view::file_explorer::FileNode], index: usize) -> String {
    let node = &nodes[index];
    if node.depth == 0 {
        return String::new();
    }

    // For each ancestor depth 1..depth, check if there's a continuing sibling.
    let mut has_continuation = vec![false; node.depth];
    for (d, cont) in has_continuation.iter_mut().enumerate() {
        // Find the ancestor at this depth by scanning backwards
        let mut ancestor_idx = None;
        for k in (0..index).rev() {
            if nodes[k].depth == d {
                ancestor_idx = Some(k);
                break;
            }
        }
        if let Some(ai) = ancestor_idx {
            *cont = !is_last_sibling(nodes, ai);
        }
    }

    let last = is_last_sibling(nodes, index);

    let mut prefix = String::new();
    for cont in &has_continuation[..node.depth - 1] {
        if *cont {
            prefix.push_str("│   ");
        } else {
            prefix.push_str("    ");
        }
    }
    if last {
        prefix.push_str("└── ");
    } else {
        prefix.push_str("├── ");
    }
    prefix
}

impl Widget for FileExplorerWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let bg = self.theme.ui.sidebar_bg.to_ratatui();
        let fg = self.theme.ui.sidebar_fg.to_ratatui();
        let raw_sel_bg = self.theme.ui.selection.to_ratatui();
        let sel_bg = if self.is_active {
            raw_sel_bg
        } else {
            blend_color(raw_sel_bg, bg, 0.2)
        };
        let normal_style = Style::default().fg(fg).bg(bg);

        // Fill background
        for y in area.y..area.y + area.height {
            for x in area.x..area.x + area.width {
                buf[(x, y)].set_char(' ').set_style(normal_style);
            }
        }

        let nodes = self.explorer.flatten_visible();
        let offset = self.explorer.scroll_offset;
        let scroll_left = self.explorer.scroll_left;
        let use_tree = self.file_tree_style.tree_style;
        let use_emoji = self.file_tree_style.show_file_type_emoji;

        // Helper: write a character at logical_x, applying scroll_left offset.
        // Returns the new logical_x after the character.
        let put_char =
            |logical_x: u16, ch: char, w: u16, style: Style, y: u16, buf: &mut Buffer| -> u16 {
                if logical_x + w > scroll_left && logical_x < scroll_left + area.width {
                    let screen_x = area.x + logical_x.saturating_sub(scroll_left);
                    if screen_x + w <= area.x + area.width {
                        buf[(screen_x, y)].set_char(ch).set_style(style);
                        for i in 1..w {
                            buf[(screen_x + i, y)].set_char(' ').set_style(style);
                        }
                    }
                }
                logical_x + w
            };

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

            let mut lx: u16 = 0;

            if use_tree {
                let prefix = build_tree_prefix(nodes, vi);
                for ch in prefix.chars() {
                    lx = put_char(lx, ch, 1, style, y, buf);
                }
            } else {
                lx += (node.depth * 2) as u16;
            }

            if use_emoji {
                let icons = &self.theme.icons;
                let icon_str = match node.kind {
                    FileNodeKind::Directory if node.expanded => &icons.directory_open,
                    FileNodeKind::Directory => &icons.directory_closed,
                    _ => icons.file_icon(&node.name),
                };
                let icon = format!("{icon_str} ");
                for ch in icon.chars() {
                    let w = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(1) as u16;
                    lx = put_char(lx, ch, w, style, y, buf);
                }
            }

            // Name (with ellipsis when truncated)
            let right_edge = scroll_left + area.width;
            let mut name_chars = node.name.chars().peekable();
            while let Some(ch) = name_chars.next() {
                if lx >= right_edge {
                    break;
                }
                let w = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(1) as u16;
                // Check if this char fits but there are more chars that won't
                if lx + w < right_edge && name_chars.peek().is_some() {
                    // Check if next char would overflow
                    let next_w = name_chars
                        .peek()
                        .map(|c| unicode_width::UnicodeWidthChar::width(*c).unwrap_or(1) as u16)
                        .unwrap_or(0);
                    if lx + w + next_w > right_edge {
                        // Next char won't fit — show ellipsis instead of current char
                        put_char(lx, '\u{2026}', 1, style, y, buf);
                        break;
                    }
                }
                if lx + w > right_edge {
                    // Current char doesn't fit — replace with ellipsis
                    if lx < right_edge {
                        put_char(lx, '\u{2026}', 1, style, y, buf);
                    }
                    break;
                }
                lx = put_char(lx, ch, w, style, y, buf);
            }
        }
    }
}
