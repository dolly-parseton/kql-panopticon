//! Theme system for the TUI

mod builtin;
mod manager;
mod types;

pub use manager::ThemeManager;
pub use types::{
    BorderType, SerializableColor, Theme, ThemeCategory, ThemeColors, ThemeStyles, UiText,
};
