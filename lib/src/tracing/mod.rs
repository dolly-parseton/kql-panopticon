//! Tracing infrastructure for execution observability
//!
//! This module provides structured tracing for pack execution, enabling:
//! - Console logging via `tracing-subscriber`
//! - TUI event forwarding via [`TuiLayer`]
//! - Structured spans for jobs, phases, and steps
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────┐
//! │           Executor / Handlers           │
//! │  info_span!("job", job_id, pack, ...)   │
//! │  info_span!("phase", phase, ...)        │
//! │  info_span!("step", name, ...)          │
//! │  tracing::info!(...), debug!(...), ...  │
//! └──────────────────┬──────────────────────┘
//!                    │
//!                    ▼
//! ┌─────────────────────────────────────────┐
//! │         Subscriber Layer Stack          │
//! ├─────────────────────────────────────────┤
//! │  FmtLayer → stderr (CLI mode)           │
//! │  TuiLayer → mpsc channel (TUI mode)     │
//! └──────────────────┬──────────────────────┘
//!                    │
//!         ┌─────────┴─────────┐
//!         ▼                   ▼
//!    ┌─────────┐      ┌─────────────┐
//!    │ Console │      │ TUI/Channel │
//!    └─────────┘      └─────────────┘
//! ```
//!
//! ## Usage
//!
//! ### CLI Mode (console output only)
//!
//! ```rust,ignore
//! use tracing_subscriber::prelude::*;
//! use tracing_subscriber::fmt;
//!
//! tracing_subscriber::registry()
//!     .with(fmt::layer().with_target(true))
//!     .init();
//! ```
//!
//! ### TUI Mode (channel forwarding)
//!
//! ```rust,ignore
//! use kql_panopticon_core::tracing::{TuiLayer, tui_channel};
//! use tracing_subscriber::prelude::*;
//!
//! let (tx, rx) = tui_channel();
//! tracing_subscriber::registry()
//!     .with(TuiLayer::new(tx))
//!     .init();
//!
//! // rx receives TuiEvent instances
//! ```
//!
//! ## Span Conventions
//!
//! The executor uses these span names:
//! - `job` - Top-level pack execution (fields: job_id, pack, workspaces, steps)
//! - `phase` - Execution phase (fields: phase, workspace, steps)
//! - `step` - Individual step execution (fields: name, step_type, workspace)
//!
//! ## Event Conventions
//!
//! Special event messages trigger typed TuiEvents:
//! - `"step.completed"` with fields: rows, duration_ms
//! - `"step.failed"` with fields: error, duration_ms
//! - `"step.skipped"` with fields: reason
//! - `"foreach.progress"` with fields: current, total

mod events;
mod file_layer;
mod layer;

pub use events::{ExecutionPhase, LogLevel, TuiEvent};
pub use file_layer::FileLayer;
pub use layer::{tui_channel, TuiLayer};

// Re-export tracing macros for convenience
pub use tracing::{debug, error, info, trace, warn};
pub use tracing::{debug_span, error_span, info_span, trace_span, warn_span};
