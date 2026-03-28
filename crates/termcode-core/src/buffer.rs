use std::fs;
use std::io::Write;
use std::path::Path;

use ropey::Rope;
use thiserror::Error;

use crate::encoding::{FileEncoding, LineEnding, detect_encoding, detect_line_ending};
use crate::position::Position;
use crate::transaction::Transaction;

#[derive(Error, Debug)]
pub enum BufferError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Encoding error: unsupported encoding {0}")]
    UnsupportedEncoding(String),
}

/// A text buffer backed by a Rope for O(log n) edits on large files.
pub struct Buffer {
    text: Rope,
    encoding: FileEncoding,
    line_ending: LineEnding,
    modified: bool,
}

impl Buffer {
    /// Create an empty buffer.
    pub fn new() -> Self {
        Self {
            text: Rope::new(),
            encoding: FileEncoding::default(),
            line_ending: LineEnding::default(),
            modified: false,
        }
    }

    /// Load a buffer from a file path.
    pub fn from_file(path: &Path) -> Result<Self, BufferError> {
        let raw_bytes = fs::read(path)?;
        let (encoding, stripped) = detect_encoding(&raw_bytes);

        let text_str = match encoding {
            FileEncoding::Utf8 | FileEncoding::Utf8Bom => {
                String::from_utf8_lossy(stripped).into_owned()
            }
            FileEncoding::Utf16Le => {
                let u16_iter = stripped
                    .chunks_exact(2)
                    .map(|c| u16::from_le_bytes([c[0], c[1]]));
                char::decode_utf16(u16_iter)
                    .map(|r| r.unwrap_or(char::REPLACEMENT_CHARACTER))
                    .collect()
            }
            FileEncoding::Utf16Be => {
                let u16_iter = stripped
                    .chunks_exact(2)
                    .map(|c| u16::from_be_bytes([c[0], c[1]]));
                char::decode_utf16(u16_iter)
                    .map(|r| r.unwrap_or(char::REPLACEMENT_CHARACTER))
                    .collect()
            }
            FileEncoding::Other(enc) => {
                let (decoded, _, had_errors) = enc.decode(stripped);
                if had_errors {
                    return Err(BufferError::UnsupportedEncoding(enc.name().to_string()));
                }
                decoded.into_owned()
            }
        };

        let line_ending = detect_line_ending(&text_str);
        let rope = Rope::from_str(&text_str);

        Ok(Self {
            text: rope,
            encoding,
            line_ending,
            modified: false,
        })
    }

    pub fn text(&self) -> &Rope {
        &self.text
    }

    pub fn text_mut(&mut self) -> &mut Rope {
        self.modified = true;
        &mut self.text
    }

    pub fn line(&self, idx: usize) -> ropey::RopeSlice<'_> {
        self.text.line(idx)
    }

    pub fn line_count(&self) -> usize {
        self.text.len_lines()
    }

    pub fn encoding(&self) -> FileEncoding {
        self.encoding
    }

    pub fn line_ending(&self) -> LineEnding {
        self.line_ending
    }

    pub fn is_modified(&self) -> bool {
        self.modified
    }

    pub fn set_modified(&mut self, modified: bool) {
        self.modified = modified;
    }

    /// Get the byte length of the buffer content.
    pub fn len_bytes(&self) -> usize {
        self.text.len_bytes()
    }

    /// Check if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.text.len_bytes() == 0
    }

    /// Convert a line/column position to a byte offset.
    pub fn pos_to_byte(&self, pos: &Position) -> usize {
        let line_count = self.text.len_lines();
        if line_count == 0 {
            return 0;
        }
        let line = pos.line.min(line_count - 1);
        let line_start = self.text.line_to_byte(line);
        let line_slice = self.text.line(line);
        let line_str: String = line_slice.into();

        let mut byte_offset = 0;
        for (col, ch) in line_str.chars().enumerate() {
            if ch == '\n' || ch == '\r' {
                break;
            }
            if col >= pos.column {
                break;
            }
            byte_offset += ch.len_utf8();
        }
        line_start + byte_offset
    }

    /// Convert a byte offset to a line/column position.
    pub fn byte_to_pos(&self, byte: usize) -> Position {
        let byte = byte.min(self.text.len_bytes());
        let line = self.text.byte_to_line(byte);
        let line_start = self.text.line_to_byte(line);
        let line_slice = self.text.line(line);
        let line_str: String = line_slice.into();

        let target_offset = byte - line_start;
        let mut col = 0;
        let mut offset = 0;
        for ch in line_str.chars() {
            if offset >= target_offset {
                break;
            }
            if ch == '\n' || ch == '\r' {
                break;
            }
            offset += ch.len_utf8();
            col += 1;
        }
        Position::new(line, col)
    }

    /// Apply a transaction to this buffer, marking it as modified.
    pub fn apply(&mut self, transaction: &Transaction) -> anyhow::Result<()> {
        transaction.apply(&mut self.text)?;
        self.modified = true;
        Ok(())
    }

    /// Save buffer contents to a file, preserving encoding and line endings.
    /// Uses atomic write (temp file + rename) to prevent data loss.
    pub fn save_to_file(&self, path: &Path) -> anyhow::Result<()> {
        let mut content: String = self.text.to_string();

        // Normalize line endings
        if self.line_ending == LineEnding::CrLf {
            // First normalize to LF, then convert to CRLF
            content = content.replace("\r\n", "\n").replace('\n', "\r\n");
        }

        let bytes = match self.encoding {
            FileEncoding::Utf8 => content.into_bytes(),
            FileEncoding::Utf8Bom => {
                let mut bytes = vec![0xEF, 0xBB, 0xBF];
                bytes.extend_from_slice(content.as_bytes());
                bytes
            }
            FileEncoding::Utf16Le => {
                let mut bytes = vec![0xFF, 0xFE]; // BOM
                for unit in content.encode_utf16() {
                    bytes.extend_from_slice(&unit.to_le_bytes());
                }
                bytes
            }
            FileEncoding::Utf16Be => {
                let mut bytes = vec![0xFE, 0xFF]; // BOM
                for unit in content.encode_utf16() {
                    bytes.extend_from_slice(&unit.to_be_bytes());
                }
                bytes
            }
            FileEncoding::Other(enc) => {
                let (encoded, _, _) = enc.encode(&content);
                encoded.into_owned()
            }
        };

        // Atomic write: write to temp file, then rename
        let dir = path.parent().unwrap_or(Path::new("."));
        let mut temp = tempfile::NamedTempFile::new_in(dir)?;
        temp.write_all(&bytes)?;
        temp.flush()?;
        temp.persist(path)?;

        Ok(())
    }
}

impl Default for Buffer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pos_to_byte_simple() {
        let mut buf = Buffer::new();
        *buf.text_mut() = Rope::from_str("hello\nworld\n");
        assert_eq!(buf.pos_to_byte(&Position::new(0, 0)), 0);
        assert_eq!(buf.pos_to_byte(&Position::new(0, 3)), 3);
        assert_eq!(buf.pos_to_byte(&Position::new(1, 0)), 6);
        assert_eq!(buf.pos_to_byte(&Position::new(1, 2)), 8);
    }

    #[test]
    fn test_byte_to_pos_simple() {
        let mut buf = Buffer::new();
        *buf.text_mut() = Rope::from_str("hello\nworld\n");
        assert_eq!(buf.byte_to_pos(0), Position::new(0, 0));
        assert_eq!(buf.byte_to_pos(3), Position::new(0, 3));
        assert_eq!(buf.byte_to_pos(6), Position::new(1, 0));
        assert_eq!(buf.byte_to_pos(8), Position::new(1, 2));
    }

    #[test]
    fn test_pos_byte_roundtrip() {
        let mut buf = Buffer::new();
        *buf.text_mut() = Rope::from_str("abc\ndef\nghi");
        for line in 0..3 {
            for col in 0..3 {
                let pos = Position::new(line, col);
                let byte = buf.pos_to_byte(&pos);
                let back = buf.byte_to_pos(byte);
                assert_eq!(pos, back, "roundtrip failed for {pos}");
            }
        }
    }

    #[test]
    fn test_save_to_file_utf8() {
        let mut buf = Buffer::new();
        *buf.text_mut() = Rope::from_str("hello world");
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.txt");
        buf.save_to_file(&path).unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content, "hello world");
    }

    #[test]
    fn test_apply_transaction() {
        let mut buf = Buffer::new();
        *buf.text_mut() = Rope::from_str("hello");
        buf.set_modified(false);
        assert!(!buf.is_modified());

        let txn = Transaction::insert(" world", 5, buf.len_bytes());
        buf.apply(&txn).unwrap();
        assert!(buf.is_modified());
        assert_eq!(buf.text().to_string(), "hello world");
    }
}
