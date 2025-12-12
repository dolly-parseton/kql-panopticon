//! TUI Editor Framework
//!
//! Provides a reusable full-screen editor widget with:
//! - Syntax highlighting via pluggable `Highlighter` trait
//! - Code completion popup
//! - KQL and YAML editing modes
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                         TuiEditor                               │
//! │  ┌──────────────────────────────────────────────────────────┐   │
//! │  │  Generic editor widget (tui-textarea based)              │   │
//! │  │  - Text buffer, cursor, selection, scrolling             │   │
//! │  │  - Line numbers, status bar                              │   │
//! │  │  - Completion popup rendering                            │   │
//! │  │  - Keybindings (Ctrl+D save, Esc cancel)                 │   │
//! │  └──────────────────────────────────────────────────────────┘   │
//! │         │                         │                             │
//! │         ▼                         ▼                             │
//! │  ┌─────────────┐           ┌─────────────┐                     │
//! │  │  KQL Mode   │           │  YAML Mode  │                     │
//! │  │  highlight  │           │  highlight  │                     │
//! │  │  complete   │           │  (simple)   │                     │
//! │  └─────────────┘           └─────────────┘                     │
//! └─────────────────────────────────────────────────────────────────┘
//! ```

mod completion;
mod highlight;
mod kql_mode;
mod widget;
pub mod yaml_mode;

pub use completion::{CompletionItem, CompletionItemKind, CompletionPopup, CompletionSource};
pub use highlight::{HighlightSpan, Highlighter, KqlHighlighter, YamlHighlighter};
pub use kql_mode::KqlEditorMode;
pub use widget::{EditorConfig, EditorResult, TuiEditor};
pub use yaml_mode::YamlEditorMode;
