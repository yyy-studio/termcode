use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::Deserialize;

/// Parsed keybinding configuration from TOML.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct KeybindingConfig {
    pub global: HashMap<String, String>,
    #[serde(rename = "mode")]
    pub modes: ModeBindings,
    #[serde(flatten)]
    unknown: UnknownSections,
}

impl KeybindingConfig {
    /// Sections of the override file that this loader does not understand.
    pub fn warnings(&self) -> Vec<String> {
        let mut warnings = self.unknown.describe("");
        warnings.extend(self.modes.unknown.describe("mode."));
        warnings.sort();
        warnings
    }
}

/// Descriptive header of a keymap preset file.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct KeymapMeta {
    pub name: String,
    pub description: String,
    /// Mode the editor rests in: `"normal"` (default) for modal keymaps, or
    /// `"insert"` for a keymap with no modal layer, which needs the editor to
    /// be ready to type into from the moment it opens.
    pub initial_mode: Option<String>,
}

impl KeymapMeta {
    /// Whether this keymap wants the editor to rest in Insert mode. Anything
    /// other than `"insert"` (including an unset or misspelled value) keeps the
    /// modal default.
    pub fn starts_in_insert(&self) -> bool {
        self.initial_mode
            .as_deref()
            .is_some_and(|m| m.eq_ignore_ascii_case("insert"))
    }

    /// The configured `initial_mode` when it names no mode a keymap may rest
    /// in. Such a value falls back to `Normal`, which looks like the setting
    /// was ignored, so it is worth reporting.
    fn unusable_initial_mode(&self) -> Option<&str> {
        let value = self.initial_mode.as_deref()?;
        let known = ["normal", "insert"]
            .iter()
            .any(|mode| value.eq_ignore_ascii_case(mode));
        (!known).then_some(value)
    }
}

/// A complete keymap loaded from `runtime/keymaps/<name>.toml`.
///
/// A preset *replaces* the built-in keymap rather than layering on top of it,
/// so every binding a preset wants must be listed in its file. User overrides
/// from `keybindings.toml` are applied afterwards, on top of the preset.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct KeymapPreset {
    pub meta: KeymapMeta,
    pub global: HashMap<String, String>,
    #[serde(rename = "mode")]
    pub modes: ModeBindings,
    #[serde(flatten)]
    unknown: UnknownSections,
}

impl KeymapPreset {
    /// Problems found in a file that still parsed, ready to be shown to the
    /// user.
    ///
    /// Sections this loader does not know are dropped in silence, so a single
    /// misspelled header (`[mode.nromal]`) would otherwise leave a keymap
    /// quietly half-bound with nothing to point at.
    pub fn warnings(&self) -> Vec<String> {
        let mut warnings = self.unknown.describe("");
        warnings.extend(self.modes.unknown.describe("mode."));
        if let Some(mode) = self.meta.unusable_initial_mode() {
            warnings.push(format!(
                "meta.initial_mode = \"{mode}\" is not a mode name (expected \"normal\" or \"insert\")"
            ));
        }
        warnings.sort();
        warnings
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ModeBindings {
    pub normal: HashMap<String, String>,
    pub insert: HashMap<String, String>,
    pub file_explorer: HashMap<String, String>,
    pub search: HashMap<String, String>,
    pub fuzzy_finder: HashMap<String, String>,
    pub command_palette: HashMap<String, String>,
    #[serde(flatten)]
    unknown: UnknownSections,
}

/// Keys of a table that matched no field of the struct they were declared in.
///
/// Serde drops those silently. Capturing them costs one `HashMap` per parsed
/// file and turns a typo from an invisible non-effect into a warning.
#[derive(Debug, Default, Deserialize)]
#[serde(transparent)]
struct UnknownSections(HashMap<String, toml::Value>);

impl UnknownSections {
    /// One message per unknown key, named the way it appears in the file:
    /// `prefix` is the path the section sits under, such as `"mode."`.
    fn describe(&self, prefix: &str) -> Vec<String> {
        self.0
            .keys()
            .map(|key| format!("unknown section [{prefix}{key}]"))
            .collect()
    }
}

/// Load keybinding overrides from a TOML file.
pub fn load_keybindings(path: &Path) -> KeybindingConfig {
    match std::fs::read_to_string(path) {
        Ok(content) => match toml::from_str::<KeybindingConfig>(&content) {
            Ok(config) => {
                for warning in config.warnings() {
                    log::warn!("Keybinding config ({}): {warning}", path.display());
                }
                config
            }
            Err(e) => {
                log::warn!("Keybinding config parse error: {e}");
                KeybindingConfig::default()
            }
        },
        Err(_) => KeybindingConfig::default(),
    }
}

/// Directories searched for keymap presets, highest priority first.
///
/// The user's own `~/.config/termcode/keymaps/` comes first so a file there can
/// replace a *shipped* preset by name. `install.sh` writes the shipped presets
/// into `<config>/runtime/keymaps/`, which `runtime_dirs()` ranks above the
/// config directory itself — without this ordering a user file could only ever
/// introduce a brand-new name.
pub fn keymap_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![crate::default::config_dir().join("keymaps")];
    for dir in crate::default::runtime_dirs() {
        let candidate = dir.join("keymaps");
        if !dirs.contains(&candidate) {
            dirs.push(candidate);
        }
    }
    dirs
}

/// Load a keymap preset by name from the standard directories.
pub fn load_keymap_preset(name: &str) -> Option<KeymapPreset> {
    load_keymap_preset_in(&keymap_dirs(), name)
}

/// Load a keymap preset by name from `dirs`, in order.
///
/// A file that fails to parse is logged and skipped so a broken preset in a
/// high-priority directory cannot mask a working one further down.
pub fn load_keymap_preset_in(dirs: &[PathBuf], name: &str) -> Option<KeymapPreset> {
    // A preset name is a bare file stem. Refusing separators keeps `[keymap]
    // preset` from reaching outside the keymap directories, matching the
    // plugin loader's no-traversal stance.
    if name.is_empty() || name.contains(['/', '\\']) || name.contains("..") {
        log::warn!("Invalid keymap preset name: {name}");
        return None;
    }
    let file_name = format!("{name}.toml");
    for dir in dirs {
        let path = dir.join(&file_name);
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        match toml::from_str::<KeymapPreset>(&content) {
            Ok(preset) => {
                for warning in preset.warnings() {
                    log::warn!("Keymap preset ({}): {warning}", path.display());
                }
                return Some(preset);
            }
            Err(e) => log::warn!("Keymap preset parse error ({}): {e}", path.display()),
        }
    }
    None
}

/// List every keymap preset discoverable in the standard directories.
pub fn list_available_keymaps() -> Vec<String> {
    list_available_keymaps_in(&keymap_dirs())
}

/// List every keymap preset name found in `dirs`. Names are deduplicated, so a
/// user file shadowing a shipped one appears once.
pub fn list_available_keymaps_in(dirs: &[PathBuf]) -> Vec<String> {
    let mut names = Vec::new();
    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("toml") {
                continue;
            }
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                if !names.iter().any(|n| n == stem) {
                    names.push(stem.to_string());
                }
            }
        }
    }
    names.sort();
    names
}

/// Parse a whitespace-separated key sequence such as `"g g"` or
/// `"ctrl+k ctrl+p"` into the chord it represents. A single combo parses to a
/// one-element sequence, so plain bindings keep working unchanged.
///
/// The space key itself must be spelled `space` (as in `"space f"`), since
/// literal spaces separate the steps of a chord.
pub fn parse_key_sequence(s: &str) -> Option<Vec<KeyEvent>> {
    let seq: Vec<KeyEvent> = s
        .split_whitespace()
        .map(parse_key_combo)
        .collect::<Option<_>>()?;
    if seq.is_empty() { None } else { Some(seq) }
}

/// Parse a key combo string like "ctrl+shift+p" into a crossterm KeyEvent.
///
/// Edge case: a bare "+" string splits into `["", ""]` (two empty parts). Since neither
/// part matches a modifier, `key_part` ends up as `Some("")`, which `parse_key_code`
/// returns `None` for -- so `parse_key_combo("+")` returns `None`. A binding like
/// `"ctrl++"` is not supported; to bind the plus key, use `"shift+="` or map the
/// `=` key directly.
pub fn parse_key_combo(s: &str) -> Option<KeyEvent> {
    let parts: Vec<&str> = s.split('+').collect();
    if parts.is_empty() {
        return None;
    }

    let mut modifiers = KeyModifiers::empty();
    let mut key_part = None;

    for part in &parts {
        match part.to_lowercase().as_str() {
            "ctrl" | "control" => modifiers |= KeyModifiers::CONTROL,
            "alt" => modifiers |= KeyModifiers::ALT,
            "shift" => modifiers |= KeyModifiers::SHIFT,
            _ => key_part = Some(*part),
        }
    }

    let key_str = key_part?;
    let code = parse_key_code(key_str)?;

    // For shift+letter, the code should be uppercase
    if modifiers.contains(KeyModifiers::SHIFT) {
        if let KeyCode::Char(c) = code {
            if c.is_ascii_lowercase() {
                return Some(KeyEvent::new(
                    KeyCode::Char(c.to_ascii_uppercase()),
                    modifiers,
                ));
            }
        }
    }

    Some(KeyEvent::new(code, modifiers))
}

fn parse_key_code(s: &str) -> Option<KeyCode> {
    let lower = s.to_lowercase();
    match lower.as_str() {
        "enter" | "return" => Some(KeyCode::Enter),
        "esc" | "escape" => Some(KeyCode::Esc),
        "backspace" => Some(KeyCode::Backspace),
        "delete" | "del" => Some(KeyCode::Delete),
        "tab" => Some(KeyCode::Tab),
        "up" => Some(KeyCode::Up),
        "down" => Some(KeyCode::Down),
        "left" => Some(KeyCode::Left),
        "right" => Some(KeyCode::Right),
        "pageup" => Some(KeyCode::PageUp),
        "pagedown" => Some(KeyCode::PageDown),
        "home" => Some(KeyCode::Home),
        "end" => Some(KeyCode::End),
        "f1" => Some(KeyCode::F(1)),
        "f2" => Some(KeyCode::F(2)),
        "f3" => Some(KeyCode::F(3)),
        "f4" => Some(KeyCode::F(4)),
        "f5" => Some(KeyCode::F(5)),
        "f6" => Some(KeyCode::F(6)),
        "f7" => Some(KeyCode::F(7)),
        "f8" => Some(KeyCode::F(8)),
        "f9" => Some(KeyCode::F(9)),
        "f10" => Some(KeyCode::F(10)),
        "f11" => Some(KeyCode::F(11)),
        "f12" => Some(KeyCode::F(12)),
        "space" => Some(KeyCode::Char(' ')),
        _ => {
            let chars: Vec<char> = s.chars().collect();
            if chars.len() == 1 {
                Some(KeyCode::Char(chars[0].to_ascii_lowercase()))
            } else {
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ctrl_shift_p() {
        let key = parse_key_combo("ctrl+shift+p").unwrap();
        assert_eq!(key.code, KeyCode::Char('P'));
        assert_eq!(key.modifiers, KeyModifiers::CONTROL | KeyModifiers::SHIFT);
    }

    #[test]
    fn parse_alt_left() {
        let key = parse_key_combo("alt+left").unwrap();
        assert_eq!(key.code, KeyCode::Left);
        assert_eq!(key.modifiers, KeyModifiers::ALT);
    }

    #[test]
    fn parse_f12() {
        let key = parse_key_combo("f12").unwrap();
        assert_eq!(key.code, KeyCode::F(12));
        assert_eq!(key.modifiers, KeyModifiers::empty());
    }

    #[test]
    fn parse_enter() {
        let key = parse_key_combo("enter").unwrap();
        assert_eq!(key.code, KeyCode::Enter);
    }

    #[test]
    fn parse_ctrl_s() {
        let key = parse_key_combo("ctrl+s").unwrap();
        assert_eq!(key.code, KeyCode::Char('s'));
        assert_eq!(key.modifiers, KeyModifiers::CONTROL);
    }

    #[test]
    fn parse_invalid_returns_none() {
        assert!(parse_key_combo("").is_none());
        assert!(parse_key_combo("ctrl+").is_none());
    }

    #[test]
    fn parse_simple_char() {
        let key = parse_key_combo("j").unwrap();
        assert_eq!(key.code, KeyCode::Char('j'));
        assert_eq!(key.modifiers, KeyModifiers::empty());
    }

    #[test]
    fn preset_names_cannot_escape_the_keymap_directories() {
        let dirs = vec![std::env::temp_dir()];
        for bad in ["../secrets", "a/b", "..", ""] {
            assert!(
                load_keymap_preset_in(&dirs, bad).is_none(),
                "{bad:?} should be rejected"
            );
        }
    }

    #[test]
    fn parse_sequence_single_combo() {
        let seq = parse_key_sequence("ctrl+s").unwrap();
        assert_eq!(seq.len(), 1);
        assert_eq!(seq[0].code, KeyCode::Char('s'));
    }

    #[test]
    fn parse_sequence_chord() {
        let seq = parse_key_sequence("g g").unwrap();
        assert_eq!(seq.len(), 2);
        assert!(seq.iter().all(|k| k.code == KeyCode::Char('g')));
    }

    #[test]
    fn parse_sequence_mixed_modifiers() {
        let seq = parse_key_sequence("ctrl+k ctrl+p").unwrap();
        assert_eq!(seq.len(), 2);
        assert_eq!(seq[0].modifiers, KeyModifiers::CONTROL);
        assert_eq!(seq[1].code, KeyCode::Char('p'));
    }

    #[test]
    fn parse_sequence_space_leader() {
        let seq = parse_key_sequence("space f").unwrap();
        assert_eq!(seq.len(), 2);
        assert_eq!(seq[0].code, KeyCode::Char(' '));
        assert_eq!(seq[1].code, KeyCode::Char('f'));
    }

    #[test]
    fn parse_sequence_rejects_empty_and_invalid() {
        assert!(parse_key_sequence("").is_none());
        assert!(parse_key_sequence("   ").is_none());
        assert!(parse_key_sequence("g nosuchkey").is_none());
    }

    #[test]
    fn preset_parses_meta_and_bindings() {
        let toml_src = r#"
[meta]
name = "vim"
description = "Vim-style modal keymap"

[global]
"ctrl+s" = "file.save"

[mode.normal]
"g g" = "cursor.home"
"d d" = "edit.delete_line"
"#;
        let preset: KeymapPreset = toml::from_str(toml_src).unwrap();
        assert_eq!(preset.meta.name, "vim");
        assert_eq!(preset.global.get("ctrl+s").unwrap(), "file.save");
        assert_eq!(preset.modes.normal.get("g g").unwrap(), "cursor.home");
        assert!(preset.warnings().is_empty(), "{:?}", preset.warnings());
    }

    #[test]
    fn misspelled_sections_are_reported() {
        let preset: KeymapPreset = toml::from_str(
            r#"
[mode.nromal]
"j" = "cursor.down"

[globals]
"ctrl+s" = "file.save"
"#,
        )
        .unwrap();
        assert_eq!(
            preset.warnings(),
            vec![
                "unknown section [globals]".to_string(),
                "unknown section [mode.nromal]".to_string(),
            ]
        );
    }

    #[test]
    fn an_initial_mode_no_keymap_can_rest_in_is_reported() {
        let preset: KeymapPreset = toml::from_str(
            r#"
[meta]
initial_mode = "insrt"
"#,
        )
        .unwrap();
        assert!(!preset.meta.starts_in_insert());
        assert_eq!(preset.warnings().len(), 1);
        assert!(preset.warnings()[0].contains("initial_mode"));
    }

    #[test]
    fn a_spelled_out_initial_mode_is_not_a_warning() {
        for mode in ["normal", "insert", "Insert"] {
            let preset: KeymapPreset =
                toml::from_str(&format!("[meta]\ninitial_mode = \"{mode}\"\n")).unwrap();
            assert!(preset.warnings().is_empty(), "{mode} should be accepted");
        }
    }

    #[test]
    fn misspelled_override_sections_are_reported() {
        let config: KeybindingConfig = toml::from_str(
            r#"
[mode.isnert]
"esc" = "mode.normal"
"#,
        )
        .unwrap();
        assert_eq!(
            config.warnings(),
            vec!["unknown section [mode.isnert]".to_string()]
        );
    }
}
