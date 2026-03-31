use std::path::PathBuf;

use crate::document::DocumentId;

/// Unique identifier for an image entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ImageId(pub usize);

/// Frontend-agnostic metadata for an opened image file.
/// Actual decoded pixel data lives in termcode-term's App.image_cache.
#[derive(Debug, Clone)]
pub struct ImageEntry {
    pub id: ImageId,
    pub path: PathBuf,
    pub format: String,
    pub file_size: u64,
    /// Original image dimensions in pixels (width, height).
    /// `None` if the image failed to decode.
    pub dimensions: Option<(u32, u32)>,
}

/// Distinguishes what a tab points to: a text document or an image.
#[derive(Debug, Clone)]
pub enum TabContent {
    Document(DocumentId),
    Image(ImageId),
}
