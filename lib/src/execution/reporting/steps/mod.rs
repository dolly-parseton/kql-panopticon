//! Reporting step handlers
//!
//! Step handlers for report generation:
//! - [`TemplateStepHandler`] - Tera template-based reports

mod template;

pub use template::TemplateStepHandler;
