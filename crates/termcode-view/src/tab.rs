use crate::document::DocumentId;
use crate::image::{ImageId, TabContent};

#[derive(Debug, Clone)]
pub struct Tab {
    pub label: String,
    pub content: TabContent,
    pub modified: bool,
}

pub struct TabManager {
    pub tabs: Vec<Tab>,
    pub active: usize,
}

impl Default for TabManager {
    fn default() -> Self {
        Self::new()
    }
}

impl TabManager {
    pub fn new() -> Self {
        Self {
            tabs: Vec::new(),
            active: 0,
        }
    }

    pub fn add(&mut self, label: String, content: TabContent) {
        self.tabs.push(Tab {
            label,
            content,
            modified: false,
        });
        self.active = self.tabs.len() - 1;
    }

    pub fn remove(&mut self, index: usize) {
        if index >= self.tabs.len() {
            return;
        }
        self.tabs.remove(index);
        if self.tabs.is_empty() {
            self.active = 0;
        } else if self.active >= self.tabs.len() {
            self.active = self.tabs.len() - 1;
        } else if self.active > index {
            self.active -= 1;
        }
    }

    pub fn set_active(&mut self, index: usize) {
        if !self.tabs.is_empty() {
            self.active = index.min(self.tabs.len() - 1);
        }
    }

    pub fn next(&mut self) {
        if !self.tabs.is_empty() {
            self.active = (self.active + 1) % self.tabs.len();
        }
    }

    pub fn prev(&mut self) {
        if !self.tabs.is_empty() {
            self.active = if self.active == 0 {
                self.tabs.len() - 1
            } else {
                self.active - 1
            };
        }
    }

    pub fn find_by_doc_id(&self, doc_id: DocumentId) -> Option<usize> {
        self.tabs
            .iter()
            .position(|t| matches!(&t.content, TabContent::Document(id) if *id == doc_id))
    }

    pub fn find_by_image_id(&self, image_id: ImageId) -> Option<usize> {
        self.tabs
            .iter()
            .position(|t| matches!(&t.content, TabContent::Image(id) if *id == image_id))
    }

    pub fn active_tab(&self) -> Option<&Tab> {
        self.tabs.get(self.active)
    }

    pub fn remove_by_doc_id(&mut self, doc_id: DocumentId) {
        if let Some(idx) = self.find_by_doc_id(doc_id) {
            self.remove(idx);
        }
    }

    pub fn remove_by_image_id(&mut self, image_id: ImageId) {
        if let Some(idx) = self.find_by_image_id(image_id) {
            self.remove(idx);
        }
    }
}
