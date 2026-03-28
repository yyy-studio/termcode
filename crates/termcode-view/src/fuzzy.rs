use std::path::Path;

use ignore::WalkBuilder;

#[derive(Debug, Clone)]
pub struct FuzzyMatch {
    pub path: String,
    pub score: i64,
    pub indices: Vec<usize>,
}

#[derive(Debug)]
pub struct FuzzyFinderState {
    pub query: String,
    pub all_files: Vec<String>,
    pub filtered: Vec<FuzzyMatch>,
    pub selected: usize,
    pub scroll_offset: usize,
    pub cursor_pos: usize,
    pub visible_height: usize,
}

impl FuzzyFinderState {
    pub fn new() -> Self {
        Self {
            query: String::new(),
            all_files: Vec::new(),
            filtered: Vec::new(),
            selected: 0,
            scroll_offset: 0,
            cursor_pos: 0,
            visible_height: 17,
        }
    }

    /// Walk project files using ignore::WalkBuilder (respects .gitignore).
    pub fn load_files(&mut self, root: &Path) {
        self.all_files.clear();

        let walker = WalkBuilder::new(root)
            .hidden(true)
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true)
            .build();

        let root_prefix = root.to_path_buf();
        for entry in walker.flatten() {
            if !entry.file_type().is_some_and(|ft| ft.is_file()) {
                continue;
            }
            if let Ok(relative) = entry.path().strip_prefix(&root_prefix) {
                self.all_files.push(relative.to_string_lossy().to_string());
            }
        }

        self.all_files.sort();
    }

    /// Re-score and sort all_files against query.
    pub fn update_filter(&mut self) {
        self.filtered.clear();

        if self.query.is_empty() {
            self.filtered = self
                .all_files
                .iter()
                .map(|p| FuzzyMatch {
                    path: p.clone(),
                    score: 0,
                    indices: Vec::new(),
                })
                .collect();
        } else {
            for path in &self.all_files {
                if let Some((score, indices)) = fuzzy_score(&self.query, path) {
                    self.filtered.push(FuzzyMatch {
                        path: path.clone(),
                        score,
                        indices,
                    });
                }
            }
            self.filtered
                .sort_by(|a, b| b.score.cmp(&a.score).then(a.path.cmp(&b.path)));
        }

        self.selected = 0;
        self.scroll_offset = 0;
    }

    pub fn move_selection(&mut self, delta: i32) {
        if self.filtered.is_empty() {
            return;
        }
        let len = self.filtered.len() as i32;
        let new = (self.selected as i32 + delta).rem_euclid(len);
        self.selected = new as usize;

        // Adjust scroll_offset to keep selected item visible
        if self.selected < self.scroll_offset {
            self.scroll_offset = self.selected;
        } else if self.visible_height > 0
            && self.selected >= self.scroll_offset + self.visible_height
        {
            self.scroll_offset = self.selected - self.visible_height + 1;
        }
    }

    pub fn selected_path(&self) -> Option<&str> {
        self.filtered.get(self.selected).map(|m| m.path.as_str())
    }
}

impl Default for FuzzyFinderState {
    fn default() -> Self {
        Self::new()
    }
}

const SCORE_CONSECUTIVE: i64 = 5;
const SCORE_WORD_START: i64 = 10;
const SCORE_EXACT_CASE: i64 = 1;
const SCORE_BASE_MATCH: i64 = 1;
const SCORE_LENGTH_DIVISOR: i64 = 10;

/// Simple fuzzy scoring algorithm.
/// Returns None if query is not a subsequence of target.
/// Score bonuses: consecutive matches, start-of-word, path separator proximity.
pub fn fuzzy_score(query: &str, target: &str) -> Option<(i64, Vec<usize>)> {
    if query.is_empty() {
        return Some((0, Vec::new()));
    }

    let query_lower: Vec<(char, char)> = query
        .chars()
        .flat_map(|orig| orig.to_lowercase().map(move |lc| (lc, orig)))
        .collect();
    let target_chars: Vec<char> = target.chars().collect();

    let mut indices = Vec::with_capacity(query_lower.len());
    let mut score: i64 = 0;
    let mut qi = 0;
    let mut prev_match_idx: Option<usize> = None;

    for (ti, &tc_orig) in target_chars.iter().enumerate() {
        if qi >= query_lower.len() {
            break;
        }
        let mut tc_lower = tc_orig.to_lowercase();
        let tc_lower_first = tc_lower.next().unwrap_or(tc_orig);
        let is_single_char = tc_lower.next().is_none();
        if is_single_char && tc_lower_first == query_lower[qi].0 {
            indices.push(ti);

            if let Some(prev) = prev_match_idx {
                if ti == prev + 1 {
                    score += SCORE_CONSECUTIVE;
                }
            }

            if ti == 0
                || matches!(
                    target_chars.get(ti.wrapping_sub(1)),
                    Some('/' | '\\' | '.' | '_' | '-' | ' ')
                )
            {
                score += SCORE_WORD_START;
            }

            if tc_orig == query_lower[qi].1 {
                score += SCORE_EXACT_CASE;
            }

            prev_match_idx = Some(ti);
            qi += 1;
            score += SCORE_BASE_MATCH;
        }
    }

    if qi == query_lower.len() {
        score -= target.len() as i64 / SCORE_LENGTH_DIVISOR;
        Some((score, indices))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuzzy_score_exact_match() {
        let result = fuzzy_score("main", "main.rs");
        assert!(result.is_some());
        let (score, indices) = result.unwrap();
        assert!(score > 0);
        assert_eq!(indices, vec![0, 1, 2, 3]);
    }

    #[test]
    fn fuzzy_score_subsequence() {
        let result = fuzzy_score("mr", "main.rs");
        assert!(result.is_some());
        let (_, indices) = result.unwrap();
        assert_eq!(indices, vec![0, 5]);
    }

    #[test]
    fn fuzzy_score_no_match() {
        let result = fuzzy_score("xyz", "main.rs");
        assert!(result.is_none());
    }

    #[test]
    fn fuzzy_score_case_insensitive() {
        let result = fuzzy_score("MAIN", "main.rs");
        assert!(result.is_some());
    }

    #[test]
    fn fuzzy_score_empty_query() {
        let result = fuzzy_score("", "main.rs");
        assert!(result.is_some());
        assert_eq!(result.unwrap().0, 0);
    }

    #[test]
    fn update_filter_sorts_by_score() {
        let mut state = FuzzyFinderState::new();
        state.all_files = vec![
            "src/utils/helper.rs".to_string(),
            "src/main.rs".to_string(),
            "README.md".to_string(),
        ];
        state.query = "main".to_string();
        state.update_filter();
        assert!(!state.filtered.is_empty());
        assert_eq!(state.filtered[0].path, "src/main.rs");
    }

    #[test]
    fn move_selection_wraps() {
        let mut state = FuzzyFinderState::new();
        state.all_files = vec!["a.rs".to_string(), "b.rs".to_string()];
        state.update_filter();
        assert_eq!(state.selected, 0);
        state.move_selection(1);
        assert_eq!(state.selected, 1);
        state.move_selection(1);
        assert_eq!(state.selected, 0);
        state.move_selection(-1);
        assert_eq!(state.selected, 1);
    }

    #[test]
    fn selected_path_returns_none_when_empty() {
        let state = FuzzyFinderState::new();
        assert!(state.selected_path().is_none());
    }

    #[test]
    fn load_files_uses_temp_dir() {
        let dir = std::env::temp_dir().join("termcode_test_fuzzy");
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(dir.join("test.rs"), "fn main() {}").unwrap();
        std::fs::write(dir.join("lib.rs"), "pub mod test;").unwrap();

        let mut state = FuzzyFinderState::new();
        state.load_files(&dir);
        assert!(state.all_files.len() >= 2);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
