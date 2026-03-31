use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use termcode_core::config_types::{EditorConfig, FileTreeStyle};
use termcode_syntax::language::LanguageRegistry;
use termcode_theme::theme::Theme;

use termcode_core::position::Position;

use crate::clipboard::ClipboardProvider;
use crate::confirm::ConfirmDialog;
use crate::document::{Document, DocumentId};
use crate::file_explorer::FileExplorer;
use crate::fuzzy::FuzzyFinderState;
use crate::image::{ImageEntry, ImageId, TabContent};
use crate::palette::CommandPaletteState;
use crate::search::SearchState;
use crate::tab::TabManager;
use crate::view::{View, ViewId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorMode {
    Normal,
    Insert,
    FileExplorer,
    Search,
    FuzzyFinder,
    CommandPalette,
}

/// A simplified completion item for display (no dependency on lsp-types).
#[derive(Debug, Clone)]
pub struct CompletionItem {
    pub label: String,
    pub detail: Option<String>,
    pub insert_text: String,
}

/// State for the autocomplete popup.
#[derive(Debug, Default)]
pub struct CompletionState {
    pub visible: bool,
    pub items: Vec<CompletionItem>,
    pub selected: usize,
    pub trigger_position: Position,
}

/// State for the hover information popup.
#[derive(Debug, Default)]
pub struct HoverState {
    pub visible: bool,
    pub content: String,
    pub position: Position,
}

/// Top-level editor state. Single source of truth.
pub struct Editor {
    pub documents: HashMap<DocumentId, Document>,
    pub views: HashMap<ViewId, View>,
    pub active_view: Option<ViewId>,
    pub theme: Theme,
    pub config: EditorConfig,
    pub file_tree_style: FileTreeStyle,
    pub language_registry: Arc<LanguageRegistry>,
    pub status_message: Option<String>,
    pub file_explorer: FileExplorer,
    pub tabs: TabManager,
    pub mode: EditorMode,
    pub search: SearchState,
    pub fuzzy_finder: FuzzyFinderState,
    pub command_palette: CommandPaletteState,
    pub completion: CompletionState,
    pub hover: HoverState,
    pub help_visible: bool,
    pub confirm_dialog: Option<ConfirmDialog>,
    pub clipboard: Option<Box<dyn ClipboardProvider>>,
    pub images: HashMap<ImageId, ImageEntry>,
    next_doc_id: usize,
    next_view_id: usize,
    next_image_id: usize,
}

impl Editor {
    pub fn new(
        theme: Theme,
        config: EditorConfig,
        language_registry: LanguageRegistry,
        root: Option<PathBuf>,
    ) -> Self {
        let root_path =
            root.unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        let file_explorer = FileExplorer::open(root_path).unwrap_or_else(|_| FileExplorer {
            root: PathBuf::from("."),
            tree: Vec::new(),
            selected: 0,
            scroll_offset: 0,
            visible: false,
            width: 30,
            viewport_height: 0,
            scroll_left: 0,
        });

        Self {
            documents: HashMap::new(),
            views: HashMap::new(),
            active_view: None,
            theme,
            config,
            file_tree_style: FileTreeStyle::default(),
            language_registry: Arc::new(language_registry),
            status_message: None,
            file_explorer,
            tabs: TabManager::new(),
            mode: EditorMode::Normal,
            search: SearchState::new(),
            fuzzy_finder: FuzzyFinderState::new(),
            command_palette: CommandPaletteState::new(),
            completion: CompletionState::default(),
            hover: HoverState::default(),
            help_visible: false,
            confirm_dialog: None,
            clipboard: None,
            images: HashMap::new(),
            next_doc_id: 0,
            next_view_id: 0,
            next_image_id: 0,
        }
    }

    /// Open a file, creating a document and a view for it.
    pub fn open_file(&mut self, path: &Path) -> anyhow::Result<(DocumentId, ViewId)> {
        let doc_id = self.next_document_id();
        let lang_id = self.language_registry.detect_language(path);
        let doc = Document::open(doc_id, path, lang_id)?;
        self.documents.insert(doc_id, doc);

        let view_id = self.next_view_id();
        let view = View::new(view_id, doc_id);
        self.views.insert(view_id, view);
        self.active_view = Some(view_id);

        let name = self.documents[&doc_id].display_name().to_string();
        self.status_message = Some(format!("Opened: {name}"));
        self.tabs.add(name, TabContent::Document(doc_id));

        Ok((doc_id, view_id))
    }

    /// Sync each tab's `modified` flag with its document's revision-based state.
    pub fn sync_tab_modified(&mut self) {
        for tab in &mut self.tabs.tabs {
            if let TabContent::Document(doc_id) = &tab.content {
                if let Some(doc) = self.documents.get(doc_id) {
                    tab.modified = doc.is_modified();
                }
            }
        }
    }

    pub fn toggle_sidebar(&mut self) {
        self.file_explorer.visible = !self.file_explorer.visible;
    }

    pub fn switch_mode(&mut self, mode: EditorMode) {
        self.mode = mode;
    }

    pub fn switch_theme(&mut self, theme: Theme) {
        self.theme = theme;
    }

    pub fn find_view_by_doc_id(&self, doc_id: DocumentId) -> Option<ViewId> {
        self.views
            .values()
            .find(|v| v.doc_id == doc_id)
            .map(|v| v.id)
    }

    pub fn active_view(&self) -> Option<&View> {
        self.active_view.and_then(|id| self.views.get(&id))
    }

    pub fn active_view_mut(&mut self) -> Option<&mut View> {
        self.active_view.and_then(|id| self.views.get_mut(&id))
    }

    pub fn active_document(&self) -> Option<&Document> {
        let view = self.active_view()?;
        self.documents.get(&view.doc_id)
    }

    pub fn active_document_mut(&mut self) -> Option<&mut Document> {
        let doc_id = self
            .active_view
            .and_then(|id| self.views.get(&id))
            .map(|v| v.doc_id)?;
        self.documents.get_mut(&doc_id)
    }

    pub fn save_document(&mut self, doc_id: DocumentId) -> anyhow::Result<()> {
        let doc = self
            .documents
            .get_mut(&doc_id)
            .ok_or_else(|| anyhow::anyhow!("Document not found"))?;
        let path = doc
            .path
            .clone()
            .ok_or_else(|| anyhow::anyhow!("No file path for document"))?;
        doc.buffer.save_to_file(&path)?;
        doc.mark_saved();
        self.status_message = Some(format!("Saved: {}", path.display()));
        Ok(())
    }

    pub fn close_document(&mut self, doc_id: DocumentId) {
        self.documents.remove(&doc_id);
        self.views.retain(|_, v| v.doc_id != doc_id);
        self.tabs.remove_by_doc_id(doc_id);
        if self
            .active_view
            .is_some_and(|id| !self.views.contains_key(&id))
        {
            self.active_view = self.views.keys().next().copied();
        }
    }

    /// Open an image file, creating an ImageEntry and a tab for it.
    /// Returns the ImageId. No View is created -- active_view is set to None.
    pub fn open_image(
        &mut self,
        path: &Path,
        format: String,
        file_size: u64,
        dimensions: Option<(u32, u32)>,
    ) -> ImageId {
        let image_id = self.next_image_id();
        let entry = ImageEntry {
            id: image_id,
            path: path.to_path_buf(),
            format,
            file_size,
            dimensions,
        };
        self.images.insert(image_id, entry);
        self.active_view = None;

        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "image".to_string());
        self.status_message = Some(format!("Opened image: {name}"));
        self.tabs.add(name, TabContent::Image(image_id));

        image_id
    }

    /// Returns the active image entry if the current tab is an image tab.
    pub fn active_image(&self) -> Option<&ImageEntry> {
        let tab = self.tabs.active_tab()?;
        if let TabContent::Image(image_id) = &tab.content {
            self.images.get(image_id)
        } else {
            None
        }
    }

    /// Close an image tab and remove its metadata.
    pub fn close_image(&mut self, image_id: ImageId) {
        self.images.remove(&image_id);
        self.tabs.remove_by_image_id(image_id);
    }

    fn next_document_id(&mut self) -> DocumentId {
        let id = DocumentId(self.next_doc_id);
        self.next_doc_id += 1;
        id
    }

    fn next_view_id(&mut self) -> ViewId {
        let id = ViewId(self.next_view_id);
        self.next_view_id += 1;
        id
    }

    fn next_image_id(&mut self) -> ImageId {
        let id = ImageId(self.next_image_id);
        self.next_image_id += 1;
        id
    }
}
