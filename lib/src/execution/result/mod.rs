//! Result storage for step execution
//!
//! This module provides file-backed result storage for step outputs:
//!
//! - [`ResultHandle`] - Read access to JSONL result files with Polars LazyFrame support
//! - [`ResultWriter`] - Write rows to JSONL files
//! - [`ResultContext`] - Container for all step result handles
//! - [`RowIterator`] - Memory-efficient iteration over result rows
//!
//! ## Design
//!
//! Results are stored as newline-delimited JSON (JSONL) files on disk.
//! This enables memory-efficient handling of large datasets - results
//! are never fully loaded into memory unless explicitly requested.
//!
//! For advanced processing, [`ResultHandle::lazy_frame()`] provides access to
//! a Polars [`LazyFrame`] for efficient columnar operations with query pushdown.
//!
//! ## Usage
//!
//! ```rust,ignore
//! use kql_panopticon_core::execution::result::{ResultWriter, ResultContext};
//!
//! // Write results
//! let mut writer = ResultWriter::for_step(output_dir, "my_step")?;
//! writer.write_row(&serde_json::json!({"id": 1, "name": "Alice"}))?;
//! writer.write_row(&serde_json::json!({"id": 2, "name": "Bob"}))?;
//! let handle = writer.finish()?;
//!
//! // Store in context
//! let mut ctx = ResultContext::new();
//! ctx.insert("my_step", handle);
//!
//! // Access results lazily via JSON
//! let first = ctx.first("my_step")?;
//! let names = ctx.column_values("my_step", "name")?;
//!
//! // Or use Polars for efficient columnar processing
//! let lf = handle.lazy_frame()?;
//! let filtered = lf
//!     .filter(col("severity").eq(lit("High")))
//!     .select([col("timestamp"), col("message")])
//!     .collect()?;
//! ```

mod context;
mod handle;
mod writer;

pub use context::ResultContext;
pub use handle::{ResultHandle, RowIterator};
pub use writer::ResultWriter;

// Re-export Polars types for consumers using lazy_frame()
pub use polars::prelude::LazyFrame;
