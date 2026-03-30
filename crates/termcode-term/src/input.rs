use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use termcode_view::editor::EditorMode;

use crate::command::CommandId;

pub struct InputMapper {
    global: Vec<(KeyEvent, CommandId)>,
    normal: Vec<(KeyEvent, CommandId)>,
    insert: Vec<(KeyEvent, CommandId)>,
    file_explorer: Vec<(KeyEvent, CommandId)>,
    search: Vec<(KeyEvent, CommandId)>,
    fuzzy_finder: Vec<(KeyEvent, CommandId)>,
    command_palette: Vec<(KeyEvent, CommandId)>,
}

fn key(modifiers: KeyModifiers, code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, modifiers)
}

impl InputMapper {
    pub fn new() -> Self {
        let none = KeyModifiers::NONE;
        let ctrl = KeyModifiers::CONTROL;
        let alt = KeyModifiers::ALT;
        let shift = KeyModifiers::SHIFT;

        let global = vec![
            (key(ctrl, KeyCode::Char('b')), "view.toggle_sidebar"),
            (key(alt, KeyCode::Right), "tab.next"),
            (key(alt, KeyCode::Left), "tab.prev"),
            (key(ctrl, KeyCode::Char('w')), "tab.close"),
            (key(ctrl, KeyCode::Char('s')), "file.save"),
            (key(ctrl, KeyCode::Char('z')), "edit.undo"),
            (key(ctrl, KeyCode::Char('y')), "edit.redo"),
            (key(ctrl, KeyCode::Char('f')), "search.open"),
            (key(ctrl, KeyCode::Char('h')), "search.open_replace"),
            (key(ctrl, KeyCode::Char('p')), "fuzzy.open"),
            (key(ctrl | shift, KeyCode::Char('P')), "palette.open"),
            (key(ctrl, KeyCode::Char('v')), "clipboard.paste"),
            (key(ctrl, KeyCode::Char('x')), "clipboard.cut"),
            (key(none, KeyCode::F(1)), "help.toggle"),
        ];

        let normal = vec![
            (key(none, KeyCode::Char('j')), "cursor.down"),
            (key(none, KeyCode::Down), "cursor.down"),
            (key(none, KeyCode::Char('k')), "cursor.up"),
            (key(none, KeyCode::Up), "cursor.up"),
            (key(none, KeyCode::Char('h')), "cursor.left"),
            (key(none, KeyCode::Left), "cursor.left"),
            (key(none, KeyCode::Char('l')), "cursor.right"),
            (key(none, KeyCode::Right), "cursor.right"),
            (key(none, KeyCode::PageDown), "cursor.page_down"),
            (key(none, KeyCode::PageUp), "cursor.page_up"),
            (key(none, KeyCode::Char('g')), "cursor.home"),
            (key(none, KeyCode::Home), "cursor.home"),
            (key(shift, KeyCode::Char('G')), "cursor.end"),
            (key(none, KeyCode::End), "cursor.end"),
            (key(none, KeyCode::Char('i')), "mode.insert"),
            (key(none, KeyCode::Char('x')), "edit.delete_char"),
            (key(none, KeyCode::Delete), "edit.delete_char"),
            (key(none, KeyCode::Char(']')), "diagnostic.next"),
            (key(none, KeyCode::Char('[')), "diagnostic.prev"),
            (key(ctrl, KeyCode::Char('d')), "goto.definition"),
            (key(none, KeyCode::F(12)), "goto.definition"),
            (key(shift, KeyCode::Char('K')), "lsp.hover"),
            (key(none, KeyCode::Char(':')), "palette.open"),
            (key(none, KeyCode::Char('`')), "palette.open"),
            (key(shift, KeyCode::Char('?')), "help.toggle"),
        ];

        let insert = vec![
            (key(none, KeyCode::Esc), "mode.normal"),
            (key(none, KeyCode::Backspace), "edit.backspace"),
            (key(none, KeyCode::Delete), "edit.delete_char"),
            (key(none, KeyCode::Enter), "edit.newline"),
            (key(none, KeyCode::Up), "cursor.up"),
            (key(none, KeyCode::Down), "cursor.down"),
            (key(none, KeyCode::Left), "cursor.left"),
            (key(none, KeyCode::Right), "cursor.right"),
        ];

        let file_explorer = vec![
            (key(none, KeyCode::Char('j')), "explorer.down"),
            (key(none, KeyCode::Down), "explorer.down"),
            (key(none, KeyCode::Char('k')), "explorer.up"),
            (key(none, KeyCode::Up), "explorer.up"),
            (key(none, KeyCode::Enter), "explorer.enter"),
            (key(none, KeyCode::Char('l')), "explorer.expand"),
            (key(none, KeyCode::Right), "explorer.expand"),
            (key(none, KeyCode::Char('h')), "explorer.collapse"),
            (key(none, KeyCode::Left), "explorer.collapse"),
            (key(none, KeyCode::Esc), "mode.normal"),
            (key(none, KeyCode::Tab), "mode.normal"),
        ];

        let search = vec![
            (key(none, KeyCode::Esc), "search.close"),
            (key(none, KeyCode::Enter), "search.next"),
            (key(shift, KeyCode::Enter), "search.prev"),
        ];

        let fuzzy_finder = vec![
            (key(none, KeyCode::Esc), "fuzzy.close"),
            (key(none, KeyCode::Up), "fuzzy.up"),
            (key(ctrl, KeyCode::Char('k')), "fuzzy.up"),
            (key(none, KeyCode::Down), "fuzzy.down"),
            (key(ctrl, KeyCode::Char('j')), "fuzzy.down"),
        ];

        let command_palette = vec![
            (key(none, KeyCode::Esc), "palette.close"),
            (key(none, KeyCode::Up), "palette.up"),
            (key(ctrl, KeyCode::Char('k')), "palette.up"),
            (key(none, KeyCode::Down), "palette.down"),
            (key(ctrl, KeyCode::Char('j')), "palette.down"),
        ];

        Self {
            global,
            normal,
            insert,
            file_explorer,
            search,
            fuzzy_finder,
            command_palette,
        }
    }

    /// Try to resolve a global keybinding (checked regardless of mode).
    pub fn resolve_global(&self, key: KeyEvent) -> Option<CommandId> {
        self.global
            .iter()
            .find(|(k, _)| keys_match(k, &key))
            .map(|(_, cmd)| *cmd)
    }

    /// Try to resolve a mode-specific keybinding.
    pub fn resolve(&self, mode: EditorMode, key: KeyEvent) -> Option<CommandId> {
        let bindings = match mode {
            EditorMode::Normal => &self.normal,
            EditorMode::Insert => &self.insert,
            EditorMode::FileExplorer => &self.file_explorer,
            EditorMode::Search => &self.search,
            EditorMode::FuzzyFinder => &self.fuzzy_finder,
            EditorMode::CommandPalette => &self.command_palette,
        };
        bindings
            .iter()
            .find(|(k, _)| keys_match(k, &key))
            .map(|(_, cmd)| *cmd)
    }

    /// Apply keybinding overrides from configuration.
    /// Validates command names against the registry. Invalid commands are logged and skipped.
    pub fn apply_overrides(
        &mut self,
        config: &termcode_config::keymap::KeybindingConfig,
        registry: &crate::command::CommandRegistry,
    ) {
        apply_binding_overrides(&mut self.global, &config.global, registry);
        apply_binding_overrides(&mut self.normal, &config.modes.normal, registry);
        apply_binding_overrides(&mut self.insert, &config.modes.insert, registry);
        apply_binding_overrides(
            &mut self.file_explorer,
            &config.modes.file_explorer,
            registry,
        );
        apply_binding_overrides(&mut self.search, &config.modes.search, registry);
        apply_binding_overrides(&mut self.fuzzy_finder, &config.modes.fuzzy_finder, registry);
        apply_binding_overrides(
            &mut self.command_palette,
            &config.modes.command_palette,
            registry,
        );
    }
}

impl Default for InputMapper {
    fn default() -> Self {
        Self::new()
    }
}

fn apply_binding_overrides(
    bindings: &mut Vec<(KeyEvent, CommandId)>,
    overrides: &std::collections::HashMap<String, String>,
    registry: &crate::command::CommandRegistry,
) {
    for (key_str, cmd_str) in overrides {
        let key = match termcode_config::keymap::parse_key_combo(key_str) {
            Some(k) => k,
            None => {
                log::warn!("Invalid key combo in keybinding override: {key_str}");
                continue;
            }
        };
        let cmd_entry = match registry.get_by_string(cmd_str) {
            Some(e) => e,
            None => {
                log::warn!("Unknown command in keybinding override: {cmd_str}");
                continue;
            }
        };
        let cmd_id: CommandId = cmd_entry.id;
        if let Some(existing) = bindings.iter_mut().find(|(k, _)| keys_match(k, &key)) {
            existing.1 = cmd_id;
        } else {
            bindings.push((key, cmd_id));
        }
    }
}

/// Compare two KeyEvents for matching, ignoring state flags.
fn keys_match(expected: &KeyEvent, actual: &KeyEvent) -> bool {
    expected.code == actual.code && expected.modifiers == actual.modifiers
}
