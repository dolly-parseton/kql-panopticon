//! Pack definitions (query packs and investigation packs)
//!
//! This module contains the data structures for pack definitions.
//! The actual implementations will be moved from the main crate.

mod query;
mod investigation;

pub use query::QueryPack;
pub use investigation::InvestigationPack;
