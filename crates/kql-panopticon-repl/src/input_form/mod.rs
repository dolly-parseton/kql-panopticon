//! Input form widget for collecting input values before execution.
//!
//! This module provides an inline TUI form that prompts users for input values
//! with type validation, navigation between fields, and submit/cancel actions.
//!
//! # Usage
//!
//! ```rust,ignore
//! use kql_panopticon_repl::input_form::{InputForm, InputFormResult};
//! use kql_panopticon_repl::session::InputDef;
//!
//! // Create form from input definitions
//! let form = InputForm::new("Enter Values", &inputs);
//!
//! // Run the form (blocks until user submits or cancels)
//! match form.run()? {
//!     InputFormResult::Submitted(values) => {
//!         // values: HashMap<String, String>
//!     }
//!     InputFormResult::Cancelled => {
//!         // User pressed Esc
//!     }
//! }
//! ```
//!
//! # Features
//!
//! - Inline viewport (doesn't take over terminal)
//! - Type-specific validation as user types
//! - Tab/Shift+Tab navigation between fields
//! - Pre-filled default values
//! - Visual error feedback
//! - Ctrl+Enter to submit from any field

pub mod field;
mod validation;
mod widget;

pub use field::InputField;
pub use validation::{validate_input_value, ValidationResult};
pub use widget::{InputForm, InputFormResult};
