use std::path::PathBuf;

/// Validate a plugin or command name: must match `[a-z0-9_-]+`.
pub fn is_valid_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
}

/// Status of a loaded plugin.
#[derive(Debug, Clone)]
pub enum PluginStatus {
    Loaded,
    Failed(String),
    Disabled,
}

/// Metadata and runtime status of a plugin.
#[derive(Debug, Clone)]
pub struct PluginInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub path: PathBuf,
    pub status: PluginStatus,
}

/// Actions deferred until after the Lua scope ends, processed by App.
#[derive(Debug, Clone)]
pub enum DeferredAction {
    OpenFile(PathBuf),
    ExecuteCommand(String),
}

/// Context data passed to hook callbacks.
#[derive(Debug, Clone)]
pub struct HookContext {
    pub path: Option<String>,
    pub filename: Option<String>,
    pub language: Option<String>,
    pub old_mode: Option<String>,
    pub new_mode: Option<String>,
    pub line: Option<usize>,
    pub col: Option<usize>,
}

impl HookContext {
    pub fn empty() -> Self {
        Self {
            path: None,
            filename: None,
            language: None,
            old_mode: None,
            new_mode: None,
            line: None,
            col: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_names() {
        assert!(is_valid_name("my-plugin"));
        assert!(is_valid_name("my_plugin"));
        assert!(is_valid_name("plugin123"));
        assert!(is_valid_name("a"));
    }

    #[test]
    fn test_invalid_names() {
        assert!(!is_valid_name("my.plugin"));
        assert!(!is_valid_name("My-Plugin"));
        assert!(!is_valid_name("plugin name"));
        assert!(!is_valid_name(""));
        assert!(!is_valid_name("plugin/name"));
    }
}
