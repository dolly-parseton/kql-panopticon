//! Event logging and persistence
//!
//! Provides structured event logging for debugging, auditing, and session replay.
//! Events are stored in memory during a session and can be flushed to disk on shutdown.
//!
//! ## Event Types
//!
//! - [`ContextEvent`] - Execution lifecycle and state change events
//! - [`EventLog`] - In-memory event storage with file persistence
//!
//! ## Usage
//!
//! ```rust,ignore
//! use kql_panopticon_core::events::{EventLog, ContextEvent};
//!
//! let mut log = EventLog::new();
//! log.log(ContextEvent::ExecutionStarted {
//!     job_id: uuid::Uuid::new_v4(),
//!     step_count: 3,
//!     workspace_count: 1,
//! });
//!
//! // On shutdown
//! log.flush_to_file("session.log")?;
//! ```

mod context_event;
mod event_log;

pub use context_event::{ContextEvent, SessionEndReason};
pub use event_log::{EventLog, TimestampedEvent, EventLogConfig};
