//! Investigation module for running investigation packs
//!
//! This module provides the types and runner for executing investigation packs
//! against Azure Log Analytics workspaces.

pub mod condition;
pub mod http;
mod report;
mod runner;
mod types;

// Re-export public types
pub use runner::InvestigationRunner;
pub use types::{InvestigationResult, ProgressUpdate, Status};
