//! Rendering regression tests for the editor's vertical scrollbar.
//!
//! The column is reserved whatever the tab holds, so what is drawn in it -- a
//! thumb, or nothing at all -- is as load-bearing as where the thumb sits.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

use termcode_term::ui::scrollbar::{self, ScrollbarWidget, thumb, top_line_for_thumb};
use termcode_theme::theme::Theme;

const TRACK: Rect = Rect {
    x: 0,
    y: 0,
    width: 1,
    height: 20,
};

/// The rows of the track carrying a thumb glyph.
fn thumb_rows(buf: &Buffer) -> Vec<u16> {
    (TRACK.y..TRACK.y + TRACK.height)
        .filter(|&y| buf[(TRACK.x, y)].symbol() != " ")
        .collect()
}

fn render(total_lines: usize, top_line: usize) -> Buffer {
    let theme = Theme::default();
    let mut buf = Buffer::empty(TRACK);
    ScrollbarWidget::new(&theme, total_lines, top_line).render(TRACK, &mut buf);
    buf
}

#[test]
fn a_document_that_fits_draws_no_thumb_but_keeps_the_column() {
    let buf = render(20, 0);
    assert!(thumb_rows(&buf).is_empty(), "no thumb for a short document");
    // The column is still painted with the editor background: an unpainted
    // cell keeps `Color::Reset`, which shows as the terminal's own background.
    let theme = Theme::default();
    for y in TRACK.y..TRACK.y + TRACK.height {
        assert_eq!(buf[(TRACK.x, y)].symbol(), " ");
        assert_eq!(buf[(TRACK.x, y)].bg, theme.ui.background.to_ratatui());
    }
}

#[test]
fn the_thumb_is_drawn_where_the_geometry_says() {
    let buf = render(100, 40);
    let (offset, length) = thumb(TRACK.height, 100, 40).expect("a thumb");
    let rows = thumb_rows(&buf);
    assert_eq!(rows.len(), length as usize);
    assert_eq!(rows[0], TRACK.y + offset);
    assert_eq!(*rows.last().unwrap(), TRACK.y + offset + length - 1);
}

#[test]
fn the_thumb_is_one_column_wide_and_a_single_glyph() {
    let buf = render(1000, 0);
    let rows = thumb_rows(&buf);
    assert!(!rows.is_empty());
    for y in rows {
        let symbol = buf[(TRACK.x, y)].symbol();
        assert_eq!(
            symbol.chars().count(),
            1,
            "the thumb glyph must occupy one cell"
        );
    }
}

#[test]
fn the_top_and_the_bottom_of_a_document_pin_the_thumb_to_the_ends() {
    let top = thumb_rows(&render(1000, 0));
    assert_eq!(top[0], TRACK.y);

    let bottom = thumb_rows(&render(1000, 1000 - TRACK.height as usize));
    assert_eq!(*bottom.last().unwrap(), TRACK.y + TRACK.height - 1);
}

#[test]
fn blanking_paints_the_reserved_column_with_the_editor_background() {
    // Not a staleness guard -- ratatui resets the back buffer before every
    // draw. The point is the *background*: a cell nobody writes to keeps
    // `Color::Reset` and renders as the terminal's default, so the reserved
    // column would be a vertical stripe beside the editor.
    let theme = Theme::default();
    let mut buf = render(1000, 0);
    assert!(!thumb_rows(&buf).is_empty());
    scrollbar::blank(&theme, TRACK, &mut buf);
    assert!(thumb_rows(&buf).is_empty(), "an image tab leaves it blank");
    for y in TRACK.y..TRACK.y + TRACK.height {
        assert_eq!(
            buf[(TRACK.x, y)].bg,
            theme.ui.background.to_ratatui(),
            "row {y} was left on the terminal default background"
        );
    }
}

#[test]
fn dragging_the_thumb_and_reading_it_back_agree() {
    // What `mouse.rs` does on a drag, then what `render.rs` does with the
    // result: the two must not disagree, or the thumb drifts under the pointer.
    let travel = {
        let (_, length) = thumb(TRACK.height, 500, 0).unwrap();
        TRACK.height - length
    };
    for offset in 0..=travel {
        let top = top_line_for_thumb(TRACK.height, 500, offset);
        let rows = thumb_rows(&render(500, top));
        assert_eq!(rows[0], TRACK.y + offset, "offset {offset} -> top {top}");
    }
}

#[test]
fn a_track_shorter_than_the_thumb_maths_does_not_panic() {
    for height in [0u16, 1, 2] {
        let area = Rect::new(0, 0, 1, height);
        let theme = Theme::default();
        let mut buf = Buffer::empty(Rect::new(0, 0, 1, height.max(1)));
        ScrollbarWidget::new(&theme, 10_000, 5_000).render(area, &mut buf);
    }
}
