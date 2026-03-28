use thiserror::Error;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt, AsyncWrite, AsyncWriteExt};

#[derive(Error, Debug)]
pub enum TransportError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Invalid header: {0}")]
    InvalidHeader(String),
    #[error("Connection closed")]
    ConnectionClosed,
}

pub type Result<T> = std::result::Result<T, TransportError>;

/// Reads LSP JSON-RPC messages from an async buffered reader.
pub struct LspReader<R> {
    reader: R,
}

impl<R: AsyncBufRead + Unpin> LspReader<R> {
    pub fn new(reader: R) -> Self {
        Self { reader }
    }

    /// Read a single JSON-RPC message (Content-Length framing).
    pub async fn read_message(&mut self) -> Result<serde_json::Value> {
        let mut content_length: Option<usize> = None;

        // Read headers until empty line.
        loop {
            let mut line = String::new();
            let bytes_read = self.reader.read_line(&mut line).await?;
            if bytes_read == 0 {
                return Err(TransportError::ConnectionClosed);
            }

            let line = line.trim();
            if line.is_empty() {
                break;
            }

            let line_lower = line.to_ascii_lowercase();
            if let Some(value) = line_lower.strip_prefix("content-length: ") {
                content_length = Some(value.parse::<usize>().map_err(|_| {
                    TransportError::InvalidHeader(format!("invalid Content-Length: {value}"))
                })?);
            }
        }

        let length = content_length
            .ok_or_else(|| TransportError::InvalidHeader("missing Content-Length".to_string()))?;

        let mut body = vec![0u8; length];
        self.reader.read_exact(&mut body).await?;
        let msg = serde_json::from_slice(&body)?;
        Ok(msg)
    }
}

/// Writes LSP JSON-RPC messages to an async writer.
pub struct LspWriter<W> {
    writer: W,
}

impl<W: AsyncWrite + Unpin> LspWriter<W> {
    pub fn new(writer: W) -> Self {
        Self { writer }
    }

    /// Write a single JSON-RPC message with Content-Length framing.
    pub async fn write_message(&mut self, msg: &serde_json::Value) -> Result<()> {
        let body = serde_json::to_string(msg)?;
        let header = format!("Content-Length: {}\r\n\r\n", body.len());
        self.writer.write_all(header.as_bytes()).await?;
        self.writer.write_all(body.as_bytes()).await?;
        self.writer.flush().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::BufReader;

    #[tokio::test]
    async fn test_write_and_read_roundtrip() {
        let (client, server) = tokio::io::duplex(4096);

        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "initialize",
            "id": 1,
            "params": {}
        });

        let mut writer = LspWriter::new(client);
        writer.write_message(&msg).await.unwrap();
        drop(writer);

        let mut reader = LspReader::new(BufReader::new(server));
        let received = reader.read_message().await.unwrap();
        assert_eq!(received, msg);
    }

    #[tokio::test]
    async fn test_multiple_messages() {
        let (client, server) = tokio::io::duplex(8192);

        let msg1 = serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "a"});
        let msg2 = serde_json::json!({"jsonrpc": "2.0", "id": 2, "method": "b"});

        let mut writer = LspWriter::new(client);
        writer.write_message(&msg1).await.unwrap();
        writer.write_message(&msg2).await.unwrap();
        drop(writer);

        let mut reader = LspReader::new(BufReader::new(server));
        assert_eq!(reader.read_message().await.unwrap(), msg1);
        assert_eq!(reader.read_message().await.unwrap(), msg2);
    }

    #[tokio::test]
    async fn test_connection_closed() {
        let (client, server) = tokio::io::duplex(1024);
        drop(client);

        let mut reader = LspReader::new(BufReader::new(server));
        let result = reader.read_message().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_unicode_content() {
        let (client, server) = tokio::io::duplex(4096);

        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {"text": "fn main() { println!(\"hello 世界\"); }"}
        });

        let mut writer = LspWriter::new(client);
        writer.write_message(&msg).await.unwrap();
        drop(writer);

        let mut reader = LspReader::new(BufReader::new(server));
        let received = reader.read_message().await.unwrap();
        assert_eq!(received, msg);
    }
}
