use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};

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
    /// The `..` row at the top of the tree, which walks the root up one level
    /// instead of behaving like the directory it points at.
    pub is_parent: bool,
}

/// What the pending inline row in the tree will create.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewEntryKind {
    File,
    Directory,
}

impl NewEntryKind {
    /// Label for the status line while the name is being typed.
    pub fn prompt(self) -> &'static str {
        match self {
            NewEntryKind::File => "New file",
            NewEntryKind::Directory => "New folder",
        }
    }
}

/// A row the tree draws in place of a node while the user types the name of a
/// file or directory that does not exist yet.
///
/// It lives here rather than in a mode of its own: the entry is part of the
/// tree the user is looking at, and a mode would have to be bound, rendered and
/// serialised everywhere `EditorMode` is matched.
#[derive(Debug, Clone)]
pub struct NewEntryInput {
    pub kind: NewEntryKind,
    /// Directory the entry is created in.
    pub parent: PathBuf,
    /// Tree index the row is drawn *before*; `tree.len()` draws it last.
    pub row: usize,
    pub depth: usize,
    pub name: String,
    /// Cursor position as a character index into `name`.
    pub cursor: usize,
}

impl NewEntryInput {
    pub fn insert_char(&mut self, c: char) {
        let byte = char_to_byte(&self.name, self.cursor);
        self.name.insert(byte, c);
        self.cursor += 1;
    }

    pub fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let byte = char_to_byte(&self.name, self.cursor - 1);
        self.name.remove(byte);
        self.cursor -= 1;
    }

    pub fn delete(&mut self) {
        if self.cursor >= self.name.chars().count() {
            return;
        }
        let byte = char_to_byte(&self.name, self.cursor);
        self.name.remove(byte);
    }

    pub fn move_left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn move_right(&mut self) {
        self.cursor = (self.cursor + 1).min(self.name.chars().count());
    }

    pub fn move_home(&mut self) {
        self.cursor = 0;
    }

    pub fn move_end(&mut self) {
        self.cursor = self.name.chars().count();
    }
}

/// `path` as an absolute path, without resolving symlinks.
///
/// `std::path::absolute` normalises away the `.` of a root like `"."` -- which
/// `Path::parent()` would otherwise answer with the empty path -- while leaving
/// symlinks alone, unlike `canonicalize`.
fn absolute(path: &Path) -> PathBuf {
    std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Byte index of the `char_pos`-th character, for `String::insert`/`remove`.
fn char_to_byte(s: &str, char_pos: usize) -> usize {
    s.char_indices()
        .nth(char_pos)
        .map(|(i, _)| i)
        .unwrap_or(s.len())
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
    pub respect_gitignore: bool,
    /// Set while a new file or directory is being named inline in the tree.
    pub new_entry: Option<NewEntryInput>,
    /// The sidebar width as it was when the divider between the sidebar and
    /// the editor was pressed, held for as long as the drag lasts. A `Drag`
    /// event carries no memory of where the drag began, so the press is what
    /// has to be remembered; keeping the original width means a press that
    /// never moved can be told from one that resized, and only the latter is
    /// written back to the config file.
    pub resizing: Option<u16>,
}

impl FileExplorer {
    pub fn open(root: PathBuf) -> anyhow::Result<Self> {
        Self::open_with_gitignore(root, true)
    }

    pub fn open_with_gitignore(root: PathBuf, respect_gitignore: bool) -> anyhow::Result<Self> {
        let mut explorer = Self {
            root: root.clone(),
            tree: Vec::new(),
            selected: 0,
            scroll_offset: 0,
            visible: false,
            width: 30,
            viewport_height: 0,
            scroll_left: 0,
            respect_gitignore,
            new_entry: None,
            resizing: None,
        };
        explorer.load_children(&root, 0, 0)?;
        explorer.insert_parent_row();
        Ok(explorer)
    }

    pub fn toggle_expand(&mut self, index: usize) -> anyhow::Result<()> {
        if index >= self.tree.len() {
            return Ok(());
        }
        if self.tree[index].kind != FileNodeKind::Directory || self.tree[index].is_parent {
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
        self.insert_parent_row();

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

    /// Refresh only the selected node's directory.
    /// If the selected node is a file/symlink, refreshes its parent directory instead.
    pub fn refresh_node(&mut self, index: usize) -> anyhow::Result<()> {
        if index >= self.tree.len() || self.tree[index].is_parent {
            return self.refresh();
        }

        // Find the target directory index to refresh
        let dir_index = if self.tree[index].kind == FileNodeKind::Directory {
            index
        } else {
            // Walk backwards to find parent directory
            let target_depth = self.tree[index].depth.saturating_sub(1);
            let mut parent = None;
            for i in (0..index).rev() {
                if self.tree[i].kind == FileNodeKind::Directory
                    && self.tree[i].depth == target_depth
                    && !self.tree[i].is_parent
                {
                    parent = Some(i);
                    break;
                }
            }
            match parent {
                Some(i) => i,
                None => return self.refresh(),
            }
        };

        // Collapse then re-expand to reload children
        if self.tree[dir_index].expanded {
            self.toggle_expand(dir_index)?; // collapse
        }
        self.toggle_expand(dir_index)?; // expand with fresh data

        self.selected = self.selected.min(self.tree.len().saturating_sub(1));
        self.ensure_visible(self.viewport_height);
        Ok(())
    }

    /// Start naming a new file or directory next to the selection.
    ///
    /// A collapsed directory is opened first: the entry is created inside the
    /// folder the user is standing on, so the row has to be somewhere visible.
    pub fn begin_new_entry(&mut self, kind: NewEntryKind) {
        let (parent, row, depth) = self.new_entry_target();
        self.new_entry = Some(NewEntryInput {
            kind,
            parent,
            row,
            depth,
            name: String::new(),
            cursor: 0,
        });
    }

    pub fn cancel_new_entry(&mut self) {
        self.new_entry = None;
    }

    /// Create the entry being named, then reload the tree and select it.
    ///
    /// The input survives a failure -- a name that already exists is worth
    /// correcting, not retyping.
    pub fn commit_new_entry(&mut self) -> anyhow::Result<PathBuf> {
        let input = match &self.new_entry {
            Some(input) => input.clone(),
            None => anyhow::bail!("Nothing is being created"),
        };

        let name = input.name.trim();
        if name.is_empty() {
            anyhow::bail!("Name is empty");
        }
        let relative = Path::new(name);
        // Only a path below the parent: an absolute path or a `..` would create
        // the entry somewhere the tree does not show.
        if relative.components().any(|c| {
            matches!(
                c,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        }) {
            anyhow::bail!("Invalid name: {name}");
        }

        let path = input.parent.join(relative);
        if path.exists() {
            anyhow::bail!("Already exists: {}", path.display());
        }
        match input.kind {
            NewEntryKind::Directory => std::fs::create_dir_all(&path)?,
            NewEntryKind::File => {
                if let Some(dir) = path.parent() {
                    std::fs::create_dir_all(dir)?;
                }
                std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&path)?;
            }
        }

        self.new_entry = None;
        self.refresh()?;
        self.select_path(&path);
        Ok(path)
    }

    /// Where a new entry belongs: the directory it goes in, the tree index its
    /// row is drawn before, and the depth to indent it to.
    fn new_entry_target(&mut self) -> (PathBuf, usize, usize) {
        let selected = self.selected;
        let Some(node) = self.tree.get(selected) else {
            return (self.root.clone(), 0, 0);
        };

        if node.is_parent {
            return (self.root.clone(), selected + 1, 0);
        }

        if node.kind == FileNodeKind::Directory {
            if !node.expanded {
                if let Err(e) = self.toggle_expand(selected) {
                    log::warn!("cannot open {}: {e}", self.tree[selected].path.display());
                }
            }
            let node = &self.tree[selected];
            return (node.path.clone(), selected + 1, node.depth + 1);
        }

        let parent = node
            .path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| self.root.clone());
        (parent, selected + 1, node.depth)
    }

    /// Move the tree's root up one level and keep the directory we came from
    /// selected, so walking up and back down lands where it started.
    pub fn navigate_to_parent(&mut self) -> anyhow::Result<()> {
        // The root is whatever the editor was opened on, often `.`, whose
        // `parent()` is the empty path -- a root that lists nothing at all.
        let current = absolute(&self.root);
        let Some(parent) = current.parent().map(Path::to_path_buf) else {
            anyhow::bail!("Already at the filesystem root");
        };
        self.set_root(parent, Some(&current))
    }

    /// Make the directory at `index` the tree's root.
    ///
    /// This is what Enter does, as opposed to the arrow keys: expanding shows a
    /// directory inside the tree it is part of, entering makes it the tree.
    pub fn navigate_into(&mut self, index: usize) -> anyhow::Result<()> {
        let Some(node) = self.tree.get(index) else {
            anyhow::bail!("Nothing selected");
        };
        if node.is_parent {
            return self.navigate_to_parent();
        }
        if node.kind != FileNodeKind::Directory {
            anyhow::bail!("Not a directory: {}", node.path.display());
        }
        let path = node.path.clone();
        self.set_root(path, None)
    }

    /// Rebuild the tree around a new root, selecting `select` if it is there.
    ///
    /// Nothing of the old tree survives the move: its expansion state belongs
    /// to paths at a different depth than the rows being rebuilt.
    fn set_root(&mut self, root: PathBuf, select: Option<&Path>) -> anyhow::Result<()> {
        self.root = root;
        self.new_entry = None;
        self.tree.clear();
        self.selected = 0;
        self.scroll_offset = 0;
        self.scroll_left = 0;
        self.refresh()?;
        // With nothing to restore, start on the first real entry rather than on
        // the `..` row, which would make Enter bounce straight back out.
        let selected = select
            .and_then(|path| self.tree.iter().position(|n| n.path == path))
            .or_else(|| self.tree.iter().position(|n| !n.is_parent))
            .unwrap_or(0);
        self.selected = selected;
        self.ensure_visible(self.viewport_height);
        Ok(())
    }

    /// Put the `..` row at the top of the tree.
    ///
    /// It is a real node so selection, scrolling and mouse hit-testing stay
    /// plain row indices; `is_parent` keeps it out of everything that would
    /// treat it as an ordinary directory (expanding it, creating a file in it).
    /// The filesystem root has no parent, so it gets no row.
    fn insert_parent_row(&mut self) {
        let Some(parent) = absolute(&self.root).parent().map(Path::to_path_buf) else {
            return;
        };
        self.tree.insert(
            0,
            FileNode {
                path: parent,
                name: "..".to_string(),
                kind: FileNodeKind::Directory,
                depth: 0,
                expanded: false,
                is_parent: true,
            },
        );
    }

    /// Select the node at `path`, if the tree currently holds one.
    /// A file the ignore rules hide has no node, so the selection stays put.
    pub fn select_path(&mut self, path: &Path) -> bool {
        let Some(index) = self.tree.iter().position(|n| n.path == path) else {
            return false;
        };
        self.selected = index;
        self.ensure_visible(self.viewport_height);
        true
    }

    /// Path of the selected entry. The `..` row is navigation rather than an
    /// entry, so commands that act on the selection see nothing selected there.
    pub fn selected_path(&self) -> Option<&Path> {
        self.tree
            .get(self.selected)
            .filter(|n| !n.is_parent)
            .map(|n| n.path.as_path())
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
        // Columns, not bytes: a CJK name is three bytes a character and two
        // columns, so `len()` would scroll the tree sideways for a name that
        // fits where it is.
        let name_len = unicode_width::UnicodeWidthStr::width(node.name.as_str()) as u16;
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
            .hidden(self.respect_gitignore)
            .git_ignore(self.respect_gitignore)
            .git_global(self.respect_gitignore)
            .git_exclude(self.respect_gitignore)
            .ignore(self.respect_gitignore)
            .parents(self.respect_gitignore)
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
                is_parent: false,
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A temporary directory that cleans itself up, so the tests can create
    /// real files -- which is the whole point of the code under test.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!("termcode-explorer-{name}"));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn explorer(dir: &TempDir) -> FileExplorer {
        // Ignore rules are off: a scratch directory outside any repository
        // would otherwise hide the files the tests just wrote.
        FileExplorer::open_with_gitignore(dir.0.clone(), false).unwrap()
    }

    fn type_name(explorer: &mut FileExplorer, name: &str) {
        let input = explorer.new_entry.as_mut().expect("an entry being named");
        for c in name.chars() {
            input.insert_char(c);
        }
    }

    #[test]
    fn a_cjk_name_that_fits_does_not_scroll_the_tree_sideways() {
        let dir = TempDir::new("cjk-scroll");
        let mut ex = explorer(&dir);
        let style = FileTreeStyle {
            tree_style: false,
            show_file_type_emoji: true,
            ..FileTreeStyle::default()
        };
        // Six Hangul syllables: twelve columns, but eighteen bytes.
        ex.tree.push(FileNode {
            path: dir.0.join("가나다라마바"),
            name: "가나다라마바".to_string(),
            kind: FileNodeKind::Directory,
            depth: 6,
            expanded: false,
            is_parent: false,
        });
        ex.selected = ex.tree.len() - 1;
        ex.width = 30;
        // Indent 12 + icon 3 + name 12 = 27 of the 30 columns: it fits, so the
        // tree stays where it is. Measured in bytes it came to 33 and scrolled.
        ex.compute_scroll_left(&style);
        assert_eq!(ex.scroll_left, 0);
    }

    #[test]
    fn a_new_file_lands_in_the_root_of_an_empty_tree() {
        let dir = TempDir::new("empty-root");
        let mut ex = explorer(&dir);
        // The only row is `..`, which creates in the root rather than one level up.
        assert!(ex.tree[0].is_parent);
        ex.begin_new_entry(NewEntryKind::File);
        type_name(&mut ex, "main.rs");

        let path = ex.commit_new_entry().unwrap();
        assert_eq!(path, dir.0.join("main.rs"));
        assert!(path.is_file());
        assert!(ex.new_entry.is_none(), "the row closes once it commits");
        assert_eq!(ex.selected_path(), Some(path.as_path()));
    }

    #[test]
    fn a_new_entry_goes_inside_the_selected_directory() {
        let dir = TempDir::new("inside-dir");
        std::fs::create_dir(dir.0.join("src")).unwrap();
        let mut ex = explorer(&dir);
        ex.selected = 1;
        assert_eq!(ex.tree[1].name, "src");

        // The directory is collapsed: naming an entry has to open it, or the
        // row would be typed into somewhere the user cannot see.
        ex.begin_new_entry(NewEntryKind::Directory);
        assert!(ex.tree[1].expanded);
        let input = ex.new_entry.as_ref().unwrap();
        assert_eq!(input.parent, dir.0.join("src"));
        assert_eq!(input.depth, 1);
        assert_eq!(input.row, 2);

        type_name(&mut ex, "api");
        let path = ex.commit_new_entry().unwrap();
        assert_eq!(path, dir.0.join("src").join("api"));
        assert!(path.is_dir());
    }

    #[test]
    fn a_new_entry_beside_a_file_is_its_sibling() {
        let dir = TempDir::new("beside-file");
        std::fs::create_dir(dir.0.join("src")).unwrap();
        std::fs::write(dir.0.join("src").join("lib.rs"), "").unwrap();
        let mut ex = explorer(&dir);
        ex.toggle_expand(1).unwrap();
        ex.selected = 2;
        assert_eq!(ex.tree[2].name, "lib.rs");

        ex.begin_new_entry(NewEntryKind::File);
        let input = ex.new_entry.as_ref().unwrap();
        assert_eq!(input.parent, dir.0.join("src"));
        assert_eq!(input.depth, ex.tree[2].depth);
    }

    #[test]
    fn a_rejected_name_keeps_the_row_open() {
        let dir = TempDir::new("rejected");
        std::fs::write(dir.0.join("taken.rs"), "").unwrap();
        let mut ex = explorer(&dir);

        for (name, reason) in [
            ("", "empty"),
            ("taken.rs", "taken"),
            ("../out.rs", "escaping"),
        ] {
            ex.begin_new_entry(NewEntryKind::File);
            type_name(&mut ex, name);
            assert!(ex.commit_new_entry().is_err(), "{reason} name was accepted");
            assert!(
                ex.new_entry.is_some(),
                "{reason} name should stay for correcting"
            );
            ex.cancel_new_entry();
        }
        assert!(!dir.0.join("out.rs").exists());
        assert!(!dir.0.parent().unwrap().join("out.rs").exists());
    }

    #[test]
    fn a_nested_name_creates_the_directories_it_needs() {
        let dir = TempDir::new("nested");
        let mut ex = explorer(&dir);
        ex.begin_new_entry(NewEntryKind::File);
        type_name(&mut ex, "a/b/c.rs");

        let path = ex.commit_new_entry().unwrap();
        assert!(path.is_file());
        assert!(dir.0.join("a").join("b").is_dir());
    }

    #[test]
    fn the_parent_row_walks_the_root_up_and_back_down() {
        let dir = TempDir::new("parent-row");
        std::fs::create_dir(dir.0.join("src")).unwrap();
        let mut ex = explorer(&dir);
        assert!(ex.tree[0].is_parent, "the tree opens with a `..` row");
        assert_eq!(ex.tree[0].name, "..");

        // `..` is not a directory to expand: it moves the root instead.
        ex.toggle_expand(0).unwrap();
        assert_eq!(ex.tree[1].name, "src");

        let child = ex.root.clone();
        ex.navigate_to_parent().unwrap();
        assert_eq!(ex.root, child.parent().unwrap());
        // The directory we came out of is where the selection lands.
        assert_eq!(ex.selected_path(), Some(child.as_path()));
    }

    #[test]
    fn entering_a_directory_makes_it_the_root() {
        let dir = TempDir::new("enter-dir");
        std::fs::create_dir(dir.0.join("src")).unwrap();
        std::fs::write(dir.0.join("src").join("lib.rs"), "").unwrap();
        let mut ex = explorer(&dir);
        assert_eq!(ex.tree[1].name, "src");

        ex.navigate_into(1).unwrap();
        assert_eq!(ex.root, dir.0.join("src"));
        // `..` first, then the directory's own entries -- and the selection
        // starts on a real one so Enter does not bounce back out.
        assert!(ex.tree[0].is_parent);
        assert_eq!(ex.tree[1].name, "lib.rs");
        assert_eq!(ex.selected, 1);

        // Back out: the directory just left is selected again.
        ex.navigate_into(0).unwrap();
        assert_eq!(ex.root, dir.0);
        assert_eq!(ex.selected_path(), Some(dir.0.join("src").as_path()));
    }

    #[test]
    fn only_a_directory_can_be_entered() {
        let dir = TempDir::new("enter-file");
        std::fs::write(dir.0.join("main.rs"), "").unwrap();
        let mut ex = explorer(&dir);
        assert!(ex.navigate_into(1).is_err());
        assert!(ex.navigate_into(99).is_err());
        assert_eq!(ex.root, dir.0);
    }

    #[test]
    fn a_relative_root_still_walks_up_to_a_real_directory() {
        // `cargo run -- .` opens the editor on a relative root, whose plain
        // `parent()` is the empty path.
        let mut ex = FileExplorer::open_with_gitignore(PathBuf::from("."), false).unwrap();
        ex.navigate_to_parent().unwrap();
        assert!(ex.root.is_absolute());
        assert!(
            !ex.tree.is_empty(),
            "the parent of `.` has to list its entries"
        );
        assert_eq!(
            ex.selected_path(),
            Some(std::env::current_dir().unwrap()).as_deref()
        );
    }

    #[test]
    fn the_filesystem_root_has_no_parent_row() {
        let mut ex = FileExplorer::open_with_gitignore(PathBuf::from("/"), false).unwrap();
        assert!(ex.tree.iter().all(|n| !n.is_parent));
        assert!(ex.navigate_to_parent().is_err());
    }

    #[test]
    fn the_name_is_edited_at_the_cursor() {
        let mut input = NewEntryInput {
            kind: NewEntryKind::File,
            parent: PathBuf::from("."),
            row: 0,
            depth: 0,
            name: String::new(),
            cursor: 0,
        };
        for c in "main.rs".chars() {
            input.insert_char(c);
        }
        input.move_home();
        for c in "곰".chars() {
            input.insert_char(c);
        }
        assert_eq!(input.name, "곰main.rs");
        assert_eq!(input.cursor, 1);

        input.backspace();
        assert_eq!(input.name, "main.rs");
        assert_eq!(input.cursor, 0);
        // Backspace at the start of the name has nothing to delete.
        input.backspace();
        assert_eq!(input.name, "main.rs");

        input.move_end();
        input.move_right();
        assert_eq!(input.cursor, input.name.chars().count());
        input.backspace();
        assert_eq!(input.name, "main.r");
    }
}
