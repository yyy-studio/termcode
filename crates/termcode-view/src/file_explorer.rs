use std::collections::HashSet;
use std::path::{Path, PathBuf};

use ignore::WalkBuilder;
use termcode_core::config_types::FileTreeStyle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileNodeKind {
    File,
    Directory,
    Symlink,
}

#[derive(Debug, Clone)]
pub struct FileNode {
    pub path: PathBuf,
    pub name: String,
    pub kind: FileNodeKind,
    pub depth: usize,
    pub expanded: bool,
}

pub struct FileExplorer {
    pub root: PathBuf,
    pub tree: Vec<FileNode>,
    pub selected: usize,
    pub scroll_offset: usize,
    pub visible: bool,
    pub width: u16,
    pub viewport_height: usize,
    pub scroll_left: u16,
}

impl FileExplorer {
    pub fn open(root: PathBuf) -> anyhow::Result<Self> {
        let mut explorer = Self {
            root: root.clone(),
            tree: Vec::new(),
            selected: 0,
            scroll_offset: 0,
            visible: false,
            width: 30,
            viewport_height: 0,
            scroll_left: 0,
        };
        explorer.load_children(&root, 0, 0)?;
        Ok(explorer)
    }

    pub fn toggle_expand(&mut self, index: usize) -> anyhow::Result<()> {
        if index >= self.tree.len() {
            return Ok(());
        }
        if self.tree[index].kind != FileNodeKind::Directory {
            return Ok(());
        }

        if self.tree[index].expanded {
            self.tree[index].expanded = false;
            let depth = self.tree[index].depth;
            let remove_start = index + 1;
            let mut remove_end = remove_start;
            while remove_end < self.tree.len() && self.tree[remove_end].depth > depth {
                remove_end += 1;
            }
            self.tree.drain(remove_start..remove_end);
        } else {
            self.tree[index].expanded = true;
            let path = self.tree[index].path.clone();
            let depth = self.tree[index].depth + 1;
            self.load_children(&path, depth, index + 1)?;
        }
        Ok(())
    }

    pub fn refresh(&mut self) -> anyhow::Result<()> {
        let expanded_paths: HashSet<PathBuf> = self
            .tree
            .iter()
            .filter(|n| n.expanded)
            .map(|n| n.path.clone())
            .collect();

        self.tree.clear();
        self.load_children(&self.root.clone(), 0, 0)?;

        let mut i = 0;
        while i < self.tree.len() {
            if self.tree[i].kind == FileNodeKind::Directory
                && expanded_paths.contains(&self.tree[i].path)
            {
                self.toggle_expand(i)?;
            }
            i += 1;
        }

        self.selected = self.selected.min(self.tree.len().saturating_sub(1));
        self.ensure_visible(self.viewport_height);
        Ok(())
    }

    pub fn selected_path(&self) -> Option<&Path> {
        self.tree.get(self.selected).map(|n| n.path.as_path())
    }

    pub fn move_selection(&mut self, delta: i32, file_tree_style: &FileTreeStyle) {
        if self.tree.is_empty() {
            return;
        }
        let new = self.selected as i32 + delta;
        self.selected = new.clamp(0, self.tree.len() as i32 - 1) as usize;
        self.ensure_visible(self.viewport_height);
        self.compute_scroll_left(file_tree_style);
    }

    /// Adjust scroll_offset so that `self.selected` is within the visible viewport.
    pub fn ensure_visible(&mut self, viewport_height: usize) {
        if viewport_height == 0 {
            return;
        }
        if self.selected < self.scroll_offset {
            self.scroll_offset = self.selected;
        } else if self.selected >= self.scroll_offset + viewport_height {
            self.scroll_offset = self.selected - viewport_height + 1;
        }
    }

    /// Adjust horizontal scroll so the selected node's filename is visible.
    /// Works like vertical `ensure_visible` — only shifts the minimum amount needed.
    pub fn compute_scroll_left(&mut self, style: &FileTreeStyle) {
        if self.tree.is_empty() || self.selected >= self.tree.len() {
            self.scroll_left = 0;
            return;
        }
        let node = &self.tree[self.selected];
        let depth = node.depth;
        let indent: u16 = if style.tree_style {
            (depth * 4) as u16
        } else {
            (depth * 2) as u16
        };
        let icon_width: u16 = if style.show_file_type_emoji { 3 } else { 0 };
        let name_start = indent + icon_width;
        let name_len = node.name.len() as u16;
        let name_end = name_start + name_len;

        let width = self.width;

        // If filename end extends beyond the right edge → shift right to show full name
        if name_end > self.scroll_left + width {
            self.scroll_left = name_end.saturating_sub(width);
        }
        // If indent is left of viewport → shift left to show tree context
        if indent < self.scroll_left {
            self.scroll_left = indent.saturating_sub(2);
        }
        // Pull back if there's unnecessary blank space on the left
        if self.scroll_left > 0 && indent < self.scroll_left + 2 {
            self.scroll_left = indent.saturating_sub(2);
        }
    }

    pub fn flatten_visible(&self) -> &[FileNode] {
        &self.tree
    }

    fn load_children(&mut self, dir: &Path, depth: usize, insert_at: usize) -> anyhow::Result<()> {
        let mut entries = Vec::new();

        let walker = WalkBuilder::new(dir)
            .max_depth(Some(1))
            .sort_by_file_name(|a, b| a.cmp(b))
            .build();

        for result in walker {
            let entry = match result {
                Ok(entry) => entry,
                Err(e) => {
                    log::warn!("skipping entry: {e}");
                    continue;
                }
            };
            let path = entry.path().to_path_buf();
            if path == dir {
                continue;
            }

            let ft = entry.file_type();
            let kind = if ft.is_some_and(|ft| ft.is_symlink()) {
                FileNodeKind::Symlink
            } else if ft.is_some_and(|ft| ft.is_dir()) {
                FileNodeKind::Directory
            } else {
                FileNodeKind::File
            };

            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();

            entries.push(FileNode {
                path,
                name,
                kind,
                depth,
                expanded: false,
            });
        }

        entries.sort_by(|a, b| {
            let a_is_dir = a.kind == FileNodeKind::Directory;
            let b_is_dir = b.kind == FileNodeKind::Directory;
            match (a_is_dir, b_is_dir) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            }
        });

        let count = entries.len();
        let tail = self.tree.split_off(insert_at);
        self.tree.reserve(count + tail.len());
        self.tree.extend(entries);
        self.tree.extend(tail);

        Ok(())
    }
}
