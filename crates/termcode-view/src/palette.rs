use crate::fuzzy::fuzzy_score;

#[derive(Debug, Clone)]
pub struct PaletteItem {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteMode {
    Commands,
    Themes,
}

#[derive(Debug)]
pub struct CommandPaletteState {
    pub query: String,
    pub all_commands: Vec<PaletteItem>,
    pub filtered: Vec<PaletteItem>,
    pub selected: usize,
    pub scroll_offset: usize,
    pub cursor_pos: usize,
    pub visible_height: usize,
    pub mode: PaletteMode,
}

impl CommandPaletteState {
    pub fn new() -> Self {
        Self {
            query: String::new(),
            all_commands: Vec::new(),
            filtered: Vec::new(),
            selected: 0,
            scroll_offset: 0,
            cursor_pos: 0,
            visible_height: 12,
            mode: PaletteMode::Commands,
        }
    }

    pub fn load_commands(&mut self, commands: Vec<PaletteItem>) {
        self.all_commands = commands;
        self.update_filter();
    }

    pub fn update_filter(&mut self) {
        self.filtered.clear();

        if self.query.is_empty() {
            self.filtered = self.all_commands.clone();
        } else {
            let mut scored: Vec<(PaletteItem, i64)> = self
                .all_commands
                .iter()
                .filter_map(|item| {
                    fuzzy_score(&self.query, &item.name).map(|(score, _)| (item.clone(), score))
                })
                .collect();
            scored.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.name.cmp(&b.0.name)));
            self.filtered = scored.into_iter().map(|(item, _)| item).collect();
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

    pub fn selected_command(&self) -> Option<&PaletteItem> {
        self.filtered.get(self.selected)
    }
}

impl Default for CommandPaletteState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_commands() -> Vec<PaletteItem> {
        vec![
            PaletteItem {
                id: "file.save".to_string(),
                name: "Save File".to_string(),
            },
            PaletteItem {
                id: "file.open".to_string(),
                name: "Open File".to_string(),
            },
            PaletteItem {
                id: "edit.undo".to_string(),
                name: "Undo".to_string(),
            },
            PaletteItem {
                id: "edit.redo".to_string(),
                name: "Redo".to_string(),
            },
        ]
    }

    #[test]
    fn update_filter_shows_all_when_empty() {
        let mut state = CommandPaletteState::new();
        state.load_commands(sample_commands());
        assert_eq!(state.filtered.len(), 4);
    }

    #[test]
    fn update_filter_fuzzy_match() {
        let mut state = CommandPaletteState::new();
        state.load_commands(sample_commands());
        state.query = "save".to_string();
        state.update_filter();
        assert!(!state.filtered.is_empty());
        assert_eq!(state.filtered[0].id, "file.save");
    }

    #[test]
    fn move_selection_wraps() {
        let mut state = CommandPaletteState::new();
        state.load_commands(sample_commands());
        assert_eq!(state.selected, 0);
        state.move_selection(-1);
        assert_eq!(state.selected, 3);
        state.move_selection(1);
        assert_eq!(state.selected, 0);
    }

    #[test]
    fn selected_command_returns_correct_item() {
        let mut state = CommandPaletteState::new();
        state.load_commands(sample_commands());
        let cmd = state.selected_command().unwrap();
        assert_eq!(cmd.id, "file.save");
    }

    #[test]
    fn no_match_returns_empty() {
        let mut state = CommandPaletteState::new();
        state.load_commands(sample_commands());
        state.query = "zzzzz".to_string();
        state.update_filter();
        assert!(state.filtered.is_empty());
        assert!(state.selected_command().is_none());
    }
}
