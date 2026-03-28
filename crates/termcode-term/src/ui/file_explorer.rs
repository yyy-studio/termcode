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

/// Return an emoji icon for a file based on its extension.
fn file_emoji(name: &str) -> &'static str {
    let ext = name.rsplit('.').next().unwrap_or("");
    match ext.to_ascii_lowercase().as_str() {
        // Text / code / config files
        "txt" | "md" | "markdown" | "rst" | "org" | "log" | "csv" | "rs" | "py" | "js" | "ts"
        | "tsx" | "jsx" | "go" | "c" | "cpp" | "h" | "hpp" | "java" | "rb" | "php" | "swift"
        | "kt" | "scala" | "zig" | "hs" | "ml" | "ex" | "exs" | "lua" | "sh" | "bash" | "zsh"
        | "fish" | "ps1" | "bat" | "cmd" | "css" | "scss" | "sass" | "less" | "html" | "htm"
        | "xml" | "sql" | "r" | "dart" | "vue" | "svelte" | "astro" | "toml" | "yaml" | "yml"
        | "json" | "json5" | "jsonc" | "ini" | "cfg" | "conf" | "env" | "lock" | "dockerfile"
        | "makefile" | "cmake" | "nix" | "tf" | "hcl" | "proto" | "graphql" | "gql" | "wasm" => {
            "📝 "
        }
        // Image files
        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "svg" | "ico" | "webp" | "avif" | "tiff"
        | "tif" | "psd" | "ai" | "eps" => "🖼️ ",
        // Audio files
        "mp3" | "wav" | "flac" | "aac" | "ogg" | "wma" | "m4a" | "opus" | "mid" | "midi" => "🎵 ",
        // Default
        _ => "📄 ",
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
        let use_tree = matches!(
            self.file_tree_style,
            FileTreeStyle::Tree | FileTreeStyle::EmojiTree
        );
        let use_emoji = matches!(
            self.file_tree_style,
            FileTreeStyle::Emoji | FileTreeStyle::EmojiTree
        );

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

            let mut x = area.x;

            if use_tree {
                // Tree-line prefix
                let prefix = build_tree_prefix(nodes, vi);
                for ch in prefix.chars() {
                    if x < area.x + area.width {
                        buf[(x, y)].set_char(ch).set_style(style);
                        x += 1;
                    }
                }
            } else {
                // Emoji-only: indent by depth
                let indent = node.depth * 2;
                x += indent as u16;
            }

            if use_emoji {
                // Emoji icon based on file type
                let icon = match node.kind {
                    FileNodeKind::Directory if node.expanded => "📂 ",
                    FileNodeKind::Directory => "📁 ",
                    _ => file_emoji(&node.name),
                };
                for ch in icon.chars() {
                    let w = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(1) as u16;
                    if x + w <= area.x + area.width {
                        buf[(x, y)].set_char(ch).set_style(style);
                        // For wide chars, mark following cells as continuation
                        for i in 1..w {
                            buf[(x + i, y)].set_char(' ').set_style(style);
                        }
                        x += w;
                    }
                }
            }

            // Name (clipped to sidebar width)
            for ch in node.name.chars() {
                if x >= area.x + area.width {
                    break;
                }
                let w = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(1) as u16;
                if x + w <= area.x + area.width {
                    buf[(x, y)].set_char(ch).set_style(style);
                    for i in 1..w {
                        buf[(x + i, y)].set_char(' ').set_style(style);
                    }
                    x += w;
                } else {
                    break;
                }
            }
        }
    }
}
