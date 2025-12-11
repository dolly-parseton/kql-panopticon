//! Completion types and services for KQL intellisense
//!
//! This module provides types and functionality for KQL code completion.

use serde::{Deserialize, Serialize};

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
    /// Character position where replacement should start
    #[serde(default)]
    pub edit_start: usize,
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
    /// Punctuation (brackets, commas, etc.)
    Punctuation,
    /// Other/unknown
    Other,
}

/// Result of completion request
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CompletionResult {
    /// Completion items
    pub items: Vec<CompletionItem>,
}
