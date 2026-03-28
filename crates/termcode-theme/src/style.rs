use serde::Deserialize;

/// A color value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    Rgb(u8, u8, u8),
    Indexed(u8),
    Reset,
}

impl Color {
    /// Parse a hex color string like "#rrggbb".
    pub fn from_hex(s: &str) -> Option<Self> {
        let s = s.strip_prefix('#')?;
        if s.len() != 6 {
            return None;
        }
        let r = u8::from_str_radix(&s[0..2], 16).ok()?;
        let g = u8::from_str_radix(&s[2..4], 16).ok()?;
        let b = u8::from_str_radix(&s[4..6], 16).ok()?;
        Some(Color::Rgb(r, g, b))
    }

    /// Convert to ratatui Color.
    pub fn to_ratatui(self) -> ratatui::style::Color {
        match self {
            Color::Rgb(r, g, b) => ratatui::style::Color::Rgb(r, g, b),
            Color::Indexed(i) => ratatui::style::Color::Indexed(i),
            Color::Reset => ratatui::style::Color::Reset,
        }
    }
}

/// Text style with optional foreground/background colors and modifiers.
#[derive(Debug, Clone, Copy, Default)]
pub struct Style {
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
}

impl Style {
    /// Convert to ratatui Style.
    pub fn to_ratatui(self) -> ratatui::style::Style {
        let mut s = ratatui::style::Style::default();
        if let Some(fg) = self.fg {
            s = s.fg(fg.to_ratatui());
        }
        if let Some(bg) = self.bg {
            s = s.bg(bg.to_ratatui());
        }
        if self.bold {
            s = s.add_modifier(ratatui::style::Modifier::BOLD);
        }
        if self.italic {
            s = s.add_modifier(ratatui::style::Modifier::ITALIC);
        }
        if self.underline {
            s = s.add_modifier(ratatui::style::Modifier::UNDERLINED);
        }
        if self.strikethrough {
            s = s.add_modifier(ratatui::style::Modifier::CROSSED_OUT);
        }
        s
    }
}

/// Deserializable style definition from TOML theme files.
#[derive(Debug, Deserialize, Default)]
pub struct StyleDef {
    pub fg: Option<String>,
    pub bg: Option<String>,
    #[serde(default)]
    pub bold: bool,
    #[serde(default)]
    pub italic: bool,
    #[serde(default)]
    pub underline: bool,
    #[serde(default)]
    pub strikethrough: bool,
}
