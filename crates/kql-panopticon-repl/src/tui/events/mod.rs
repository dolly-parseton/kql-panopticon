//! TUI event system
//!
//! Provides widget events with before/after state for undo support,
//! and real-time event channels for UI updates.

mod widget_event;
pub mod tui_event;

pub use widget_event::{WidgetEvent, WidgetState};
pub use tui_event::{TuiEvent, TuiEventSender, TuiEventReceiver, channel};
