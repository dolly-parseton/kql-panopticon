//! Shared types for KQL Language FFI

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
    /// Convert from a string (case-insensitive)
    pub fn from_str(s: &str) -> Self {
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

/// Classification kind for syntax highlighting
///
/// These values match the `ClassificationKind` enum from Kusto.Language
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ClassificationKind {
    /// Plain text (no special highlighting)
    PlainText,
    /// A comment
    Comment,
    /// Punctuation characters: (), ;:
    Punctuation,
    /// A directive: #
    Directive,
    /// A non-string literal (number, boolean, etc.)
    Literal,
    /// A string literal
    StringLiteral,
    /// A type name
    Type,
    /// An identifier
    Identifier,
    /// A column name
    Column,
    /// A table name
    Table,
    /// A database name
    Database,
    /// A scalar function
    ScalarFunction,
    /// An aggregate function
    AggregateFunction,
    /// A keyword
    Keyword,
    /// An operator
    Operator,
    /// A variable
    Variable,
    /// A parameter
    Parameter,
    /// A command keyword
    CommandKeyword,
    /// A query operator (pipe operators like where, project, etc.)
    QueryOperator,
    /// A scalar operator (mathematical/logical operators)
    ScalarOperator,
    /// A materializable expression
    MaterializedViewFunction,
    /// Plugin name
    Plugin,
    /// Option name
    Option,
    /// Client directive
    ClientDirective,
    /// Query parameter
    QueryParameter,
    /// Cluster name
    Cluster,
}

impl ClassificationKind {
    /// Convert from a string
    pub fn from_str(s: &str) -> Self {
        match s {
            "PlainText" => Self::PlainText,
            "Comment" => Self::Comment,
            "Punctuation" => Self::Punctuation,
            "Directive" => Self::Directive,
            "Literal" => Self::Literal,
            "StringLiteral" => Self::StringLiteral,
            "Type" => Self::Type,
            "Identifier" => Self::Identifier,
            "Column" => Self::Column,
            "Table" => Self::Table,
            "Database" => Self::Database,
            "ScalarFunction" => Self::ScalarFunction,
            "AggregateFunction" => Self::AggregateFunction,
            "Keyword" => Self::Keyword,
            "Operator" => Self::Operator,
            "Variable" => Self::Variable,
            "Parameter" => Self::Parameter,
            "CommandKeyword" => Self::CommandKeyword,
            "QueryOperator" => Self::QueryOperator,
            "ScalarOperator" => Self::ScalarOperator,
            "MaterializedViewFunction" => Self::MaterializedViewFunction,
            "Plugin" => Self::Plugin,
            "Option" => Self::Option,
            "ClientDirective" => Self::ClientDirective,
            "QueryParameter" => Self::QueryParameter,
            "Cluster" => Self::Cluster,
            _ => Self::PlainText,
        }
    }
}

/// A classified span for syntax highlighting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassifiedSpan {
    /// Start offset (0-based)
    pub start: usize,
    /// Length of the span
    pub length: usize,
    /// Classification kind
    pub kind: ClassificationKind,
}

/// Result of syntax classification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassificationResult {
    /// Classified spans
    pub spans: Vec<ClassifiedSpan>,
}

/// A completion item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionItem {
    /// Display label
    pub label: String,
    /// Kind of completion
    pub kind: CompletionKind,
    /// Optional detail text
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// Text to insert (if different from label)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub insert_text: Option<String>,
    /// Sort order (lower = higher priority)
    #[serde(default)]
    pub sort_order: i32,
}

/// Kind of completion item
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum CompletionKind {
    /// A keyword
    Keyword,
    /// A function
    Function,
    /// An aggregate function
    AggregateFunction,
    /// A table
    Table,
    /// A column
    Column,
    /// A variable
    Variable,
    /// An operator
    Operator,
    /// A parameter
    Parameter,
    /// A database
    Database,
    /// A cluster
    Cluster,
    /// A type
    Type,
    /// Other/unknown
    Other,
}

/// Result of completion request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionResult {
    /// Completion items
    pub items: Vec<CompletionItem>,
}
