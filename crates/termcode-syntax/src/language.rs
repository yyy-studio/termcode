use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use tree_sitter::Language;

/// Language identifier. Lowercase ASCII (e.g., "rust", "python").
pub type LanguageId = Arc<str>;

/// Bridge for tree-sitter-toml which uses the old tree-sitter 0.20 API.
/// The underlying C ABI is identical across tree-sitter versions.
pub(crate) fn toml_language() -> Language {
    // tree_sitter_toml uses tree-sitter 0.20 whose Language is `*const TSLanguage`.
    // tree-sitter 0.24's Language is also `*const TSLanguage` with identical layout.
    unsafe { std::mem::transmute(tree_sitter_toml::language()) }
}

/// Configuration for a supported language.
#[derive(Clone)]
pub struct LanguageConfig {
    pub id: LanguageId,
    pub name: String,
    pub file_extensions: Vec<String>,
    pub highlight_query: String,
    pub grammar: Option<Language>,
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
            grammar: Some(tree_sitter_rust::LANGUAGE.into()),
        });
        reg.register(LanguageConfig {
            id: Arc::from("python"),
            name: "Python".to_string(),
            file_extensions: vec!["py".to_string(), "pyi".to_string()],
            highlight_query: String::new(),
            grammar: Some(tree_sitter_python::LANGUAGE.into()),
        });
        reg.register(LanguageConfig {
            id: Arc::from("javascript"),
            name: "JavaScript".to_string(),
            file_extensions: vec!["js".to_string(), "mjs".to_string(), "cjs".to_string()],
            highlight_query: String::new(),
            grammar: Some(tree_sitter_javascript::LANGUAGE.into()),
        });
        reg.register(LanguageConfig {
            id: Arc::from("typescript"),
            name: "TypeScript".to_string(),
            file_extensions: vec![
                "ts".to_string(),
                "tsx".to_string(),
                "mts".to_string(),
                "cts".to_string(),
            ],
            highlight_query: String::new(),
            grammar: Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
        });
        reg.register(LanguageConfig {
            id: Arc::from("toml"),
            name: "TOML".to_string(),
            file_extensions: vec!["toml".to_string()],
            highlight_query: String::new(),
            grammar: Some(toml_language()),
        });
        reg.register(LanguageConfig {
            id: Arc::from("json"),
            name: "JSON".to_string(),
            file_extensions: vec!["json".to_string(), "jsonp".to_string(), "jsonl".to_string()],
            highlight_query: String::new(),
            grammar: Some(tree_sitter_json::LANGUAGE.into()),
        });
        reg.register(LanguageConfig {
            id: Arc::from("markdown"),
            name: "Markdown".to_string(),
            file_extensions: vec!["md".to_string(), "markdown".to_string()],
            highlight_query: String::new(),
            grammar: Some(tree_sitter_md::LANGUAGE.into()),
        });
        reg.register(LanguageConfig {
            id: Arc::from("c"),
            name: "C".to_string(),
            file_extensions: vec!["c".to_string(), "h".to_string()],
            highlight_query: String::new(),
            grammar: Some(tree_sitter_c::LANGUAGE.into()),
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
            grammar: Some(tree_sitter_cpp::LANGUAGE.into()),
        });
        reg.register(LanguageConfig {
            id: Arc::from("go"),
            name: "Go".to_string(),
            file_extensions: vec!["go".to_string()],
            highlight_query: String::new(),
            grammar: Some(tree_sitter_go::LANGUAGE.into()),
        });
        reg.register(LanguageConfig {
            id: Arc::from("bash"),
            name: "Bash".to_string(),
            file_extensions: vec!["sh".to_string(), "bash".to_string()],
            highlight_query: String::new(),
            grammar: Some(tree_sitter_bash::LANGUAGE.into()),
        });

        reg
    }
}

impl LanguageRegistry {
    /// Load highlight query files from runtime directories.
    /// Scans `runtime_dir/queries/{lang_id}/highlights.scm` for each registered language.
    pub fn load_queries(&mut self, runtime_dir: &Path) {
        let queries_dir = runtime_dir.join("queries");
        for config in self.languages.values_mut() {
            let query_path = queries_dir.join(config.id.as_ref()).join("highlights.scm");
            match std::fs::read_to_string(&query_path) {
                Ok(query) => {
                    Arc::make_mut(config).highlight_query = query;
                }
                Err(_) => {
                    log::debug!(
                        "No highlight query found for {} at {}",
                        config.id,
                        query_path.display()
                    );
                }
            }
        }
    }
}

impl Default for LanguageRegistry {
    fn default() -> Self {
        Self::with_builtins()
    }
}
