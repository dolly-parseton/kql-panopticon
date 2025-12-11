//! Variable parsing and substitution
//!
//! Provides unified variable handling for both query packs and investigation packs.
//!
//! ## Variable Syntax
//!
//! The variable system supports multiple syntaxes:
//!
//! - `{{inputs.name}}` - User-provided input values
//! - `{{step.*.column}}` - All values from a column (array)
//! - `{{step.first.column}}` - First row value
//! - `{{step[N].column}}` - Nth row value
//! - `{{alias.column}}` - Current row in foreach iteration
//! - `{{secrets.name}}` - Environment variable secrets
//!
//! ## Quote Styles
//!
//! When substituting arrays into KQL, values can be quoted:
//!
//! - `Single` - Single quotes: `'value1', 'value2'`
//! - `Double` - Double quotes: `"value1", "value2"`
//! - `Verbatim` - No quotes: `value1, value2`
//!
//! ## Chunking
//!
//! Large arrays can be automatically chunked to avoid query size limits.

mod parser;
mod substitution;

pub use parser::{VarRef, VarRefType};
pub use substitution::{substitute, SubstitutionContext, QuoteStyle};

use serde::{Deserialize, Serialize};

/// Extracted value from a step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExtractedValue {
    /// Single value
    Single(String),
    /// Array of values
    Array(Vec<String>),
    /// Null/missing value
    Null,
}

impl ExtractedValue {
    /// Check if value is empty
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Single(s) => s.is_empty(),
            Self::Array(a) => a.is_empty(),
            Self::Null => true,
        }
    }

    /// Get count of values
    pub fn len(&self) -> usize {
        match self {
            Self::Single(_) => 1,
            Self::Array(a) => a.len(),
            Self::Null => 0,
        }
    }

    /// Convert to string for substitution
    pub fn to_substitution_string(&self, quote_style: QuoteStyle) -> String {
        match self {
            Self::Single(s) => quote_style.quote(s),
            Self::Array(a) => a
                .iter()
                .map(|s| quote_style.quote(s))
                .collect::<Vec<_>>()
                .join(", "),
            Self::Null => String::new(),
        }
    }

    /// Get as single value (first if array)
    pub fn as_single(&self) -> Option<&str> {
        match self {
            Self::Single(s) => Some(s),
            Self::Array(a) => a.first().map(|s| s.as_str()),
            Self::Null => None,
        }
    }

    /// Get as array (wraps single in vec)
    pub fn as_array(&self) -> Vec<&str> {
        match self {
            Self::Single(s) => vec![s.as_str()],
            Self::Array(a) => a.iter().map(|s| s.as_str()).collect(),
            Self::Null => vec![],
        }
    }
}
