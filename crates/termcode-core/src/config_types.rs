use serde::Deserialize;

/// Editor configuration options.
/// Defined in core so termcode-view can use it without depending on termcode-config.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct EditorConfig {
    pub tab_size: usize,
    pub insert_spaces: bool,
    pub auto_save: bool,
    pub auto_save_delay_ms: u64,
    pub word_wrap: bool,
    pub line_numbers: LineNumberStyle,
    pub scroll_off: usize,
    pub mouse_enabled: bool,
}

impl Default for EditorConfig {
    fn default() -> Self {
        Self {
            tab_size: 4,
            insert_spaces: true,
            auto_save: false,
            auto_save_delay_ms: 1000,
            word_wrap: false,
            line_numbers: LineNumberStyle::Absolute,
            scroll_off: 5,
            mouse_enabled: true,
        }
    }
}

/// Line number display style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LineNumberStyle {
    Absolute,
    Relative,
    RelativeAbsolute,
    None,
}

/// File tree display style.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileTreeStyle {
    /// Emoji icons only (📁 📂 📄)
    Emoji,
    /// Tree lines only (├── └── │)
    Tree,
    /// Emoji icons + tree lines combined
    #[default]
    EmojiTree,
}
