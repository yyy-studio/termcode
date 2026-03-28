use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

/// Language identifier. Lowercase ASCII (e.g., "rust", "python").
pub type LanguageId = Arc<str>;

/// Configuration for a supported language.
pub struct LanguageConfig {
    pub id: LanguageId,
    pub name: String,
    pub file_extensions: Vec<String>,
    pub highlight_query: String,
}

/// Global registry of available languages.
pub struct LanguageRegistry {
    languages: HashMap<LanguageId, Arc<LanguageConfig>>,
    extension_map: HashMap<String, LanguageId>,
}

impl LanguageRegistry {
    pub fn new() -> Self {
        Self {
            languages: HashMap::new(),
            extension_map: HashMap::new(),
        }
    }

    /// Register a language configuration.
    pub fn register(&mut self, config: LanguageConfig) {
        let id = config.id.clone();
        for ext in &config.file_extensions {
            self.extension_map.insert(ext.clone(), id.clone());
        }
        self.languages.insert(id, Arc::new(config));
    }

    /// Detect language from file extension.
    pub fn detect_language(&self, path: &Path) -> Option<LanguageId> {
        let ext = path.extension()?.to_str()?;
        self.extension_map.get(ext).cloned()
    }

    pub fn get(&self, id: &str) -> Option<&Arc<LanguageConfig>> {
        self.languages.get(id)
    }

    /// Build a registry with built-in language definitions.
    pub fn with_builtins() -> Self {
        let mut reg = Self::new();

        reg.register(LanguageConfig {
            id: Arc::from("rust"),
            name: "Rust".to_string(),
            file_extensions: vec!["rs".to_string()],
            highlight_query: String::new(),
        });
        reg.register(LanguageConfig {
            id: Arc::from("python"),
            name: "Python".to_string(),
            file_extensions: vec!["py".to_string(), "pyi".to_string()],
            highlight_query: String::new(),
        });
        reg.register(LanguageConfig {
            id: Arc::from("javascript"),
            name: "JavaScript".to_string(),
            file_extensions: vec!["js".to_string(), "mjs".to_string(), "cjs".to_string()],
            highlight_query: String::new(),
        });
        reg.register(LanguageConfig {
            id: Arc::from("typescript"),
            name: "TypeScript".to_string(),
            file_extensions: vec!["ts".to_string(), "tsx".to_string()],
            highlight_query: String::new(),
        });
        reg.register(LanguageConfig {
            id: Arc::from("toml"),
            name: "TOML".to_string(),
            file_extensions: vec!["toml".to_string()],
            highlight_query: String::new(),
        });
        reg.register(LanguageConfig {
            id: Arc::from("json"),
            name: "JSON".to_string(),
            file_extensions: vec!["json".to_string()],
            highlight_query: String::new(),
        });
        reg.register(LanguageConfig {
            id: Arc::from("markdown"),
            name: "Markdown".to_string(),
            file_extensions: vec!["md".to_string(), "markdown".to_string()],
            highlight_query: String::new(),
        });
        reg.register(LanguageConfig {
            id: Arc::from("c"),
            name: "C".to_string(),
            file_extensions: vec!["c".to_string(), "h".to_string()],
            highlight_query: String::new(),
        });
        reg.register(LanguageConfig {
            id: Arc::from("cpp"),
            name: "C++".to_string(),
            file_extensions: vec![
                "cpp".to_string(),
                "cc".to_string(),
                "cxx".to_string(),
                "hpp".to_string(),
            ],
            highlight_query: String::new(),
        });
        reg.register(LanguageConfig {
            id: Arc::from("go"),
            name: "Go".to_string(),
            file_extensions: vec!["go".to_string()],
            highlight_query: String::new(),
        });

        reg
    }
}

impl Default for LanguageRegistry {
    fn default() -> Self {
        Self::with_builtins()
    }
}
