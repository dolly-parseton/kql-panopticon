//! Variable parsing, substitution, and condition evaluation
//!
//! Provides unified variable handling using a pipe-based transform syntax.
//!
//! ## Variable Syntax
//!
//! Variables use `{{source | transform | transform}}` syntax:
//!
//! ### Sources
//!
//! - `{{inputs.name}}` - User-provided input values
//! - `{{secrets.name}}` - Environment variable secrets
//! - `{{step}}` - Step result (for step-level transforms)
//! - `{{step.column}}` - Column from step result
//!
//! ### Step-Level Transforms
//!
//! - `| is_empty` - Boolean: true if no rows
//! - `| is_not_empty` - Boolean: true if has rows
//! - `| length` - Integer: row count
//! - `| any(field > value)` - Boolean: any row matches
//! - `| all(field == value)` - Boolean: all rows match
//! - `| filter(field > value)` - Filtered step (chain with `.column`)
//!
//! ### Column Transforms
//!
//! - `| first` - Scalar: first row value
//! - `| at(N)` - Scalar: Nth row value
//! - `| array` - Array: all values
//! - `| unique` - Deduplicated values
//! - `| str_join(sep)` - Scalar: joined string
//! - `| for_each` - Iterator: triggers iteration (HTTP only)
//!
//! ### Comparison Transforms
//!
//! - `| eq(value)` - Boolean: equals
//! - `| neq(value)` - Boolean: not equals
//! - `| gt(value)` - Boolean: greater than
//! - `| gte(value)` - Boolean: greater than or equal
//! - `| lt(value)` - Boolean: less than
//! - `| lte(value)` - Boolean: less than or equal
//!
//! ## Conditions
//!
//! Conditions use the same syntax in `when:` clauses:
//!
//! ```yaml
//! when: "{{step | is_not_empty}} and {{step | any(score > 90)}}"
//! ```
//!
//! ## Example
//!
//! ```rust,ignore
//! use kql_panopticon_core::variable::{
//!     SubstitutionBuilder, EvaluationContext, ContextType,
//!     substitute, evaluate_condition,
//! };
//!
//! let ctx = EvaluationContext::new()
//!     .with_input("user", "alice@example.com")
//!     .with_step_results(results);
//!
//! // Simple substitution
//! let query = "SigninLogs | where User == '{{inputs.user}}'";
//! let resolved = substitute(query, &ctx)?;
//!
//! // With validation
//! let builder = SubstitutionBuilder::new(query, &ctx)?;
//! builder.validate(ContextType::KqlQuery)?;
//! let resolved = builder.substitute()?;
//!
//! // For-each iteration (HTTP only)
//! let builder = SubstitutionBuilder::new(url, &ctx)?;
//! for resolved_url in builder.substitute_iter()? {
//!     // Use each URL
//! }
//!
//! // Condition evaluation
//! let should_run = evaluate_condition("{{step | is_not_empty}}", &ctx)?;
//! ```
//!
//! See [README](./README.md) for complete documentation.

// Pipe-based variable system
mod builder;
mod evaluation;
mod pipeline;
mod predicate;
mod source;
mod transform;

// Public API
pub use builder::{
    contains_vars, evaluate_condition as evaluate_condition_new, substitute, ContextType,
    SubstitutionBuilder,
};
pub use evaluation::{EvaluationContext, TransformResult};
pub use pipeline::Pipeline;
pub use predicate::{ComparisonOp, Predicate, PredicateValue};
pub use source::Source;
pub use transform::{Transform, TransformType};
