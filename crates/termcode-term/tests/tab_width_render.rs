//! What the renderer draws for a `\t`, checked against what every measurement
//! path says it drew.
//!
//! The defect these guard: the widget expanded a tab to the next **4**-column
//! stop while every measurement counted it as **0** columns, so on any line with
//! a tab the drawn picture and the measured one were two different pictures --
//! the cursor sat in the wrong cell, a click landed on the wrong character, the
//! horizontal scrollbar had no thumb, and a selection over an indent painted
//! nothing.
//!
//! Everything here reads the **real frame buffer**: the widget is rendered and
//! the cells are inspected. A helper agreeing with another helper is what let
//! the two pictures drift in the first place.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

use termcode_core::config_types::EditorConfig;
use termcode_core::diagnostic::{Diagnostic, DiagnosticSeverity};
use termcode_core::position::Position;
use termcode_core::selection::Selection;
use termcode_term::display_width::TabStops;
use termcode_term::ui::editor_view::EditorViewWidget;
use termcode_term::ui::scrollbar::{content_width, offset_for_thumb, thumb};
use termcode_theme::theme::Theme;
use termcode_view::document::{Document, DocumentId};
use termcode_view::editor::EditorMode;
use termcode_view::search::{SearchMatch, SearchState};
use termcode_view::view::{View, ViewId};

/// The tab sizes every case is run at: the default, one wider, one narrower.
/// A rule that only holds at 4 is the hardcoded constant coming back.
const SIZES: [usize; 3] = [4, 8, 2];

/// A one-line document. The gutter is 3 columns wide for any line count under
/// 1000, so the code area starts at `CODE_START` in every fixture here.
const CODE_START: u16 = 4;

fn area(width: u16) -> Rect {
    Rect::new(0, 0, width, 2)
}

fn doc_of(line: &str) -> Document {
    let mut doc = Document::new(DocumentId(0));
    doc.buffer.text_mut().insert(0, &format!("{line}\n"));
    doc
}

fn config(tab_size: usize) -> EditorConfig {
    EditorConfig {
        tab_size,
        ..EditorConfig::default()
    }
}

fn view_at(area: Rect, cursor_col: usize, left_col: usize) -> View {
    let mut view = View::new(ViewId(0), DocumentId(0));
    view.area_width = area.width;
    view.area_height = area.height;
    view.cursor = Position {
        line: 0,
        column: cursor_col,
    };
    view.scroll.left_col = left_col;
    view
}

fn render_at(
    doc: &Document,
    view: &View,
    tab_size: usize,
    area: Rect,
    mode: EditorMode,
    search: Option<&SearchState>,
) -> Buffer {
    let theme = Theme::default();
    let config = config(tab_size);
    let mut buf = Buffer::empty(area);
    EditorViewWidget::new(doc, view, &theme, mode, search, &config, true).render(area, &mut buf);
    buf
}

/// The code area's cells, one entry per **column**, from the first code column
/// to the end of the area.
///
/// A `Vec` of cells rather than a `String`: the columns are what this file is
/// about, and `str::find` would answer in bytes -- three of them for a '\u{ac00}'
/// that occupies two columns.
fn code_cells(buf: &Buffer, area: Rect) -> Vec<String> {
    (CODE_START..area.width)
        .map(|x| buf[(x, 0)].symbol().to_string())
        .collect()
}

/// The first column `ch` is drawn in.
fn first_col(cells: &[String], ch: char) -> Option<usize> {
    cells.iter().position(|c| c == &ch.to_string())
}

/// The last column `ch` is drawn in.
fn last_col(cells: &[String], ch: char) -> Option<usize> {
    cells.iter().rposition(|c| c == &ch.to_string())
}

/// How many columns the renderer painted for `line`, read back from the frame
/// the way the clipped-tab tests read one: select the whole line and see how
/// far the selection background reaches.
///
/// This deliberately does **not** count "one past the last cell holding
/// something other than a blank". That measures a different thing from
/// `content_width` -- which counts a tab's expansion and a trailing space alike
/// -- so any line ending in whitespace measured short, and the agreement below
/// held only because every fixture happened to end in a visible character.
/// `"x\t"` alone broke it.
fn painted_width(line: &str, size: usize, area: Rect) -> usize {
    let mut doc = doc_of(line);
    select_whole_line(&mut doc, line);
    let buf = render_at(
        &doc,
        &view_at(area, 0, 0),
        size,
        area,
        EditorMode::Normal,
        None,
    );
    let sel_bg = Theme::default().ui.selection.to_ratatui();
    (CODE_START..area.width)
        .rposition(|x| buf[(x, 0)].bg == sel_bg)
        .map_or(0, |i| i + 1)
}

/// Every line spec the agreement test walks, each exercising one shape a tab
/// can take part in.
const LINES: [&str; 9] = [
    "\tx",                 // a leading tab
    "\t\t\tx",             // consecutive tabs
    "a\tb",                // a tab between ASCII
    "한\tx",               // a tab after a CJK character: it starts at column 2
    "a한\tx",              // ... and one starting from an odd column
    "e\u{0301}\tx",        // a combining mark immediately before a tab
    "\t한\te\u{0301}x\ty", // all of them on one line
    "x\t",                 // a *trailing* tab: columns nothing is drawn in
    "x ",                  // and a trailing space, which is the same trap
];

#[test]
fn every_character_is_drawn_at_the_column_the_source_names() {
    let area = area(60);
    for size in SIZES {
        let tabs = TabStops::new(size);
        for line in LINES {
            let doc = doc_of(line);
            let view = view_at(area, 0, 0);
            let buf = render_at(&doc, &view, size, area, EditorMode::Normal, None);

            for (i, ch) in line.chars().enumerate() {
                let start = tabs.col_at_char(line, i);
                let next = tabs.next_col(start, ch);
                assert!(
                    next <= (area.width - CODE_START) as usize,
                    "fixture too narrow for size={size} line={line:?}"
                );
                if ch == '\t' {
                    // The whole expansion is blank -- and it is *painted*
                    // blank, which the highlight tests below can see.
                    for c in start..next {
                        assert_eq!(
                            buf[(CODE_START + c as u16, 0)].symbol(),
                            " ",
                            "size={size} line={line:?}: column {c} of a tab"
                        );
                    }
                } else if next > start {
                    assert_eq!(
                        buf[(CODE_START + start as u16, 0)].symbol(),
                        ch.to_string(),
                        "size={size} line={line:?}: char {i} is not at column {start}"
                    );
                }
            }
        }
    }
}

#[test]
fn a_leading_tab_is_a_whole_stop_of_blanks_and_the_text_follows_it() {
    for size in SIZES {
        let area = area(60);
        let doc = doc_of("\tx");
        let buf = render_at(
            &doc,
            &view_at(area, 0, 0),
            size,
            area,
            EditorMode::Normal,
            None,
        );
        let cells = code_cells(&buf, area);
        assert_eq!(
            first_col(&cells, 'x'),
            Some(size),
            "size={size}: a tab at column 0 must reach exactly the first stop"
        );
    }
}

#[test]
fn consecutive_tabs_land_on_consecutive_stops() {
    for size in SIZES {
        let area = area(60);
        let doc = doc_of("\t\t\tx");
        let buf = render_at(
            &doc,
            &view_at(area, 0, 0),
            size,
            area,
            EditorMode::Normal,
            None,
        );
        assert_eq!(
            first_col(&code_cells(&buf, area), 'x'),
            Some(size * 3),
            "size={size}"
        );
    }
}

#[test]
fn a_tab_after_a_cjk_character_completes_the_stop_it_started_in() {
    for size in SIZES {
        let area = area(60);
        // '한' occupies columns 0-1, so the tab runs from column 2.
        let doc = doc_of("한\tx");
        let buf = render_at(
            &doc,
            &view_at(area, 0, 0),
            size,
            area,
            EditorMode::Normal,
            None,
        );
        let expected = (2 / size + 1) * size;
        assert_eq!(
            first_col(&code_cells(&buf, area), 'x'),
            Some(expected),
            "size={size}"
        );
    }
}

/// The line from the defect report: 13 tabs of indentation and 40 characters
/// of code.
fn reported_line() -> String {
    format!("{}{}", "\t".repeat(13), "x".repeat(40))
}

#[test]
fn the_reported_line_ends_at_column_91_at_the_default_tab_size() {
    // 13 tabs x 4 columns = 52, plus 40 characters = 92 columns, so the last
    // painted column is 91.
    let area = area(CODE_START + 100);
    let line = reported_line();
    let doc = doc_of(&line);
    let buf = render_at(
        &doc,
        &view_at(area, 0, 0),
        4,
        area,
        EditorMode::Normal,
        None,
    );
    let cells = code_cells(&buf, area);
    assert_eq!(
        first_col(&cells, 'x'),
        Some(52),
        "the indent is 52 columns wide"
    );
    assert_eq!(
        last_col(&cells, 'x'),
        Some(91),
        "the last character is drawn in column 91"
    );

    // And the scroll total is that column plus one -- the number the thumb is
    // sized from. Before the fix this line measured 40 and had no thumb at all.
    assert_eq!(content_width(&doc, 0, 1, 100, TabStops::new(4)), 92);
}

#[test]
fn the_scroll_total_follows_the_configured_tab_size() {
    let line = reported_line();
    let doc = doc_of(&line);
    for (size, expected) in [(4usize, 92usize), (8, 144), (2, 66)] {
        assert_eq!(
            content_width(&doc, 0, 1, 60, TabStops::new(size)),
            expected,
            "size={size}: 13 tabs of {size} columns plus 40 characters"
        );
    }
}

#[test]
fn the_scroll_total_is_the_last_painted_column_plus_one() {
    let area = area(CODE_START + 160);
    for size in SIZES {
        for line in LINES {
            let doc = doc_of(line);
            assert_eq!(
                content_width(&doc, 0, 1, 160, TabStops::new(size)),
                painted_width(line, size, area),
                "size={size} line={line:?}"
            );
        }
    }
}

#[test]
fn the_thumb_exists_for_the_reported_line_and_reaches_its_end() {
    let line = reported_line();
    let doc = doc_of(&line);
    let code_width = 56u16;
    let total = content_width(&doc, 0, 1, code_width as usize, TabStops::new(4));
    assert_eq!(total, 92);

    let (_, length) = thumb(code_width, total, 0).expect(
        "a 92-column line in a 56-column code area has somewhere to scroll, so it has a thumb",
    );

    // Dragging the thumb to the right end of the track: the line's last
    // character comes on screen.
    let left_col = offset_for_thumb(code_width, total, code_width - length);
    let area = area(CODE_START + code_width);
    let buf = render_at(
        &doc,
        &view_at(area, line.chars().count(), left_col),
        4,
        area,
        EditorMode::Normal,
        None,
    );
    assert_eq!(
        last_col(&code_cells(&buf, area), 'x'),
        Some(91 - left_col),
        "left_col={left_col}: the end of the line is not on screen"
    );
}

fn select_whole_line(doc: &mut Document, line: &str) {
    doc.selection = Selection::single(0, line.len());
}

#[test]
fn a_tab_straddling_the_right_edge_paints_the_columns_that_fit() {
    // A code area two columns wide and a tab four columns long: the two columns
    // that fit are painted, and the 'X' behind the tab is off screen.
    let area = area(CODE_START + 2);
    let line = "\tX";
    let mut doc = doc_of(line);
    select_whole_line(&mut doc, line);
    let buf = render_at(
        &doc,
        &view_at(area, 0, 0),
        4,
        area,
        EditorMode::Normal,
        None,
    );

    let sel_bg = Theme::default().ui.selection.to_ratatui();
    for x in CODE_START..CODE_START + 2 {
        assert_eq!(
            buf[(x, 0)].bg,
            sel_bg,
            "column {} of the clipped tab was left unpainted",
            x - CODE_START
        );
    }
    assert!(
        first_col(&code_cells(&buf, area), 'X').is_none(),
        "the character behind the tab is past the right edge"
    );
}

#[test]
fn a_tab_straddling_left_col_paints_the_columns_that_remain() {
    // `left_col = 2` cuts the 4-column tab in half: its columns 2 and 3 are on
    // screen, and the 'X' at column 4 lands right after them.
    let area = area(CODE_START + 10);
    let line = "\tX";
    let mut doc = doc_of(line);
    select_whole_line(&mut doc, line);
    let buf = render_at(
        &doc,
        &view_at(area, 0, 2),
        4,
        area,
        EditorMode::Normal,
        None,
    );

    let sel_bg = Theme::default().ui.selection.to_ratatui();
    assert_eq!(buf[(CODE_START, 0)].bg, sel_bg, "column 2 of the tab");
    assert_eq!(buf[(CODE_START + 1, 0)].bg, sel_bg, "column 3 of the tab");
    assert_eq!(buf[(CODE_START + 2, 0)].symbol(), "X");
}

/// A search state whose single active match covers the whole of `line`.
fn search_over(line: &str) -> SearchState {
    let mut search = SearchState::new();
    search.query = line.to_string();
    search.matches = vec![SearchMatch {
        start: 0,
        end: line.len(),
    }];
    search.current_match = Some(0);
    search
}

#[test]
fn a_tab_straddling_the_right_edge_paints_the_columns_that_fit_in_search_too() {
    // `visible_span` is shared by the render loop, the selection highlight and
    // the search highlight; the two straddle cases above drive it through the
    // selection, so they are run through the search as well. Otherwise a
    // clipping rule that came back in only one of the three would go unseen.
    let area = area(CODE_START + 2);
    let line = "\tX";
    let doc = doc_of(line);
    let search = search_over(line);
    let buf = render_at(
        &doc,
        &view_at(area, 0, 0),
        4,
        area,
        EditorMode::Search,
        Some(&search),
    );

    let active_bg = Theme::default().ui.search_match_active.to_ratatui();
    for x in CODE_START..CODE_START + 2 {
        assert_eq!(
            buf[(x, 0)].bg,
            active_bg,
            "column {} of the clipped tab was left unpainted",
            x - CODE_START
        );
    }
}

#[test]
fn a_tab_straddling_left_col_paints_the_columns_that_remain_in_search_too() {
    let area = area(CODE_START + 10);
    let line = "\tX";
    let doc = doc_of(line);
    let search = search_over(line);
    let buf = render_at(
        &doc,
        &view_at(area, 0, 2),
        4,
        area,
        EditorMode::Search,
        Some(&search),
    );

    let active_bg = Theme::default().ui.search_match_active.to_ratatui();
    assert_eq!(buf[(CODE_START, 0)].bg, active_bg, "column 2 of the tab");
    assert_eq!(
        buf[(CODE_START + 1, 0)].bg,
        active_bg,
        "column 3 of the tab"
    );
    assert_eq!(buf[(CODE_START + 2, 0)].symbol(), "X");
    assert_eq!(
        buf[(CODE_START + 2, 0)].bg,
        active_bg,
        "the character after the tab is inside the match too"
    );
}

#[test]
fn a_selection_over_an_indent_highlights_every_column_of_it() {
    for size in SIZES {
        let area = area(60);
        let line = "\t\tcode";
        let mut doc = doc_of(line);
        select_whole_line(&mut doc, line);
        let buf = render_at(
            &doc,
            &view_at(area, 0, 0),
            size,
            area,
            EditorMode::Normal,
            None,
        );

        let sel_bg = Theme::default().ui.selection.to_ratatui();
        // Two tabs, then the four characters of "code".
        for c in 0..(size * 2 + 4) {
            assert_eq!(
                buf[(CODE_START + c as u16, 0)].bg,
                sel_bg,
                "size={size}: column {c} of the selection was left unhighlighted"
            );
        }
        // And it stops where the line does.
        assert_ne!(buf[(CODE_START + (size * 2 + 4) as u16, 0)].bg, sel_bg);
    }
}

#[test]
fn a_search_match_over_a_tab_highlights_the_tabs_whole_expansion() {
    for size in SIZES {
        let area = area(60);
        // The match spans "a\tb": three bytes from the start of the line.
        let line = "a\tb";
        let doc = doc_of(line);
        let mut search = SearchState::new();
        search.query = "a\tb".to_string();
        search.matches = vec![SearchMatch { start: 0, end: 3 }];
        search.current_match = Some(0);

        let buf = render_at(
            &doc,
            &view_at(area, 0, 0),
            size,
            area,
            EditorMode::Search,
            Some(&search),
        );

        let active_bg = Theme::default().ui.search_match_active.to_ratatui();
        // 'a' at column 0, the tab from 1 to the next stop, 'b' after it.
        let after_tab = (1 / size + 1) * size;
        for c in 0..=after_tab {
            assert_eq!(
                buf[(CODE_START + c as u16, 0)].bg,
                active_bg,
                "size={size}: column {c} of the match was left unhighlighted"
            );
        }
        assert_ne!(buf[(CODE_START + (after_tab + 1) as u16, 0)].bg, active_bg);
    }
}

#[test]
fn a_diagnostic_over_a_tab_underlines_the_tabs_whole_expansion() {
    for size in SIZES {
        let area = area(60);
        let line = "\t\tcode";
        let mut doc = doc_of(line);
        doc.diagnostics = vec![Diagnostic {
            range: (
                Position { line: 0, column: 0 },
                Position { line: 0, column: 6 },
            ),
            severity: DiagnosticSeverity::Error,
            message: "underline me".to_string(),
            source: None,
        }];
        let buf = render_at(
            &doc,
            &view_at(area, 0, 0),
            size,
            area,
            EditorMode::Normal,
            None,
        );

        for c in 0..(size * 2 + 4) {
            assert!(
                buf[(CODE_START + c as u16, 0)]
                    .style()
                    .add_modifier
                    .contains(ratatui::style::Modifier::UNDERLINED),
                "size={size}: column {c} of the diagnostic was not underlined"
            );
        }
        assert!(
            !buf[(CODE_START + (size * 2 + 4) as u16, 0)]
                .style()
                .add_modifier
                .contains(ratatui::style::Modifier::UNDERLINED),
            "size={size}: the underline ran past the end of the diagnostic"
        );
    }
}
