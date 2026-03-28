#[derive(Debug, Clone)]
pub struct SearchMatch {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug)]
pub struct SearchState {
    pub query: String,
    pub matches: Vec<SearchMatch>,
    pub current_match: Option<usize>,
    pub replace_text: String,
    pub replace_mode: bool,
    pub replace_focused: bool,
    pub cursor_pos: usize,
    pub replace_cursor_pos: usize,
}

impl SearchState {
    pub fn new() -> Self {
        Self {
            query: String::new(),
            matches: Vec::new(),
            current_match: None,
            replace_text: String::new(),
            replace_mode: false,
            replace_focused: false,
            cursor_pos: 0,
            replace_cursor_pos: 0,
        }
    }

    pub fn clear(&mut self) {
        self.query.clear();
        self.matches.clear();
        self.current_match = None;
        self.replace_text.clear();
        self.replace_mode = false;
        self.replace_focused = false;
        self.cursor_pos = 0;
        self.replace_cursor_pos = 0;
    }

    /// Perform literal case-insensitive substring search.
    /// Takes `&str` to avoid cross-crate ropey type exposure.
    /// Tracks byte positions in the original text to avoid offset mismatches
    /// caused by lowercasing changing byte lengths.
    pub fn find_matches(&mut self, text: &str) {
        self.matches.clear();
        self.current_match = None;

        if self.query.is_empty() {
            return;
        }

        let query_chars: Vec<char> = self.query.chars().flat_map(|c| c.to_lowercase()).collect();
        if query_chars.is_empty() {
            return;
        }

        // Collect (byte_offset, lowercased_char) pairs from original text
        let text_indexed: Vec<(usize, char)> = text
            .char_indices()
            .flat_map(|(byte_idx, c)| {
                // For chars whose lowercase is a single char (the common case),
                // map to the original byte index. Multi-char lowercase expansions
                // (e.g. German eszett) get the same byte index for all produced chars.
                let lower_chars: Vec<char> = c.to_lowercase().collect();
                lower_chars.into_iter().map(move |lc| (byte_idx, lc))
            })
            .collect();

        let mut i = 0;
        while i + query_chars.len() <= text_indexed.len() {
            let matched = query_chars
                .iter()
                .enumerate()
                .all(|(qi, &qc)| text_indexed[i + qi].1 == qc);
            if matched {
                let match_start = text_indexed[i].0;
                // End byte: find the byte position after the last matched char
                let last_matched_byte = text_indexed[i + query_chars.len() - 1].0;
                // Advance past the last matched original char to get end byte
                let match_end = text[last_matched_byte..]
                    .chars()
                    .next()
                    .map(|c| last_matched_byte + c.len_utf8())
                    .unwrap_or(text.len());
                self.matches.push(SearchMatch {
                    start: match_start,
                    end: match_end,
                });
                // Advance past the match to produce non-overlapping matches
                // (standard find-and-replace behavior, prevents replace_all panic)
                i += query_chars.len();
            } else {
                i += 1;
            }
        }

        // Deduplicate: multi-char lowercase expansions (e.g. 'ß' -> "ss") can
        // produce duplicate ranges since expanded chars share the same source byte index.
        self.matches.dedup_by_key(|m| (m.start, m.end));

        if !self.matches.is_empty() {
            self.current_match = Some(0);
        }
    }

    pub fn next_match(&mut self) {
        if self.matches.is_empty() {
            return;
        }
        self.current_match = Some(match self.current_match {
            Some(idx) => (idx + 1) % self.matches.len(),
            None => 0,
        });
    }

    pub fn prev_match(&mut self) {
        if self.matches.is_empty() {
            return;
        }
        self.current_match = Some(match self.current_match {
            Some(0) => self.matches.len() - 1,
            Some(idx) => idx - 1,
            None => self.matches.len() - 1,
        });
    }

    pub fn current(&self) -> Option<&SearchMatch> {
        self.current_match.and_then(|idx| self.matches.get(idx))
    }

    pub fn match_count(&self) -> usize {
        self.matches.len()
    }
}

impl Default for SearchState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_matches_literal() {
        let mut state = SearchState::new();
        state.query = "hello".to_string();
        state.find_matches("say hello world hello");
        assert_eq!(state.match_count(), 2);
        assert_eq!(state.matches[0].start, 4);
        assert_eq!(state.matches[0].end, 9);
        assert_eq!(state.matches[1].start, 16);
        assert_eq!(state.matches[1].end, 21);
    }

    #[test]
    fn find_matches_case_insensitive() {
        let mut state = SearchState::new();
        state.query = "Hello".to_string();
        state.find_matches("HELLO hello HeLLo");
        assert_eq!(state.match_count(), 3);
    }

    #[test]
    fn find_matches_empty_query() {
        let mut state = SearchState::new();
        state.find_matches("some text");
        assert_eq!(state.match_count(), 0);
        assert!(state.current_match.is_none());
    }

    #[test]
    fn find_matches_no_results() {
        let mut state = SearchState::new();
        state.query = "xyz".to_string();
        state.find_matches("some text");
        assert_eq!(state.match_count(), 0);
        assert!(state.current_match.is_none());
    }

    #[test]
    fn next_match_wraps() {
        let mut state = SearchState::new();
        state.query = "a".to_string();
        state.find_matches("a b a");
        assert_eq!(state.current_match, Some(0));
        state.next_match();
        assert_eq!(state.current_match, Some(1));
        state.next_match();
        assert_eq!(state.current_match, Some(0));
    }

    #[test]
    fn prev_match_wraps() {
        let mut state = SearchState::new();
        state.query = "a".to_string();
        state.find_matches("a b a");
        assert_eq!(state.current_match, Some(0));
        state.prev_match();
        assert_eq!(state.current_match, Some(1));
        state.prev_match();
        assert_eq!(state.current_match, Some(0));
    }

    #[test]
    fn clear_resets_state() {
        let mut state = SearchState::new();
        state.query = "test".to_string();
        state.find_matches("test test");
        assert_eq!(state.match_count(), 2);
        state.clear();
        assert!(state.query.is_empty());
        assert!(state.matches.is_empty());
        assert!(state.current_match.is_none());
    }
}
