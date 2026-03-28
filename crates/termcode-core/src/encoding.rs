use encoding_rs::Encoding as EncodingRs;

/// Supported file encodings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FileEncoding {
    #[default]
    Utf8,
    Utf8Bom,
    Utf16Le,
    Utf16Be,
    Other(&'static EncodingRs),
}

impl std::fmt::Display for FileEncoding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Utf8 => write!(f, "UTF-8"),
            Self::Utf8Bom => write!(f, "UTF-8 BOM"),
            Self::Utf16Le => write!(f, "UTF-16 LE"),
            Self::Utf16Be => write!(f, "UTF-16 BE"),
            Self::Other(enc) => write!(f, "{}", enc.name()),
        }
    }
}

/// Line ending style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEnding {
    Lf,
    CrLf,
}

impl Default for LineEnding {
    fn default() -> Self {
        if cfg!(windows) { Self::CrLf } else { Self::Lf }
    }
}

impl LineEnding {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Lf => "\n",
            Self::CrLf => "\r\n",
        }
    }
}

/// Detect encoding from raw bytes. Returns (encoding, BOM-stripped bytes).
pub fn detect_encoding(bytes: &[u8]) -> (FileEncoding, &[u8]) {
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        (FileEncoding::Utf8Bom, &bytes[3..])
    } else if bytes.starts_with(&[0xFF, 0xFE]) {
        (FileEncoding::Utf16Le, &bytes[2..])
    } else if bytes.starts_with(&[0xFE, 0xFF]) {
        (FileEncoding::Utf16Be, &bytes[2..])
    } else {
        (FileEncoding::Utf8, bytes)
    }
}

/// Detect the dominant line ending in text.
pub fn detect_line_ending(text: &str) -> LineEnding {
    let crlf_count = text.matches("\r\n").count();
    let lf_count = text.matches('\n').count().saturating_sub(crlf_count);
    if crlf_count > lf_count {
        LineEnding::CrLf
    } else {
        LineEnding::Lf
    }
}
