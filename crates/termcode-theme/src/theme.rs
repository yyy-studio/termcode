use std::collections::HashMap;

use crate::palette::Palette;
use crate::style::{Color, Style};

/// Pane focus indicator style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PaneFocusStyle {
    #[default]
    TitleBar,
    Border,
    AccentLine,
}

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
    pub pane_focus_style: PaneFocusStyle,
    pub pane_active_fg: Color,
    pub pane_active_bg: Color,
    pub pane_inactive_fg: Color,
    pub pane_inactive_bg: Color,
    pub panel_borders: bool,
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
            pane_focus_style: PaneFocusStyle::default(),
            pane_active_fg: bg,
            pane_active_bg: Color::Rgb(97, 175, 239),
            pane_inactive_fg: Color::Rgb(75, 82, 99),
            pane_inactive_bg: Color::Rgb(33, 37, 43),
            panel_borders: false,
        }
    }
}

/// File type icon configuration.
#[derive(Debug, Clone)]
pub struct Icons {
    pub directory_open: String,
    pub directory_closed: String,
    pub file_default: String,
    /// Extension -> icon mapping (e.g., "rs" -> "🦀")
    pub extensions: HashMap<String, String>,
}

impl Default for Icons {
    fn default() -> Self {
        let mut extensions = HashMap::new();
        // Code / text files
        for ext in &[
            "txt",
            "md",
            "markdown",
            "rst",
            "org",
            "log",
            "csv",
            "rs",
            "py",
            "js",
            "ts",
            "tsx",
            "jsx",
            "go",
            "c",
            "cpp",
            "h",
            "hpp",
            "java",
            "rb",
            "php",
            "swift",
            "kt",
            "scala",
            "zig",
            "hs",
            "ml",
            "ex",
            "exs",
            "lua",
            "sh",
            "bash",
            "zsh",
            "fish",
            "ps1",
            "bat",
            "cmd",
            "css",
            "scss",
            "sass",
            "less",
            "html",
            "htm",
            "xml",
            "sql",
            "r",
            "dart",
            "vue",
            "svelte",
            "astro",
            "toml",
            "yaml",
            "yml",
            "json",
            "json5",
            "jsonc",
            "ini",
            "cfg",
            "conf",
            "env",
            "lock",
            "dockerfile",
            "makefile",
            "cmake",
            "nix",
            "tf",
            "hcl",
            "proto",
            "graphql",
            "gql",
            "wasm",
        ] {
            extensions.insert(ext.to_string(), "📝".to_string());
        }
        // Image files
        for ext in &[
            "png", "jpg", "jpeg", "gif", "bmp", "svg", "ico", "webp", "avif", "tiff", "tif", "psd",
            "ai", "eps",
        ] {
            extensions.insert(ext.to_string(), "🖼️".to_string());
        }
        // Audio files
        for ext in &[
            "mp3", "wav", "flac", "aac", "ogg", "wma", "m4a", "opus", "mid", "midi",
        ] {
            extensions.insert(ext.to_string(), "🎵".to_string());
        }

        Self {
            directory_open: "📂".to_string(),
            directory_closed: "📁".to_string(),
            file_default: "📄".to_string(),
            extensions,
        }
    }
}

impl Icons {
    /// Get the icon for a file by its name/extension.
    pub fn file_icon(&self, name: &str) -> &str {
        let ext = name.rsplit('.').next().unwrap_or("");
        let ext_lower = ext.to_ascii_lowercase();
        self.extensions
            .get(&ext_lower)
            .map(|s| s.as_str())
            .unwrap_or(&self.file_default)
    }
}

/// A complete theme with palette, syntax scopes, and UI colors.
#[derive(Debug, Clone)]
pub struct Theme {
    pub name: String,
    pub palette: Palette,
    pub scopes: HashMap<String, Style>,
    pub ui: UiColors,
    pub icons: Icons,
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
            icons: Icons::default(),
        }
    }
}
