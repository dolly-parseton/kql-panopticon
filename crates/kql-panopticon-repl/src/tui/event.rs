//! Event handling for the TUI application
//!
//! Currently thin wrapper around crossterm events.
//! May be extended for async notifications, background tasks, etc.

use crossterm::event::KeyEvent;

/// Application events
#[derive(Debug, Clone)]
pub enum Event {
    /// Keyboard input
    Key(KeyEvent),
    /// Terminal resize
    Resize(u16, u16),
    /// Tick (for animations, async updates)
    Tick,
}
