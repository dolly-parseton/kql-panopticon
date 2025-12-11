//! Variable parsing, substitution, and condition evaluation
//!
//! Provides unified variable handling and condition evaluation for pack execution.
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
//! ## Condition Syntax
//!
//! Conditions are used in `when` clauses for conditional step execution:
//!
//! - `step is empty` / `step is not empty` - Check if step has results
//! - `step.length == N` - Check row count
//! - `step.first.field == value` - Check first row field (aligns with variable syntax)
//! - `step.any(field > value)` - Check if any row matches
//! - `step.all(field == value)` - Check if all rows match
//! - Boolean: `and`, `or`, `not`
//!
//! See [`condition`] module and [README](./README.md) for complete documentation.
//!
//! ## Execution vs Validation
//!
//! - **Execution**: Uses `SubstitutionContext` with actual step results
//! - **Validation**: Uses `ValidationContext` with example values or placeholders
//!
//! ## Example
//!
//! ```rust,ignore
//! use kql_panopticon_core::variable::{substitute, evaluate_condition, SubstitutionContext};
//!
//! let context = SubstitutionContext::new()
//!     .with_input("user", "alice@example.com")
//!     .with_step_results("logins", vec![
//!         serde_json::json!({"IP": "10.0.0.1"}),
//!         serde_json::json!({"IP": "10.0.0.2"}),
//!     ]);
//!
//! // Variable substitution
//! let query = "SigninLogs | where User == '{{inputs.user}}' | where IP in ({{logins.*.IP}})";
//! let result = substitute(query, &context)?;
//! // Result: SigninLogs | where User == 'alice@example.com' | where IP in ('10.0.0.1','10.0.0.2')
//!
//! // Condition evaluation
//! let should_run = evaluate_condition("logins is not empty", &context.step_results);
//! assert!(should_run);
//! ```

mod condition;
mod parser;
mod substitution;

pub use condition::evaluate_condition;
pub use parser::{VarRef, VarRefType};
pub use substitution::{
    substitute, substitute_for_validation, substitute_for_validation_with_quote_style,
    substitute_with_quote_style, SubstitutionContext, ValidationContext,
};
