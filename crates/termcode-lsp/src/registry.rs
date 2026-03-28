use std::collections::HashMap;

use termcode_config::config::LspServerConfig;

use crate::client::{LspClient, LspError, Result};

pub struct LspRegistry {
    clients: HashMap<String, LspClient>,
    configs: HashMap<String, LspServerConfig>,
}

impl LspRegistry {
    pub fn new(configs: Vec<LspServerConfig>) -> Self {
        let configs = configs
            .into_iter()
            .map(|c| (c.language.clone(), c))
            .collect();
        Self {
            clients: HashMap::new(),
            configs,
        }
    }

    pub async fn start_for_language(&mut self, language: &str, root_uri: &str) -> Result<()> {
        if self.clients.contains_key(language) {
            return Ok(());
        }

        let config = self
            .configs
            .get(language)
            .ok_or(LspError::NotStarted)?
            .clone();

        let mut client = LspClient::start(&config).await?;
        client.initialize(root_uri).await?;
        self.clients.insert(language.to_string(), client);
        Ok(())
    }

    pub fn get(&self, language: &str) -> Option<&LspClient> {
        self.clients.get(language)
    }

    pub fn get_mut(&mut self, language: &str) -> Option<&mut LspClient> {
        self.clients.get_mut(language)
    }

    pub async fn shutdown_all(&mut self) {
        for (lang, mut client) in self.clients.drain() {
            if let Err(e) = client.shutdown().await {
                log::warn!("Failed to shut down LSP server for {lang}: {e}");
            }
        }
    }

    pub fn has_server(&self, language: &str) -> bool {
        self.configs.contains_key(language)
    }

    pub fn has_running_client(&self, language: &str) -> bool {
        self.clients.contains_key(language)
    }

    /// Take the notification receiver from a client (can only be done once per client).
    pub fn take_notification_rx(
        &mut self,
        language: &str,
    ) -> Option<tokio::sync::mpsc::UnboundedReceiver<serde_json::Value>> {
        self.clients.get_mut(language)?.notification_rx.take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_new_stores_configs() {
        let configs = vec![
            LspServerConfig {
                language: "rust".to_string(),
                command: "rust-analyzer".to_string(),
                args: vec![],
            },
            LspServerConfig {
                language: "python".to_string(),
                command: "pylsp".to_string(),
                args: vec![],
            },
        ];
        let registry = LspRegistry::new(configs);
        assert!(registry.has_server("rust"));
        assert!(registry.has_server("python"));
        assert!(!registry.has_server("javascript"));
    }

    #[test]
    fn test_registry_has_running_client() {
        let registry = LspRegistry::new(vec![]);
        assert!(!registry.has_running_client("rust"));
    }

    #[test]
    fn test_registry_get_nonexistent() {
        let mut registry = LspRegistry::new(vec![]);
        assert!(registry.get("rust").is_none());
        assert!(registry.get_mut("rust").is_none());
    }
}
