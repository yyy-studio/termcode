use std::collections::HashMap;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use termcode_config::keymap::{KeymapPreset, parse_key_sequence};
use termcode_view::editor::EditorMode;

use crate::command::{CommandId, CommandRegistry};

/// One or more key presses that together trigger a command. Single-key
/// bindings are just one-element sequences.
type KeySeq = Vec<KeyEvent>;
type Binding = (KeySeq, CommandId);

/// Outcome of feeding a key press to the mapper.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyResolution {
    /// A complete binding matched; the pending buffer has been cleared.
    Match(CommandId),
    /// The keys so far are a prefix of at least one binding. The key was
    /// consumed and the caller should wait for the next one.
    Pending,
    /// Nothing matched. The caller may handle the key itself.
    NoMatch,
}

pub struct InputMapper {
    global: Vec<Binding>,
    normal: Vec<Binding>,
    insert: Vec<Binding>,
    file_explorer: Vec<Binding>,
    search: Vec<Binding>,
    fuzzy_finder: Vec<Binding>,
    command_palette: Vec<Binding>,
    settings: Vec<Binding>,
    /// Keys of a partially typed chord, waiting for completion.
    pending: KeySeq,
}

/// Every mode with a binding table, in the order a rebinding looks for the one
/// a command already lives in.
const ALL_MODES: [EditorMode; 7] = [
    EditorMode::Normal,
    EditorMode::Insert,
    EditorMode::FileExplorer,
    EditorMode::Search,
    EditorMode::FuzzyFinder,
    EditorMode::CommandPalette,
    EditorMode::Settings,
];

/// The `[mode.<name>]` section a mode's bindings are written to, matching the
/// field names of `termcode_config::keymap::ModeBindings`.
pub fn mode_section_name(mode: EditorMode) -> &'static str {
    match mode {
        EditorMode::Normal => "normal",
        EditorMode::Insert => "insert",
        EditorMode::FileExplorer => "file_explorer",
        EditorMode::Search => "search",
        EditorMode::FuzzyFinder => "fuzzy_finder",
        EditorMode::CommandPalette => "command_palette",
        EditorMode::Settings => "settings",
    }
}

fn key(modifiers: KeyModifiers, code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, modifiers)
}

/// Keys for the settings screen.
///
/// These are also the fallback for a preset that declares no `[mode.settings]`
/// -- see [`InputMapper::from_preset`].
fn default_settings_bindings() -> Vec<(KeyEvent, CommandId)> {
    let none = KeyModifiers::NONE;
    let ctrl = KeyModifiers::CONTROL;
    vec![
        (key(none, KeyCode::Esc), "settings.close"),
        (key(none, KeyCode::Up), "settings.up"),
        (key(ctrl, KeyCode::Char('k')), "settings.up"),
        (key(none, KeyCode::Down), "settings.down"),
        (key(ctrl, KeyCode::Char('j')), "settings.down"),
        (key(none, KeyCode::PageUp), "settings.page_up"),
        (key(none, KeyCode::PageDown), "settings.page_down"),
        (key(none, KeyCode::Left), "settings.focus_out"),
        (key(none, KeyCode::Right), "settings.focus_in"),
        (key(none, KeyCode::Enter), "settings.activate"),
        (key(none, KeyCode::Char(' ')), "settings.activate"),
        (key(none, KeyCode::Tab), "settings.toggle_focus"),
    ]
}

/// Wrap single-key bindings into one-element sequences.
fn seqs(bindings: Vec<(KeyEvent, CommandId)>) -> Vec<Binding> {
    bindings.into_iter().map(|(k, c)| (vec![k], c)).collect()
}

impl InputMapper {
    /// The built-in keymap: a hybrid of VS Code-style global shortcuts and a
    /// small modal layer. Used when no `[keymap] preset` is configured.
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
            (key(ctrl, KeyCode::Char('c')), "clipboard.copy"),
            (key(none, KeyCode::F(1)), "help.toggle"),
            // F2 rather than the conventional Ctrl+, because most terminals
            // have no encoding for Ctrl with a punctuation key.
            (key(none, KeyCode::F(2)), "settings.open"),
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
            (key(none, KeyCode::Char('0')), "cursor.line_start"),
            (key(none, KeyCode::Char('$')), "cursor.line_end"),
            (key(shift, KeyCode::Char('$')), "cursor.line_end"),
            (key(none, KeyCode::PageDown), "cursor.page_down"),
            (key(none, KeyCode::PageUp), "cursor.page_up"),
            (key(none, KeyCode::Char('g')), "cursor.home"),
            (key(none, KeyCode::Home), "cursor.line_start"),
            (key(shift, KeyCode::Char('G')), "cursor.end"),
            (key(none, KeyCode::End), "cursor.line_end"),
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
            (key(none, KeyCode::Home), "cursor.line_start"),
            (key(none, KeyCode::End), "cursor.line_end"),
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
            (key(none, KeyCode::Char('r')), "explorer.refresh"),
            (key(shift, KeyCode::Char('R')), "explorer.refresh_all"),
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
            global: seqs(global),
            normal: seqs(normal),
            insert: seqs(insert),
            file_explorer: seqs(file_explorer),
            search: seqs(search),
            fuzzy_finder: seqs(fuzzy_finder),
            command_palette: seqs(command_palette),
            settings: seqs(default_settings_bindings()),
            pending: Vec::new(),
        }
    }

    /// Build a mapper from a keymap preset, discarding the built-in defaults.
    ///
    /// A preset defines the whole keymap, so anything it omits is unbound. That
    /// keeps presets from inheriting bindings that contradict them — a Vim
    /// preset does not silently keep `Ctrl+D` on "go to definition".
    ///
    /// `[mode.settings]` is the one exception. That mode consults no global
    /// table, so a preset written before the settings screen existed — or one a
    /// user wrote from an older example — would leave it with no working keys
    /// at all. The settings screen is infrastructure rather than an editing
    /// surface, so a preset that says nothing about it gets the defaults
    /// instead of nothing. Declaring even one binding there takes full control,
    /// as with every other mode.
    pub fn from_preset(preset: &KeymapPreset, registry: &CommandRegistry) -> Self {
        Self {
            global: build_bindings(&preset.global, registry, "global"),
            normal: build_bindings(&preset.modes.normal, registry, "mode.normal"),
            insert: build_bindings(&preset.modes.insert, registry, "mode.insert"),
            file_explorer: build_bindings(
                &preset.modes.file_explorer,
                registry,
                "mode.file_explorer",
            ),
            search: build_bindings(&preset.modes.search, registry, "mode.search"),
            fuzzy_finder: build_bindings(&preset.modes.fuzzy_finder, registry, "mode.fuzzy_finder"),
            command_palette: build_bindings(
                &preset.modes.command_palette,
                registry,
                "mode.command_palette",
            ),
            settings: if preset.modes.settings.is_empty() {
                seqs(default_settings_bindings())
            } else {
                build_bindings(&preset.modes.settings, registry, "mode.settings")
            },
            pending: Vec::new(),
        }
    }

    /// Feed one key press to the mapper.
    ///
    /// Mode-specific bindings take precedence over global ones, so a preset can
    /// reclaim a global shortcut inside a mode — for example binding `Ctrl+W` to
    /// "delete word" in Insert mode without losing "close tab" elsewhere.
    ///
    /// Global bindings are not consulted in the overlay modes (Search, fuzzy
    /// finder, command palette), where keys belong to the overlay's text input.
    pub fn resolve_key(&mut self, mode: EditorMode, key: KeyEvent) -> KeyResolution {
        let mut candidate = std::mem::take(&mut self.pending);
        candidate.push(key);

        // Each table is settled completely — exact match, then prefix — before
        // the next one is consulted. Checking every table for an exact match
        // first would let a global single-key binding fire while the mode table
        // holds a chord starting with that key, which contradicts the
        // mode-before-global precedence above.
        for table in self.tables(mode) {
            if let Some((_, cmd)) = table.iter().find(|(seq, _)| seq_matches(seq, &candidate)) {
                return KeyResolution::Match(cmd);
            }
            if table_has_longer_binding(table, &candidate) {
                self.pending = candidate;
                return KeyResolution::Pending;
            }
        }
        // Dead sequence: drop it rather than retrying the last key on its own,
        // so a mistyped chord can never fire an unrelated command.
        KeyResolution::NoMatch
    }

    /// Abandon a partially typed chord. Returns whether anything was pending.
    pub fn clear_pending(&mut self) -> bool {
        let had = !self.pending.is_empty();
        self.pending.clear();
        had
    }

    pub fn has_pending(&self) -> bool {
        !self.pending.is_empty()
    }

    /// The keys buffered for a chord in progress. Callers need these to recover
    /// the keystrokes when a chord dies in Insert mode.
    pub fn pending(&self) -> &[KeyEvent] {
        &self.pending
    }

    /// Human-readable form of the pending chord, for the status bar.
    pub fn pending_display(&self) -> String {
        format_key_sequence(&self.pending)
    }

    /// The key sequence bound to `command_id` in `mode`, formatted for display.
    /// Mode bindings win over global ones, matching resolution order.
    pub fn binding_for(&self, mode: EditorMode, command_id: &str) -> Option<String> {
        for table in self.tables(mode) {
            if let Some((seq, _)) = table.iter().find(|(_, cmd)| *cmd == command_id) {
                return Some(format_key_sequence(seq));
            }
        }
        None
    }

    /// Which table currently binds `command_id`, and to what keys.
    ///
    /// The settings screen writes a rebinding back into the section the command
    /// already lives in: moving `search.next` would be pointless in `[global]`,
    /// which the overlay modes never consult. `None` for the mode means the
    /// binding is global; `None` overall means the command is unbound.
    pub fn binding_scope(&self, command_id: &str) -> Option<(Option<EditorMode>, String)> {
        for mode in ALL_MODES {
            if let Some((seq, _)) = self
                .mode_table(mode)
                .iter()
                .find(|(_, cmd)| *cmd == command_id)
            {
                return Some((Some(mode), format_key_sequence(seq)));
            }
        }
        self.global
            .iter()
            .find(|(_, cmd)| *cmd == command_id)
            .map(|(seq, _)| (None, format_key_sequence(seq)))
    }

    /// Every key sequence bound to `command_id`, formatted for display.
    ///
    /// `keybindings.toml` is indexed by key, not by command, so adding a
    /// binding never removes the one a preset already gave the command. The
    /// settings screen shows all of them rather than the first, which would
    /// leave a just-changed row still displaying the old keys.
    pub fn bindings_for(&self, command_id: &str) -> Vec<String> {
        let mut keys: Vec<String> = Vec::new();
        for table in ALL_MODES
            .iter()
            .map(|mode| self.mode_table(*mode))
            .chain(std::iter::once(&self.global))
        {
            for (seq, cmd) in table {
                if *cmd != command_id {
                    continue;
                }
                let formatted = format_key_sequence(seq);
                // The same keys can be declared in several mode tables; listing
                // them once each would read as a conflict that is not there.
                if !keys.contains(&formatted) {
                    keys.push(formatted);
                }
            }
        }
        keys
    }

    /// Bindings already in the table for `mode` (or `[global]`) that `seq`
    /// would collide with: the same sequence bound elsewhere, or a sequence
    /// that shares a prefix with it.
    ///
    /// An exact match fires immediately, so a binding that is the prefix of a
    /// longer one makes the longer one unreachable within its table -- the
    /// hazard `tests/keymap_presets.rs` guards the shipped presets against, and
    /// which a user rebinding keys can otherwise walk straight into.
    pub fn conflicts(
        &self,
        mode: Option<EditorMode>,
        seq: &[KeyEvent],
        command_id: &str,
    ) -> Vec<(String, CommandId)> {
        let table = match mode {
            Some(mode) => self.mode_table(mode),
            None => &self.global,
        };
        table
            .iter()
            .filter(|(existing, cmd)| {
                *cmd != command_id
                    && (existing.starts_with(seq) || seq.starts_with(existing.as_slice()))
            })
            .map(|(existing, cmd)| (format_key_sequence(existing), *cmd))
            .collect()
    }

    /// The bindings declared for `mode` alone, without the global fallback.
    fn mode_table(&self, mode: EditorMode) -> &Vec<Binding> {
        match mode {
            EditorMode::Normal => &self.normal,
            EditorMode::Insert => &self.insert,
            EditorMode::FileExplorer => &self.file_explorer,
            EditorMode::Search => &self.search,
            EditorMode::FuzzyFinder => &self.fuzzy_finder,
            EditorMode::CommandPalette => &self.command_palette,
            EditorMode::Settings => &self.settings,
        }
    }

    /// Apply keybinding overrides from configuration on top of the current map.
    /// Validates command names against the registry. Invalid commands are logged
    /// and skipped.
    pub fn apply_overrides(
        &mut self,
        config: &termcode_config::keymap::KeybindingConfig,
        registry: &CommandRegistry,
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
        apply_binding_overrides(&mut self.settings, &config.modes.settings, registry);
    }

    /// Tables to search for `mode`, in precedence order.
    fn tables(&self, mode: EditorMode) -> Vec<&Vec<Binding>> {
        let mode_table = self.mode_table(mode);
        match mode {
            // Settings joins the overlay modes: its keys drive the screen in
            // front of the user, and a global `Ctrl+P` firing from inside it
            // would leave the settings behind a finder it cannot return to.
            EditorMode::Search
            | EditorMode::FuzzyFinder
            | EditorMode::CommandPalette
            | EditorMode::Settings => {
                vec![mode_table]
            }
            _ => vec![mode_table, &self.global],
        }
    }
}

/// Whether `table` holds a binding that `candidate` is a strict prefix of.
fn table_has_longer_binding(table: &[Binding], candidate: &[KeyEvent]) -> bool {
    table.iter().any(|(seq, _)| {
        seq.len() > candidate.len() && seq_matches(&seq[..candidate.len()], candidate)
    })
}

impl Default for InputMapper {
    fn default() -> Self {
        Self::new()
    }
}

/// Turn a `key string -> command` table from TOML into validated bindings.
fn build_bindings(
    source: &HashMap<String, String>,
    registry: &CommandRegistry,
    section: &str,
) -> Vec<Binding> {
    let mut bindings = Vec::with_capacity(source.len());
    for (key_str, cmd_str) in source {
        let Some(seq) = parse_key_sequence(key_str) else {
            log::warn!("Invalid key sequence in keymap [{section}]: {key_str}");
            continue;
        };
        let Some(entry) = registry.get_by_string(cmd_str) else {
            log::warn!("Unknown command in keymap [{section}]: {cmd_str}");
            continue;
        };
        bindings.push((seq, entry.id));
    }
    // HashMap iteration order is arbitrary; sort so resolution is deterministic
    // when a keymap accidentally binds the same sequence twice.
    bindings.sort_by(|a, b| a.1.cmp(b.1));
    bindings
}

fn apply_binding_overrides(
    bindings: &mut Vec<Binding>,
    overrides: &HashMap<String, String>,
    registry: &CommandRegistry,
) {
    for (key_str, cmd_str) in overrides {
        let Some(seq) = parse_key_sequence(key_str) else {
            log::warn!("Invalid key combo in keybinding override: {key_str}");
            continue;
        };
        let Some(cmd_entry) = registry.get_by_string(cmd_str) else {
            log::warn!("Unknown command in keybinding override: {cmd_str}");
            continue;
        };
        let cmd_id: CommandId = cmd_entry.id;
        if let Some(existing) = bindings.iter_mut().find(|(s, _)| seq_matches(s, &seq)) {
            existing.1 = cmd_id;
        } else {
            bindings.push((seq, cmd_id));
        }
    }
}

/// Compare two key sequences, ignoring the state flags crossterm may attach.
fn seq_matches(expected: &[KeyEvent], actual: &[KeyEvent]) -> bool {
    expected.len() == actual.len()
        && expected
            .iter()
            .zip(actual)
            .all(|(e, a)| e.code == a.code && e.modifiers == a.modifiers)
}

/// Render a key sequence the way a user would type it: `"Ctrl+P"`, `"g g"`.
fn format_key_sequence(seq: &[KeyEvent]) -> String {
    seq.iter().map(format_key).collect::<Vec<_>>().join(" ")
}

fn format_key(key: &KeyEvent) -> String {
    let mut out = String::new();
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        out.push_str("Ctrl+");
    }
    if key.modifiers.contains(KeyModifiers::ALT) {
        out.push_str("Alt+");
    }
    if key.modifiers.contains(KeyModifiers::SHIFT) {
        out.push_str("Shift+");
    }
    // Ctrl/Alt combos are conventionally written with a capital letter
    // ("Ctrl+S"), even though the binding itself stores the lowercase char.
    let capitalize = key
        .modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT);
    let code = match key.code {
        KeyCode::Char(' ') => "Space".to_string(),
        KeyCode::Char(c) if capitalize => c.to_ascii_uppercase().to_string(),
        KeyCode::Char(c) => c.to_string(),
        KeyCode::F(n) => format!("F{n}"),
        KeyCode::Enter => "Enter".to_string(),
        KeyCode::Esc => "Esc".to_string(),
        KeyCode::Backspace => "Backspace".to_string(),
        KeyCode::Delete => "Del".to_string(),
        KeyCode::Tab => "Tab".to_string(),
        KeyCode::BackTab => "Shift+Tab".to_string(),
        KeyCode::Up => "Up".to_string(),
        KeyCode::Down => "Down".to_string(),
        KeyCode::Left => "Left".to_string(),
        KeyCode::Right => "Right".to_string(),
        KeyCode::Home => "Home".to_string(),
        KeyCode::End => "End".to_string(),
        KeyCode::PageUp => "PgUp".to_string(),
        KeyCode::PageDown => "PgDn".to_string(),
        other => format!("{other:?}"),
    };
    out.push_str(&code);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::register_builtin_commands;

    fn registry() -> CommandRegistry {
        let mut r = CommandRegistry::new();
        register_builtin_commands(&mut r);
        r
    }

    fn preset_from(toml_src: &str) -> KeymapPreset {
        toml::from_str(toml_src).unwrap()
    }

    fn press(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
    }

    fn ctrl(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
    }

    #[test]
    fn default_map_resolves_single_keys() {
        let mut m = InputMapper::new();
        assert_eq!(
            m.resolve_key(EditorMode::Normal, press('j')),
            KeyResolution::Match("cursor.down")
        );
        assert_eq!(
            m.resolve_key(EditorMode::Normal, ctrl('s')),
            KeyResolution::Match("file.save")
        );
    }

    #[test]
    fn chord_reports_pending_then_matches() {
        let preset = preset_from(
            r#"
[mode.normal]
"g g" = "cursor.home"
"#,
        );
        let mut m = InputMapper::from_preset(&preset, &registry());
        assert_eq!(
            m.resolve_key(EditorMode::Normal, press('g')),
            KeyResolution::Pending
        );
        assert!(m.has_pending());
        assert_eq!(m.pending_display(), "g");
        assert_eq!(
            m.resolve_key(EditorMode::Normal, press('g')),
            KeyResolution::Match("cursor.home")
        );
        assert!(!m.has_pending());
    }

    #[test]
    fn dead_chord_is_discarded_without_firing_anything() {
        let preset = preset_from(
            r#"
[mode.normal]
"g g" = "cursor.home"
"x" = "edit.delete_char"
"#,
        );
        let mut m = InputMapper::from_preset(&preset, &registry());
        assert_eq!(
            m.resolve_key(EditorMode::Normal, press('g')),
            KeyResolution::Pending
        );
        // "g x" is not bound, and must not fall back to the bare "x" binding.
        assert_eq!(
            m.resolve_key(EditorMode::Normal, press('x')),
            KeyResolution::NoMatch
        );
        assert!(!m.has_pending());
        assert_eq!(
            m.resolve_key(EditorMode::Normal, press('x')),
            KeyResolution::Match("edit.delete_char")
        );
    }

    #[test]
    fn exact_match_wins_over_a_longer_binding_sharing_its_prefix() {
        let preset = preset_from(
            r#"
[mode.normal]
"g" = "cursor.home"
"g g" = "cursor.end"
"#,
        );
        let mut m = InputMapper::from_preset(&preset, &registry());
        assert_eq!(
            m.resolve_key(EditorMode::Normal, press('g')),
            KeyResolution::Match("cursor.home")
        );
    }

    #[test]
    fn mode_binding_takes_precedence_over_global() {
        let preset = preset_from(
            r#"
[global]
"ctrl+w" = "tab.close"

[mode.insert]
"ctrl+w" = "edit.backspace"
"#,
        );
        let mut m = InputMapper::from_preset(&preset, &registry());
        assert_eq!(
            m.resolve_key(EditorMode::Insert, ctrl('w')),
            KeyResolution::Match("edit.backspace")
        );
        assert_eq!(
            m.resolve_key(EditorMode::Normal, ctrl('w')),
            KeyResolution::Match("tab.close")
        );
    }

    #[test]
    fn mode_chord_prefix_beats_an_exact_global_binding() {
        let preset = preset_from(
            r#"
[global]
"ctrl+w" = "tab.close"

[mode.normal]
"ctrl+w q" = "app.quit"
"#,
        );
        let mut m = InputMapper::from_preset(&preset, &registry());
        // Normal mode owns a chord starting with Ctrl+W, so the global exact
        // binding must not fire on the first key.
        assert_eq!(
            m.resolve_key(EditorMode::Normal, ctrl('w')),
            KeyResolution::Pending
        );
        assert_eq!(
            m.resolve_key(EditorMode::Normal, press('q')),
            KeyResolution::Match("app.quit")
        );
        // Insert mode has no such chord, so there the global binding wins.
        assert_eq!(
            m.resolve_key(EditorMode::Insert, ctrl('w')),
            KeyResolution::Match("tab.close")
        );
    }

    #[test]
    fn a_global_chord_still_resolves_when_the_mode_has_nothing() {
        let preset = preset_from(
            r#"
[global]
"ctrl+k ctrl+p" = "palette.open"
"#,
        );
        let mut m = InputMapper::from_preset(&preset, &registry());
        assert_eq!(
            m.resolve_key(EditorMode::Normal, ctrl('k')),
            KeyResolution::Pending
        );
        assert_eq!(
            m.resolve_key(EditorMode::Normal, ctrl('p')),
            KeyResolution::Match("palette.open")
        );
    }

    #[test]
    fn global_bindings_do_not_leak_into_overlay_modes() {
        let preset = preset_from(
            r#"
[global]
"ctrl+f" = "search.open"

[mode.search]
"esc" = "search.close"
"#,
        );
        let mut m = InputMapper::from_preset(&preset, &registry());
        assert_eq!(
            m.resolve_key(EditorMode::Search, ctrl('f')),
            KeyResolution::NoMatch
        );
    }

    #[test]
    fn preset_replaces_defaults_entirely() {
        let preset = preset_from(
            r#"
[mode.normal]
"j" = "cursor.down"
"#,
        );
        let mut m = InputMapper::from_preset(&preset, &registry());
        // Present in the built-in map, absent from this preset.
        assert_eq!(
            m.resolve_key(EditorMode::Normal, ctrl('s')),
            KeyResolution::NoMatch
        );
        assert_eq!(
            m.resolve_key(EditorMode::Normal, press('j')),
            KeyResolution::Match("cursor.down")
        );
    }

    #[test]
    fn unknown_command_in_preset_is_skipped() {
        let preset = preset_from(
            r#"
[mode.normal]
"j" = "no.such.command"
"k" = "cursor.up"
"#,
        );
        let mut m = InputMapper::from_preset(&preset, &registry());
        assert_eq!(
            m.resolve_key(EditorMode::Normal, press('j')),
            KeyResolution::NoMatch
        );
        assert_eq!(
            m.resolve_key(EditorMode::Normal, press('k')),
            KeyResolution::Match("cursor.up")
        );
    }

    #[test]
    fn clear_pending_cancels_a_chord() {
        let preset = preset_from(
            r#"
[mode.normal]
"g g" = "cursor.home"
"#,
        );
        let mut m = InputMapper::from_preset(&preset, &registry());
        m.resolve_key(EditorMode::Normal, press('g'));
        assert!(m.clear_pending());
        assert!(!m.clear_pending());
        assert_eq!(
            m.resolve_key(EditorMode::Normal, press('g')),
            KeyResolution::Pending
        );
    }

    #[test]
    fn overrides_apply_on_top_of_a_preset() {
        let preset = preset_from(
            r#"
[mode.normal]
"j" = "cursor.down"
"#,
        );
        let reg = registry();
        let mut m = InputMapper::from_preset(&preset, &reg);
        let overrides: termcode_config::keymap::KeybindingConfig = toml::from_str(
            r#"
[mode.normal]
"j" = "cursor.up"
"d d" = "edit.delete_line"
"#,
        )
        .unwrap();
        m.apply_overrides(&overrides, &reg);
        assert_eq!(
            m.resolve_key(EditorMode::Normal, press('j')),
            KeyResolution::Match("cursor.up")
        );
        assert_eq!(
            m.resolve_key(EditorMode::Normal, press('d')),
            KeyResolution::Pending
        );
        assert_eq!(
            m.resolve_key(EditorMode::Normal, press('d')),
            KeyResolution::Match("edit.delete_line")
        );
    }

    #[test]
    fn binding_for_formats_the_sequence() {
        let preset = preset_from(
            r#"
[global]
"ctrl+shift+p" = "palette.open"

[mode.normal]
"g d" = "goto.definition"
"space f" = "fuzzy.open"
"#,
        );
        let m = InputMapper::from_preset(&preset, &registry());
        assert_eq!(
            m.binding_for(EditorMode::Normal, "palette.open").as_deref(),
            Some("Ctrl+Shift+P")
        );
        assert_eq!(
            m.binding_for(EditorMode::Normal, "goto.definition")
                .as_deref(),
            Some("g d")
        );
        assert_eq!(
            m.binding_for(EditorMode::Normal, "fuzzy.open").as_deref(),
            Some("Space f")
        );
        assert_eq!(m.binding_for(EditorMode::Normal, "file.save"), None);
    }
}
