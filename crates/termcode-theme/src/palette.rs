use std::collections::HashMap;

use crate::style::Color;

/// Named color palette. Colors can reference palette names or hex values.
#[derive(Debug, Clone, Default)]
pub struct Palette {
    colors: HashMap<String, Color>,
}

impl Palette {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, name: String, color: Color) {
        self.colors.insert(name, color);
    }

    /// Resolve a color string: either a palette name or a hex "#rrggbb".
    pub fn resolve(&self, value: &str) -> Option<Color> {
        if value.starts_with('#') {
            Color::from_hex(value)
        } else {
            self.colors.get(value).copied()
        }
    }
}
