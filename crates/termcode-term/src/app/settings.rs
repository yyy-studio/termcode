//! The settings screen.
//!
//! Three jobs live here: describing the current configuration as rows the
//! screen can show, applying a row the user changed to the running editor, and
//! writing that change back to the file it came from.
//!
//! Rows are rebuilt from the live state every time the category changes, so the
//! screen never shows a value the editor has since moved past.

use std::path::Path;

use crossterm::event::{KeyCode, KeyEvent};

use termcode_config::keymap::key_sequence_to_config;
use termcode_config::writer;
use termcode_core::config_types::LineNumberStyle;
use termcode_view::editor::EditorMode;
use termcode_view::settings::{
    SettingItem, SettingTarget, SettingValue, SettingsAction, SettingsCategory,
};

use super::{App, BUILTIN_KEYMAP, list_available_themes};
use crate::input::{KeyResolution, mode_section_name};

/// The `line_numbers` values, spelled the way `LineNumberStyle` deserialises
/// them.
const LINE_NUMBER_STYLES: [(&str, LineNumberStyle); 4] = [
    ("absolute", LineNumberStyle::Absolute),
    ("relative", LineNumberStyle::Relative),
    ("relative_absolute", LineNumberStyle::RelativeAbsolute),
    ("none", LineNumberStyle::None),
];

/// Build a [`SettingValue::Choice`] over `options`, selecting `current` (or the
/// first entry when the current value is not on the list, which is what a theme
/// deleted from disk looks like).
fn choice(options: Vec<String>, current: &str) -> SettingValue {
    let selected = options.iter().position(|o| o == current).unwrap_or(0);
    SettingValue::Choice { options, selected }
}

fn config_target(path: &[&str]) -> SettingTarget {
    SettingTarget::Config(path.iter().map(|k| (*k).to_string()).collect())
}

impl App {
    pub(super) fn open_settings(&mut self) {
        self.editor.settings.message = None;
        self.editor.settings.cancel_capture();
        self.settings_capture.clear();
        self.reload_settings_items();
        self.editor.switch_mode(EditorMode::Settings);
    }

    fn close_settings(&mut self) {
        self.editor.settings.cancel_capture();
        self.settings_capture.clear();
        self.editor.switch_to_default_mode();
    }

    /// Refill the right-hand pane from the editor's current state.
    fn reload_settings_items(&mut self) {
        let items = match self.editor.settings.category() {
            SettingsCategory::Appearance => self.appearance_items(),
            SettingsCategory::Editor => self.editor_items(),
            SettingsCategory::Keybindings => self.keybinding_items(),
            SettingsCategory::Plugins => self.plugin_items(),
        };
        self.editor.settings.load_items(items);
    }

    fn appearance_items(&self) -> Vec<SettingItem> {
        let mut keymaps = vec![BUILTIN_KEYMAP.to_string()];
        keymaps.extend(termcode_config::keymap::list_available_keymaps());

        vec![
            // Previewed live: seeing a theme is the only way to choose one, and
            // a colour scheme cannot take the keyboard away.
            SettingItem::new(
                "Theme",
                choice(list_available_themes(), &self.theme_name),
                config_target(&["theme"]),
            )
            .with_live_preview()
            .with_detail("Previews as you move; Enter keeps it, Esc puts the old one back"),
            // Deliberately *not* previewed: applying a keymap changes the keys
            // driving the list you are choosing from.
            SettingItem::new(
                "Keymap Preset",
                choice(keymaps, &self.keymap_name),
                config_target(&["keymap", "preset"]),
            )
            .with_detail("Replaces the whole keymap; keybindings.toml still applies on top"),
            SettingItem::new(
                "Show Sidebar",
                SettingValue::Bool(self.editor.file_explorer.visible),
                config_target(&["ui", "sidebar_visible"]),
            ),
            SettingItem::new(
                "Sidebar Width",
                SettingValue::Int {
                    value: self.editor.file_explorer.width as i64,
                    min: crate::layout::MIN_SIDEBAR_WIDTH as i64,
                    max: crate::layout::MAX_SIDEBAR_WIDTH as i64,
                    step: 1,
                },
                config_target(&["ui", "sidebar_width"]),
            ),
            SettingItem::new(
                "Tree Lines",
                SettingValue::Bool(self.editor.file_tree_style.tree_style),
                config_target(&["ui", "tree_style"]),
            ),
            SettingItem::new(
                "File Type Icons",
                SettingValue::Bool(self.editor.file_tree_style.show_file_type_emoji),
                config_target(&["ui", "show_file_type_emoji"]),
            ),
            SettingItem::new(
                "Respect .gitignore",
                SettingValue::Bool(self.editor.file_tree_style.respect_gitignore),
                config_target(&["ui", "respect_gitignore"]),
            ),
        ]
    }

    fn editor_items(&self) -> Vec<SettingItem> {
        let config = &self.editor.config;
        let current_style = LINE_NUMBER_STYLES
            .iter()
            .find(|(_, style)| *style == config.line_numbers)
            .map(|(name, _)| *name)
            .unwrap_or("absolute");

        vec![
            SettingItem::new(
                "Tab Size",
                SettingValue::Int {
                    value: config.tab_size as i64,
                    min: 1,
                    max: 16,
                    step: 1,
                },
                config_target(&["editor", "tab_size"]),
            ),
            SettingItem::new(
                "Insert Spaces",
                SettingValue::Bool(config.insert_spaces),
                config_target(&["editor", "insert_spaces"]),
            ),
            SettingItem::new(
                "Line Numbers",
                choice(
                    LINE_NUMBER_STYLES
                        .iter()
                        .map(|(name, _)| (*name).to_string())
                        .collect(),
                    current_style,
                ),
                config_target(&["editor", "line_numbers"]),
            ),
            SettingItem::new(
                "Scroll Off",
                SettingValue::Int {
                    value: config.scroll_off as i64,
                    min: 0,
                    max: 30,
                    step: 1,
                },
                config_target(&["editor", "scroll_off"]),
            )
            .with_detail("Lines kept between the cursor and the edge of the viewport"),
            SettingItem::new(
                "Mouse Support",
                SettingValue::Bool(config.mouse_enabled),
                config_target(&["editor", "mouse_enabled"]),
            )
            .needing_restart(),
            SettingItem::new(
                "Chord Timeout (ms)",
                SettingValue::Int {
                    value: self.chord_timeout.as_millis() as i64,
                    min: 100,
                    max: 5000,
                    step: 100,
                },
                config_target(&["keymap", "chord_timeout_ms"]),
            )
            .with_detail("How long an unfinished multi-key sequence waits for its next key"),
        ]
    }

    /// One row per command the palette lists, showing every key bound to it.
    ///
    /// A rebinding is written into the section the command is already bound in.
    /// Overlay modes never consult `[global]`, so moving `search.next` there
    /// would leave it unreachable; commands that are unbound today default to
    /// `[global]`, which is where a general shortcut belongs.
    ///
    /// Adding keys does not take the old ones away: `keybindings.toml` maps a
    /// key to a command, and has no way to say that a key the preset bound
    /// should stop working. The row lists all of them so the result is visible.
    fn keybinding_items(&self) -> Vec<SettingItem> {
        let mut items: Vec<SettingItem> = self
            .command_registry
            .list_commands()
            .into_iter()
            .map(|(id, name)| {
                let mode = self
                    .input_mapper
                    .binding_scope(id)
                    .and_then(|(mode, _)| mode);
                let bound = self.input_mapper.bindings_for(id);
                let keys = (!bound.is_empty()).then(|| bound.join(", "));
                let item = SettingItem::new(
                    name,
                    SettingValue::KeyBinding(keys),
                    SettingTarget::Keybinding {
                        mode: mode.map(|m| mode_section_name(m).to_string()),
                        command: id.to_string(),
                    },
                );
                match mode {
                    Some(mode) => {
                        item.with_detail(format!("{id}  ·  [mode.{}]", mode_section_name(mode)))
                    }
                    None => item.with_detail(format!("{id}  ·  [global]")),
                }
            })
            .collect();
        items.sort_by(|a, b| a.label.cmp(&b.label));
        items
    }

    fn plugin_items(&self) -> Vec<SettingItem> {
        let mut items = vec![
            SettingItem::new(
                "Plugins Enabled",
                SettingValue::Bool(self.app_config.plugins.enabled),
                config_target(&["plugins", "enabled"]),
            )
            .needing_restart(),
        ];

        if let Some(pm) = &self.plugin_manager {
            for plugin in pm.list_plugins() {
                let enabled = self
                    .app_config
                    .plugins
                    .overrides
                    .get(&plugin.name)
                    .and_then(|o| o.enabled)
                    .unwrap_or(true);
                items.push(
                    SettingItem::new(
                        format!("Plugin: {}", plugin.name),
                        SettingValue::Bool(enabled),
                        SettingTarget::Config(vec![
                            "plugins".to_string(),
                            "overrides".to_string(),
                            plugin.name.clone(),
                            "enabled".to_string(),
                        ]),
                    )
                    .needing_restart()
                    .with_detail(format!("{} v{}", plugin.description, plugin.version)),
                );
            }
        }

        // LSP servers are started from the config file at boot and have no
        // runtime switch, so they are listed for reference only.
        for server in &self.app_config.lsp {
            items.push(
                SettingItem::new(
                    format!("LSP: {}", server.language),
                    SettingValue::Info(server.command.clone()),
                    SettingTarget::ReadOnly,
                )
                .with_detail("Edit [[lsp]] in config.toml to change this"),
            );
        }

        items
    }

    pub(super) fn handle_settings_key(&mut self, key: KeyEvent) {
        if self.editor.settings.capturing.is_some() {
            self.handle_settings_capture_key(key);
            return;
        }
        // A dead chord is simply dropped here: nothing on this screen takes
        // typed text, so there is nowhere to put the keys back.
        match self.feed_key(EditorMode::Settings, key) {
            KeyResolution::Match(cmd_id) => self.run_settings_command(cmd_id),
            // This screen consults no global table, so a keymap binding nothing
            // here would leave Ctrl+Q as the only way out of it.
            KeyResolution::NoMatch if key.code == KeyCode::Esc && key.modifiers.is_empty() => {
                if self.editor.settings.picker.is_some() {
                    let action = self.editor.settings.picker_cancel();
                    self.handle_settings_action(action);
                } else {
                    self.close_settings();
                }
            }
            _ => {}
        }
    }

    fn run_settings_command(&mut self, cmd_id: &str) {
        if self.editor.settings.picker.is_some() {
            self.run_picker_command(cmd_id);
            return;
        }
        let page = self.editor.settings.visible_height.max(1) as i32;
        let action = match cmd_id {
            "settings.close" => {
                self.close_settings();
                return;
            }
            "settings.up" => self.editor.settings.move_selection(-1),
            "settings.down" => self.editor.settings.move_selection(1),
            "settings.page_up" => self.editor.settings.move_selection(-page),
            "settings.page_down" => self.editor.settings.move_selection(page),
            "settings.toggle_focus" => {
                self.editor.settings.toggle_focus();
                SettingsAction::None
            }
            // The arrows move between the two panes; nothing on this screen is
            // edited by passing over it.
            "settings.focus_out" => self.editor.settings.focus_out(),
            "settings.focus_in" => self.editor.settings.focus_in(),
            "settings.activate" => self.editor.settings.activate_selected(),
            // Anything else a keymap binds in this mode still runs.
            other => {
                self.dispatch_command(other);
                return;
            }
        };

        self.handle_settings_action(action);
    }

    /// Keys while the value list is open. It borrows the settings mode's own
    /// bindings rather than needing a mode of its own, so any keymap that can
    /// drive the screen can drive the list.
    fn run_picker_command(&mut self, cmd_id: &str) {
        let page = self
            .editor
            .settings
            .picker
            .as_ref()
            .map_or(1, |p| p.visible_height.max(1)) as i32;
        let settings = &mut self.editor.settings;
        let action = match cmd_id {
            // Left is "back out one level" everywhere on this screen, and the
            // list is a level of its own.
            "settings.close" | "settings.focus_out" => settings.picker_cancel(),
            "settings.up" => settings.picker_move(-1),
            "settings.down" => settings.picker_move(1),
            "settings.page_up" => settings.picker_move(-page),
            "settings.page_down" => settings.picker_move(page),
            "settings.activate" => settings.picker_commit(),
            // The list owns the keyboard while it is open: running an unrelated
            // command underneath it would act on a screen the user cannot see.
            _ => SettingsAction::None,
        };
        self.handle_settings_action(action);
    }

    fn handle_settings_action(&mut self, action: SettingsAction) {
        match action {
            SettingsAction::CategoryChanged => self.reload_settings_items(),
            SettingsAction::Changed(index) => self.apply_and_save_setting(index),
            // A preview is deliberately not written to disk: the user is still
            // looking, and may yet back out.
            SettingsAction::Preview(index) | SettingsAction::PreviewReverted(index) => {
                self.apply_setting(index)
            }
            SettingsAction::CaptureKey(_) => {
                self.settings_capture.clear();
                self.editor.settings.message =
                    Some("Press the keys, then Enter to bind (Esc cancels)".to_string());
            }
            SettingsAction::None => {}
        }
    }

    /// Collect keys for a rebinding.
    ///
    /// Enter commits what has been pressed so far, so a chord like `g g` can be
    /// entered one key at a time. Pressing Enter first captures Enter itself,
    /// which is the only way to bind it. Esc always cancels and therefore
    /// cannot be bound from this screen.
    fn handle_settings_capture_key(&mut self, key: KeyEvent) {
        if key.code == KeyCode::Esc && key.modifiers.is_empty() {
            self.editor.settings.cancel_capture();
            self.settings_capture.clear();
            self.editor.settings.message = Some("Rebinding cancelled".to_string());
            return;
        }
        if key.code == KeyCode::Enter
            && key.modifiers.is_empty()
            && !self.settings_capture.is_empty()
        {
            self.commit_captured_binding();
            return;
        }
        self.settings_capture.push(key);
        let display = key_sequence_to_config(&self.settings_capture)
            .unwrap_or_else(|| "(unsupported key)".to_string());
        self.editor.settings.capturing = Some(display);
    }

    fn commit_captured_binding(&mut self) {
        let captured = std::mem::take(&mut self.settings_capture);
        self.editor.settings.cancel_capture();

        let Some((mode_name, command)) =
            self.editor
                .settings
                .selected_item()
                .and_then(|item| match &item.target {
                    SettingTarget::Keybinding { mode, command } => {
                        Some((mode.clone(), command.clone()))
                    }
                    _ => None,
                })
        else {
            return;
        };

        let Some(keys) = key_sequence_to_config(&captured) else {
            self.editor.settings.message =
                Some("That key cannot be written to a config file".to_string());
            return;
        };
        // Round-trip through the parser so the binding stored in memory is
        // exactly the one the file will produce on the next start.
        let Some(sequence) = termcode_config::keymap::parse_key_sequence(&keys) else {
            self.editor.settings.message = Some(format!("'{keys}' cannot be parsed back"));
            return;
        };

        let scope = mode_name.as_deref().and_then(section_mode);
        let conflicts = self.input_mapper.conflicts(scope, &sequence, &command);

        if self.kb_config.table_mut(mode_name.as_deref()).is_none() {
            self.editor.settings.message = Some(format!(
                "'{}' is not a keymap section",
                mode_name.unwrap_or_default()
            ));
            return;
        }

        let path = writer::keybinding_path(mode_name.as_deref(), &keys);
        if let Err(e) = writer::set_value(
            &self.keybindings_path,
            &path,
            toml_edit::Value::from(command.clone()),
        ) {
            self.editor.settings.message = Some(format!("Save failed: {e}"));
            return;
        }

        // Mirror the write into the in-memory overrides so the rebuilt mapper
        // matches the file without re-reading it.
        if let Some(table) = self.kb_config.table_mut(mode_name.as_deref()) {
            table.insert(keys.clone(), command.clone());
        }
        self.rebuild_input_mapper();
        self.reload_settings_items();

        self.editor.settings.message = Some(if conflicts.is_empty() {
            format!("Bound {keys} to {command}")
        } else {
            let shadowed = conflicts
                .iter()
                .map(|(existing, cmd)| format!("{existing} ({cmd})"))
                .collect::<Vec<_>>()
                .join(", ");
            format!("Bound {keys} to {command}  |  conflicts with {shadowed}")
        });
    }

    /// Apply the row to the running editor without touching the config file.
    /// Used for previews, which the user has not committed to yet.
    fn apply_setting(&mut self, index: usize) {
        let Some(item) = self.editor.settings.items.get(index).cloned() else {
            return;
        };
        if let SettingTarget::Config(path) = &item.target {
            let keys: Vec<&str> = path.iter().map(String::as_str).collect();
            self.apply_config_value(&keys, &item.value);
        }
    }

    /// Apply the row the user just changed to the running editor, then write it
    /// to the config file.
    /// Write a sidebar width that was dragged out with the mouse to the same
    /// key the settings screen's `Sidebar Width` row writes, so the two agree
    /// on where the value lives and neither is stale after the other ran.
    pub(crate) fn persist_sidebar_width(&mut self, width: u16) {
        let value = SettingValue::Int {
            value: width as i64,
            min: crate::layout::MIN_SIDEBAR_WIDTH as i64,
            max: crate::layout::MAX_SIDEBAR_WIDTH as i64,
            step: 1,
        };
        let keys = ["ui", "sidebar_width"];
        self.editor.status_message = Some(
            match persist_config_value(&self.config_path, &keys, &value) {
                Ok(()) => format!("Sidebar width {width}"),
                Err(e) => format!("Save failed: {e}"),
            },
        );
    }

    fn apply_and_save_setting(&mut self, index: usize) {
        let Some(item) = self.editor.settings.items.get(index).cloned() else {
            return;
        };
        let SettingTarget::Config(path) = &item.target else {
            return;
        };
        let keys: Vec<&str> = path.iter().map(String::as_str).collect();

        self.apply_config_value(&keys, &item.value);

        let result = persist_config_value(&self.config_path, &keys, &item.value);
        self.editor.settings.message = Some(match result {
            Ok(()) if item.restart_required => {
                format!("Saved {} (restart required)", keys.join("."))
            }
            Ok(()) => format!("Saved {}", keys.join(".")),
            Err(e) => format!("Save failed: {e}"),
        });
    }

    /// Make a changed setting take effect now, where that is possible at all.
    ///
    /// Settings marked `restart_required` reach this function too and fall
    /// through: they are still written to disk, they just cannot be applied to
    /// a running editor.
    fn apply_config_value(&mut self, keys: &[&str], value: &SettingValue) {
        let text = value.display();
        let flag = matches!(value, SettingValue::Bool(true));
        let number = match value {
            SettingValue::Int { value, .. } => *value,
            _ => 0,
        };

        match keys {
            ["theme"] => self.apply_theme(&text),
            ["keymap", "preset"] => self.apply_keymap(&text),
            ["keymap", "chord_timeout_ms"] => {
                self.chord_timeout = std::time::Duration::from_millis(number.max(0) as u64);
            }
            ["ui", "sidebar_visible"] => self.editor.file_explorer.visible = flag,
            ["ui", "sidebar_width"] => self.editor.file_explorer.width = number as u16,
            ["ui", "tree_style"] => self.editor.file_tree_style.tree_style = flag,
            ["ui", "show_file_type_emoji"] => {
                self.editor.file_tree_style.show_file_type_emoji = flag
            }
            ["ui", "respect_gitignore"] => {
                self.editor.file_tree_style.respect_gitignore = flag;
                self.editor.file_explorer.respect_gitignore = flag;
                // The tree was walked under the old rule, so it has to be
                // rebuilt for the change to show.
                if let Err(e) = self.editor.file_explorer.refresh() {
                    log::warn!("File tree refresh after a settings change failed: {e}");
                }
            }
            ["editor", "tab_size"] => self.editor.config.tab_size = number.max(1) as usize,
            ["editor", "insert_spaces"] => self.editor.config.insert_spaces = flag,
            ["editor", "scroll_off"] => self.editor.config.scroll_off = number.max(0) as usize,
            ["editor", "line_numbers"] => {
                if let Some((_, style)) = LINE_NUMBER_STYLES.iter().find(|(name, _)| *name == text)
                {
                    self.editor.config.line_numbers = *style;
                }
            }
            // `editor.mouse_enabled` and everything under `plugins` are read
            // once during startup; there is nothing to update here.
            _ => {}
        }
    }
}

/// The editor mode a `[mode.<name>]` section stands for.
fn section_mode(section: &str) -> Option<EditorMode> {
    [
        EditorMode::Normal,
        EditorMode::Insert,
        EditorMode::FileExplorer,
        EditorMode::Search,
        EditorMode::FuzzyFinder,
        EditorMode::CommandPalette,
        EditorMode::Settings,
    ]
    .into_iter()
    .find(|mode| mode_section_name(*mode) == section)
}

/// Write a setting to `config.toml`.
///
/// Selecting the built-in keymap writes an empty `[keymap] preset` rather than
/// the label: `"(built-in)"` is not a file that could be loaded on the next
/// start, and *removing* the key would fall back to the default preset instead.
fn persist_config_value(
    config_path: &Path,
    keys: &[&str],
    value: &SettingValue,
) -> anyhow::Result<()> {
    if keys == ["keymap", "preset"] && value.display() == BUILTIN_KEYMAP {
        return writer::set_value(config_path, keys, toml_edit::Value::from(""));
    }
    let toml_value = match value {
        SettingValue::Bool(flag) => toml_edit::Value::from(*flag),
        SettingValue::Int { value, .. } => toml_edit::Value::from(*value),
        SettingValue::Choice { .. } => toml_edit::Value::from(value.display()),
        SettingValue::KeyBinding(_) | SettingValue::Info(_) => {
            anyhow::bail!("this setting is not stored in config.toml")
        }
    };
    writer::set_value(config_path, keys, toml_value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;
    use termcode_config::config::AppConfig;
    use termcode_view::settings::{SettingsCategory, SettingsFocus};

    /// An app whose settings screen is open, writing to a config file of its
    /// own so the developer's real `config.toml` is never touched.
    fn app_with_settings(name: &str) -> (App, std::path::PathBuf) {
        let dir = std::env::temp_dir().join("termcode-settings-tests");
        std::fs::create_dir_all(&dir).unwrap();
        let config_path = dir.join(format!("{name}-config.toml"));
        let keybindings_path = dir.join(format!("{name}-keybindings.toml"));
        let _ = std::fs::remove_file(&config_path);
        let _ = std::fs::remove_file(&keybindings_path);

        let mut app = App::with_config(None, AppConfig::default());
        app.config_path = config_path.clone();
        app.keybindings_path = keybindings_path;
        app.open_settings();
        (app, config_path)
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn item_labelled<'a>(app: &'a App, label: &str) -> &'a SettingItem {
        app.editor
            .settings
            .items
            .iter()
            .find(|item| item.label == label)
            .unwrap_or_else(|| panic!("no setting labelled {label}"))
    }

    fn select(app: &mut App, label: &str) {
        let index = app
            .editor
            .settings
            .items
            .iter()
            .position(|item| item.label == label)
            .unwrap_or_else(|| panic!("no setting labelled {label}"));
        app.editor.settings.selected = index;
    }

    #[test]
    fn opening_settings_shows_the_current_values() {
        let (app, _path) = app_with_settings("open");
        assert_eq!(app.editor.mode, EditorMode::Settings);
        assert_eq!(app.editor.settings.category(), SettingsCategory::Appearance);
        assert_eq!(
            item_labelled(&app, "Sidebar Width").value.display(),
            app.editor.file_explorer.width.to_string()
        );
    }

    #[test]
    fn changing_a_value_applies_it_and_writes_the_file() {
        let (mut app, config_path) = app_with_settings("apply");
        app.editor.settings.category_index = 1; // Editor
        app.reload_settings_items();
        select(&mut app, "Tab Size");

        // A number is chosen from a list, like every other multi-value setting.
        app.run_settings_command("settings.activate");
        app.run_settings_command("settings.down");
        assert_eq!(app.editor.config.tab_size, 4, "not applied while choosing");
        app.run_settings_command("settings.activate");

        assert_eq!(app.editor.config.tab_size, 5, "must apply to the editor");
        let written = std::fs::read_to_string(&config_path).unwrap();
        assert!(written.contains("tab_size = 5"), "{written}");
    }

    #[test]
    fn a_restart_only_setting_is_still_saved_and_says_so() {
        let (mut app, config_path) = app_with_settings("restart");
        app.editor.settings.category_index = 1;
        app.reload_settings_items();
        select(&mut app, "Mouse Support");

        app.run_settings_command("settings.activate");

        let written = std::fs::read_to_string(&config_path).unwrap();
        assert!(written.contains("mouse_enabled = false"), "{written}");
        assert!(
            app.editor
                .settings
                .message
                .as_deref()
                .unwrap_or_default()
                .contains("restart"),
            "{:?}",
            app.editor.settings.message
        );
    }

    #[test]
    fn the_chord_timeout_takes_effect_without_a_restart() {
        let (mut app, _path) = app_with_settings("timeout");
        app.editor.settings.category_index = 1;
        app.reload_settings_items();
        select(&mut app, "Chord Timeout (ms)");

        app.run_settings_command("settings.activate");
        app.run_settings_command("settings.down");
        app.run_settings_command("settings.activate");

        assert_eq!(app.chord_timeout.as_millis(), 1100);
    }

    #[test]
    fn the_arrows_only_move_between_panes() {
        let (mut app, config_path) = app_with_settings("panes");
        app.editor.settings.category_index = 1; // Editor
        app.reload_settings_items();
        select(&mut app, "Insert Spaces");
        let before = app.editor.config.insert_spaces;

        // Left steps out to the categories, right steps back in; neither may
        // touch the value the cursor happens to be sitting on.
        app.run_settings_command("settings.focus_out");
        assert_eq!(app.editor.settings.focus, SettingsFocus::Categories);
        app.run_settings_command("settings.focus_in");
        assert_eq!(app.editor.settings.focus, SettingsFocus::Items);
        assert_eq!(app.editor.config.insert_spaces, before);
        assert!(!config_path.exists(), "nothing may have been written");

        // Space does what Enter does.
        app.run_settings_command("settings.activate");
        assert_ne!(app.editor.config.insert_spaces, before);
    }

    #[test]
    fn a_captured_chord_is_written_and_bound_immediately() {
        let (mut app, _path) = app_with_settings("rebind");
        app.editor.settings.category_index = 2; // Keybindings
        app.reload_settings_items();
        select(&mut app, "Save File");

        app.run_settings_command("settings.activate");
        assert!(app.editor.settings.capturing.is_some());

        app.handle_settings_key(key(KeyCode::Char('g')));
        app.handle_settings_key(key(KeyCode::Char('g')));
        app.handle_settings_key(key(KeyCode::Enter));

        let written = std::fs::read_to_string(&app.keybindings_path).unwrap();
        assert!(written.contains("\"g g\""), "{written}");
        assert!(written.contains("file.save"), "{written}");
        // The preset's Ctrl+S is still bound: keybindings.toml cannot take a
        // key away, so the row has to show both.
        assert_eq!(
            app.input_mapper.bindings_for("file.save"),
            vec!["Ctrl+S".to_string(), "g g".to_string()]
        );
        assert_eq!(
            item_labelled(&app, "Save File").value.display(),
            "Ctrl+S, g g"
        );
    }

    #[test]
    fn esc_abandons_a_capture_without_writing_anything() {
        let (mut app, _path) = app_with_settings("cancel");
        app.editor.settings.category_index = 2;
        app.reload_settings_items();
        select(&mut app, "Save File");

        app.run_settings_command("settings.activate");
        app.handle_settings_key(key(KeyCode::Char('z')));
        app.handle_settings_key(key(KeyCode::Esc));

        assert!(app.editor.settings.capturing.is_none());
        assert!(app.settings_capture.is_empty());
        assert!(!app.keybindings_path.exists());
    }

    #[test]
    fn a_rebinding_onto_keys_in_use_says_what_it_displaces() {
        let (mut app, _path) = app_with_settings("conflict");
        app.editor.settings.category_index = 2;
        app.reload_settings_items();
        // Undo is global in the built-in keymap, and so is Ctrl+S -- which
        // belongs to Save. Taking it over has to be reported, not silent.
        select(&mut app, "Undo");

        app.run_settings_command("settings.activate");
        app.handle_settings_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
        app.handle_settings_key(key(KeyCode::Enter));

        let message = app.editor.settings.message.clone().unwrap_or_default();
        assert!(message.contains("conflicts with"), "{message}");
        assert!(message.contains("file.save"), "{message}");
    }

    #[test]
    fn rebinding_a_command_to_the_keys_it_already_has_is_not_a_conflict() {
        let (mut app, _path) = app_with_settings("self");
        app.editor.settings.category_index = 2;
        app.reload_settings_items();
        select(&mut app, "Save File");

        app.run_settings_command("settings.activate");
        app.handle_settings_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
        app.handle_settings_key(key(KeyCode::Enter));

        assert_eq!(
            app.editor.settings.message.as_deref(),
            Some("Bound ctrl+s to file.save")
        );
    }

    #[test]
    fn selecting_the_builtin_keymap_empties_the_preset_key() {
        let (mut app, config_path) = app_with_settings("builtin");
        std::fs::write(&config_path, "[keymap]\npreset = \"vim\"\n").unwrap();
        app.keymap_name = "vim".to_string();
        app.reload_settings_items();
        select(&mut app, "Keymap Preset");
        // The list puts the built-in first, so walking to the top lands on it.
        app.run_settings_command("settings.activate");
        for _ in 0..10 {
            app.run_settings_command("settings.up");
        }
        app.run_settings_command("settings.activate");
        assert_eq!(
            item_labelled(&app, "Keymap Preset").value.display(),
            BUILTIN_KEYMAP
        );

        let written = std::fs::read_to_string(&config_path).unwrap();
        // Empty, not absent: an absent key means "the default preset".
        assert!(written.contains(r#"preset = """#), "{written}");
        assert_eq!(app.editor.mode, EditorMode::Settings, "must stay open");
    }

    /// Which Appearance rows preview as their list is browsed.
    ///
    /// This is the invariant the lockout hinged on, and it holds no matter what
    /// is installed on the machine running the test.
    #[test]
    fn only_the_theme_previews_while_its_list_is_open() {
        let (app, _path) = app_with_settings("preview-flags");
        assert!(
            item_labelled(&app, "Theme").live_preview,
            "a theme has to be seen to be chosen"
        );
        assert!(
            !item_labelled(&app, "Keymap Preset").live_preview,
            "previewing a keymap would change the keys driving the list"
        );
    }

    /// Put one row of our own on screen.
    ///
    /// The Appearance rows are built from whatever themes and keymaps are
    /// installed, which is nothing at all on a CI machine -- so anything about
    /// how the list *behaves* is tested against a row the test owns.
    fn with_row(app: &mut App, item: SettingItem) {
        app.editor.settings.load_items(vec![item]);
        app.editor.settings.selected = 0;
    }

    fn width_row() -> SettingItem {
        SettingItem::new(
            "Sidebar Width",
            SettingValue::Int {
                value: 30,
                min: 28,
                max: 32,
                step: 1,
            },
            config_target(&["ui", "sidebar_width"]),
        )
    }

    #[test]
    fn a_width_dragged_out_with_the_mouse_lands_in_the_config_file() {
        let (mut app, config_path) = app_with_settings("mouse-resize");
        app.terminal_size = (120, 24);
        app.editor.file_explorer.visible = true;
        app.editor.file_explorer.width = 30;

        // The divider is the sidebar's last column; drag it eight to the right.
        let at = |kind, x| crossterm::event::MouseEvent {
            kind,
            column: x,
            row: 5,
            modifiers: KeyModifiers::NONE,
        };
        use crossterm::event::{MouseButton, MouseEventKind};
        app.handle_mouse(at(MouseEventKind::Down(MouseButton::Left), 29));
        app.handle_mouse(at(MouseEventKind::Drag(MouseButton::Left), 37));
        assert_eq!(app.editor.file_explorer.width, 38);
        assert!(!config_path.exists(), "nothing is written mid-drag");

        app.handle_mouse(at(MouseEventKind::Up(MouseButton::Left), 37));
        let written = std::fs::read_to_string(&config_path).unwrap();
        assert!(written.contains("sidebar_width = 38"), "{written}");

        // The settings row reads the same value, so the two paths agree.
        app.reload_settings_items();
        assert_eq!(
            item_labelled(&app, "Sidebar Width").value.display(),
            "38",
            "the settings screen shows what the drag produced"
        );
    }

    #[test]
    fn a_value_is_not_applied_while_the_list_is_still_open() {
        let (mut app, config_path) = app_with_settings("no-preview");
        with_row(&mut app, width_row());
        let before = app.editor.file_explorer.width;

        app.run_settings_command("settings.activate");
        // Walking past a value must not apply it: that is how moving through
        // the keymap list could take away the keys driving the list.
        app.run_settings_command("settings.down");
        assert_eq!(app.editor.file_explorer.width, before, "not applied yet");
        assert!(!config_path.exists(), "nothing may be written yet");

        app.run_settings_command("settings.activate");
        assert_eq!(
            app.editor.file_explorer.width, 31,
            "Enter is what applies it"
        );
        assert!(
            std::fs::read_to_string(&config_path)
                .unwrap()
                .contains("sidebar_width = 31")
        );
    }

    #[test]
    fn a_previewing_row_applies_as_the_list_moves_and_rolls_back() {
        let (mut app, config_path) = app_with_settings("preview");
        with_row(&mut app, width_row().with_live_preview());

        app.run_settings_command("settings.activate");
        app.run_settings_command("settings.down");
        assert_eq!(app.editor.file_explorer.width, 31, "should be showing");
        assert!(!config_path.exists(), "a preview must not be saved");

        app.run_settings_command("settings.close");
        assert_eq!(app.editor.file_explorer.width, 30, "Esc must roll it back");
        assert!(!config_path.exists());
    }

    #[test]
    fn a_previewed_value_is_saved_once_it_is_chosen() {
        let (mut app, config_path) = app_with_settings("preview-commit");
        with_row(&mut app, width_row().with_live_preview());

        app.run_settings_command("settings.activate");
        app.run_settings_command("settings.down");
        app.run_settings_command("settings.activate");

        assert_eq!(app.editor.file_explorer.width, 31);
        let written = std::fs::read_to_string(&config_path).unwrap();
        assert!(written.contains("sidebar_width = 31"), "{written}");
        assert!(app.editor.settings.picker.is_none());
    }

    #[test]
    fn left_backs_out_of_the_value_list() {
        let (mut app, config_path) = app_with_settings("list-back");
        with_row(&mut app, width_row().with_live_preview());

        app.run_settings_command("settings.activate");
        app.run_settings_command("settings.down");
        app.run_settings_command("settings.focus_out");

        assert!(
            app.editor.settings.picker.is_none(),
            "the list should close"
        );
        assert_eq!(app.editor.file_explorer.width, 30, "preview rolled back");
        assert_eq!(
            app.editor.settings.focus,
            SettingsFocus::Items,
            "backing out of the list is not backing out of the pane"
        );
        assert!(!config_path.exists());
    }

    #[test]
    fn the_list_swallows_keys_that_would_act_on_the_screen_behind_it() {
        let (mut app, _path) = app_with_settings("list-owns-keys");
        with_row(&mut app, width_row());
        app.run_settings_command("settings.activate");

        // Tab would switch panes, and closing the whole screen mid-choice
        // would strand a preview.
        app.run_settings_command("settings.toggle_focus");
        assert_eq!(app.editor.settings.focus, SettingsFocus::Items);
        assert_eq!(app.editor.settings.selected, 0);
        assert!(app.editor.settings.picker.is_some());
        assert_eq!(app.editor.mode, EditorMode::Settings);
    }

    #[test]
    fn esc_closes_the_screen_even_when_a_keymap_binds_nothing_here() {
        let (mut app, _path) = app_with_settings("escape-hatch");
        // A keymap with no `[mode.settings]` section at all.
        let bare: termcode_config::keymap::KeymapPreset =
            toml::from_str("[meta]\nname = \"bare\"\n").unwrap();
        app.input_mapper = crate::input::InputMapper::from_preset(&bare, &app.command_registry);

        app.handle_settings_key(key(KeyCode::Esc));

        assert_ne!(app.editor.mode, EditorMode::Settings);
    }

    #[test]
    fn switching_category_reloads_the_rows() {
        let (mut app, _path) = app_with_settings("category");
        app.editor.settings.set_focus(SettingsFocus::Categories);
        app.run_settings_command("settings.down");
        assert_eq!(app.editor.settings.category(), SettingsCategory::Editor);
        assert!(
            app.editor
                .settings
                .items
                .iter()
                .any(|item| item.label == "Tab Size")
        );
    }

    #[test]
    fn read_only_rows_are_never_written() {
        let path = std::env::temp_dir().join("termcode-settings-readonly.toml");
        let _ = std::fs::remove_file(&path);
        let result = persist_config_value(&path, &["lsp"], &SettingValue::Info("rust".into()));
        assert!(result.is_err());
        assert!(!path.exists());
    }

    #[test]
    fn every_mode_section_name_maps_back_to_its_mode() {
        for mode in [
            EditorMode::Normal,
            EditorMode::Insert,
            EditorMode::FileExplorer,
            EditorMode::Search,
            EditorMode::FuzzyFinder,
            EditorMode::CommandPalette,
            EditorMode::Settings,
        ] {
            assert_eq!(section_mode(mode_section_name(mode)), Some(mode));
        }
        assert_eq!(section_mode("nromal"), None);
    }
}
