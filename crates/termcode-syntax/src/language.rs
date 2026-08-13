use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use tree_sitter::Language;
use tree_sitter_highlight::HighlightConfiguration;

use crate::highlighter::build_highlight_config;

/// Language identifier. Lowercase ASCII (e.g., "rust", "python").
pub type LanguageId = Arc<str>;

/// Highlight configurations of languages that can be injected into another
/// language, keyed by the name used in `injections.scm` (e.g., "javascript").
pub type InjectionConfigs = Arc<HashMap<String, Arc<HighlightConfiguration>>>;

/// Configuration for a supported language.
#[derive(Clone)]
pub struct LanguageConfig {
    pub id: LanguageId,
    pub name: String,
    pub file_extensions: Vec<String>,
    pub highlight_query: String,
    /// Injection query (`injections.scm`). Empty when the language has none.
    pub injection_query: String,
    /// Highlight configs for languages this one injects. Empty unless
    /// `injection_query` is set; resolved by `LanguageRegistry::load_queries`.
    pub injections: InjectionConfigs,
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
            injection_query: String::new(),
            injections: InjectionConfigs::default(),
            grammar: Some(tree_sitter_rust::LANGUAGE.into()),
        });
        reg.register(LanguageConfig {
            id: Arc::from("python"),
            name: "Python".to_string(),
            file_extensions: vec!["py".to_string(), "pyi".to_string()],
            highlight_query: String::new(),
            injection_query: String::new(),
            injections: InjectionConfigs::default(),
            grammar: Some(tree_sitter_python::LANGUAGE.into()),
        });
        reg.register(LanguageConfig {
            id: Arc::from("javascript"),
            name: "JavaScript".to_string(),
            file_extensions: vec!["js".to_string(), "mjs".to_string(), "cjs".to_string()],
            highlight_query: String::new(),
            injection_query: String::new(),
            injections: InjectionConfigs::default(),
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
            injection_query: String::new(),
            injections: InjectionConfigs::default(),
            grammar: Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
        });
        reg.register(LanguageConfig {
            id: Arc::from("toml"),
            name: "TOML".to_string(),
            file_extensions: vec!["toml".to_string()],
            highlight_query: String::new(),
            injection_query: String::new(),
            injections: InjectionConfigs::default(),
            grammar: Some(tree_sitter_toml_ng::LANGUAGE.into()),
        });
        reg.register(LanguageConfig {
            id: Arc::from("json"),
            name: "JSON".to_string(),
            file_extensions: vec!["json".to_string(), "jsonp".to_string(), "jsonl".to_string()],
            highlight_query: String::new(),
            injection_query: String::new(),
            injections: InjectionConfigs::default(),
            grammar: Some(tree_sitter_json::LANGUAGE.into()),
        });
        reg.register(LanguageConfig {
            id: Arc::from("markdown"),
            name: "Markdown".to_string(),
            file_extensions: vec!["md".to_string(), "markdown".to_string()],
            highlight_query: String::new(),
            injection_query: String::new(),
            injections: InjectionConfigs::default(),
            grammar: Some(tree_sitter_md::LANGUAGE.into()),
        });
        reg.register(LanguageConfig {
            id: Arc::from("c"),
            name: "C".to_string(),
            file_extensions: vec!["c".to_string(), "h".to_string()],
            highlight_query: String::new(),
            injection_query: String::new(),
            injections: InjectionConfigs::default(),
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
            injection_query: String::new(),
            injections: InjectionConfigs::default(),
            grammar: Some(tree_sitter_cpp::LANGUAGE.into()),
        });
        reg.register(LanguageConfig {
            id: Arc::from("go"),
            name: "Go".to_string(),
            file_extensions: vec!["go".to_string()],
            highlight_query: String::new(),
            injection_query: String::new(),
            injections: InjectionConfigs::default(),
            grammar: Some(tree_sitter_go::LANGUAGE.into()),
        });
        reg.register(LanguageConfig {
            id: Arc::from("bash"),
            name: "Bash".to_string(),
            file_extensions: vec!["sh".to_string(), "bash".to_string()],
            highlight_query: String::new(),
            injection_query: String::new(),
            injections: InjectionConfigs::default(),
            grammar: Some(tree_sitter_bash::LANGUAGE.into()),
        });
        reg.register(LanguageConfig {
            id: Arc::from("html"),
            name: "HTML".to_string(),
            file_extensions: vec!["html".to_string(), "htm".to_string(), "xhtml".to_string()],
            highlight_query: String::new(),
            injection_query: String::new(),
            injections: InjectionConfigs::default(),
            grammar: Some(tree_sitter_html::LANGUAGE.into()),
        });
        reg.register(LanguageConfig {
            id: Arc::from("css"),
            name: "CSS".to_string(),
            file_extensions: vec!["css".to_string()],
            highlight_query: String::new(),
            injection_query: String::new(),
            injections: InjectionConfigs::default(),
            grammar: Some(tree_sitter_css::LANGUAGE.into()),
        });

        reg
    }
}

impl LanguageRegistry {
    /// Load highlight and injection query files from runtime directories.
    /// Scans `runtime_dir/queries/{lang_id}/{highlights,injections}.scm` for each
    /// registered language, then resolves injected-language highlight configs.
    pub fn load_queries(&mut self, runtime_dir: &Path) {
        let queries_dir = runtime_dir.join("queries");
        let mut loaded_any = false;
        for config in self.languages.values_mut() {
            let lang_dir = queries_dir.join(config.id.as_ref());

            let query_path = lang_dir.join("highlights.scm");
            match std::fs::read_to_string(&query_path) {
                Ok(query) => {
                    Arc::make_mut(config).highlight_query = query;
                    loaded_any = true;
                }
                Err(_) => {
                    log::debug!(
                        "No highlight query found for {} at {}",
                        config.id,
                        query_path.display()
                    );
                }
            }

            let injection_path = lang_dir.join("injections.scm");
            if let Ok(query) = std::fs::read_to_string(&injection_path) {
                Arc::make_mut(config).injection_query = query;
                loaded_any = true;
            }
        }

        // Only rebuild injection configs when this directory actually contributed
        // queries, so scanning several runtime dirs does not recompile them each time.
        if loaded_any {
            self.resolve_injections();
        }
    }

    /// Build highlight configs for every highlightable language and hand them to
    /// the languages that declare injections, so nested code (e.g. JavaScript
    /// inside an HTML `<script>`) can be highlighted with its own grammar.
    fn resolve_injections(&mut self) {
        let injecting: Vec<LanguageId> = self
            .languages
            .iter()
            .filter(|(_, config)| !config.injection_query.is_empty())
            .map(|(id, _)| id.clone())
            .collect();
        if injecting.is_empty() {
            return;
        }

        let mut configs: HashMap<String, Arc<HighlightConfiguration>> = HashMap::new();
        for (id, config) in &self.languages {
            if let Some(hl_config) = build_highlight_config(config) {
                configs.insert(id.to_string(), Arc::new(hl_config));
            }
        }
        let configs: InjectionConfigs = Arc::new(configs);

        for id in injecting {
            if let Some(config) = self.languages.get_mut(&id) {
                Arc::make_mut(config).injections = configs.clone();
            }
        }
    }
}

impl Default for LanguageRegistry {
    fn default() -> Self {
        Self::with_builtins()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runtime_dir() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../runtime")
    }

    #[test]
    fn test_html_detected_by_extension() {
        let reg = LanguageRegistry::with_builtins();
        assert_eq!(
            reg.detect_language(Path::new("index.html")).as_deref(),
            Some("html")
        );
        assert_eq!(
            reg.detect_language(Path::new("style.css")).as_deref(),
            Some("css")
        );
    }

    #[test]
    fn test_load_queries_resolves_html_injections() {
        let mut reg = LanguageRegistry::with_builtins();
        reg.load_queries(&runtime_dir());

        let html = reg.get("html").expect("html should be registered");
        assert!(!html.highlight_query.is_empty(), "html highlights.scm");
        assert!(!html.injection_query.is_empty(), "html injections.scm");
        assert!(
            html.injections.contains_key("javascript"),
            "javascript config should be available for <script> injection"
        );
        assert!(
            html.injections.contains_key("css"),
            "css config should be available for <style> injection"
        );
    }

    #[test]
    fn test_languages_without_injections_stay_empty() {
        let mut reg = LanguageRegistry::with_builtins();
        reg.load_queries(&runtime_dir());

        let rust = reg.get("rust").expect("rust should be registered");
        assert!(rust.injection_query.is_empty());
        assert!(rust.injections.is_empty());
    }
}
