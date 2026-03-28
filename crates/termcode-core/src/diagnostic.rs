use crate::position::Position;

/// A diagnostic message (error, warning, etc.) associated with a text range.
#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub range: (Position, Position),
    pub severity: DiagnosticSeverity,
    pub message: String,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Info,
    Hint,
}
