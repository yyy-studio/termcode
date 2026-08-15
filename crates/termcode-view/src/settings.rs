//! State for the settings screen.
//!
//! The screen is two panes: a fixed list of categories on the left, and the
//! items of the selected category on the right. Items are *not* built here --
//! the available themes, keymaps, plugins and keybindings are all known to the
//! frontend, not to this crate -- so the owner fills [`SettingsState::items`]
//! whenever the category changes, the same way the command palette is fed its
//! command list.

/// The groups a setting can belong to, in the order they are listed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsCategory {
    Appearance,
    Editor,
    Keybindings,
    Plugins,
}

impl SettingsCategory {
    pub const ALL: [SettingsCategory; 4] = [
        SettingsCategory::Appearance,
        SettingsCategory::Editor,
        SettingsCategory::Keybindings,
        SettingsCategory::Plugins,
    ];

    pub fn title(&self) -> &'static str {
        match self {
            SettingsCategory::Appearance => "Appearance",
            SettingsCategory::Editor => "Editor",
            SettingsCategory::Keybindings => "Keybindings",
            SettingsCategory::Plugins => "Plugins",
        }
    }
}

/// Where the settings file this item writes to expects to find it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingTarget {
    /// A dotted path in `config.toml`, outermost key first.
    Config(Vec<String>),
    /// A binding in `keybindings.toml`. `mode` is `None` for `[global]`.
    Keybinding {
        mode: Option<String>,
        command: String,
    },
    /// Shown for reference only; nothing to write.
    ReadOnly,
}

/// What a setting holds, and therefore how it is edited.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingValue {
    /// Toggled by Enter or Space.
    Bool(bool),
    /// A number in a range. The value list offers one entry per `step`, so a
    /// millisecond setting does not list every value between its bounds.
    Int {
        value: i64,
        min: i64,
        max: i64,
        step: i64,
    },
    /// Chosen from a list.
    Choice {
        options: Vec<String>,
        selected: usize,
    },
    /// A key sequence, rebound by pressing keys after Enter. `None` means the
    /// command is currently unbound.
    KeyBinding(Option<String>),
    /// Text with nothing to edit.
    Info(String),
}

impl SettingValue {
    /// How the value reads in the right-hand column.
    pub fn display(&self) -> String {
        match self {
            SettingValue::Bool(true) => "[x]".to_string(),
            SettingValue::Bool(false) => "[ ]".to_string(),
            SettingValue::Int { value, .. } => value.to_string(),
            SettingValue::Choice { options, selected } => options
                .get(*selected)
                .cloned()
                .unwrap_or_else(|| "-".to_string()),
            SettingValue::KeyBinding(Some(keys)) => keys.clone(),
            SettingValue::KeyBinding(None) => "(unbound)".to_string(),
            SettingValue::Info(text) => text.clone(),
        }
    }

    /// Whether the arrows and Enter do anything on this value.
    pub fn is_editable(&self) -> bool {
        !matches!(self, SettingValue::Info(_))
    }
}

/// One row of the right-hand pane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingItem {
    pub label: String,
    pub value: SettingValue,
    pub target: SettingTarget,
    /// The change is written to disk but cannot take effect until the editor
    /// is restarted, so the screen has to say so.
    pub restart_required: bool,
    /// Extra context shown under the label, such as what a choice affects.
    pub detail: Option<String>,
    /// Apply each option as the highlight passes over it in the picker, so the
    /// user can see what they are choosing.
    ///
    /// Only safe where the change cannot take the keyboard away: a theme is
    /// fine, a keymap is not.
    pub live_preview: bool,
}

impl SettingItem {
    pub fn new(label: impl Into<String>, value: SettingValue, target: SettingTarget) -> Self {
        Self {
            label: label.into(),
            value,
            target,
            restart_required: false,
            detail: None,
            live_preview: false,
        }
    }

    pub fn needing_restart(mut self) -> Self {
        self.restart_required = true;
        self
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn with_live_preview(mut self) -> Self {
        self.live_preview = true;
        self
    }
}

/// The list opened on top of the settings screen when a multi-value setting is
/// activated.
///
/// Choices are picked from a list rather than cycled in place. Cycling applied
/// every value on the way to the one you wanted, which for the keymap meant
/// briefly running under a keymap you did not ask for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsPicker {
    /// Row of the right-hand pane this picker belongs to.
    pub item_index: usize,
    pub title: String,
    pub options: Vec<String>,
    pub selected: usize,
    /// What the setting was when the picker opened, so cancelling can put it
    /// back and the list can mark it.
    pub original: usize,
    pub scroll_offset: usize,
    pub visible_height: usize,
    /// See [`SettingItem::live_preview`].
    pub preview: bool,
}

impl SettingsPicker {
    /// Tell the picker how many options actually fit on screen, so paging
    /// moves by a screenful and the highlight cannot scroll out of view.
    pub fn set_visible_height(&mut self, height: usize) {
        self.visible_height = height;
        self.clamp_scroll();
    }

    fn clamp_scroll(&mut self) {
        // Never leave blank rows below a list that still has entries above.
        let max_offset = self.options.len().saturating_sub(self.visible_height);
        self.scroll_offset = self.scroll_offset.min(max_offset);
        if self.selected < self.scroll_offset {
            self.scroll_offset = self.selected;
        } else if self.visible_height > 0
            && self.selected >= self.scroll_offset + self.visible_height
        {
            self.scroll_offset = self.selected - self.visible_height + 1;
        }
    }
}

/// Which pane the arrow keys move in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsFocus {
    Categories,
    Items,
}

/// What the caller should do after a key was handled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsAction {
    /// Nothing to do.
    None,
    /// The selected category changed; refill the items.
    CategoryChanged,
    /// The item at this index holds a new value that should be applied and
    /// saved.
    Changed(usize),
    /// The item at this index is a keybinding waiting for a key press.
    CaptureKey(usize),
    /// The item at this index holds the value the picker is highlighting.
    /// Apply it so the user can see it, but do **not** save: they have not
    /// chosen it yet.
    Preview(usize),
    /// A preview was abandoned. The item is back to the value it had before
    /// the picker opened; apply that, again without saving.
    PreviewReverted(usize),
}

#[derive(Debug)]
pub struct SettingsState {
    pub category_index: usize,
    pub items: Vec<SettingItem>,
    pub selected: usize,
    pub scroll_offset: usize,
    pub visible_height: usize,
    pub focus: SettingsFocus,
    /// Set while a keybinding item is waiting for keys; holds what has been
    /// pressed so far, for display.
    pub capturing: Option<String>,
    /// Set while a multi-value setting is being picked from a list.
    pub picker: Option<SettingsPicker>,
    /// Result of the last save, shown at the bottom of the screen.
    pub message: Option<String>,
}

impl SettingsState {
    pub fn new() -> Self {
        Self {
            category_index: 0,
            items: Vec::new(),
            selected: 0,
            scroll_offset: 0,
            visible_height: 12,
            // Items are what the user came for; the categories are navigation.
            focus: SettingsFocus::Items,
            capturing: None,
            picker: None,
            message: None,
        }
    }

    pub fn category(&self) -> SettingsCategory {
        SettingsCategory::ALL[self.category_index.min(SettingsCategory::ALL.len() - 1)]
    }

    /// Replace the right-hand pane, keeping the cursor in range. Called every
    /// time the category changes and after a change that rewrites a value.
    pub fn load_items(&mut self, items: Vec<SettingItem>) {
        // The picker points at a row by index; the replacement rows may not
        // have the same one there.
        self.picker = None;
        self.items = items;
        self.selected = self.selected.min(self.items.len().saturating_sub(1));
        self.clamp_scroll();
    }

    /// Move within the focused pane. Categories wrap; items do not, so holding
    /// Down at the end of a long list does not jump back to the top.
    ///
    /// Moving also drops the last save message: it occupies the line the
    /// selected item's own description would otherwise use, and leaving it
    /// there would attach a message about one row to every row after it.
    pub fn move_selection(&mut self, delta: i32) -> SettingsAction {
        self.message = None;
        match self.focus {
            SettingsFocus::Categories => {
                let len = SettingsCategory::ALL.len() as i32;
                let next = (self.category_index as i32 + delta).rem_euclid(len) as usize;
                if next == self.category_index {
                    return SettingsAction::None;
                }
                self.category_index = next;
                self.selected = 0;
                self.scroll_offset = 0;
                SettingsAction::CategoryChanged
            }
            SettingsFocus::Items => {
                if self.items.is_empty() {
                    return SettingsAction::None;
                }
                let last = self.items.len() as i32 - 1;
                self.selected = (self.selected as i32 + delta).clamp(0, last) as usize;
                self.clamp_scroll();
                SettingsAction::None
            }
        }
    }

    pub fn set_focus(&mut self, focus: SettingsFocus) {
        self.focus = focus;
    }

    pub fn toggle_focus(&mut self) {
        self.message = None;
        self.focus = match self.focus {
            SettingsFocus::Categories => SettingsFocus::Items,
            SettingsFocus::Items => SettingsFocus::Categories,
        };
    }

    pub fn selected_item(&self) -> Option<&SettingItem> {
        self.items.get(self.selected)
    }

    /// Step one level out: from the settings to the categories they belong to.
    /// Already at the categories, there is nowhere further left to go.
    pub fn focus_out(&mut self) -> SettingsAction {
        self.message = None;
        self.focus = SettingsFocus::Categories;
        SettingsAction::None
    }

    /// Step one level in: from the categories to their settings.
    pub fn focus_in(&mut self) -> SettingsAction {
        self.message = None;
        self.focus = SettingsFocus::Items;
        SettingsAction::None
    }

    /// Act on the selected row (from Enter or Space).
    ///
    /// Every setting with more than two values opens a list rather than
    /// changing on the spot, so the arrows are free to mean "move between the
    /// two panes" and nothing is applied by passing over it.
    pub fn activate_selected(&mut self) -> SettingsAction {
        if self.focus != SettingsFocus::Items {
            // On a category this is just "take me into it".
            return self.focus_in();
        }
        let index = self.selected;
        let Some(item) = self.items.get(index) else {
            return SettingsAction::None;
        };
        match &item.value {
            // A switch has only one other value: there is no list to show.
            SettingValue::Bool(_) => {
                if let Some(SettingValue::Bool(flag)) =
                    self.items.get_mut(index).map(|item| &mut item.value)
                {
                    *flag = !*flag;
                }
                self.message = None;
                SettingsAction::Changed(index)
            }
            SettingValue::KeyBinding(_) => {
                self.capturing = Some(String::new());
                SettingsAction::CaptureKey(index)
            }
            SettingValue::Choice { .. } | SettingValue::Int { .. } => self.open_picker(),
            SettingValue::Info(_) => SettingsAction::None,
        }
    }

    /// Open the value list for the selected row.
    ///
    /// A number gets one entry per step of its range, so picking `4` and
    /// picking `one-dark` work the same way.
    fn open_picker(&mut self) -> SettingsAction {
        let index = self.selected;
        let height = self.visible_height;
        let Some(item) = self.items.get(index) else {
            return SettingsAction::None;
        };
        let (options, selected) = match &item.value {
            SettingValue::Choice { options, selected } => (options.clone(), *selected),
            SettingValue::Int {
                value,
                min,
                max,
                step,
            } => {
                let step = (*step).max(1);
                let options: Vec<String> = (*min..=*max)
                    .step_by(step as usize)
                    .map(|n| n.to_string())
                    .collect();
                let last = options.len().saturating_sub(1);
                let selected = (((*value - *min).max(0) / step) as usize).min(last);
                (options, selected)
            }
            _ => return SettingsAction::None,
        };
        if options.is_empty() {
            return SettingsAction::None;
        }
        let mut picker = SettingsPicker {
            item_index: index,
            title: item.label.clone(),
            options,
            // Open centred on the value in use rather than at the top: a
            // number's list can be seventy entries long, and the one that
            // matters is the one already set.
            scroll_offset: selected.saturating_sub(height / 2),
            selected,
            original: selected,
            visible_height: height,
            preview: item.live_preview,
        };
        picker.clamp_scroll();
        self.picker = Some(picker);
        self.message = None;
        SettingsAction::None
    }

    /// Move the highlight in the open picker.
    ///
    /// Under live preview the row's value follows the highlight, so what the
    /// screen shows and what the editor is running stay the same thing.
    pub fn picker_move(&mut self, delta: i32) -> SettingsAction {
        let Some(picker) = &mut self.picker else {
            return SettingsAction::None;
        };
        if picker.options.is_empty() {
            return SettingsAction::None;
        }
        let len = picker.options.len() as i32;
        let next = (picker.selected as i32 + delta).clamp(0, len - 1) as usize;
        if next == picker.selected {
            return SettingsAction::None;
        }
        picker.selected = next;
        picker.clamp_scroll();
        if !picker.preview {
            return SettingsAction::None;
        }
        let (index, selected) = (picker.item_index, picker.selected);
        self.set_choice(index, selected);
        SettingsAction::Preview(index)
    }

    /// Take the highlighted option (from Enter).
    pub fn picker_commit(&mut self) -> SettingsAction {
        let Some(picker) = self.picker.take() else {
            return SettingsAction::None;
        };
        self.set_choice(picker.item_index, picker.selected);
        SettingsAction::Changed(picker.item_index)
    }

    /// Close the picker, keeping the value the setting had when it opened.
    pub fn picker_cancel(&mut self) -> SettingsAction {
        let Some(picker) = self.picker.take() else {
            return SettingsAction::None;
        };
        if !picker.preview || picker.selected == picker.original {
            return SettingsAction::None;
        }
        // A preview is live in the editor; putting the row back is not enough.
        self.set_choice(picker.item_index, picker.original);
        SettingsAction::PreviewReverted(picker.item_index)
    }

    /// Put the option at `option` into the row, in whatever form that row
    /// stores its value.
    fn set_choice(&mut self, index: usize, option: usize) {
        match self.items.get_mut(index).map(|item| &mut item.value) {
            Some(SettingValue::Choice { selected, .. }) => *selected = option,
            // The list was built by stepping the range, so the option's
            // position is what turns it back into a number.
            Some(SettingValue::Int {
                value,
                min,
                max,
                step,
            }) => *value = (*min + option as i64 * (*step).max(1)).clamp(*min, *max),
            _ => {}
        }
    }

    pub fn cancel_capture(&mut self) {
        self.capturing = None;
    }

    fn clamp_scroll(&mut self) {
        if self.selected < self.scroll_offset {
            self.scroll_offset = self.selected;
        } else if self.visible_height > 0
            && self.selected >= self.scroll_offset + self.visible_height
        {
            self.scroll_offset = self.selected - self.visible_height + 1;
        }
    }
}

impl Default for SettingsState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state_with_items() -> SettingsState {
        let mut state = SettingsState::new();
        state.load_items(vec![
            SettingItem::new(
                "Tab Size",
                SettingValue::Int {
                    value: 4,
                    min: 1,
                    max: 16,
                    step: 1,
                },
                SettingTarget::Config(vec!["editor".into(), "tab_size".into()]),
            ),
            SettingItem::new(
                "Insert Spaces",
                SettingValue::Bool(true),
                SettingTarget::Config(vec!["editor".into(), "insert_spaces".into()]),
            ),
            SettingItem::new(
                "Line Numbers",
                SettingValue::Choice {
                    options: vec!["absolute".into(), "relative".into()],
                    selected: 0,
                },
                SettingTarget::Config(vec!["editor".into(), "line_numbers".into()]),
            ),
        ]);
        state
    }

    #[test]
    fn categories_wrap_and_ask_for_a_reload() {
        let mut state = SettingsState::new();
        state.set_focus(SettingsFocus::Categories);
        assert_eq!(state.category(), SettingsCategory::Appearance);
        assert_eq!(state.move_selection(-1), SettingsAction::CategoryChanged);
        assert_eq!(state.category(), SettingsCategory::Plugins);
    }

    #[test]
    fn item_selection_stops_at_the_ends() {
        let mut state = state_with_items();
        state.move_selection(-1);
        assert_eq!(state.selected, 0, "must not wrap to the last item");
        state.move_selection(10);
        assert_eq!(state.selected, 2, "must not wrap to the first item");
    }

    #[test]
    fn a_number_is_offered_as_a_list_over_its_range() {
        let mut state = state_with_items();
        assert_eq!(state.activate_selected(), SettingsAction::None);
        let picker = state.picker.as_ref().expect("picker should be open");
        // Tab Size is 1..=16 by 1, currently 4.
        assert_eq!(picker.options.len(), 16);
        assert_eq!(picker.options.first().map(String::as_str), Some("1"));
        assert_eq!(picker.options.last().map(String::as_str), Some("16"));
        assert_eq!(picker.selected, 3);

        state.picker_move(-3);
        assert_eq!(state.picker_commit(), SettingsAction::Changed(0));
        assert_eq!(state.selected_item().unwrap().value.display(), "1");
    }

    #[test]
    fn a_numeric_list_is_built_from_the_step_not_every_value() {
        let mut state = SettingsState::new();
        state.load_items(vec![SettingItem::new(
            "Chord Timeout",
            SettingValue::Int {
                value: 1000,
                min: 200,
                max: 5000,
                step: 100,
            },
            SettingTarget::Config(vec!["keymap".into(), "chord_timeout_ms".into()]),
        )]);
        state.activate_selected();
        let picker = state.picker.as_ref().unwrap();
        assert_eq!(picker.options.len(), 49, "200..=5000 by 100");
        assert_eq!(picker.options[picker.selected], "1000");

        state.picker_move(1);
        state.picker_commit();
        assert_eq!(state.selected_item().unwrap().value.display(), "1100");
    }

    #[test]
    fn a_choice_opens_a_picker_instead_of_changing_in_place() {
        let mut state = state_with_items();
        state.selected = 2;
        // Activating may not change the value on the spot: cycling in place is
        // what made stepping to a keymap apply every keymap on the way.
        assert_eq!(state.activate_selected(), SettingsAction::None);
        assert_eq!(state.selected_item().unwrap().value.display(), "absolute");
        let picker = state.picker.as_ref().expect("picker should be open");
        assert_eq!(picker.item_index, 2);
        assert_eq!(picker.options, vec!["absolute", "relative"]);
        assert_eq!(picker.selected, 0);
    }

    #[test]
    fn committing_the_picker_is_what_changes_the_value() {
        let mut state = state_with_items();
        state.selected = 2;
        state.activate_selected();
        assert_eq!(
            state.picker_move(1),
            SettingsAction::None,
            "no preview here"
        );
        // The row still reads the old value until it is committed.
        assert_eq!(state.selected_item().unwrap().value.display(), "absolute");
        assert_eq!(state.picker_commit(), SettingsAction::Changed(2));
        assert!(state.picker.is_none());
        assert_eq!(state.selected_item().unwrap().value.display(), "relative");
    }

    #[test]
    fn cancelling_a_picker_leaves_the_value_alone() {
        let mut state = state_with_items();
        state.selected = 2;
        state.activate_selected();
        state.picker_move(1);
        assert_eq!(state.picker_cancel(), SettingsAction::None);
        assert!(state.picker.is_none());
        assert_eq!(state.selected_item().unwrap().value.display(), "absolute");
    }

    #[test]
    fn a_previewing_picker_follows_the_highlight_and_rolls_back() {
        let mut state = SettingsState::new();
        state.load_items(vec![
            SettingItem::new(
                "Theme",
                SettingValue::Choice {
                    options: vec!["one-dark".into(), "gruvbox-dark".into(), "lazygit".into()],
                    selected: 0,
                },
                SettingTarget::Config(vec!["theme".into()]),
            )
            .with_live_preview(),
        ]);
        state.activate_selected();

        assert_eq!(state.picker_move(1), SettingsAction::Preview(0));
        assert_eq!(
            state.selected_item().unwrap().value.display(),
            "gruvbox-dark"
        );
        assert_eq!(state.picker_move(1), SettingsAction::Preview(0));
        assert_eq!(state.selected_item().unwrap().value.display(), "lazygit");

        // Backing out has to undo what the previews applied.
        assert_eq!(state.picker_cancel(), SettingsAction::PreviewReverted(0));
        assert_eq!(state.selected_item().unwrap().value.display(), "one-dark");
    }

    #[test]
    fn a_preview_returned_to_its_starting_value_needs_no_rollback() {
        let mut state = SettingsState::new();
        state.load_items(vec![
            SettingItem::new(
                "Theme",
                SettingValue::Choice {
                    options: vec!["one-dark".into(), "gruvbox-dark".into()],
                    selected: 0,
                },
                SettingTarget::Config(vec!["theme".into()]),
            )
            .with_live_preview(),
        ]);
        state.activate_selected();
        state.picker_move(1);
        state.picker_move(-1);
        assert_eq!(state.picker_cancel(), SettingsAction::None);
        assert_eq!(state.selected_item().unwrap().value.display(), "one-dark");
    }

    #[test]
    fn the_picker_highlight_stops_at_the_ends() {
        let mut state = state_with_items();
        state.selected = 2;
        state.activate_selected();
        state.picker_move(-5);
        assert_eq!(state.picker.as_ref().unwrap().selected, 0);
        state.picker_move(5);
        assert_eq!(state.picker.as_ref().unwrap().selected, 1);
    }

    #[test]
    fn reloading_rows_closes_an_open_picker() {
        let mut state = state_with_items();
        state.selected = 2;
        state.activate_selected();
        assert!(state.picker.is_some());
        state.load_items(vec![SettingItem::new(
            "Only",
            SettingValue::Bool(false),
            SettingTarget::ReadOnly,
        )]);
        assert!(state.picker.is_none());
    }

    #[test]
    fn enter_toggles_a_bool() {
        let mut state = state_with_items();
        state.selected = 1;
        assert_eq!(state.activate_selected(), SettingsAction::Changed(1));
        assert_eq!(state.selected_item().unwrap().value.display(), "[ ]");
    }

    #[test]
    fn enter_on_a_keybinding_starts_a_capture() {
        let mut state = SettingsState::new();
        state.load_items(vec![SettingItem::new(
            "Save",
            SettingValue::KeyBinding(Some("ctrl+s".into())),
            SettingTarget::Keybinding {
                mode: None,
                command: "file.save".into(),
            },
        )]);
        assert_eq!(state.activate_selected(), SettingsAction::CaptureKey(0));
        assert_eq!(state.capturing.as_deref(), Some(""));
        state.cancel_capture();
        assert!(state.capturing.is_none());
    }

    #[test]
    fn read_only_items_never_report_a_change() {
        let mut state = SettingsState::new();
        state.load_items(vec![SettingItem::new(
            "LSP: rust-analyzer",
            SettingValue::Info("rust".into()),
            SettingTarget::ReadOnly,
        )]);
        assert_eq!(state.activate_selected(), SettingsAction::None);
        assert!(state.picker.is_none(), "there is nothing to choose from");
    }

    #[test]
    fn the_arrows_move_between_the_two_panes() {
        let mut state = state_with_items();
        assert_eq!(state.focus, SettingsFocus::Items);

        state.focus_out();
        assert_eq!(state.focus, SettingsFocus::Categories);
        // Already at the outermost level: nowhere further to go.
        state.focus_out();
        assert_eq!(state.focus, SettingsFocus::Categories);
        assert_eq!(
            state.items[0].value.display(),
            "4",
            "and nothing was edited"
        );

        state.focus_in();
        assert_eq!(state.focus, SettingsFocus::Items);
        state.focus_in();
        assert_eq!(state.focus, SettingsFocus::Items);
        assert!(state.picker.is_none(), "the arrows do not open the list");
    }

    #[test]
    fn activating_a_category_steps_into_its_settings() {
        let mut state = state_with_items();
        state.set_focus(SettingsFocus::Categories);
        assert_eq!(state.activate_selected(), SettingsAction::None);
        assert_eq!(state.focus, SettingsFocus::Items);
        assert_eq!(state.items[0].value.display(), "4", "nothing was edited");
    }

    #[test]
    fn moving_clears_the_last_save_message() {
        let mut state = state_with_items();
        state.message = Some("Saved editor.tab_size".to_string());
        state.move_selection(1);
        assert!(state.message.is_none());
    }

    #[test]
    fn reloading_shorter_items_keeps_the_cursor_in_range() {
        let mut state = state_with_items();
        state.selected = 2;
        state.load_items(vec![SettingItem::new(
            "Only",
            SettingValue::Bool(false),
            SettingTarget::ReadOnly,
        )]);
        assert_eq!(state.selected, 0);
    }

    #[test]
    fn scrolling_follows_the_selection() {
        let mut state = SettingsState::new();
        state.visible_height = 2;
        state.load_items(
            (0..5)
                .map(|i| {
                    SettingItem::new(
                        format!("Item {i}"),
                        SettingValue::Bool(false),
                        SettingTarget::ReadOnly,
                    )
                })
                .collect(),
        );
        state.move_selection(3);
        assert_eq!(state.selected, 3);
        assert_eq!(state.scroll_offset, 2);
        state.move_selection(-3);
        assert_eq!(state.scroll_offset, 0);
    }
}
