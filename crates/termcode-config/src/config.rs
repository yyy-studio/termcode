use std::path::Path;

use serde::Deserialize;
use thiserror::Error;

use termcode_core::config_types::{EditorConfig, FileTreeStyle};

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("TOML parse error: {0}")]
    Toml(#[from] toml::de::Error),
}

/// Configuration for a single LSP server.
#[derive(Debug, Clone, Deserialize)]
pub struct LspServerConfig {
    pub language: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
}

/// Top-level application configuration.
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub theme: String,
    pub editor: EditorConfig,
    pub ui: UiConfig,
    #[serde(default)]
    pub lsp: Vec<LspServerConfig>,
}

/// UI-related configuration.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct UiConfig {
    pub sidebar_width: u16,
    pub sidebar_visible: bool,
    pub show_minimap: bool,
    pub show_tab_bar: bool,
    pub show_top_bar: bool,
    pub file_tree_style: FileTreeStyle,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            sidebar_width: 30,
            sidebar_visible: true,
            show_minimap: false,
            show_tab_bar: true,
            show_top_bar: true,
            file_tree_style: FileTreeStyle::default(),
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            theme: "one-dark".to_string(),
            editor: EditorConfig::default(),
            ui: UiConfig::default(),
            lsp: Vec::new(),
        }
    }
}

impl AppConfig {
    /// Load configuration from a TOML file, falling back to defaults on error.
    pub fn load(path: &Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(content) => match toml::from_str(&content) {
                Ok(config) => config,
                Err(e) => {
                    log::warn!("Config parse error: {e}, using defaults");
                    Self::default()
                }
            },
            Err(_) => Self::default(),
        }
    }
}
