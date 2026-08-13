//! Regression tests for rendering minified files whose lines exceed u16 display columns.
//!
//! A line wider than 65535 columns used to wrap the u16 column arithmetic in
//! `EditorViewWidget`, producing a buffer index outside the render area and
//! aborting the process (leaving the terminal in raw mode).

use std::fs;
use std::path::PathBuf;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

use termcode_core::config_types::LineNumberStyle;
use termcode_core::position::Position;
use termcode_core::selection::Selection;
use termcode_term::ui::editor_view::EditorViewWidget;
use termcode_theme::theme::Theme;
use termcode_view::document::{Document, DocumentId};
use termcode_view::editor::EditorMode;
use termcode_view::search::{SearchMatch, SearchState};
use termcode_view::view::{View, ViewId};

const LINE_LEN: usize = 200_000;
const AREA: Rect = Rect {
    x: 0,
    y: 0,
    width: 98,
    height: 71,
};

/// Writes a one-line "minified" file and returns its path, removing it on drop.
struct TempFile(PathBuf);

impl TempFile {
    fn new(name: &str, contents: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("termcode-{}-{}.js", name, std::process::id()));
        fs::write(&path, contents).expect("write temp file");
        Self(path)
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn long_line_doc(name: &str) -> (TempFile, Document) {
    let contents = format!("{}\n", "a".repeat(LINE_LEN));
    let file = TempFile::new(name, &contents);
    let doc = Document::open(DocumentId(0), &file.0, None).expect("open document");
    (file, doc)
}

fn view_at(cursor_col: usize, left_col: usize) -> View {
    let mut view = View::new(ViewId(0), DocumentId(0));
    view.area_width = AREA.width;
    view.area_height = AREA.height;
    view.cursor = Position {
        line: 0,
        column: cursor_col,
    };
    view.scroll.left_col = left_col;
    view
}

fn render(doc: &Document, view: &View, search: Option<&SearchState>) -> Buffer {
    let theme = Theme::default();
    let mut buf = Buffer::empty(AREA);
    EditorViewWidget::new(
        doc,
        view,
        &theme,
        EditorMode::Normal,
        search,
        LineNumberStyle::Absolute,
        true,
    )
    .render(AREA, &mut buf);
    buf
}

/// First column of the code area, after the 3-wide gutter and its separator.
const CODE_START: u16 = 4;

#[test]
fn renders_line_wider_than_u16_at_left_edge() {
    let (_file, doc) = long_line_doc("left-edge");
    let view = view_at(0, 0);
    let buf = render(&doc, &view, None);
    assert_eq!(buf[(CODE_START, 0)].symbol(), "a");
    assert_eq!(buf[(AREA.width - 1, 0)].symbol(), "a");
}

#[test]
fn renders_line_wider_than_u16_when_scrolled_past_u16() {
    let (_file, doc) = long_line_doc("scrolled");
    // Horizontal scroll beyond u16::MAX: the old code truncated left_col to u16.
    let view = view_at(150_000, 149_911);
    let buf = render(&doc, &view, None);
    assert_eq!(buf[(CODE_START, 0)].symbol(), "a");
    assert_eq!(buf[(AREA.width - 1, 0)].symbol(), "a");
}

#[test]
fn renders_selection_spanning_a_very_long_line() {
    let (_file, mut doc) = long_line_doc("selection");
    doc.selection = Selection::single(0, LINE_LEN);
    let view = view_at(0, 0);
    let buf = render(&doc, &view, None);

    let sel_bg = Theme::default().ui.selection.to_ratatui();
    assert_eq!(buf[(CODE_START, 0)].bg, sel_bg);
    assert_eq!(buf[(AREA.width - 1, 0)].bg, sel_bg);
}

#[test]
fn renders_selection_scrolled_past_u16_columns() {
    let (_file, mut doc) = long_line_doc("selection-scrolled");
    doc.selection = Selection::single(0, LINE_LEN);
    let view = view_at(150_000, 149_911);
    let buf = render(&doc, &view, None);

    let sel_bg = Theme::default().ui.selection.to_ratatui();
    assert_eq!(buf[(CODE_START, 0)].bg, sel_bg);
    assert_eq!(buf[(AREA.width - 1, 0)].bg, sel_bg);
}

fn render_search(doc: &Document, view: &View, search: &SearchState) -> Buffer {
    let theme = Theme::default();
    let mut buf = Buffer::empty(AREA);
    EditorViewWidget::new(
        doc,
        view,
        &theme,
        EditorMode::Search,
        Some(search),
        LineNumberStyle::Absolute,
        true,
    )
    .render(AREA, &mut buf);
    buf
}

fn search_with_match(start: usize) -> SearchState {
    let mut search = SearchState::new();
    search.query = "a".to_string();
    search.matches = vec![SearchMatch {
        start,
        end: start + 1,
    }];
    search.current_match = Some(0);
    search
}

#[test]
fn renders_search_match_at_the_left_edge() {
    let (_file, doc) = long_line_doc("search");
    let search = search_with_match(0);
    let buf = render_search(&doc, &view_at(0, 0), &search);

    let active_bg = Theme::default().ui.search_match_active.to_ratatui();
    assert_eq!(buf[(CODE_START, 0)].bg, active_bg);
}

#[test]
fn renders_search_match_beyond_u16_columns() {
    let (_file, doc) = long_line_doc("search-scrolled");
    // Match sits past column 65535, and the viewport is scrolled onto it.
    let match_col = 150_000;
    let search = search_with_match(match_col);
    let buf = render_search(&doc, &view_at(match_col, match_col - 10), &search);

    let active_bg = Theme::default().ui.search_match_active.to_ratatui();
    assert_eq!(buf[(CODE_START + 10, 0)].bg, active_bg);
    // Only the single matched cell is highlighted.
    assert_ne!(buf[(CODE_START + 9, 0)].bg, active_bg);
    assert_ne!(buf[(CODE_START + 11, 0)].bg, active_bg);
}
