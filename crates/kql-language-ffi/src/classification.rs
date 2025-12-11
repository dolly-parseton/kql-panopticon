//! Classification types and services for KQL syntax highlighting
//!
//! This module provides types and functionality for classifying KQL syntax
//! elements for syntax highlighting purposes.

use serde::{Deserialize, Serialize};

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
    /// Parse from a string
    #[allow(dead_code)]
    pub fn parse(s: &str) -> Self {
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
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ClassificationResult {
    /// Classified spans
    pub spans: Vec<ClassifiedSpan>,
}
