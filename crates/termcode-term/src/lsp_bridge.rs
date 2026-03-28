use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Mutex, mpsc};

use termcode_config::config::LspServerConfig;
use termcode_core::position::Position;
use termcode_lsp::registry::LspRegistry;
use termcode_lsp::types::{
    CompletionItem, LspResponse, diagnostic_from_lsp, lsp_to_position, parse_uri, position_to_lsp,
};

use crate::event::AppEvent;

const DEBOUNCE_DELAY: Duration = Duration::from_millis(100);

/// Parameters for a queued didOpen notification.
pub struct DidOpenParams {
    pub uri: String,
    pub language_id: String,
    pub version: i32,
    pub text: String,
}

pub struct LspBridge {
    runtime: tokio::runtime::Runtime,
    registry: Arc<Mutex<LspRegistry>>,
    event_tx: mpsc::UnboundedSender<AppEvent>,
    /// Tracks debounce abort handles per URI so new edits cancel pending didChange.
    debounce_handles: Arc<Mutex<HashMap<String, tokio::task::AbortHandle>>>,
}

impl LspBridge {
    pub fn new(configs: Vec<LspServerConfig>, event_tx: mpsc::UnboundedSender<AppEvent>) -> Self {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("failed to create tokio runtime");

        let registry = Arc::new(Mutex::new(LspRegistry::new(configs)));
        let debounce_handles = Arc::new(Mutex::new(HashMap::new()));

        Self {
            runtime,
            registry,
            event_tx,
            debounce_handles,
        }
    }

    /// Start a language server and optionally send didOpen after initialization succeeds.
    /// When `did_open` is provided, didOpen is sent inside the same async task after
    /// the server is fully initialized, avoiding the race condition of independent tasks.
    pub fn start_server_with_did_open(
        &self,
        language: &str,
        root_uri: &str,
        did_open: Option<DidOpenParams>,
    ) {
        let registry = self.registry.clone();
        let event_tx = self.event_tx.clone();
        let language = language.to_string();
        let root_uri = root_uri.to_string();

        self.runtime.spawn(async move {
            let result = {
                let mut reg = registry.lock().await;
                reg.start_for_language(&language, &root_uri).await
            };

            let response = match result {
                Ok(()) => {
                    let mut reg = registry.lock().await;
                    if let Some(rx) = reg.take_notification_rx(&language) {
                        let tx = event_tx.clone();
                        tokio::spawn(async move {
                            forward_notifications(rx, tx).await;
                        });
                    }
                    if let Some(params) = did_open {
                        if let Some(client) = reg.get(&language) {
                            if let Err(e) = client
                                .notify_did_open(
                                    &params.uri,
                                    &params.language_id,
                                    params.version,
                                    &params.text,
                                )
                                .await
                            {
                                log::warn!("LSP didOpen error: {e}");
                            }
                        }
                    }
                    let trigger_characters = reg
                        .get(&language)
                        .map(|c| c.trigger_characters())
                        .unwrap_or_default();
                    LspResponse::ServerStarted {
                        language: language.clone(),
                        trigger_characters,
                    }
                }
                Err(e) => LspResponse::ServerError {
                    language: language.clone(),
                    error: e.to_string(),
                },
            };
            let _ = event_tx.send(AppEvent::Lsp(response));
        });
    }

    pub fn notify_did_open(
        &self,
        language: &str,
        uri: &str,
        language_id: &str,
        version: i32,
        text: &str,
    ) {
        let registry = self.registry.clone();
        let language = language.to_string();
        let uri = uri.to_string();
        let language_id = language_id.to_string();
        let text = text.to_string();

        self.runtime.spawn(async move {
            let reg = registry.lock().await;
            if let Some(client) = reg.get(&language) {
                if let Err(e) = client
                    .notify_did_open(&uri, &language_id, version, &text)
                    .await
                {
                    log::warn!("LSP didOpen error: {e}");
                }
            }
        });
    }

    pub fn notify_did_change(&self, language: &str, uri: &str, version: i32, text: &str) {
        let registry = self.registry.clone();
        let language = language.to_string();
        let uri_str = uri.to_string();
        let text = text.to_string();
        let debounce_handles = self.debounce_handles.clone();

        self.runtime.spawn(async move {
            {
                let mut handles = debounce_handles.lock().await;
                if let Some(handle) = handles.remove(&uri_str) {
                    handle.abort();
                }
            }

            let registry_clone = registry.clone();
            let uri_clone = uri_str.clone();
            let debounce_handles_clone = debounce_handles.clone();

            let task = tokio::spawn(async move {
                tokio::time::sleep(DEBOUNCE_DELAY).await;

                let reg = registry_clone.lock().await;
                if let Some(client) = reg.get(&language) {
                    if let Err(e) = client.notify_did_change(&uri_clone, version, &text).await {
                        log::warn!("LSP didChange error: {e}");
                    }
                }

                let mut handles = debounce_handles_clone.lock().await;
                handles.remove(&uri_clone);
            });

            let mut handles = debounce_handles.lock().await;
            handles.insert(uri_str, task.abort_handle());
        });
    }

    pub fn notify_did_save(&self, language: &str, uri: &str) {
        let registry = self.registry.clone();
        let language = language.to_string();
        let uri = uri.to_string();

        self.runtime.spawn(async move {
            let reg = registry.lock().await;
            if let Some(client) = reg.get(&language) {
                if let Err(e) = client.notify_did_save(&uri).await {
                    log::warn!("LSP didSave error: {e}");
                }
            }
        });
    }

    pub fn notify_did_close(&self, language: &str, uri: &str) {
        let registry = self.registry.clone();
        let language = language.to_string();
        let uri = uri.to_string();
        let debounce_handles = self.debounce_handles.clone();

        self.runtime.spawn(async move {
            // Cancel any pending debounced didChange for this URI before sending didClose.
            {
                let mut handles = debounce_handles.lock().await;
                if let Some(handle) = handles.remove(&uri) {
                    handle.abort();
                }
            }
            let reg = registry.lock().await;
            if let Some(client) = reg.get(&language) {
                if let Err(e) = client.notify_did_close(&uri).await {
                    log::warn!("LSP didClose error: {e}");
                }
            }
        });
    }

    pub fn request_completion(&self, language: &str, uri: &str, position: Position) {
        let registry = self.registry.clone();
        let event_tx = self.event_tx.clone();
        let language = language.to_string();
        let uri = uri.to_string();

        self.runtime.spawn(async move {
            let params = lsp_types::CompletionParams {
                text_document_position: lsp_types::TextDocumentPositionParams {
                    text_document: lsp_types::TextDocumentIdentifier {
                        uri: parse_uri(&uri),
                    },
                    position: position_to_lsp(&position),
                },
                context: None,
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            };
            let result = {
                let reg = registry.lock().await;
                if let Some(client) = reg.get(&language) {
                    Some(
                        client
                            .send_request::<lsp_types::request::Completion>(params)
                            .await,
                    )
                } else {
                    None
                }
            };

            if let Some(Ok(Some(response))) = result {
                let items: Vec<CompletionItem> = match response {
                    lsp_types::CompletionResponse::Array(items) => {
                        items.iter().map(CompletionItem::from).collect()
                    }
                    lsp_types::CompletionResponse::List(list) => {
                        list.items.iter().map(CompletionItem::from).collect()
                    }
                };
                let _ = event_tx.send(AppEvent::Lsp(LspResponse::Completion { items }));
            }
        });
    }

    pub fn request_hover(&self, language: &str, uri: &str, position: Position) {
        let registry = self.registry.clone();
        let event_tx = self.event_tx.clone();
        let language = language.to_string();
        let uri = uri.to_string();

        self.runtime.spawn(async move {
            let params = lsp_types::HoverParams {
                text_document_position_params: lsp_types::TextDocumentPositionParams {
                    text_document: lsp_types::TextDocumentIdentifier {
                        uri: parse_uri(&uri),
                    },
                    position: position_to_lsp(&position),
                },
                work_done_progress_params: Default::default(),
            };
            let result = {
                let reg = registry.lock().await;
                if let Some(client) = reg.get(&language) {
                    Some(
                        client
                            .send_request::<lsp_types::request::HoverRequest>(params)
                            .await,
                    )
                } else {
                    None
                }
            };

            if let Some(Ok(Some(hover))) = result {
                let contents = match hover.contents {
                    lsp_types::HoverContents::Scalar(s) => match s {
                        lsp_types::MarkedString::String(s) => s,
                        lsp_types::MarkedString::LanguageString(ls) => ls.value,
                    },
                    lsp_types::HoverContents::Array(arr) => arr
                        .into_iter()
                        .map(|s| match s {
                            lsp_types::MarkedString::String(s) => s,
                            lsp_types::MarkedString::LanguageString(ls) => ls.value,
                        })
                        .collect::<Vec<_>>()
                        .join("\n"),
                    lsp_types::HoverContents::Markup(mc) => mc.value,
                };
                let _ = event_tx.send(AppEvent::Lsp(LspResponse::Hover { contents }));
            }
        });
    }

    pub fn request_definition(&self, language: &str, uri: &str, position: Position) {
        let registry = self.registry.clone();
        let event_tx = self.event_tx.clone();
        let language = language.to_string();
        let uri = uri.to_string();

        self.runtime.spawn(async move {
            let params = lsp_types::GotoDefinitionParams {
                text_document_position_params: lsp_types::TextDocumentPositionParams {
                    text_document: lsp_types::TextDocumentIdentifier {
                        uri: parse_uri(&uri),
                    },
                    position: position_to_lsp(&position),
                },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            };
            let result = {
                let reg = registry.lock().await;
                if let Some(client) = reg.get(&language) {
                    Some(
                        client
                            .send_request::<lsp_types::request::GotoDefinition>(params)
                            .await,
                    )
                } else {
                    None
                }
            };

            if let Some(Ok(Some(response))) = result {
                let location = match response {
                    lsp_types::GotoDefinitionResponse::Scalar(loc) => Some(loc),
                    lsp_types::GotoDefinitionResponse::Array(locs) => locs.into_iter().next(),
                    lsp_types::GotoDefinitionResponse::Link(links) => {
                        links.into_iter().next().map(|link| lsp_types::Location {
                            uri: link.target_uri,
                            range: link.target_selection_range,
                        })
                    }
                };

                if let Some(loc) = location {
                    let _ = event_tx.send(AppEvent::Lsp(LspResponse::Definition {
                        uri: loc.uri.as_str().to_string(),
                        position: lsp_to_position(&loc.range.start),
                    }));
                }
            }
        });
    }

    pub fn has_server(&self, language: &str) -> bool {
        let registry = self.registry.clone();
        self.runtime.block_on(async {
            let reg = registry.lock().await;
            reg.has_server(language)
        })
    }

    pub fn has_running_client(&self, language: &str) -> bool {
        let registry = self.registry.clone();
        self.runtime.block_on(async {
            let reg = registry.lock().await;
            reg.has_running_client(language)
        })
    }

    pub fn shutdown(&self) {
        let registry = self.registry.clone();
        self.runtime.block_on(async {
            let mut reg = registry.lock().await;
            reg.shutdown_all().await;
        });
    }
}

/// Forwards server notifications to the app event loop.
async fn forward_notifications(
    mut rx: mpsc::UnboundedReceiver<serde_json::Value>,
    tx: mpsc::UnboundedSender<AppEvent>,
) {
    while let Some(msg) = rx.recv().await {
        let method = msg.get("method").and_then(|m| m.as_str());

        match method {
            Some("textDocument/publishDiagnostics") => {
                if let Some(params) = msg.get("params") {
                    if let Ok(diag_params) = serde_json::from_value::<
                        lsp_types::PublishDiagnosticsParams,
                    >(params.clone())
                    {
                        let diagnostics = diag_params
                            .diagnostics
                            .iter()
                            .map(diagnostic_from_lsp)
                            .collect();
                        let _ = tx.send(AppEvent::Lsp(LspResponse::Diagnostics {
                            uri: diag_params.uri.as_str().to_string(),
                            diagnostics,
                        }));
                    }
                }
            }
            _ => {
                // Other server notifications -- log and ignore for now.
                if let Some(m) = method {
                    log::debug!("Unhandled LSP notification: {m}");
                }
            }
        }
    }
}
