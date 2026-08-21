//! Rendering regression tests for the editor's horizontal scrollbar.
//!
//! The row is reserved whatever the tab holds, so what is drawn in it -- a
//! thumb, or nothing at all -- is as load-bearing as where the thumb sits.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

use termcode_term::ui::editor_view::display_width_capped_chars;
use termcode_term::ui::scrollbar::{
    self, HScrollbarWidget, SCAN_BUDGET, content_width, h_track, offset_for_thumb, thumb,
};
use termcode_theme::theme::Theme;
use termcode_view::document::{Document, DocumentId};

/// The reserved row of an 80-column frame with a 20-column sidebar, as
/// `compute_layout` cuts it, and the track a 3-column gutter leaves in it.
const ROW: Rect = Rect {
    x: 20,
    y: 22,
    width: 59,
    height: 1,
};
const GUTTER: u16 = 3;

fn track() -> Rect {
    h_track(ROW, GUTTER).expect("a track")
}

/// The columns of the track carrying a thumb glyph.
fn thumb_cols(buf: &Buffer, area: Rect) -> Vec<u16> {
    (area.x..area.x + area.width)
        .filter(|&x| buf[(x, area.y)].symbol() != " ")
        .collect()
}

fn doc_of_widths(widths: &[usize]) -> Document {
    let text: String = widths
        .iter()
        .map(|w| format!("{}\n", "x".repeat(*w)))
        .collect();
    let mut doc = Document::new(DocumentId(0));
    doc.buffer.text_mut().insert(0, &text);
    doc
}

/// A frame-sized buffer, so the row's real coordinates are addressable.
fn frame_buffer() -> Buffer {
    Buffer::empty(Rect::new(0, 0, 80, 24))
}

fn render(total: usize, left_col: usize) -> Buffer {
    let theme = Theme::default();
    let mut buf = frame_buffer();
    HScrollbarWidget::new(&theme, total, left_col).render(track(), &mut buf);
    buf
}

#[test]
fn content_that_fits_draws_no_thumb_but_keeps_the_track() {
    let doc = doc_of_widths(&[10, 20, 5]);
    let code_width = track().width as usize;
    let total = content_width(&doc, 0, 3, code_width);
    let buf = render(total, 0);

    let area = track();
    assert!(
        thumb_cols(&buf, area).is_empty(),
        "no thumb when everything fits"
    );
    // The track is still painted with the editor background: an unpainted cell
    // keeps `Color::Reset`, which shows as the terminal's own background.
    let theme = Theme::default();
    for x in area.x..area.x + area.width {
        assert_eq!(buf[(x, area.y)].symbol(), " ");
        assert_eq!(buf[(x, area.y)].bg, theme.ui.background.to_ratatui());
    }
}

#[test]
fn the_thumb_is_drawn_where_the_geometry_says() {
    let area = track();
    let doc = doc_of_widths(&[400]);
    let code_width = area.width as usize;
    let total = content_width(&doc, 0, 1, code_width);
    let buf = render(total, 40);

    let (offset, length) = thumb(area.width, total, 40).expect("a thumb");
    let cols = thumb_cols(&buf, area);
    assert_eq!(cols.len(), length as usize);
    assert_eq!(cols[0], area.x + offset);
    assert_eq!(*cols.last().unwrap(), area.x + offset + length - 1);
}

#[test]
fn the_thumb_is_one_row_tall_and_a_single_glyph() {
    let area = track();
    let buf = render(5_000, 0);
    let cols = thumb_cols(&buf, area);
    assert!(!cols.is_empty());
    for x in cols {
        let symbol = buf[(x, area.y)].symbol();
        assert_eq!(
            symbol.chars().count(),
            1,
            "the thumb glyph must occupy one cell"
        );
    }
}

#[test]
fn the_ends_of_the_content_pin_the_thumb_to_the_ends_of_the_track() {
    let area = track();
    let total = 5_000usize;
    let left = thumb_cols(&render(total, 0), area);
    assert_eq!(left[0], area.x, "left_col 0 starts at the track's start");

    let max_left = total - area.width as usize;
    let right = thumb_cols(&render(total, max_left), area);
    assert_eq!(
        *right.last().unwrap(),
        area.x + area.width - 1,
        "max_left reaches the track's end"
    );
}

#[test]
fn an_inherited_left_col_leaves_the_track_empty_rather_than_inventing_a_thumb() {
    // The long line has scrolled out of view; `left_col` stayed behind it.
    // Everything on screen fits, so the bar says so and draws nothing: the
    // total is what is on screen, and `left_col` is not part of it. `mouse.rs`
    // is what leads back -- a press on the empty track returns to column 0.
    let area = track();
    let doc = doc_of_widths(&[8, 12, 9]);
    let code_width = area.width as usize;
    let left_col = 500usize;
    let total = content_width(&doc, 0, 3, code_width);

    assert!(thumb_cols(&render(total, left_col), area).is_empty());
}

#[test]
fn a_left_col_past_what_the_bar_measures_pins_the_thumb_rather_than_overflowing() {
    // The other way to end up right of the bar's reach, and the one that still
    // has a thumb: the line on screen is wider than the code area but the
    // cursor has carried `left_col` past the scan horizon. `thumb`'s clamp is
    // what keeps it inside the track, now that no floor grows the total to meet
    // it.
    let area = track();
    let doc = doc_of_widths(&[SCAN_BUDGET * 4]);
    let code_width = area.width as usize;
    let total = content_width(&doc, 0, 1, code_width);
    assert_eq!(total, SCAN_BUDGET);

    let cols = thumb_cols(&render(total, 150_000), area);
    assert_eq!(*cols.last().unwrap(), area.x + area.width - 1);
    assert!(cols.iter().all(|x| *x < area.x + area.width));
}

#[test]
fn the_gutter_columns_of_the_row_carry_no_track() {
    // `h_track` is the single source of the track's columns, and it starts
    // after the gutter plus its separator -- the gutter does not scroll, so a
    // thumb under the line numbers would claim that they do.
    let area = track();
    assert_eq!(area.x, ROW.x + GUTTER + 1);
    assert_eq!(area.x + area.width, ROW.x + ROW.width);

    let buf = render(5_000, 0);
    for x in ROW.x..area.x {
        assert_eq!(
            buf[(x, ROW.y)].symbol(),
            " ",
            "column {x} is the gutter's, not the track's"
        );
    }
}

#[test]
fn blanking_paints_the_reserved_row_with_the_editor_background() {
    // Not a staleness guard -- ratatui resets the back buffer before every
    // draw. The point is the *background*: `Cell::reset` leaves `Color::Reset`,
    // which the backend emits as the terminal's default, so an unpainted row
    // would be a stripe under the editor and an unpainted corner a notch.
    let theme = Theme::default();
    let mut buf = render(5_000, 0);
    assert!(!thumb_cols(&buf, track()).is_empty());

    scrollbar::blank(&theme, ROW, &mut buf);
    assert!(
        thumb_cols(&buf, track()).is_empty(),
        "an image tab leaves the whole row blank"
    );
    for x in ROW.x..ROW.x + ROW.width {
        assert_eq!(
            buf[(x, ROW.y)].bg,
            theme.ui.background.to_ratatui(),
            "column {x} was left on the terminal default background"
        );
    }
}

#[test]
fn dragging_the_thumb_and_reading_it_back_agree() {
    // What `mouse.rs` does on a drag, then what `render.rs` does with the
    // result: the two must not disagree, or the thumb drifts under the pointer.
    let area = track();
    let total = 2_000usize;
    let travel = {
        let (_, length) = thumb(area.width, total, 0).unwrap();
        area.width - length
    };
    for offset in 0..=travel {
        let left_col = offset_for_thumb(area.width, total, offset);
        let cols = thumb_cols(&render(total, left_col), area);
        assert_eq!(
            cols[0],
            area.x + offset,
            "offset {offset} -> left_col {left_col}"
        );
    }
}

#[test]
fn the_thumb_resizes_as_the_widest_visible_line_changes() {
    // Inherent to a visible-lines total, and deliberate: the alternative was a
    // document-wide cache that every edit would have to invalidate.
    let area = track();
    let code_width = area.width as usize;
    let doc = doc_of_widths(&[400, 5, 5, 5]);

    let wide = content_width(&doc, 0, 2, code_width);
    let narrow = content_width(&doc, 2, 2, code_width);
    assert!(wide > narrow);

    let wide_cols = thumb_cols(&render(wide, 0), area).len();
    assert!(wide_cols > 0);
    assert_eq!(
        thumb(area.width, narrow, 0),
        None,
        "the short lines fit, so the thumb goes away entirely"
    );
}

#[test]
fn a_track_narrower_than_the_thumb_maths_does_not_panic() {
    let theme = Theme::default();
    for width in [0u16, 1, 2] {
        let area = Rect::new(0, 0, width, 1);
        let mut buf = Buffer::empty(Rect::new(0, 0, width.max(1), 1));
        HScrollbarWidget::new(&theme, 10_000, 5_000).render(area, &mut buf);
    }
    // And a row with nothing left in it after the gutter has no track at all.
    assert_eq!(h_track(ROW, ROW.width), None);
}

#[test]
fn the_scan_costs_the_budget_and_not_the_length_of_the_line() {
    // The observable *cost*, not the value it returns. This is the half the
    // value cannot show: collecting the line into a `String` before the capped
    // loop returned exactly the right number while walking and copying every
    // character of the line first -- 26.8 ms per call on a five-million-column
    // line, once per visible line per frame. `content_width` measures each
    // line through this function, so bounding what it pulls is what bounds the
    // frame.
    for len in [SCAN_BUDGET * 2, SCAN_BUDGET * 20] {
        let mut pulled = 0usize;
        let (width, scanned) = {
            let chars = (0..len).map(|_| 'x').inspect(|_| pulled += 1);
            display_width_capped_chars(chars, SCAN_BUDGET)
        };
        assert_eq!(
            (width, scanned, pulled),
            (SCAN_BUDGET, SCAN_BUDGET, SCAN_BUDGET),
            "len={len}: twenty times the line must not be twenty times the work"
        );
    }

    // And the half a column budget alone does not bound. A tab has no display
    // width, so on a line built out of them the width never reaches the cap and
    // only the *character* limit can stop the walk. The returned width is 0
    // either way -- it is the count of characters pulled that tells the two
    // implementations apart.
    for len in [SCAN_BUDGET * 2, SCAN_BUDGET * 20] {
        let mut pulled = 0usize;
        let (width, scanned) = {
            let chars = (0..len).map(|_| '\t').inspect(|_| pulled += 1);
            display_width_capped_chars(chars, SCAN_BUDGET)
        };
        assert_eq!(
            (width, scanned, pulled),
            (0, SCAN_BUDGET, SCAN_BUDGET),
            "len={len}: a line of zero-width characters was walked to its end"
        );
    }
}

#[test]
fn a_minified_line_leaves_the_thumb_where_the_last_frame_drew_it() {
    // Two frames of the same screen, one drawn after a drag has written
    // `left_col`: the measurement takes the same four inputs both times and the
    // thumb lands in the same columns. While the cap was `left_col + k` the
    // total grew with every frame, the end of the track moved out from under
    // the thumb, and it crawled rightwards without ever arriving.
    let area = track();
    let code_width = area.width as usize;
    let doc = doc_of_widths(&[SCAN_BUDGET * 4]);

    let total = content_width(&doc, 0, 1, code_width);
    assert_eq!(
        total, SCAN_BUDGET,
        "the budget, not the line, is what bounds a line this wide"
    );

    let (_, length) = thumb(area.width, total, 0).expect("a thumb");
    for offset in [0u16, 7, area.width / 2, area.width - length] {
        let left_col = offset_for_thumb(area.width, total, offset);
        let redrawn = content_width(&doc, 0, 1, code_width);
        assert_eq!(redrawn, total, "the total moved between two frames");
        assert_eq!(
            thumb_cols(&render(redrawn, left_col), area),
            thumb_cols(&render(total, left_col), area),
            "offset {offset}: the two frames disagree"
        );
    }

    let end = offset_for_thumb(area.width, total, area.width - length);
    let cols = thumb_cols(&render(total, end), area);
    assert_eq!(
        cols.last().copied(),
        Some(area.x + area.width - 1),
        "a drag to the end of the track draws the thumb against the end of it"
    );
}
