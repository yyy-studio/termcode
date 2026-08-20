use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::Widget;

use termcode_theme::theme::Theme;

/// The thumb. Only the thumb is drawn -- the track stays the editor's
/// background, so the column reads as empty space until there is something to
/// scroll.
///
/// `▐` is East Asian Ambiguous, like the box-drawing glyphs the panel borders
/// already rely on, so it occupies one column in every terminal this project
/// targets. It lives behind a const so swapping it for `│` or a reversed space
/// is one edit.
const THUMB_GLYPH: char = '▐';

/// Where the thumb sits in a track this tall: `(offset_from_track_top, length)`.
///
/// `None` when there is nothing to scroll -- a thumb filling the whole track
/// reads as a scrollable document that will not move.
///
/// `max_top` is the same clamp `View::scroll_down` applies, so the wheel and
/// the thumb cannot disagree about where the bottom is. Both endpoints are
/// exact: `top_line == 0` gives offset 0, and `top_line == max_top` gives
/// `offset + length == track_height`. The middle is allowed to be approximate.
pub fn thumb(track_height: u16, total_lines: usize, top_line: usize) -> Option<(u16, u16)> {
    if track_height == 0 || total_lines <= track_height as usize {
        return None;
    }

    let track = track_height as usize;
    let length = (track * track / total_lines).clamp(1, track);
    let max_top = total_lines - track;
    let travel = track - length;
    let offset = (top_line.min(max_top) * travel)
        .div_ceil(max_top)
        .min(travel);

    Some((offset as u16, length as u16))
}

/// The inverse of [`thumb`]: the top line a thumb dragged to `thumb_offset`
/// selects, clamped to `0..=max_top`.
///
/// `thumb_offset == track_height - length` maps exactly to `max_top`, so a drag
/// to the bottom of the track really reaches the last screen.
pub fn top_line_for_thumb(track_height: u16, total_lines: usize, thumb_offset: u16) -> usize {
    if track_height == 0 || total_lines <= track_height as usize {
        return 0;
    }

    let track = track_height as usize;
    let length = (track * track / total_lines).clamp(1, track);
    let max_top = total_lines - track;
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

    (thumb_offset as usize * max_top / travel).min(max_top)
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

/// Paint the reserved column with the editor background. The column is reserved
/// whatever the tab holds, so a tab with no thumb -- an image, a short document
/// -- must still colour it: a cell nothing writes to keeps the `Color::Reset`
/// background `Cell::reset` gives it, which renders as the terminal's default
/// background rather than the editor's, and the reserved column would read as a
/// vertical stripe down the right-hand edge.
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
                    let top = top_line_for_thumb(track, total, offset);
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
                top_line_for_thumb(1, total, 0),
                0,
                "total={total}: the thumb is drawn at the top, so it means the top"
            );
        }
    }

    #[test]
    fn dragging_to_the_ends_reaches_the_ends() {
        let (_, length) = thumb(20, 100, 0).unwrap();
        assert_eq!(top_line_for_thumb(20, 100, 0), 0);
        assert_eq!(top_line_for_thumb(20, 100, 20 - length), 80);
        // Past the bottom of the track pins to the last screen.
        assert_eq!(top_line_for_thumb(20, 100, 200), 80);
    }

    #[test]
    fn nothing_to_scroll_stays_at_the_top() {
        assert_eq!(top_line_for_thumb(20, 20, 5), 0);
        assert_eq!(top_line_for_thumb(0, 500, 5), 0);
    }
}
