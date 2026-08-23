use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::Widget;

use crate::display_width::TabStops;
use termcode_theme::theme::Theme;
use termcode_view::document::Document;
use termcode_view::editor::Editor;

/// The thumb. Only the thumb is drawn -- the track stays the editor's
/// background, so the column reads as empty space until there is something to
/// scroll.
///
/// `▐` is East Asian Ambiguous, like the box-drawing glyphs the panel borders
/// already rely on, so it occupies one column in every terminal this project
/// targets. It lives behind a const so swapping it for `│` or a reversed space
/// is one edit.
const THUMB_GLYPH: char = '▐';

/// The horizontal thumb, on the same terms: `▄` is East Asian Ambiguous, the
/// same class as `▐` and as the box-drawing glyphs the panel borders already
/// depend on, so it is one column in every terminal this project targets. It
/// lives behind a const so swapping it for `─`, `█` or a reversed space is one
/// edit.
///
/// The width-safe fallback, if a user ever reports the row shifting: paint a
/// space with `scrollbar_thumb` as the *background*. A space has no width
/// question at all.
const H_THUMB_GLYPH: char = '▄';

/// Where the thumb sits in a track this long: `(offset_from_track_start, length)`.
///
/// There is no axis in this: `total` is however many units of content there
/// are and `offset` how far in the viewport starts -- lines down the document
/// for the vertical bar, display columns across a line for the horizontal one.
/// Both bars call it, so both inherit the same endpoints and the same rounding.
///
/// `None` when there is nothing to scroll -- a thumb filling the whole track
/// reads as scrollable content that will not move.
///
/// `max_offset` is `total - track_len`: the offset at which the last unit of
/// content sits against the far end of the viewport. Vertically that is
/// `View::scroll_down`'s own clamp, so the wheel and the thumb cannot disagree
/// about where the bottom is. Horizontally there is no such clamp on `left_col`
/// -- the cursor may carry the view past everything the bar measures -- and
/// `offset.min(max_offset)` below is what keeps the thumb inside its track when
/// it does. Both endpoints are exact: `offset == 0` gives thumb offset 0, and
/// `offset == max_offset` gives `thumb_offset + length == track_len`. The
/// middle is allowed to be approximate.
pub fn thumb(track_len: u16, total: usize, offset: usize) -> Option<(u16, u16)> {
    if track_len == 0 || total <= track_len as usize {
        return None;
    }

    let track = track_len as usize;
    let length = (track * track / total).clamp(1, track);
    let max_offset = total - track;
    let travel = track - length;
    let thumb_offset = (offset.min(max_offset) * travel)
        .div_ceil(max_offset)
        .min(travel);

    Some((thumb_offset as u16, length as u16))
}

/// The inverse of [`thumb`]: the content offset a thumb dragged to
/// `thumb_offset` selects, clamped to `0..=max_offset`.
///
/// `thumb_offset == track_len - length` maps exactly to `max_offset`, so a drag
/// to the end of the track really reaches the last screen.
pub fn offset_for_thumb(track_len: u16, total: usize, thumb_offset: u16) -> usize {
    if track_len == 0 || total <= track_len as usize {
        return 0;
    }

    let track = track_len as usize;
    let length = (track * track / total).clamp(1, track);
    let max_offset = total - track;
    let travel = track - length;
    if travel == 0 {
        // A thumb with nowhere to travel fills its whole track, and [`thumb`]
        // reports it at offset 0 for every top line. This must answer with the
        // line that offset stands for, or the two disagree: returning `max_top`
        // put the document on its last screen while the thumb sat at the top of
        // the track. Only a one-row track gets here -- the minimum thumb length
        // is 1, so it is the one height where the thumb cannot be shorter than
        // the track it sits in.
        return 0;
    }

    (thumb_offset as usize * max_offset / travel).min(max_offset)
}

/// The editor's vertical scrollbar: a thumb on an otherwise blank column.
pub struct ScrollbarWidget<'a> {
    theme: &'a Theme,
    total_lines: usize,
    top_line: usize,
}

impl<'a> ScrollbarWidget<'a> {
    pub fn new(theme: &'a Theme, total_lines: usize, top_line: usize) -> Self {
        Self {
            theme,
            total_lines,
            top_line,
        }
    }
}

impl Widget for ScrollbarWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        blank(self.theme, area, buf);

        let Some((offset, length)) = thumb(area.height, self.total_lines, self.top_line) else {
            return;
        };

        let style = Style::default()
            .fg(self.theme.ui.scrollbar_thumb.to_ratatui())
            .bg(self.theme.ui.background.to_ratatui());
        for row in 0..length {
            let y = area.y + offset + row;
            for x in area.x..area.x + area.width {
                buf[(x, y)].set_char(THUMB_GLYPH).set_style(style);
            }
        }
    }
}

/// The track inside the reserved horizontal row: the row minus the gutter and
/// the one column separating it from the code, i.e. everything from
/// `row.x + gutter_width + 1` on.
///
/// The gutter does not scroll, so it gets no track (FR-HSCROLL-004) -- a thumb
/// running under the line numbers would claim they move with the text. `None`
/// when the gutter leaves nothing behind, and the caller then draws no thumb.
///
/// This is the single source of the track's columns, shared by
/// [`HScrollbarWidget`]'s caller in `render.rs` and by `mouse.rs`, in the same
/// way `explorer_toolbar::buttons` and `file_explorer::chevron_span` are shared.
/// The corner where the row meets the vertical bar was already excluded when
/// `compute_layout` cut the row out of `editor_area`, so it is not subtracted
/// again here.
pub fn h_track(row: Rect, gutter_width: u16) -> Option<Rect> {
    let lead = gutter_width.saturating_add(1);
    if row.width <= lead || row.height == 0 {
        return None;
    }
    Some(Rect::new(row.x + lead, row.y, row.width - lead, row.height))
}

/// Columns -- and characters -- that one [`content_width`] call may examine
/// across **all** the lines it looks at, together.
///
/// The scan runs once per frame and again on every drag event, so an unbounded
/// one would make a minified file cost O(line length) to draw. The budget is
/// deliberately a *shared* one, not a per-line cap: the cost of a frame is
/// bounded by this number however many lines are on screen, not by this number
/// times the viewport height.
///
/// It is a constant, and it is the last thing in [`content_width`] that could
/// have made the total depend on the horizontal scroll position. A cap of the
/// form `left_col + k` grew the total with every scroll, so the thumb never
/// approached the end of the track: the destination receded as fast as it was
/// approached.
///
/// The price is a horizon: a line wider than the budget cannot be reached past
/// it by dragging the thumb. 50,000 columns is far past any line a person
/// reads, and the cursor motions (`End`, search) still scroll there directly --
/// the view then sits past what the bar measures, the thumb pins to the right
/// end of the track, and dragging it back works from there.
pub const SCAN_BUDGET: usize = 50_000;

/// The horizontal scroll total: the widest line *currently on screen*, as far
/// as [`SCAN_BUDGET`] reaches.
///
/// Two parts, both deliberate:
///
/// - The widest **visible** line, not the widest in the document. A
///   document-wide maximum would have to be invalidated on every edit, undo,
///   redo and LSP-applied change; the cost of getting that wrong is a thumb
///   that lies. The visible consequence is that the thumb changes size as the
///   document scrolls vertically, which is the honest reading of "how far can
///   this screen scroll".
/// - The budget, spent across the visible lines and shared between them: a line
///   is measured only as far as what is left of it, and once it is gone the
///   remaining lines are not measured at all. The `RopeSlice` is walked lazily
///   and abandoned mid-line, so nothing here is proportional to a line's length.
///
/// What is *not* here is as load-bearing: `left_col`. The answer is a function
/// of the document, the viewport and the track alone, so a horizontal drag --
/// which writes `left_col` and nothing else -- cannot move it. An earlier
/// version floored the total at `left_col + code_width` to keep a thumb on
/// screen for a view parked to the right of everything visible, and that floor
/// fed the drag its own output: the pointer named a position, the position
/// moved the total, the moved total renamed the position, and a held pointer
/// crept instead of arriving. What replaces the floor is not another number but
/// a rule in `mouse.rs`: where there is no thumb there is nothing to scroll on
/// this screen, and pressing the empty track returns the view to column 0.
///
/// A line's trailing `\n`/`\r` are not trimmed: neither has a display width, so
/// neither can change the answer -- and trimming would mean finding the end of
/// the line, which is the traversal this is avoiding.
///
/// `code_width == 0` returns 0: there is no track to draw in, so there is
/// nothing worth measuring for.
///
/// `tab_stops` is passed in rather than defaulted: a tab's width is a function
/// of the column it starts at, and this must report the width the *renderer*
/// draws. The stops are counted from column 0 of each line, which is absolute
/// and independent of `left_col` -- so taking them here does not reintroduce
/// the scroll-position dependency the paragraph above removed.
pub fn content_width(
    doc: &Document,
    top_line: usize,
    visible_lines: usize,
    code_width: usize,
    tab_stops: TabStops,
) -> usize {
    if code_width == 0 {
        return 0;
    }

    let end = top_line
        .saturating_add(visible_lines)
        .min(doc.buffer.line_count());

    let mut budget = SCAN_BUDGET;
    let mut widest = 0usize;
    for line in top_line..end {
        if budget == 0 {
            break;
        }
        let (width, scanned) = tab_stops.width_capped_chars(doc.buffer.line(line).chars(), budget);
        widest = widest.max(width);
        // Saturating, not `-=`. `width_capped_chars` is contracted to
        // examine at most `cap` characters, so the subtraction cannot go
        // negative today -- but a wrap here is the *silent* failure of the two:
        // `budget` would land near `usize::MAX` and the next line would be
        // scanned whole, which is exactly the unbounded scan this budget
        // exists to prevent, and it would come back in release builds only.
        // The `debug_assert` is what makes the contract loud where it can be.
        debug_assert!(
            scanned <= budget,
            "the scan examined {scanned} characters of a {budget}-character budget"
        );
        budget = budget.saturating_sub(scanned);
    }

    widest
}

/// The horizontal scroll total for the active view: [`content_width`] of the
/// document on screen, measured over the viewport the track belongs to.
///
/// One function rather than the same three lines written out in `render.rs` and
/// in `mouse.rs`. Both need the identical number, and neither can tell from its
/// own side when the other has drifted -- a thumb drawn through one measurement
/// and dragged through another is a thumb that is not under the pointer holding
/// it.
///
/// There is nothing to latch and nothing to invalidate: the answer depends on
/// the document, `top_line`, the viewport height and `code_width`, and a
/// horizontal drag writes none of them. The press, every drag event and the
/// frame drawn after the release all measure the same thing and get the same
/// answer, so the thumb does not move when the button comes up.
///
/// `code_width` is the track's width -- `h_track` is what defines it -- and the
/// viewport height comes from `view.area_height` rather than from `AppLayout`,
/// so callers on both sides describe one viewport.
pub fn hscroll_total(editor: &Editor, code_width: usize) -> usize {
    let (Some(doc), Some(view)) = (editor.active_document(), editor.active_view()) else {
        return 0;
    };
    content_width(
        doc,
        view.scroll.top_line,
        view.area_height as usize,
        code_width,
        TabStops::from_config(&editor.config),
    )
}

/// The editor's horizontal scrollbar: a thumb on an otherwise blank track.
///
/// Rendered into the **track**, not the whole reserved row -- `render.rs`
/// blanks the row first and then hands this widget what `h_track` cut out of
/// it, so the gutter columns are painted but never scrolled over.
pub struct HScrollbarWidget<'a> {
    theme: &'a Theme,
    total: usize,
    left_col: usize,
}

impl<'a> HScrollbarWidget<'a> {
    pub fn new(theme: &'a Theme, total: usize, left_col: usize) -> Self {
        Self {
            theme,
            total,
            left_col,
        }
    }
}

impl Widget for HScrollbarWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        blank(self.theme, area, buf);

        let Some((offset, length)) = thumb(area.width, self.total, self.left_col) else {
            return;
        };

        let style = Style::default()
            .fg(self.theme.ui.scrollbar_thumb.to_ratatui())
            .bg(self.theme.ui.background.to_ratatui());
        for col in 0..length {
            let x = area.x + offset + col;
            for y in area.y..area.y + area.height {
                buf[(x, y)].set_char(H_THUMB_GLYPH).set_style(style);
            }
        }
    }
}

/// Paint a reserved region -- the scrollbar column, the scrollbar row, the
/// gutter part of that row, or the corner between them -- with the editor
/// background.
///
/// Every one of them is reserved whatever the tab holds, so a tab with no thumb
/// -- an image, a short document -- must still colour it. Not as a staleness
/// guard: ratatui resets the back buffer before every draw. It is that a cell
/// nothing writes to keeps the `Color::Reset` background `Cell::reset` gives
/// it, which the backend emits as the *terminal's* default background rather
/// than the editor's, so the reserved region would read as a stripe or a notch
/// beside the text.
pub fn blank(theme: &Theme, area: Rect, buf: &mut Buffer) {
    let style = Style::default().bg(theme.ui.background.to_ratatui());
    for y in area.y..area.y + area.height {
        for x in area.x..area.x + area.width {
            buf[(x, y)].set_char(' ').set_style(style);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use termcode_view::document::DocumentId;

    /// A document whose lines are the given widths, in order.
    fn doc_of_widths(widths: &[usize]) -> Document {
        let text: String = widths
            .iter()
            .map(|w| format!("{}\n", "x".repeat(*w)))
            .collect();
        let mut doc = Document::new(DocumentId(0));
        doc.buffer.text_mut().insert(0, &text);
        doc
    }

    /// The reserved row of an 80-column frame with a 20-column sidebar, as
    /// `compute_layout` cuts it.
    const ROW: Rect = Rect {
        x: 20,
        y: 22,
        width: 59,
        height: 1,
    };

    #[test]
    fn the_track_starts_after_the_gutter_and_ends_where_the_row_does() {
        let track = h_track(ROW, 3).expect("a track");
        // The gutter plus the one column separating it from the code, exactly
        // as `render.rs` and `mouse.rs` compute `code_start`.
        assert_eq!(track.x, ROW.x + 4);
        assert_eq!(track.y, ROW.y);
        assert_eq!(track.height, ROW.height);
        // The corner was excluded when the row was carved, so `h_track` must
        // not take a second column off the right-hand end.
        assert_eq!(track.x + track.width, ROW.x + ROW.width);

        // Line numbers off: the separator column is still not part of the code.
        let track = h_track(ROW, 0).expect("a track");
        assert_eq!(track.x, ROW.x + 1);
        assert_eq!(track.x + track.width, ROW.x + ROW.width);
    }

    #[test]
    fn a_gutter_that_fills_the_row_leaves_no_track() {
        assert_eq!(h_track(ROW, ROW.width - 1), None);
        assert_eq!(h_track(ROW, ROW.width), None);
        assert_eq!(h_track(ROW, u16::MAX), None);
        assert_eq!(h_track(Rect::new(20, 22, 0, 1), 0), None);
        assert_eq!(h_track(Rect::new(20, 22, 59, 0), 3), None);
    }

    #[test]
    fn the_widest_visible_line_is_the_total() {
        // The wide lines are above and below the viewport; only rows 2..5 count.
        // Every line here is far inside `SCAN_BUDGET`, so what is being
        // measured is the *viewport*, not the bound.
        let doc = doc_of_widths(&[150, 10, 30, 12, 7, 140]);
        assert_eq!(content_width(&doc, 1, 4, 20, TabStops::new(4)), 30);
        // Slide the viewport onto the wide first line and it takes over.
        assert_eq!(content_width(&doc, 0, 4, 20, TabStops::new(4)), 150);
        // And onto the wide last line, likewise.
        assert_eq!(content_width(&doc, 2, 4, 20, TabStops::new(4)), 140);
    }

    #[test]
    fn a_minified_line_is_measured_no_further_than_the_budget() {
        let doc = doc_of_widths(&[SCAN_BUDGET * 4]);
        // A line four times the budget reports the budget, not its length --
        // and nothing here restates how the budget is derived, because it is
        // not derived from anything.
        assert_eq!(content_width(&doc, 0, 1, 40, TabStops::new(4)), SCAN_BUDGET);
    }

    #[test]
    fn a_held_column_names_one_position_and_keeps_it_in_every_regime() {
        // What the drag rests on, at the arithmetic level: measure the total,
        // map the pointer through it, write the result to `left_col`, measure
        // again -- and get the same number, from the first application on.
        //
        // Nothing latched, and no fixed point argued for: the total simply does
        // not take `left_col`, so the mapping cannot be fed its own output. The
        // earlier design floored the total at `left_col + code_width` and this
        // walked instead of settling (408, 340, 289, 251, ... for one column of
        // one fixture), which is what the latch was added to hide.
        //
        // A column short of the end of the track is the only useful place to
        // test this from: `offset == max_offset` maps to itself whatever the
        // total is, so the far end settles even under a mapping that creeps.
        let code_width = 54usize;
        let track = code_width as u16;

        // Three regimes: the budget bounds the total; the visible lines are
        // short while `left_col` is parked far right of them; and `left_col` is
        // out past the scan horizon entirely.
        let minified = doc_of_widths(&[SCAN_BUDGET * 4]);
        let short_lines = doc_of_widths(&[8, 12, 9]);
        let cases = [
            (&minified, 3usize, 0usize),
            (&minified, 3, 150_000),
            (&short_lines, 3, 500),
        ];

        for (doc, visible, start) in cases {
            for held in [0u16, 7, track / 3, track / 2, track - 2] {
                let mut left_col = start;
                // The starting position is in the trace so that "it settled"
                // is read against where the drag began, not asserted in a
                // vacuum: everything from the first mapping on must be one
                // number, wherever the view was when the pointer took hold.
                let mut seen = vec![left_col];
                for _ in 0..8 {
                    let total = content_width(doc, 0, visible, code_width, TabStops::new(4));
                    left_col = offset_for_thumb(track, total, held);
                    seen.push(left_col);
                }
                assert!(
                    seen[1..].windows(2).all(|w| w[0] == w[1]),
                    "held={held} start={start}: the view walked instead of settling: {seen:?}"
                );
            }
        }
    }

    #[test]
    fn the_budget_is_shared_across_the_visible_lines() {
        // A per-line cap would answer 45,000: every line would get the whole
        // budget to itself and the widest would win. A shared budget cannot --
        // the first line spends 40,000 of it, the second is measured only as
        // far as the ~10,000 that is left, and the third is never looked at.
        // The cost of a frame is therefore one budget, not one per row.
        let doc = doc_of_widths(&[40_000, 30_000, 45_000]);
        assert_eq!(content_width(&doc, 0, 3, 40, TabStops::new(4)), 40_000);

        // Order is what decides which line the budget reaches, which is the
        // honest consequence of bounding the scan: put the widest first and it
        // is the one that is fully measured.
        let doc = doc_of_widths(&[45_000, 30_000, 40_000]);
        assert_eq!(content_width(&doc, 0, 3, 40, TabStops::new(4)), 45_000);

        // Lines that comfortably fit the budget are all measured in full, so
        // ordinary documents are unaffected by any of this.
        let doc = doc_of_widths(&[10, 900, 40, 120]);
        assert_eq!(content_width(&doc, 0, 4, 40, TabStops::new(4)), 900);
    }

    #[test]
    fn a_line_of_zero_width_characters_does_not_run_away_with_the_budget() {
        // A combining mark has no display width, so a column budget alone would
        // never fill and the scan would walk the whole line. The character half
        // of the budget is what stops it: the first line spends the entire
        // budget on characters that add no width, so it contributes 0 columns
        // and the second line is never reached. (A run of tabs was this
        // example until tabs became column-accurate -- see
        // `a_line_of_tabs_is_bounded_by_the_column_half_of_the_budget` for what
        // happens to that shape now.)
        //
        // That the 80-column line under it goes unmeasured is the honest cost
        // of a shared budget, not a defect: a line of this shape is pathological
        // and the alternative is a frame that walks it to the end.
        //
        // Removing the character half of the cap gives the *same* answer -- a
        // line of zero-width characters measures 0 however far it is walked --
        // so what catches that here is the `debug_assert` inside
        // `content_width`, which fires when the scan reports having examined
        // more than it was lent. The cost itself is asserted directly in
        // `tests/hscrollbar_render.rs`.
        let mut doc = Document::new(DocumentId(0));
        doc.buffer.text_mut().insert(
            0,
            &format!(
                "{}\n{}\n",
                "\u{0301}".repeat(SCAN_BUDGET * 4),
                "x".repeat(80)
            ),
        );
        assert_eq!(
            content_width(&doc, 0, 2, 40, TabStops::new(4)),
            0,
            "the first line spent the whole budget and the second was not reached"
        );
    }

    #[test]
    fn a_line_of_tabs_is_bounded_by_the_column_half_of_the_budget() {
        // The other half of the pair above. A tab is measured as the columns it
        // is drawn in, so a line of them fills the *column* budget after
        // `SCAN_BUDGET / tab_size` characters -- a quarter of the walk, and the
        // reported width is the budget rather than 0.
        let mut doc = Document::new(DocumentId(0));
        doc.buffer
            .text_mut()
            .insert(0, &format!("{}\n", "\t".repeat(SCAN_BUDGET)));
        assert_eq!(content_width(&doc, 0, 1, 40, TabStops::new(4)), SCAN_BUDGET);
        // And the stops the caller asks for are the stops that are measured.
        let mut doc = Document::new(DocumentId(0));
        doc.buffer.text_mut().insert(0, "\t\t\tx\n");
        assert_eq!(content_width(&doc, 0, 1, 40, TabStops::new(4)), 13);
        assert_eq!(content_width(&doc, 0, 1, 40, TabStops::new(8)), 25);
        assert_eq!(content_width(&doc, 0, 1, 40, TabStops::new(2)), 7);
    }

    #[test]
    fn a_left_col_past_every_visible_line_leaves_no_thumb_at_all() {
        // The long line has scrolled out of view but `left_col` stayed behind
        // it. The total is what is *on screen*, which fits, so there is nothing
        // to scroll and no thumb -- the honest reading, and the one that keeps
        // the total free of `left_col`.
        //
        // This is the assertion the floor would break: floored at `left_col +
        // code_width` the total is 540 here and a thumb is drawn pinned right.
        // The way back to the content is `mouse.rs`'s rule that a press on an
        // empty track returns the view to column 0, tested there.
        let doc = doc_of_widths(&[8, 12, 9]);
        let code_width = 40usize;
        let total = content_width(&doc, 0, 3, code_width, TabStops::new(4));
        assert_eq!(total, 12, "the widest visible line, nothing else");
        assert_eq!(thumb(code_width as u16, total, 500), None);
    }

    #[test]
    fn an_empty_document_and_an_empty_viewport_measure_nothing() {
        let empty = Document::new(DocumentId(0));
        assert_eq!(content_width(&empty, 0, 30, 40, TabStops::new(4)), 0);
        assert_eq!(content_width(&empty, 99, 30, 40, TabStops::new(4)), 0);

        let doc = doc_of_widths(&[100, 100]);
        // Zero visible lines: nothing to scan.
        assert_eq!(content_width(&doc, 0, 0, 40, TabStops::new(4)), 0);
        // A viewport past the end of the document, likewise.
        assert_eq!(content_width(&doc, 500, 30, 40, TabStops::new(4)), 0);
        // No track: nothing to draw in, and no scan worth running.
        assert_eq!(content_width(&doc, 0, 30, 0, TabStops::new(4)), 0);
    }

    #[test]
    fn content_that_fits_the_code_area_has_no_horizontal_thumb() {
        let doc = doc_of_widths(&[10, 20, 5]);
        let code_width = 40usize;
        let total = content_width(&doc, 0, 3, code_width, TabStops::new(4));
        assert_eq!(thumb(code_width as u16, total, 0), None);
    }

    #[test]
    fn the_horizontal_ends_are_exact() {
        let doc = doc_of_widths(&[400]);
        for code_width in [2usize, 10, 40, 57] {
            let total = content_width(&doc, 0, 1, code_width, TabStops::new(4));
            let track = code_width as u16;

            let (offset, _) = thumb(track, total, 0).expect("a thumb");
            assert_eq!(
                offset, 0,
                "code_width={code_width}: left_col 0 is the start"
            );

            let max_left = total - code_width;
            let (offset, length) = thumb(track, total, max_left).expect("a thumb");
            assert_eq!(
                offset + length,
                track,
                "code_width={code_width}: max_left reaches the end"
            );
        }
    }

    #[test]
    fn a_one_column_code_area_is_the_degenerate_horizontal_track() {
        // A wide gutter in a narrow pane gets here far more easily than any
        // terminal gets to a one-row editor: the thumb fills its whole track
        // and has nowhere to travel.
        let doc = doc_of_widths(&[400]);
        let total = content_width(&doc, 0, 1, 1, TabStops::new(4));
        assert_eq!(thumb(1, total, 0), Some((0, 1)));
        assert_eq!(thumb(1, total, total - 1), Some((0, 1)));
        assert_eq!(
            offset_for_thumb(1, total, 0),
            0,
            "the thumb is drawn at the start, so it means the start"
        );
    }

    #[test]
    fn a_horizontal_thumb_offset_round_trips_back_to_itself() {
        for code_width in [1u16, 2, 5, 20, 40, 57] {
            for widest in [code_width as usize + 1, 120, 999, 100_000] {
                let doc = doc_of_widths(&[widest]);
                let total = content_width(&doc, 0, 1, code_width as usize, TabStops::new(4));
                let (_, length) = thumb(code_width, total, 0).unwrap();
                let travel = code_width - length;
                for offset in 0..=travel {
                    let left_col = offset_for_thumb(code_width, total, offset);
                    let (round_tripped, _) = thumb(code_width, total, left_col).unwrap();
                    assert_eq!(
                        round_tripped, offset,
                        "code_width={code_width} total={total} offset={offset}"
                    );
                }
            }
        }
    }

    #[test]
    fn a_document_that_fits_has_no_thumb() {
        assert_eq!(thumb(20, 20, 0), None);
        assert_eq!(thumb(20, 5, 0), None);
        assert_eq!(thumb(0, 1000, 0), None);
    }

    #[test]
    fn the_top_of_the_document_puts_the_thumb_at_the_top() {
        let (offset, length) = thumb(20, 100, 0).unwrap();
        assert_eq!(offset, 0);
        assert_eq!(length, 4);
    }

    #[test]
    fn the_bottom_of_the_document_puts_the_thumb_against_the_bottom() {
        for (track, total) in [(20u16, 100usize), (10, 11), (37, 4000), (5, 1_000_000)] {
            let max_top = total - track as usize;
            let (offset, length) = thumb(track, total, max_top).unwrap();
            assert_eq!(
                offset + length,
                track,
                "track={track} total={total} must reach the bottom"
            );
        }
    }

    #[test]
    fn a_huge_document_still_has_a_visible_thumb() {
        let (offset, length) = thumb(20, 1_000_000, 0).unwrap();
        assert_eq!(length, 1);
        assert_eq!(offset, 0);
    }

    #[test]
    fn the_thumb_never_runs_past_the_track() {
        for track in [1u16, 2, 5, 20, 37, 60] {
            for total in [track as usize + 1, 100, 999, 100_000] {
                let max_top = total - track as usize;
                for top in [0, 1, max_top / 3, max_top / 2, max_top - 1, max_top] {
                    let (offset, length) = thumb(track, total, top).unwrap();
                    assert!(length >= 1);
                    assert!(
                        offset + length <= track,
                        "track={track} total={total} top={top} -> {offset}+{length}"
                    );
                }
            }
        }
    }

    #[test]
    fn a_thumb_offset_round_trips_back_to_itself() {
        // `1` is the height where the thumb fills the track and has nowhere to
        // travel: the only offset it can be dragged to is 0, and that must read
        // back as the line `thumb` draws at offset 0.
        for track in [1u16, 2, 5, 20, 37, 60] {
            for total in [track as usize + 1, 100, 999, 100_000] {
                let travel = {
                    let (_, length) = thumb(track, total, 0).unwrap();
                    track - length
                };
                for offset in 0..=travel {
                    let top = offset_for_thumb(track, total, offset);
                    let (round_tripped, _) = thumb(track, total, top).unwrap();
                    assert_eq!(
                        round_tripped, offset,
                        "track={track} total={total} offset={offset} -> top={top}"
                    );
                }
            }
        }
    }

    #[test]
    fn a_thumb_with_nowhere_to_travel_agrees_with_the_one_that_is_drawn() {
        // A one-row track: the minimum thumb length fills it, so offset 0 is the
        // only position there is and every top line draws the same thumb. The
        // round trip above cannot see this -- an offset that is always 0 comes
        // back as 0 whatever the line in between -- so the *line* is what has to
        // be checked here.
        for total in [2usize, 100, 100_000] {
            assert_eq!(thumb(1, total, 0), Some((0, 1)));
            assert_eq!(thumb(1, total, total - 1), Some((0, 1)));
            assert_eq!(
                offset_for_thumb(1, total, 0),
                0,
                "total={total}: the thumb is drawn at the top, so it means the top"
            );
        }
    }

    #[test]
    fn dragging_to_the_ends_reaches_the_ends() {
        let (_, length) = thumb(20, 100, 0).unwrap();
        assert_eq!(offset_for_thumb(20, 100, 0), 0);
        assert_eq!(offset_for_thumb(20, 100, 20 - length), 80);
        // Past the bottom of the track pins to the last screen.
        assert_eq!(offset_for_thumb(20, 100, 200), 80);
    }

    #[test]
    fn nothing_to_scroll_stays_at_the_top() {
        assert_eq!(offset_for_thumb(20, 20, 5), 0);
        assert_eq!(offset_for_thumb(0, 500, 5), 0);
    }
}
