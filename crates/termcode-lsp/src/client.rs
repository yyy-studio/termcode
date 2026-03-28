use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use lsp_types::notification::Notification;
use lsp_types::request::Request;
use serde::Serialize;
use thiserror::Error;
use tokio::io::BufReader;
use tokio::process::{Child, Command};
use tokio::sync::{Mutex, mpsc, oneshot};

use termcode_config::config::LspServerConfig;

use crate::transport::{LspReader, LspWriter};
use crate::types::parse_uri;

#[derive(Error, Debug)]
pub enum LspError {
    #[error("Transport error: {0}")]
    Transport(#[from] crate::transport::TransportError),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Server not started")]
    NotStarted,
    #[error("Request failed: {0}")]
    RequestFailed(String),
    #[error("Request cancelled")]
    RequestCancelled,
    #[error("Server exited")]
    ServerExited,
}

pub type Result<T> = std::result::Result<T, LspError>;

type PendingRequests = Arc<Mutex<HashMap<i64, oneshot::Sender<serde_json::Value>>>>;

type SharedWriter = Arc<Mutex<LspWriter<tokio::process::ChildStdin>>>;

pub struct LspClient {
    _process: Child,
    writer: SharedWriter,
    next_request_id: Arc<AtomicI64>,
    pending_requests: PendingRequests,
    pub notification_rx: Option<mpsc::UnboundedReceiver<serde_json::Value>>,
    pub server_capabilities: Option<lsp_types::ServerCapabilities>,
}

impl LspClient {
    /// Start a language server process and set up transport.
    pub async fn start(config: &LspServerConfig) -> Result<Self> {
        let mut process = Command::new(&config.command)
            .args(&config.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()?;

        let stdin = process.stdin.take().ok_or(LspError::NotStarted)?;
        let stdout = process.stdout.take().ok_or(LspError::NotStarted)?;

        let writer = Arc::new(Mutex::new(LspWriter::new(stdin)));
        let reader = LspReader::new(BufReader::new(stdout));

        let (notification_tx, notification_rx) = mpsc::unbounded_channel();
        let pending_requests: PendingRequests = Arc::new(Mutex::new(HashMap::new()));

        let pending_clone = pending_requests.clone();
        let writer_clone = writer.clone();
        tokio::spawn(async move {
            Self::reader_loop(reader, pending_clone, notification_tx, writer_clone).await;
        });

        Ok(Self {
            _process: process,
            writer,
            next_request_id: Arc::new(AtomicI64::new(1)),
            pending_requests,
            notification_rx: Some(notification_rx),
            server_capabilities: None,
        })
    }

    async fn reader_loop(
        mut reader: LspReader<BufReader<tokio::process::ChildStdout>>,
        pending: PendingRequests,
        notification_tx: mpsc::UnboundedSender<serde_json::Value>,
        writer: SharedWriter,
    ) {
        loop {
            match reader.read_message().await {
                Ok(msg) => {
                    let has_method = msg.get("method").is_some();
                    let has_id = msg.get("id").is_some();

                    if has_id && !has_method {
                        if let Some(id) = msg.get("id").and_then(|v| v.as_i64()) {
                            let mut pending = pending.lock().await;
                            if let Some(sender) = pending.remove(&id) {
                                let _ = sender.send(msg);
                            }
                        }
                    } else if has_id && has_method {
                        // Server-initiated request: respond with MethodNotFound error.
                        let id = msg.get("id").cloned().unwrap_or(serde_json::Value::Null);
                        let error_response = serde_json::json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "error": {
                                "code": -32601,
                                "message": "Method not found"
                            }
                        });
                        let mut w = writer.lock().await;
                        if let Err(e) = w.write_message(&error_response).await {
                            log::warn!("Failed to respond to server request: {e}");
                        }
                    } else {
                        let _ = notification_tx.send(msg);
                    }
                }
                Err(crate::transport::TransportError::ConnectionClosed) => break,
                Err(e) => {
                    log::warn!("LSP transport read error: {e}");
                    break;
                }
            }
        }
    }

    #[allow(deprecated)]
    pub async fn initialize(&mut self, root_uri: &str) -> Result<lsp_types::InitializeResult> {
        let params = lsp_types::InitializeParams {
            root_uri: Some(parse_uri(root_uri)),
            capabilities: lsp_types::ClientCapabilities {
                text_document: Some(lsp_types::TextDocumentClientCapabilities {
                    completion: Some(lsp_types::CompletionClientCapabilities {
                        completion_item: Some(lsp_types::CompletionItemCapability {
                            snippet_support: Some(false),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }),
                    hover: Some(lsp_types::HoverClientCapabilities::default()),
                    publish_diagnostics: Some(
                        lsp_types::PublishDiagnosticsClientCapabilities::default(),
                    ),
                    definition: Some(lsp_types::GotoCapability::default()),
                    synchronization: Some(lsp_types::TextDocumentSyncClientCapabilities::default()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            ..Default::default()
        };

        let result: lsp_types::InitializeResult = self
            .send_request_with_timeout::<lsp_types::request::Initialize>(
                params,
                std::time::Duration::from_secs(30),
            )
            .await?;

        self.server_capabilities = Some(result.capabilities.clone());

        self.send_notification::<lsp_types::notification::Initialized>(
            lsp_types::InitializedParams {},
        )
        .await?;

        Ok(result)
    }

    pub async fn shutdown(&mut self) -> Result<()> {
        let _: () = self
            .send_request::<lsp_types::request::Shutdown>(())
            .await?;
        self.send_notification::<lsp_types::notification::Exit>(())
            .await?;
        Ok(())
    }

    pub async fn send_request<R: Request>(&self, params: R::Params) -> Result<R::Result>
    where
        R::Params: Serialize,
        R::Result: serde::de::DeserializeOwned,
    {
        self.send_request_with_timeout::<R>(params, std::time::Duration::from_secs(5))
            .await
    }

    pub async fn send_request_with_timeout<R: Request>(
        &self,
        params: R::Params,
        timeout: std::time::Duration,
    ) -> Result<R::Result>
    where
        R::Params: Serialize,
        R::Result: serde::de::DeserializeOwned,
    {
        let id = self.next_request_id.fetch_add(1, Ordering::Relaxed);

        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": R::METHOD,
            "params": serde_json::to_value(&params)?
        });

        let (tx, rx) = oneshot::channel();

        {
            let mut pending = self.pending_requests.lock().await;
            pending.insert(id, tx);
        }

        {
            let mut w = self.writer.lock().await;
            if let Err(e) = w.write_message(&msg).await {
                let mut pending = self.pending_requests.lock().await;
                pending.remove(&id);
                return Err(e.into());
            }
        }

        let response = match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(val)) => val,
            Ok(Err(_)) => {
                // Sender dropped (request cancelled).
                let mut pending = self.pending_requests.lock().await;
                pending.remove(&id);
                return Err(LspError::RequestCancelled);
            }
            Err(_) => {
                // Timeout: remove pending entry to prevent leak.
                let mut pending = self.pending_requests.lock().await;
                pending.remove(&id);
                return Err(LspError::RequestFailed("request timed out".to_string()));
            }
        };

        if let Some(error) = response.get("error") {
            let message = error
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("unknown error");
            return Err(LspError::RequestFailed(message.to_string()));
        }

        let result = response
            .get("result")
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        let parsed = serde_json::from_value(result)?;
        Ok(parsed)
    }

    pub async fn send_notification<N: Notification>(&self, params: N::Params) -> Result<()>
    where
        N::Params: Serialize,
    {
        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": N::METHOD,
            "params": serde_json::to_value(&params)?
        });
        let mut w = self.writer.lock().await;
        w.write_message(&msg).await?;
        Ok(())
    }

    pub async fn notify_did_open(
        &self,
        uri: &str,
        language_id: &str,
        version: i32,
        text: &str,
    ) -> Result<()> {
        self.send_notification::<lsp_types::notification::DidOpenTextDocument>(
            lsp_types::DidOpenTextDocumentParams {
                text_document: lsp_types::TextDocumentItem {
                    uri: parse_uri(uri),
                    language_id: language_id.to_string(),
                    version,
                    text: text.to_string(),
                },
            },
        )
        .await
    }

    pub async fn notify_did_change(&self, uri: &str, version: i32, text: &str) -> Result<()> {
        self.send_notification::<lsp_types::notification::DidChangeTextDocument>(
            lsp_types::DidChangeTextDocumentParams {
                text_document: lsp_types::VersionedTextDocumentIdentifier {
                    uri: parse_uri(uri),
                    version,
                },
                content_changes: vec![lsp_types::TextDocumentContentChangeEvent {
                    range: None,
                    range_length: None,
                    text: text.to_string(),
                }],
            },
        )
        .await
    }

    pub async fn notify_did_save(&self, uri: &str) -> Result<()> {
        self.send_notification::<lsp_types::notification::DidSaveTextDocument>(
            lsp_types::DidSaveTextDocumentParams {
                text_document: lsp_types::TextDocumentIdentifier {
                    uri: parse_uri(uri),
                },
                text: None,
            },
        )
        .await
    }

    pub async fn notify_did_close(&self, uri: &str) -> Result<()> {
        self.send_notification::<lsp_types::notification::DidCloseTextDocument>(
            lsp_types::DidCloseTextDocumentParams {
                text_document: lsp_types::TextDocumentIdentifier {
                    uri: parse_uri(uri),
                },
            },
        )
        .await
    }

    /// Returns trigger characters from server capabilities, if any.
    pub fn trigger_characters(&self) -> Vec<String> {
        self.server_capabilities
            .as_ref()
            .and_then(|caps| caps.completion_provider.as_ref())
            .and_then(|cp| cp.trigger_characters.clone())
            .unwrap_or_default()
    }
}
