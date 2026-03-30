use crate::document::DocumentId;

/// The action to perform after the user confirms.
#[derive(Debug, Clone)]
pub enum ConfirmAction {
    /// Close a single tab with unsaved changes.
    CloseTab(DocumentId),
    /// Quit the editor with unsaved files.
    QuitAll,
}

/// State for the unsaved-changes confirmation dialog.
#[derive(Debug, Clone)]
pub struct ConfirmDialog {
    pub action: ConfirmAction,
    pub message: String,
    pub buttons: Vec<String>,
    pub selected_button: usize,
}

impl ConfirmDialog {
    pub fn new(action: ConfirmAction, message: String, buttons: Vec<String>) -> Self {
        Self {
            action,
            message,
            buttons,
            selected_button: 0,
        }
    }

    pub fn select_next(&mut self) {
        if !self.buttons.is_empty() {
            self.selected_button = (self.selected_button + 1) % self.buttons.len();
        }
    }

    pub fn select_prev(&mut self) {
        if !self.buttons.is_empty() {
            self.selected_button = if self.selected_button == 0 {
                self.buttons.len() - 1
            } else {
                self.selected_button - 1
            };
        }
    }
}
