//! Fixtures shared by the command tests, which sit next to the commands they
//! exercise rather than in one module.

use std::path::PathBuf;

use termcode_core::config_types::EditorConfig;
use termcode_syntax::language::LanguageRegistry;
use termcode_theme::theme::Theme;
use termcode_view::clipboard::ClipboardProvider;
use termcode_view::editor::Editor;

use super::sync_selection_from_cursor;

/// In-memory clipboard so yank/paste can be exercised without a display server.
#[derive(Default)]
struct MemClipboard(Option<String>);

impl ClipboardProvider for MemClipboard {
    fn get_text(&mut self) -> Option<String> {
        self.0.clone()
    }
    fn set_text(&mut self, text: &str) -> anyhow::Result<()> {
        self.0 = Some(text.to_string());
        Ok(())
    }
}

pub(super) struct TestFile(PathBuf);

impl Drop for TestFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

pub(super) fn editor_with(name: &str, contents: &str) -> (Editor, TestFile) {
    let path = std::env::temp_dir().join(format!("termcode-cmd-test-{name}.txt"));
    std::fs::write(&path, contents).unwrap();
    let mut editor = Editor::new(
        Theme::default(),
        EditorConfig::default(),
        LanguageRegistry::new(),
        None,
    );
    editor.open_file(&path).unwrap();
    editor.clipboard = Some(Box::new(MemClipboard::default()));
    (editor, TestFile(path))
}

pub(super) fn set_cursor(editor: &mut Editor, line: usize, column: usize) {
    let view = editor.active_view_mut().unwrap();
    view.cursor.line = line;
    view.cursor.column = column;
    sync_selection_from_cursor(editor);
}

pub(super) fn cursor(editor: &Editor) -> (usize, usize) {
    let view = editor.active_view().unwrap();
    (view.cursor.line, view.cursor.column)
}

pub(super) fn text(editor: &Editor) -> String {
    editor.active_document().unwrap().buffer.text().to_string()
}
