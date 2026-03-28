use std::path::{Path, PathBuf};

use termcode_core::buffer::{Buffer, BufferError};
use termcode_core::diagnostic::Diagnostic;
use termcode_core::history::History;
use termcode_core::selection::Selection;
use termcode_core::transaction::Transaction;
use termcode_syntax::highlighter::SyntaxHighlighter;
use termcode_syntax::language::LanguageId;

/// Unique identifier for a document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DocumentId(pub usize);

/// A document combines a text buffer with syntax highlighting and metadata.
pub struct Document {
    pub id: DocumentId,
    pub buffer: Buffer,
    pub path: Option<PathBuf>,
    pub syntax: Option<SyntaxHighlighter>,
    pub language_id: Option<LanguageId>,
    pub diagnostics: Vec<Diagnostic>,
    pub selection: Selection,
    pub history: History,
    pub last_saved_revision: usize,
    /// Monotonically increasing version for LSP versioned text document identifiers.
    pub version: i32,
}

impl Document {
    /// Create a new empty document.
    pub fn new(id: DocumentId) -> Self {
        Self {
            id,
            buffer: Buffer::new(),
            path: None,
            syntax: None,
            language_id: None,
            diagnostics: Vec::new(),
            selection: Selection::point(0),
            history: History::new(),
            last_saved_revision: 0,
            version: 0,
        }
    }

    /// Open a document from a file path.
    pub fn open(
        id: DocumentId,
        path: &Path,
        language_id: Option<LanguageId>,
    ) -> Result<Self, BufferError> {
        let buffer = Buffer::from_file(path)?;

        let syntax = language_id.as_ref().map(SyntaxHighlighter::new);

        Ok(Self {
            id,
            buffer,
            path: Some(path.to_path_buf()),
            syntax,
            language_id,
            diagnostics: Vec::new(),
            selection: Selection::point(0),
            history: History::new(),
            last_saved_revision: 0,
            version: 0,
        })
    }

    /// Get the display name for this document.
    pub fn display_name(&self) -> &str {
        self.path
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("[untitled]")
    }

    /// Apply a transaction: compute inverse, apply to buffer, then commit to history.
    /// The inverse is computed before apply (it needs the original rope state).
    /// History is only committed after a successful buffer apply to prevent divergence.
    pub fn apply_transaction(&mut self, transaction: &Transaction) -> anyhow::Result<()> {
        let inverse = transaction.invert(self.buffer.text());
        self.buffer.apply(transaction)?;
        self.version += 1;
        self.history
            .commit_with_inverse(transaction.clone(), inverse);
        if let Some(ref sel) = transaction.selection {
            self.selection = sel.clone();
        } else {
            self.selection = self.selection.map(&transaction.changes);
        }
        Ok(())
    }

    /// Undo the last edit. Returns true if an undo was performed.
    pub fn undo(&mut self) -> anyhow::Result<bool> {
        if let Some(inverse) = self.history.undo() {
            if let Some(ref sel) = inverse.selection {
                self.selection = sel.clone();
            } else {
                self.selection = self.selection.map(&inverse.changes);
            }
            self.buffer.apply(&inverse)?;
            self.version += 1;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Redo the last undone edit. Returns true if a redo was performed.
    pub fn redo(&mut self) -> anyhow::Result<bool> {
        if let Some(txn) = self.history.redo() {
            if let Some(ref sel) = txn.selection {
                self.selection = sel.clone();
            } else {
                self.selection = self.selection.map(&txn.changes);
            }
            self.buffer.apply(&txn)?;
            self.version += 1;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Check if the document has unsaved changes.
    pub fn is_modified(&self) -> bool {
        !self.history.is_at_saved(self.last_saved_revision)
    }

    /// Mark the current state as saved.
    pub fn mark_saved(&mut self) {
        self.last_saved_revision = self.history.current_revision();
        self.buffer.set_modified(false);
    }
}
