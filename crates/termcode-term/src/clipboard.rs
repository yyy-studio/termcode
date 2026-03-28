use termcode_view::clipboard::ClipboardProvider;

/// System clipboard implementation backed by arboard.
/// Lazily initializes the clipboard on first use to handle headless environments.
pub struct ArboardClipboard {
    clipboard: Option<arboard::Clipboard>,
    init_attempted: bool,
}

impl ArboardClipboard {
    pub fn new() -> Self {
        Self {
            clipboard: None,
            init_attempted: false,
        }
    }

    fn ensure_init(&mut self) -> Option<&mut arboard::Clipboard> {
        if !self.init_attempted {
            self.init_attempted = true;
            match arboard::Clipboard::new() {
                Ok(cb) => self.clipboard = Some(cb),
                Err(e) => log::warn!("Clipboard unavailable: {e}"),
            }
        }
        self.clipboard.as_mut()
    }
}

impl Default for ArboardClipboard {
    fn default() -> Self {
        Self::new()
    }
}

impl ClipboardProvider for ArboardClipboard {
    fn get_text(&mut self) -> Option<String> {
        self.ensure_init()?.get_text().ok()
    }

    fn set_text(&mut self, text: &str) -> anyhow::Result<()> {
        let cb = self
            .ensure_init()
            .ok_or_else(|| anyhow::anyhow!("Clipboard unavailable"))?;
        cb.set_text(text)
            .map_err(|e| anyhow::anyhow!("Clipboard write failed: {e}"))
    }
}

/// Mock clipboard for testing (no system clipboard dependency).
pub struct MockClipboard {
    content: Option<String>,
}

impl MockClipboard {
    pub fn new() -> Self {
        Self { content: None }
    }
}

impl Default for MockClipboard {
    fn default() -> Self {
        Self::new()
    }
}

impl ClipboardProvider for MockClipboard {
    fn get_text(&mut self) -> Option<String> {
        self.content.clone()
    }

    fn set_text(&mut self, text: &str) -> anyhow::Result<()> {
        self.content = Some(text.to_string());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_clipboard_round_trip() {
        let mut cb = MockClipboard::new();
        assert!(cb.get_text().is_none());
        cb.set_text("hello world").unwrap();
        assert_eq!(cb.get_text(), Some("hello world".to_string()));
    }

    #[test]
    fn mock_clipboard_overwrite() {
        let mut cb = MockClipboard::new();
        cb.set_text("first").unwrap();
        cb.set_text("second").unwrap();
        assert_eq!(cb.get_text(), Some("second".to_string()));
    }
}
