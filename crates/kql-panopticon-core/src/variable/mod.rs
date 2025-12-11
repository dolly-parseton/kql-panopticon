//! Variable parsing and substitution
//!
//! Provides unified variable handling for pack execution.
//!
//! ## Variable Syntax
//!
//! The variable system supports these syntaxes:
//!
//! - `{{inputs.name}}` - User-provided input values
//! - `{{secrets.name}}` - Environment variable secrets
//! - `{{step.*.column}}` - All values from a column (array)
//! - `{{step.first.column}}` - First row value
//! - `{{step[N].column}}` - Nth row value
//! - `{{alias.column}}` - Current row in foreach iteration
//!
//! ## Execution vs Validation
//!
//! - **Execution**: Uses `SubstitutionContext` with actual step results
//! - **Validation**: Uses `ValidationContext` with example values or placeholders
//!
//! ## Example
//!
//! ```rust,ignore
//! use kql_panopticon_core::variable::{substitute, SubstitutionContext};
//!
//! let context = SubstitutionContext::new()
//!     .with_input("user", "alice@example.com")
//!     .with_step_results("logins", vec![
//!         serde_json::json!({"IP": "10.0.0.1"}),
//!         serde_json::json!({"IP": "10.0.0.2"}),
//!     ]);
//!
//! let query = "SigninLogs | where User == '{{inputs.user}}' | where IP in ({{logins.*.IP}})";
//! let result = substitute(query, &context)?;
//! // Result: SigninLogs | where User == 'alice@example.com' | where IP in ('10.0.0.1','10.0.0.2')
//! ```

mod parser;
mod substitution;

pub use parser::{VarRef, VarRefType};
pub use substitution::{
    substitute, substitute_with_quote_style,
    substitute_for_validation, substitute_for_validation_with_quote_style,
    SubstitutionContext, ValidationContext,
};
