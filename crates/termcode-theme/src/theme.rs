use std::collections::HashMap;

use crate::palette::Palette;
use crate::style::{Color, Style};

/// UI color slots for non-syntax elements.
#[derive(Debug, Clone)]
pub struct UiColors {
    pub background: Color,
    pub foreground: Color,
    pub cursor: Color,
    pub selection: Color,
    pub cursor_line_bg: Color,
    pub line_number: Color,
    pub line_number_active: Color,
    pub status_bar_bg: Color,
    pub status_bar_fg: Color,
    pub tab_active_bg: Color,
    pub tab_inactive_bg: Color,
    pub sidebar_bg: Color,
    pub sidebar_fg: Color,
    pub border: Color,
    pub error: Color,
    pub warning: Color,
    pub info: Color,
    pub hint: Color,
    pub search_match: Color,
    pub search_match_active: Color,
}

impl Default for UiColors {
    fn default() -> Self {
        let bg = Color::Rgb(40, 44, 52);
        let fg = Color::Rgb(171, 178, 191);
        Self {
            background: bg,
            foreground: fg,
            cursor: Color::Rgb(82, 139, 255),
            selection: Color::Rgb(62, 68, 81),
            cursor_line_bg: Color::Rgb(44, 48, 56),
            line_number: Color::Rgb(75, 82, 99),
            line_number_active: fg,
            status_bar_bg: Color::Rgb(33, 37, 43),
            status_bar_fg: fg,
            tab_active_bg: bg,
            tab_inactive_bg: Color::Rgb(33, 37, 43),
            sidebar_bg: Color::Rgb(33, 37, 43),
            sidebar_fg: fg,
            border: Color::Rgb(24, 26, 31),
            error: Color::Rgb(224, 108, 117),
            warning: Color::Rgb(229, 192, 123),
            info: Color::Rgb(97, 175, 239),
            hint: Color::Rgb(86, 182, 194),
            search_match: Color::Rgb(229, 192, 123),
            search_match_active: Color::Rgb(209, 154, 102),
        }
    }
}

/// A complete theme with palette, syntax scopes, and UI colors.
#[derive(Debug, Clone)]
pub struct Theme {
    pub name: String,
    pub palette: Palette,
    pub scopes: HashMap<String, Style>,
    pub ui: UiColors,
}

impl Theme {
    /// Resolve a highlight scope name to a Style.
    /// Falls back through dot-separated scopes: "function.name" -> "function" -> default.
    pub fn resolve(&self, scope: &str) -> Style {
        if let Some(style) = self.scopes.get(scope) {
            return *style;
        }
        // Fallback: try parent scope
        if let Some(dot_pos) = scope.rfind('.') {
            return self.resolve(&scope[..dot_pos]);
        }
        // Default: foreground color only
        Style {
            fg: Some(self.ui.foreground),
            ..Default::default()
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            name: "default".to_string(),
            palette: Palette::default(),
            scopes: HashMap::new(),
            ui: UiColors::default(),
        }
    }
}
