//! Terminal display widths, in two halves that must not be mixed up.
//!
//! **Buffer text** -- [`TabStops`]. Document text can contain a literal `\t`,
//! and a tab's width is a function of the **column it starts at**, not a
//! property of the character: it advances to the next multiple of `tab_size`.
//! That is why this half offers no `fn width(ch) -> usize` at all. A
//! per-character width function cannot answer for a tab, and the answer the old
//! one gave (0, because `UnicodeWidthChar::width` returns `None`) is precisely
//! what let the renderer and every measurement path draw two different
//! pictures of the same line. Here you accumulate a column through
//! [`TabStops::next_col`] or you get nothing.
//!
//! **UI strings** -- the `ui_*` free functions. A status bar segment, a tab
//! label, a hover line, a settings row, a dialog button, a search query: text
//! the editor composes or the user types into a one-line input, none of which
//! can contain a literal tab. These are tab-unaware on purpose, and named so a
//! call site cannot pick the wrong half by accident: `TabStops` is a type you
//! must construct with a `tab_size`, `ui_*` plainly says what it is for.

use termcode_core::config_types::EditorConfig;
use unicode_width::UnicodeWidthChar;

/// The single source of the rule that a tab's width is a function of the column
/// it starts at.
///
/// There is deliberately **no `Default` impl**: a `TabStops::default()` would be
/// a silent `tab_size` of 4, which is exactly the hardcoded constant this type
/// exists to remove. [`TabStops::from_config`] is the only intended constructor
/// outside tests -- every caller in the editor holds an `&Editor` or an
/// `&EditorConfig` and can build one on the spot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TabStops {
    size: usize,
}

impl TabStops {
    /// The stops the running configuration asks for.
    pub fn from_config(config: &EditorConfig) -> Self {
        Self::new(config.tab_size)
    }

    /// Stops every `size` columns, clamped to `1..=64` -- both ends for the
    /// same reason, and in the same place, because both come from the same
    /// input. A hand-edited `config.toml` can say `tab_size = 0`, and this is
    /// the only code that divides by it, so the division by zero in
    /// [`Self::next_col`] is prevented here rather than at every call site; it
    /// can equally say `tab_size = 18446744073709551615`, on which the stop
    /// arithmetic `(col / size + 1) * size` overflows. The settings screen
    /// offers at most 16, so the ceiling is well clear of anything a user can
    /// choose from the UI and only bites values that were never meant to be
    /// typed.
    pub fn new(size: usize) -> Self {
        Self {
            size: size.clamp(1, 64),
        }
    }

    /// The column at which the character *after* `ch` begins, given that `ch`
    /// begins at `col`. **The** rule; every other method here is a fold over
    /// it, so a tab, a CJK character and a combining mark are advanced by one
    /// line of code and cannot disagree.
    ///
    /// `col` is counted from column 0 of the line, never from `left_col`:
    /// scrolling horizontally must not move where the stops fall.
    pub fn next_col(&self, col: usize, ch: char) -> usize {
        if ch == '\t' {
            (col / self.size + 1) * self.size
        } else {
            col + ch.width().unwrap_or(0)
        }
    }

    /// The display column at which the character at `char_idx` begins.
    pub fn col_at_char(&self, line: &str, char_idx: usize) -> usize {
        line.chars()
            .take(char_idx)
            .fold(0usize, |col, ch| self.next_col(col, ch))
    }

    /// The index of the character whose span covers `display_col`.
    ///
    /// "Covers" generalises the old "second cell of a wide character" rule to
    /// "any column inside a tab's expansion" without a special case: the
    /// character kept is the first whose `next_col` is past the wanted column.
    /// A column beyond the line's width answers with the line's character
    /// count.
    ///
    /// The scan is deliberately **not** capped. Its callers' columns are
    /// already bounded by the viewport, and a cap here would return a wrong
    /// character index rather than a bounded one.
    pub fn char_at_col(&self, line: &str, display_col: usize) -> usize {
        let mut col = 0usize;
        for (i, ch) in line.chars().enumerate() {
            let next = self.next_col(col, ch);
            if next > display_col {
                return i;
            }
            col = next;
        }
        line.chars().count()
    }

    /// Display width of `line`, stopping once `cap` columns are reached -- or
    /// `cap` characters have been examined, whichever comes first. The `&str`
    /// convenience form of [`Self::width_capped_chars`]; one caller, the
    /// diagnostic underline in `ui::editor_view`.
    pub fn width_capped(&self, line: &str, cap: usize) -> usize {
        self.width_capped_chars(line.chars(), cap).0
    }

    /// The same measurement over a bare character iterator, reporting how much
    /// of the budget it spent: `(width, chars_examined)`.
    ///
    /// It takes an iterator rather than a `&str` so `ui::scrollbar::content_width`
    /// can walk a `RopeSlice` **lazily**. Collecting a line into a `String`
    /// first made the scan O(line length) whatever the cap said, since the
    /// allocation and the copy happened before the loop that stops -- 26.8 ms
    /// per call on a five-million-column line, once per visible line per frame.
    ///
    /// `cap` bounds two things at once, and both are load-bearing:
    ///
    /// - the **columns** counted, which is what the caller wants back, and
    /// - the **characters** examined, which is what the scan costs. A column
    ///   budget alone bounds nothing on a line built out of zero-width
    ///   characters (every combining mark), because such a line advances the
    ///   width by nothing however far it is walked. A run of tabs used to be
    ///   the other example and no longer is: a tab now advances the width by up
    ///   to `size` columns, so the column limit does bite on it.
    ///
    /// Because a tab advances by up to `size`, the column limit can be
    /// overshot by the last character pulled -- `width.min(cap)` is what still
    /// holds the contract that the returned width never exceeds `cap`.
    ///
    /// The count is what lets a caller spread one budget across several lines,
    /// and what a test can observe: feed it a counting iterator and a line ten
    /// times longer must not cost ten times more.
    pub fn width_capped_chars(
        &self,
        mut chars: impl Iterator<Item = char>,
        cap: usize,
    ) -> (usize, usize) {
        let mut width = 0usize;
        let mut scanned = 0usize;
        // The limits are tested *before* the character is pulled, not after, so
        // a scan that stops at the cap really touches `cap` characters and not
        // one more. A `for` loop cannot do that: it has already advanced the
        // iterator by the time the body runs, and the count a test observes
        // would be off by one against the count the budget was decremented by.
        while width < cap && scanned < cap {
            let Some(ch) = chars.next() else { break };
            // `width` is the column the character starts at, so this is the
            // same fold as everywhere else.
            width = self.next_col(width, ch);
            scanned += 1;
        }
        (width.min(cap), scanned)
    }
}

/// Terminal display width of a single character of a **UI string**.
/// CJK/fullwidth characters return 2, most others return 1. Control characters
/// -- a tab among them -- return 0; see the module doc for why that is only
/// correct away from buffer text.
pub fn ui_char_width(ch: char) -> usize {
    ch.width().unwrap_or(0)
}

/// Total display width of a **UI string**.
pub fn ui_str_width(s: &str) -> usize {
    s.chars().map(|ch| ch.width().unwrap_or(0)).sum()
}

/// The display column at which the character at `char_idx` of a **UI string**
/// begins. Used to scroll a one-line input (a search or palette query) by
/// columns rather than by characters.
pub fn ui_col_at_char(text: &str, char_idx: usize) -> usize {
    text.chars()
        .take(char_idx)
        .map(|ch| ch.width().unwrap_or(0))
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIZES: [usize; 3] = [4, 8, 2];

    #[test]
    fn ui_char_width_ascii() {
        assert_eq!(ui_char_width('a'), 1);
        assert_eq!(ui_char_width(' '), 1);
    }

    #[test]
    fn ui_char_width_cjk() {
        assert_eq!(ui_char_width('한'), 2);
        assert_eq!(ui_char_width('글'), 2);
        assert_eq!(ui_char_width('中'), 2);
        assert_eq!(ui_char_width('あ'), 2);
    }

    #[test]
    fn ui_col_at_char_sums_character_widths() {
        let line = "ab한글cd";
        assert_eq!(ui_col_at_char(line, 0), 0);
        assert_eq!(ui_col_at_char(line, 1), 1);
        assert_eq!(ui_col_at_char(line, 2), 2); // before '한'
        assert_eq!(ui_col_at_char(line, 3), 4); // before '글'
        assert_eq!(ui_col_at_char(line, 4), 6); // before 'c'
        assert_eq!(ui_col_at_char(line, 5), 7); // before 'd'
    }

    #[test]
    fn ui_str_width_sums_the_string() {
        assert_eq!(ui_str_width("hello"), 5);
        assert_eq!(ui_str_width("한글"), 4);
        assert_eq!(ui_str_width("ab한글cd"), 8);
    }

    #[test]
    fn the_empty_string_measures_zero_either_way() {
        assert_eq!(ui_col_at_char("", 0), 0);
        assert_eq!(ui_str_width(""), 0);
        for size in SIZES {
            let tabs = TabStops::new(size);
            assert_eq!(tabs.col_at_char("", 0), 0);
            assert_eq!(tabs.char_at_col("", 0), 0);
            assert_eq!(tabs.char_at_col("", 99), 0);
        }
    }

    #[test]
    fn a_tab_advances_to_the_next_stop_from_every_column() {
        for size in SIZES {
            let tabs = TabStops::new(size);
            // Two full stops' worth of starting columns: from anywhere inside a
            // stop the tab lands on that stop's end, never short and never past.
            for col in 0..(size * 2) {
                let expected = (col / size + 1) * size;
                assert_eq!(
                    tabs.next_col(col, '\t'),
                    expected,
                    "size={size} col={col}: a tab must land on the next stop"
                );
                assert!(tabs.next_col(col, '\t') > col, "a tab always advances");
            }
        }
    }

    #[test]
    fn a_tab_at_column_zero_is_a_whole_stop_wide() {
        for size in SIZES {
            assert_eq!(TabStops::new(size).next_col(0, '\t'), size);
        }
    }

    #[test]
    fn consecutive_tabs_land_on_consecutive_stops() {
        for size in SIZES {
            let tabs = TabStops::new(size);
            let line = "\t\t\t";
            for (i, expected) in [size, size * 2, size * 3].into_iter().enumerate() {
                assert_eq!(tabs.col_at_char(line, i + 1), expected, "size={size}");
            }
        }
    }

    #[test]
    fn a_tab_after_a_cjk_character_starts_from_an_even_column() {
        for size in SIZES {
            let tabs = TabStops::new(size);
            // '한' is two columns, so the tab starts at column 2.
            assert_eq!(tabs.col_at_char("한\t", 1), 2);
            assert_eq!(tabs.col_at_char("한\tx", 2), (2 / size + 1) * size);
            // And from an odd column: 'a' then '한' ends at 3.
            assert_eq!(tabs.col_at_char("a한\tx", 3), (3 / size + 1) * size);
        }
    }

    #[test]
    fn a_combining_mark_moves_no_column_on_either_side_of_a_tab() {
        for size in SIZES {
            let tabs = TabStops::new(size);
            // U+0301 COMBINING ACUTE ACCENT is genuinely zero-width.
            assert_eq!(tabs.next_col(3, '\u{0301}'), 3);
            // Before the tab: the tab still starts where the 'e' ended.
            assert_eq!(tabs.col_at_char("e\u{0301}\tx", 3), size);
            // After the tab: the mark adds nothing to the stop the tab reached.
            assert_eq!(tabs.col_at_char("\t\u{0301}x", 2), size);
        }
    }

    #[test]
    fn col_at_char_and_char_at_col_round_trip_over_a_mixed_line() {
        // Tabs, ASCII, CJK and a combining mark on one line.
        let line = "\tab\t한글\te\u{0301}x\t";
        for size in SIZES {
            let tabs = TabStops::new(size);
            let total = tabs.col_at_char(line, line.chars().count());
            for col in 0..total {
                let idx = tabs.char_at_col(line, col);
                let start = tabs.col_at_char(line, idx);
                // The round trip lands on the start column of the character
                // that covers `col` -- which is `col` itself only when `col` is
                // that character's first column.
                assert!(
                    start <= col,
                    "size={size} col={col}: char {idx} starts at {start}, past the column it covers"
                );
                let ch = line.chars().nth(idx).expect("a character");
                assert!(
                    tabs.next_col(start, ch) > col,
                    "size={size} col={col}: char {idx} does not reach the column"
                );
                // Idempotent: measuring the start column again names the same
                // character, which is what makes clicking where the cursor
                // already is leave it alone.
                assert_eq!(tabs.char_at_col(line, start), idx, "size={size} col={col}");
            }
        }
    }

    #[test]
    fn a_column_past_the_line_answers_with_its_character_count() {
        let line = "\tab\t";
        for size in SIZES {
            let tabs = TabStops::new(size);
            let count = line.chars().count();
            let total = tabs.col_at_char(line, count);
            assert_eq!(tabs.char_at_col(line, total), count);
            assert_eq!(tabs.char_at_col(line, total + 500), count);
        }
    }

    #[test]
    fn a_tab_size_of_zero_behaves_as_one_and_does_not_divide_by_zero() {
        let tabs = TabStops::new(0);
        assert_eq!(tabs, TabStops::new(1));
        assert_eq!(tabs.next_col(0, '\t'), 1);
        assert_eq!(tabs.next_col(7, '\t'), 8);
        assert_eq!(tabs.col_at_char("\t\t\t", 3), 3);
        assert_eq!(tabs.char_at_col("\t\t\t", 2), 2);
        // And through the config, which is how it would actually arrive.
        let config = EditorConfig {
            tab_size: 0,
            ..EditorConfig::default()
        };
        assert_eq!(TabStops::from_config(&config), TabStops::new(1));
    }

    #[test]
    fn an_absurd_tab_size_is_capped_and_does_not_overflow() {
        // The other end of the same hand-edited file. `(col / size + 1) * size`
        // overflows for a `size` near `usize::MAX`, which in release builds
        // wraps silently to a column somewhere near zero.
        let tabs = TabStops::new(usize::MAX);
        assert_eq!(tabs, TabStops::new(64));
        assert_eq!(tabs.next_col(0, '\t'), 64);
        assert_eq!(tabs.next_col(64, '\t'), 128);
        assert_eq!(tabs.col_at_char("\t\t", 2), 128);
        let config = EditorConfig {
            tab_size: usize::MAX,
            ..EditorConfig::default()
        };
        assert_eq!(TabStops::from_config(&config), TabStops::new(64));
    }

    #[test]
    fn from_config_takes_the_configured_size() {
        for size in SIZES {
            let config = EditorConfig {
                tab_size: size,
                ..EditorConfig::default()
            };
            assert_eq!(TabStops::from_config(&config), TabStops::new(size));
        }
    }

    /// The observable cost of a capped scan, not the width it returns.
    ///
    /// The width alone cannot see the regression this guards: collecting the
    /// line into a `String` before the loop gave exactly the same answer while
    /// walking and copying every character, 26.8 ms on a five-million-column
    /// line against 3.1 µs. Counting what the iterator is asked for is what
    /// makes the difference visible.
    fn scan_cost(len: usize, ch: char, cap: usize) -> (usize, usize, usize) {
        let mut pulled = 0usize;
        let (width, scanned) = {
            let chars = (0..len).map(|_| ch).inspect(|_| pulled += 1);
            TabStops::new(4).width_capped_chars(chars, cap)
        };
        (width, scanned, pulled)
    }

    #[test]
    fn a_capped_scan_costs_the_cap_however_long_the_line_is() {
        // Ten times the line, and a hundred times, must not be ten or a hundred
        // times the work.
        for len in [10_000usize, 100_000, 1_000_000] {
            assert_eq!(
                scan_cost(len, 'x', 100),
                (100, 100, 100),
                "len={len}: the scan must stop at the cap"
            );
        }
    }

    #[test]
    fn a_line_shorter_than_the_cap_is_scanned_once_and_no_more() {
        assert_eq!(scan_cost(30, 'x', 100), (30, 30, 30));
        assert_eq!(scan_cost(0, 'x', 100), (0, 0, 0));
        // A cap of zero measures nothing at all, which is what lets a caller
        // stop asking once its budget is gone.
        assert_eq!(scan_cost(1_000, 'x', 0), (0, 0, 0));
    }

    #[test]
    fn zero_width_characters_cannot_outrun_the_cap() {
        // The column budget alone bounds nothing here: a combining mark counts
        // as zero columns, so the width never reaches the cap however far the
        // line is walked. The character limit is the half of the cap that stops
        // it.
        //
        // A run of tabs used to be this test's example, back when a tab was
        // measured as zero columns. It is no longer one -- a tab now advances
        // the width to the next stop -- but the character limit it justified is
        // still necessary, which is why the example moved rather than the test
        // going away. See the tab case below for the other half.
        assert_eq!(scan_cost(1_000_000, '\u{0301}', 100), (0, 100, 100));
    }

    #[test]
    fn a_line_of_tabs_is_now_bounded_by_the_column_limit() {
        // Four columns per character at `size = 4`, so a cap of 100 columns is
        // reached in 25 characters -- the half of the cap a run of tabs used to
        // slip past entirely.
        assert_eq!(scan_cost(1_000, '\t', 100), (100, 25, 25));
        // The character limit still applies underneath: a cap of 10 stops after
        // 3 tabs, at 12 columns, reported as the capped 10.
        assert_eq!(scan_cost(1_000, '\t', 10), (10, 3, 3));
    }

    #[test]
    fn a_wide_character_counts_its_columns_and_the_width_stays_capped() {
        // Two columns per character, so the column limit bites first: 50
        // characters fill a cap of 100.
        assert_eq!(scan_cost(1_000, '한', 100), (100, 50, 50));
        // And the returned width never exceeds the cap even when the last
        // character straddles it.
        assert_eq!(scan_cost(1_000, '한', 101), (101, 51, 51));
    }

    #[test]
    fn the_str_form_and_the_iterator_form_agree() {
        for size in SIZES {
            let tabs = TabStops::new(size);
            for (line, cap) in [
                ("hello", 100usize),
                ("hello", 3),
                ("한글abc", 4),
                ("\t\tabc", 100),
                ("\t\tabc", 3),
                ("", 10),
            ] {
                assert_eq!(
                    tabs.width_capped(line, cap),
                    tabs.width_capped_chars(line.chars(), cap).0,
                    "size={size} line={line:?} cap={cap}"
                );
            }
        }
    }

    #[test]
    fn an_uncapped_width_agrees_with_the_column_after_the_last_character() {
        let line = "\tab\t한글\te\u{0301}x\t";
        for size in SIZES {
            let tabs = TabStops::new(size);
            let count = line.chars().count();
            assert_eq!(
                tabs.width_capped(line, usize::MAX),
                tabs.col_at_char(line, count),
                "size={size}"
            );
        }
    }
}
