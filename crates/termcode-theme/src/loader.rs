use std::collections::HashMap;
use std::path::Path;

use thiserror::Error;

use crate::palette::Palette;
use crate::style::{Color, Style, StyleDef};
use crate::theme::{PaneFocusStyle, Theme, UiColors};

#[derive(Error, Debug)]
pub enum ThemeError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("TOML parse error: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("Invalid color: {0}")]
    InvalidColor(String),
}

/// Raw TOML theme structure for deserialization.
#[derive(Debug, serde::Deserialize)]
struct ThemeFile {
    #[serde(default)]
    meta: MetaDef,
    #[serde(default)]
    palette: HashMap<String, String>,
    #[serde(default)]
    ui: UiDef,
    #[serde(default)]
    scopes: HashMap<String, StyleDef>,
}

#[derive(Debug, serde::Deserialize, Default)]
struct MetaDef {
    #[serde(default = "default_theme_name")]
    name: String,
}

fn default_theme_name() -> String {
    "unnamed".to_string()
}

#[derive(Debug, serde::Deserialize, Default)]
struct UiDef {
    background: Option<String>,
    foreground: Option<String>,
    cursor: Option<String>,
    selection: Option<String>,
    cursor_line_bg: Option<String>,
    line_number: Option<String>,
    line_number_active: Option<String>,
    status_bar_bg: Option<String>,
    status_bar_fg: Option<String>,
    tab_active_bg: Option<String>,
    tab_inactive_bg: Option<String>,
    sidebar_bg: Option<String>,
    sidebar_fg: Option<String>,
    border: Option<String>,
    error: Option<String>,
    warning: Option<String>,
    info: Option<String>,
    hint: Option<String>,
    search_match: Option<String>,
    search_match_active: Option<String>,
    pane_focus_style: Option<String>,
    pane_active_fg: Option<String>,
    pane_active_bg: Option<String>,
    pane_inactive_fg: Option<String>,
    pane_inactive_bg: Option<String>,
    panel_borders: Option<bool>,
}

/// Load a theme from a TOML file.
pub fn load_theme(path: &Path) -> Result<Theme, ThemeError> {
    let content = std::fs::read_to_string(path)?;
    parse_theme(&content)
}

/// Parse a theme from a TOML string.
pub fn parse_theme(toml_str: &str) -> Result<Theme, ThemeError> {
    let file: ThemeFile = toml::from_str(toml_str)?;

    // Build palette
    let mut palette = Palette::new();
    for (name, hex) in &file.palette {
        if let Some(color) = Color::from_hex(hex) {
            palette.insert(name.clone(), color);
        }
    }

    // Resolve UI colors
    let defaults = UiColors::default();
    let resolve = |val: &Option<String>, fallback: Color| -> Color {
        val.as_ref()
            .and_then(|v| palette.resolve(v))
            .unwrap_or(fallback)
    };

    let pane_focus_style = file
        .ui
        .pane_focus_style
        .as_deref()
        .map(|s| match s.to_lowercase().as_str() {
            "title_bar" => PaneFocusStyle::TitleBar,
            "border" => PaneFocusStyle::Border,
            "accent_line" => PaneFocusStyle::AccentLine,
            _ => PaneFocusStyle::TitleBar,
        })
        .unwrap_or_default();

    let ui = UiColors {
        background: resolve(&file.ui.background, defaults.background),
        foreground: resolve(&file.ui.foreground, defaults.foreground),
        cursor: resolve(&file.ui.cursor, defaults.cursor),
        selection: resolve(&file.ui.selection, defaults.selection),
        cursor_line_bg: resolve(&file.ui.cursor_line_bg, defaults.cursor_line_bg),
        line_number: resolve(&file.ui.line_number, defaults.line_number),
        line_number_active: resolve(&file.ui.line_number_active, defaults.line_number_active),
        status_bar_bg: resolve(&file.ui.status_bar_bg, defaults.status_bar_bg),
        status_bar_fg: resolve(&file.ui.status_bar_fg, defaults.status_bar_fg),
        tab_active_bg: resolve(&file.ui.tab_active_bg, defaults.tab_active_bg),
        tab_inactive_bg: resolve(&file.ui.tab_inactive_bg, defaults.tab_inactive_bg),
        sidebar_bg: resolve(&file.ui.sidebar_bg, defaults.sidebar_bg),
        sidebar_fg: resolve(&file.ui.sidebar_fg, defaults.sidebar_fg),
        border: resolve(&file.ui.border, defaults.border),
        error: resolve(&file.ui.error, defaults.error),
        warning: resolve(&file.ui.warning, defaults.warning),
        info: resolve(&file.ui.info, defaults.info),
        hint: resolve(&file.ui.hint, defaults.hint),
        search_match: resolve(&file.ui.search_match, defaults.search_match),
        search_match_active: resolve(&file.ui.search_match_active, defaults.search_match_active),
        pane_focus_style,
        pane_active_fg: resolve(&file.ui.pane_active_fg, defaults.pane_active_fg),
        pane_active_bg: resolve(&file.ui.pane_active_bg, defaults.pane_active_bg),
        pane_inactive_fg: resolve(&file.ui.pane_inactive_fg, defaults.pane_inactive_fg),
        pane_inactive_bg: resolve(&file.ui.pane_inactive_bg, defaults.pane_inactive_bg),
        panel_borders: file.ui.panel_borders.unwrap_or(false),
    };

    // Resolve syntax scopes
    let mut scopes = HashMap::new();
    for (scope_name, style_def) in &file.scopes {
        let style = Style {
            fg: style_def.fg.as_ref().and_then(|v| palette.resolve(v)),
            bg: style_def.bg.as_ref().and_then(|v| palette.resolve(v)),
            bold: style_def.bold,
            italic: style_def.italic,
            underline: style_def.underline,
            strikethrough: style_def.strikethrough,
        };
        scopes.insert(scope_name.clone(), style);
    }

    Ok(Theme {
        name: file.meta.name,
        palette,
        scopes,
        ui,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_one_dark_theme() {
        let toml = include_str!("../../../runtime/themes/one-dark.toml");
        let theme = parse_theme(toml).expect("one-dark should parse");
        assert_eq!(theme.name, "One Dark");
        assert!(!theme.scopes.is_empty());
    }

    #[test]
    fn parse_gruvbox_dark_theme() {
        let toml = include_str!("../../../runtime/themes/gruvbox-dark.toml");
        let theme = parse_theme(toml).expect("gruvbox-dark should parse");
        assert_eq!(theme.name, "Gruvbox Dark");
        assert!(!theme.scopes.is_empty());
    }

    #[test]
    fn parse_catppuccin_mocha_theme() {
        let toml = include_str!("../../../runtime/themes/catppuccin-mocha.toml");
        let theme = parse_theme(toml).expect("catppuccin-mocha should parse");
        assert_eq!(theme.name, "Catppuccin Mocha");
        assert!(!theme.scopes.is_empty());
    }

    #[test]
    fn parse_explicit_pane_focus_style() {
        let toml = r##"
[meta]
name = "test"
[palette]
red = "#ff0000"
blue = "#0000ff"
[ui]
pane_focus_style = "border"
pane_active_fg = "red"
pane_active_bg = "blue"
"##;
        let theme = parse_theme(toml).expect("should parse");
        assert_eq!(theme.ui.pane_focus_style, PaneFocusStyle::Border);
        assert_eq!(theme.ui.pane_active_fg, Color::Rgb(255, 0, 0));
        assert_eq!(theme.ui.pane_active_bg, Color::Rgb(0, 0, 255));
    }

    #[test]
    fn parse_missing_pane_fields_uses_defaults() {
        let toml = "[meta]\nname = \"test\"\n";
        let theme = parse_theme(toml).expect("should parse");
        let defaults = UiColors::default();
        assert_eq!(theme.ui.pane_focus_style, PaneFocusStyle::TitleBar);
        assert_eq!(theme.ui.pane_active_fg, defaults.pane_active_fg);
        assert_eq!(theme.ui.pane_active_bg, defaults.pane_active_bg);
        assert_eq!(theme.ui.pane_inactive_fg, defaults.pane_inactive_fg);
        assert_eq!(theme.ui.pane_inactive_bg, defaults.pane_inactive_bg);
    }

    #[test]
    fn parse_unknown_pane_focus_style_fallback() {
        let toml = r##"
[meta]
name = "test"
[ui]
pane_focus_style = "unknown_value"
"##;
        let theme = parse_theme(toml).expect("should parse");
        assert_eq!(theme.ui.pane_focus_style, PaneFocusStyle::TitleBar);
    }

    #[test]
    fn parse_pane_focus_style_case_insensitive() {
        for (input, expected) in [
            ("Title_Bar", PaneFocusStyle::TitleBar),
            ("BORDER", PaneFocusStyle::Border),
            ("Accent_Line", PaneFocusStyle::AccentLine),
            ("title_bar", PaneFocusStyle::TitleBar),
        ] {
            let toml = format!("[meta]\nname = \"test\"\n[ui]\npane_focus_style = \"{input}\"\n");
            let theme = parse_theme(&toml).expect("should parse");
            assert_eq!(theme.ui.pane_focus_style, expected, "input: {input}");
        }
    }
}
