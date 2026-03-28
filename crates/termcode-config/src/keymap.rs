use std::collections::HashMap;
use std::path::Path;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::Deserialize;

/// Parsed keybinding configuration from TOML.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct KeybindingConfig {
    pub global: HashMap<String, String>,
    #[serde(rename = "mode")]
    pub modes: ModeBindings,
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
}

/// Load keybinding overrides from a TOML file.
pub fn load_keybindings(path: &Path) -> KeybindingConfig {
    match std::fs::read_to_string(path) {
        Ok(content) => match toml::from_str(&content) {
            Ok(config) => config,
            Err(e) => {
                log::warn!("Keybinding config parse error: {e}");
                KeybindingConfig::default()
            }
        },
        Err(_) => KeybindingConfig::default(),
    }
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
}
