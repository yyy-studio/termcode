use termcode_core::position::Position;

use crate::document::DocumentId;

/// Unique identifier for a view (viewport into a document).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ViewId(pub usize);

/// A viewport into a document.
pub struct View {
    pub id: ViewId,
    pub doc_id: DocumentId,
    pub cursor: Position,
    pub scroll: ScrollState,
    /// Visible area dimensions (set during layout).
    pub area_height: u16,
    pub area_width: u16,
}

/// Scroll position for a view.
#[derive(Debug, Clone, Default)]
pub struct ScrollState {
    pub top_line: usize,
    pub left_col: usize,
}

impl View {
    pub fn new(id: ViewId, doc_id: DocumentId) -> Self {
        Self {
            id,
            doc_id,
            cursor: Position::default(),
            scroll: ScrollState::default(),
            area_height: 0,
            area_width: 0,
        }
    }

    /// Ensure the cursor is visible within the viewport, adjusting scroll.
    pub fn ensure_cursor_visible(&mut self, scroll_off: usize) {
        let height = self.area_height as usize;
        if height == 0 {
            return;
        }

        // Vertical scroll
        if self.cursor.line < self.scroll.top_line + scroll_off {
            self.scroll.top_line = self.cursor.line.saturating_sub(scroll_off);
        }
        if self.cursor.line + scroll_off >= self.scroll.top_line + height {
            self.scroll.top_line = (self.cursor.line + scroll_off + 1).saturating_sub(height);
        }
    }

    /// Scroll down by a number of lines.
    pub fn scroll_down(&mut self, lines: usize, max_line: usize) {
        let max_top = max_line.saturating_sub(self.area_height as usize);
        self.scroll.top_line = (self.scroll.top_line + lines).min(max_top);
    }

    /// Scroll up by a number of lines.
    pub fn scroll_up(&mut self, lines: usize) {
        self.scroll.top_line = self.scroll.top_line.saturating_sub(lines);
    }
}
