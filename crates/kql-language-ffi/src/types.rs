//! Validation types for KQL Language FFI

use serde::{Deserialize, Serialize};

/// Result of validating a KQL query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    /// Whether the query is valid (no errors)
    pub valid: bool,
    /// Diagnostics (errors and warnings)
    pub diagnostics: Vec<Diagnostic>,
}

impl ValidationResult {
    /// Create a valid result with no diagnostics
    pub fn valid() -> Self {
        Self {
            valid: true,
            diagnostics: Vec::new(),
        }
    }

    /// Create an invalid result with the given diagnostics
    pub fn invalid(diagnostics: Vec<Diagnostic>) -> Self {
        Self {
            valid: false,
            diagnostics,
        }
    }

    /// Check if the validation passed (no errors)
    pub fn is_valid(&self) -> bool {
        self.valid && !self.has_errors()
    }

    /// Check if there are any errors
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.severity == DiagnosticSeverity::Error)
    }

    /// Check if there are any warnings
    pub fn has_warnings(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.severity == DiagnosticSeverity::Warning)
    }

    /// Get all diagnostics
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Get only error diagnostics
    pub fn errors(&self) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == DiagnosticSeverity::Error)
    }

    /// Get only warning diagnostics
    pub fn warnings(&self) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == DiagnosticSeverity::Warning)
    }
}

/// A diagnostic message from validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    /// The diagnostic message
    pub message: String,
    /// Severity level
    pub severity: DiagnosticSeverity,
    /// Start offset in the query (0-based, character position)
    pub start: usize,
    /// End offset in the query (0-based, character position)
    pub end: usize,
    /// Line number (1-based)
    pub line: usize,
    /// Column number (1-based)
    pub column: usize,
    /// Error/warning code (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

impl Diagnostic {
    /// Get the length of the diagnostic span
    pub fn length(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    /// Check if this is an error
    pub fn is_error(&self) -> bool {
        self.severity == DiagnosticSeverity::Error
    }

    /// Check if this is a warning
    pub fn is_warning(&self) -> bool {
        self.severity == DiagnosticSeverity::Warning
    }
}

/// Severity level of a diagnostic
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum DiagnosticSeverity {
    /// An error that prevents the query from being valid
    Error,
    /// A warning about potential issues
    Warning,
    /// Informational message
    Information,
    /// A hint or suggestion
    Hint,
}

impl DiagnosticSeverity {
    /// Parse from a string (case-insensitive)
    #[allow(dead_code)]
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "error" => Self::Error,
            "warning" => Self::Warning,
            "information" | "info" => Self::Information,
            "hint" | "suggestion" => Self::Hint,
            _ => Self::Error, // Default to error for unknown
        }
    }
}

impl std::fmt::Display for DiagnosticSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Error => write!(f, "Error"),
            Self::Warning => write!(f, "Warning"),
            Self::Information => write!(f, "Information"),
            Self::Hint => write!(f, "Hint"),
        }
    }
}

