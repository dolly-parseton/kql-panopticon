//! Theme type definitions

use ratatui::style::{Color, Modifier, Style};
use serde::{Deserialize, Serialize};

/// Theme category for organization
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeCategory {
    Light,
    Dark,
    Other,
}

impl ThemeCategory {
    /// Get all categories in consistent display order
    pub const fn all() -> [Self; 3] {
        [Self::Dark, Self::Light, Self::Other]
    }

    /// Get category identifier for tree/selection purposes
    pub fn id(&self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
            Self::Other => "other",
        }
    }
}

impl Default for ThemeCategory {
    fn default() -> Self {
        Self::Dark
    }
}

impl std::fmt::Display for ThemeCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Dark => write!(f, "Dark Themes"),
            Self::Light => write!(f, "Light Themes"),
            Self::Other => write!(f, "Other Themes"),
        }
    }
}

/// Customizable UI text elements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiText {
    /// Application name shown in title
    #[serde(default = "default_app_name")]
    pub app_name: String,
    /// Title format template. Use {app} for app name, {view} for current view.
    /// Examples: "{app} ({view})", "{app} > {view}", "{app} - {view}"
    #[serde(default = "default_title_format")]
    pub title_format: String,

    // Title component colors (None = use theme text color)
    /// Color for the app name portion
    #[serde(default)]
    pub title_app_color: Option<SerializableColor>,
    /// Color for separator characters (everything between {app} and {view})
    #[serde(default)]
    pub title_separator_color: Option<SerializableColor>,
    /// Color for the current view portion
    #[serde(default)]
    pub title_view_color: Option<SerializableColor>,
}

/// A color theme for the TUI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
    pub name: String,
    #[serde(default)]
    pub category: ThemeCategory,
    pub colors: ThemeColors,
    #[serde(default)]
    pub styles: ThemeStyles,
    #[serde(default)]
    pub ui_text: UiText,
    /// Override terminal background with theme's background color (converted to RGB)
    #[serde(default)]
    pub override_terminal_background: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeColors {
    // Core colors
    pub primary: SerializableColor,
    pub secondary: SerializableColor,
    pub accent: SerializableColor,
    pub background: SerializableColor,
    pub text: SerializableColor,
    pub text_dim: SerializableColor,

    // Status colors
    pub success: SerializableColor,
    pub error: SerializableColor,
    pub warning: SerializableColor,

    // Per-element colors
    #[serde(default = "default_prompt_border_color")]
    pub prompt_border: SerializableColor,

    #[serde(default = "default_modal_overlay_color")]
    pub modal_overlay: SerializableColor,

    #[serde(default = "default_cursor_color")]
    pub cursor: SerializableColor,

    #[serde(default = "default_selection_color")]
    pub selection: SerializableColor,

    // Command syntax colors
    #[serde(default = "default_command_prefix_color")]
    pub command_prefix: SerializableColor,

    #[serde(default = "default_command_name_color")]
    pub command_name: SerializableColor,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeStyles {
    #[serde(default = "default_border_type")]
    pub border_type: BorderType,
    #[serde(default)]
    pub bold_borders: bool,
    #[serde(default)]
    pub spinners: SpinnerConfig,
}

/// Spinner animation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpinnerConfig {
    /// Animation speed in milliseconds per frame
    #[serde(default = "default_spinner_speed")]
    pub speed_ms: u64,

    /// Running status spinner
    #[serde(default = "default_running_spinner")]
    pub running: SpinnerFrames,

    /// Completed status icon
    #[serde(default = "default_completed_icon")]
    pub completed: String,

    /// Failed status icon
    #[serde(default = "default_failed_icon")]
    pub failed: String,
}

/// Animation frames for a spinner
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpinnerFrames {
    /// Character frames to cycle through
    pub frames: Vec<String>,
    /// Optional color for each frame (if omitted, uses status color)
    #[serde(default)]
    pub colors: Vec<Option<SerializableColor>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BorderType {
    Rounded,
    Square,
    Thick,
    Double,
}

/// Color wrapper for serde support
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SerializableColor {
    Named(String),
    Rgb { r: u8, g: u8, b: u8 },
}

impl From<SerializableColor> for Color {
    fn from(color: SerializableColor) -> Self {
        match color {
            SerializableColor::Named(name) => match name.to_lowercase().as_str() {
                "black" => Color::Black,
                "red" => Color::Red,
                "green" => Color::Green,
                "yellow" => Color::Yellow,
                "blue" => Color::Blue,
                "magenta" => Color::Magenta,
                "cyan" => Color::Cyan,
                "gray" | "grey" => Color::Gray,
                "darkgray" | "darkgrey" => Color::DarkGray,
                "lightred" => Color::LightRed,
                "lightgreen" => Color::LightGreen,
                "lightyellow" => Color::LightYellow,
                "lightblue" => Color::LightBlue,
                "lightmagenta" => Color::LightMagenta,
                "lightcyan" => Color::LightCyan,
                "white" => Color::White,
                _ => Color::White, // Fallback
            },
            SerializableColor::Rgb { r, g, b } => Color::Rgb(r, g, b),
        }
    }
}

impl SerializableColor {
    /// Convert to RGB Color, using standard xterm values for named colors.
    /// This ensures the color cannot be overridden by terminal themes.
    pub fn to_rgb(&self) -> Color {
        match self {
            SerializableColor::Rgb { r, g, b } => Color::Rgb(*r, *g, *b),
            SerializableColor::Named(name) => match name.to_lowercase().as_str() {
                "black" => Color::Rgb(0, 0, 0),
                "red" => Color::Rgb(205, 0, 0),
                "green" => Color::Rgb(0, 205, 0),
                "yellow" => Color::Rgb(205, 205, 0),
                "blue" => Color::Rgb(0, 0, 238),
                "magenta" => Color::Rgb(205, 0, 205),
                "cyan" => Color::Rgb(0, 205, 205),
                "gray" | "grey" => Color::Rgb(128, 128, 128),
                "darkgray" | "darkgrey" => Color::Rgb(85, 85, 85),
                "lightred" => Color::Rgb(255, 0, 0),
                "lightgreen" => Color::Rgb(0, 255, 0),
                "lightyellow" => Color::Rgb(255, 255, 0),
                "lightblue" => Color::Rgb(92, 92, 255),
                "lightmagenta" => Color::Rgb(255, 0, 255),
                "lightcyan" => Color::Rgb(0, 255, 255),
                "white" => Color::Rgb(229, 229, 229),
                _ => Color::Rgb(229, 229, 229), // Fallback to white
            },
        }
    }
}

impl Theme {
    /// Create a style with the given color and optional modifiers
    fn color_style(&self, color: &SerializableColor, modifiers: Modifier) -> Style {
        Style::default()
            .fg(color.clone().into())
            .add_modifier(modifiers)
    }

    /// Create a border style with optional bold
    fn border_color_style(&self, color: &SerializableColor) -> Style {
        let mut style = Style::default().fg(color.clone().into());
        if self.styles.bold_borders {
            style = style.add_modifier(Modifier::BOLD);
        }
        style
    }

    /// Get style for block status
    pub fn status_style(&self, status: &crate::app::BlockStatus) -> Style {
        let color = match status {
            crate::app::BlockStatus::Running => &self.colors.warning,
            crate::app::BlockStatus::Completed => &self.colors.success,
            crate::app::BlockStatus::Failed => &self.colors.error,
        };
        self.color_style(color, Modifier::BOLD)
    }

    /// Get style for borders
    pub fn border_style(&self) -> Style {
        self.border_color_style(&self.colors.primary)
    }

    /// Get style for highlighted/selected items
    pub fn highlight_style(&self) -> Style {
        Style::default()
            .bg(self.colors.primary.clone().into())
            .fg(self.colors.background.clone().into())
            .add_modifier(Modifier::BOLD)
    }

    /// Get style for modal background
    pub fn modal_style(&self) -> Style {
        Style::default()
            .bg(self.background_color()) // Respects force_black_background
            .fg(self.colors.text.clone().into())
    }

    /// Get style for text
    pub fn text_style(&self) -> Style {
        self.color_style(&self.colors.text, Modifier::empty())
    }

    /// Get style for dimmed text
    pub fn text_dim_style(&self) -> Style {
        self.color_style(&self.colors.text_dim, Modifier::empty())
    }

    /// Get border type for ratatui
    pub fn border_type(&self) -> ratatui::widgets::BorderType {
        match self.styles.border_type {
            BorderType::Rounded => ratatui::widgets::BorderType::Rounded,
            BorderType::Square => ratatui::widgets::BorderType::Plain,
            BorderType::Thick => ratatui::widgets::BorderType::Thick,
            BorderType::Double => ratatui::widgets::BorderType::Double,
        }
    }

    /// Get style for prompt/input border
    pub fn prompt_border_style(&self) -> Style {
        self.border_color_style(&self.colors.prompt_border)
    }

    /// Get style for modal overlay
    pub fn modal_overlay_style(&self) -> Style {
        let bg = if self.override_terminal_background {
            self.colors.modal_overlay.to_rgb()
        } else {
            self.colors.modal_overlay.clone().into()
        };
        Style::default()
            .bg(bg)
            .fg(self.colors.text.clone().into())
    }

    /// Get style for cursor
    pub fn cursor_style(&self) -> Style {
        Style::default()
            .bg(self.colors.cursor.clone().into())
            .fg(self.colors.background.clone().into())
    }

    /// Get style for text selection
    pub fn selection_style(&self) -> Style {
        Style::default()
            .bg(self.colors.selection.clone().into())
            .fg(self.colors.text.clone().into())
            .add_modifier(Modifier::BOLD)
    }

    /// Get effective background color (respects override_terminal_background)
    pub fn background_color(&self) -> Color {
        if self.override_terminal_background {
            self.colors.background.to_rgb()
        } else {
            self.colors.background.clone().into()
        }
    }

    /// Get style for main background areas
    pub fn background_style(&self) -> Style {
        Style::default()
            .bg(self.background_color())
            .fg(self.colors.text.clone().into())
    }

    /// Format title using the theme's title template
    ///
    /// Replaces {app} with app name and {view} with the provided view name.
    /// If view is empty, returns just the app name without formatting.
    pub fn format_title(&self, view: &str) -> String {
        if view.is_empty() {
            self.ui_text.app_name.clone()
        } else {
            self.ui_text.title_format
                .replace("{app}", &self.ui_text.app_name)
                .replace("{view}", view)
        }
    }

    /// Get style for command prefix (`:`)
    pub fn command_prefix_style(&self) -> Style {
        self.color_style(&self.colors.command_prefix, Modifier::empty())
    }

    /// Get style for command name (`ws`, `theme`, etc.)
    pub fn command_name_style(&self) -> Style {
        self.color_style(&self.colors.command_name, Modifier::empty())
    }

    /// Format command text with syntax highlighting
    ///
    /// Highlights colon commands like `:ws` or `:theme solarized-dark`:
    /// - `:` (prefix) gets command_prefix color
    /// - `ws`/`theme` (command name) gets command_name color
    /// - Arguments remain in regular text color
    ///
    /// Highlights let statements like `let name: input = value`:
    /// - `let` keyword gets command_name color
    /// - variable name gets accent color
    /// - `: input =` gets text_dim color
    /// - value gets regular text color
    ///
    /// Returns a Vec of Span elements for rendering
    pub fn highlight_command(&self, command: &str) -> Vec<ratatui::text::Span<'static>> {
        use ratatui::text::Span;

        let command = command.trim();

        // Check for let statement
        if command.starts_with("let ") {
            return self.highlight_let_statement(command);
        }

        if !command.starts_with(':') {
            // Not a command, return as-is with text color
            return vec![Span::styled(
                command.to_string(),
                self.text_style(),
            )];
        }

        let mut spans = Vec::new();

        // Find where the command name ends (space or end of string)
        let command_end = command
            .find(char::is_whitespace)
            .unwrap_or(command.len());

        // Split into prefix (:), command name, and rest
        let prefix = &command[0..1]; // ":"
        let cmd_name = &command[1..command_end]; // "ws" or "theme"
        let rest = if command_end < command.len() {
            &command[command_end..]
        } else {
            ""
        };

        // Add colored prefix
        spans.push(Span::styled(
            prefix.to_string(),
            self.command_prefix_style(),
        ));

        // Add colored command name
        spans.push(Span::styled(
            cmd_name.to_string(),
            self.command_name_style(),
        ));

        // Add rest (arguments) in regular text color
        if !rest.is_empty() {
            spans.push(Span::styled(
                rest.to_string(),
                self.text_style(),
            ));
        }

        spans
    }

    /// Highlight a let statement: let {name}: input = {value}
    fn highlight_let_statement(&self, command: &str) -> Vec<ratatui::text::Span<'static>> {
        use ratatui::text::Span;

        let mut spans = Vec::new();

        // "let " keyword
        spans.push(Span::styled(
            "let ".to_string(),
            self.command_name_style(),
        ));

        let rest = &command[4..]; // Skip "let "

        // Find the colon
        if let Some(colon_pos) = rest.find(':') {
            let name = &rest[..colon_pos];

            // Variable name in accent color
            spans.push(Span::styled(
                name.to_string(),
                Style::default().fg(self.colors.accent.clone().into()),
            ));

            let after_name = &rest[colon_pos..];

            // Find the equals sign
            if let Some(eq_pos) = after_name.find('=') {
                // ": type =" in dim color
                let type_part = &after_name[..=eq_pos];
                spans.push(Span::styled(
                    type_part.to_string(),
                    self.text_dim_style(),
                ));

                // Value in regular text color
                let value = &after_name[eq_pos + 1..];
                if !value.is_empty() {
                    spans.push(Span::styled(
                        value.to_string(),
                        self.text_style(),
                    ));
                }
            } else {
                // No equals sign, show rest in dim (error state)
                spans.push(Span::styled(
                    after_name.to_string(),
                    self.text_dim_style(),
                ));
            }
        } else {
            // No colon, show rest in text color (incomplete/error)
            spans.push(Span::styled(
                rest.to_string(),
                self.text_style(),
            ));
        }

        spans
    }

    // ===== Status Bar / Connection Status Methods =====

    /// Get spinner frame for running status
    pub fn get_spinner_frame(&self, frame_idx: usize) -> &str {
        let frames = &self.styles.spinners.running.frames;
        if frames.is_empty() {
            return "⠋"; // Fallback
        }
        &frames[frame_idx % frames.len()]
    }

    /// Get spinner color for a specific frame (if defined), otherwise None
    pub fn get_spinner_color(&self, frame_idx: usize) -> Option<Color> {
        let colors = &self.styles.spinners.running.colors;
        if colors.is_empty() {
            return None;
        }
        colors
            .get(frame_idx % colors.len())
            .and_then(|c| c.clone().map(|sc| sc.into()))
    }

    /// Get completed status icon
    pub fn completed_icon(&self) -> &str {
        &self.styles.spinners.completed
    }

    /// Get failed status icon
    pub fn failed_icon(&self) -> &str {
        &self.styles.spinners.failed
    }

    /// Get style for running/in-progress status (uses warning color or spinner frame color)
    pub fn running_status_style(&self, frame_idx: usize) -> Style {
        let color = self
            .get_spinner_color(frame_idx)
            .unwrap_or_else(|| self.colors.warning.clone().into());
        Style::default().fg(color)
    }

    /// Get style for success/completed status
    pub fn success_status_style(&self) -> Style {
        Style::default().fg(self.colors.success.clone().into())
    }

    /// Get style for error/failed status
    pub fn error_status_style(&self) -> Style {
        Style::default().fg(self.colors.error.clone().into())
    }

    /// Format title as styled spans with color-coded components
    ///
    /// Returns a Vec of Span elements with different colors for:
    /// - App name (title_app_color or text color)
    /// - Separator text (title_separator_color or text_dim)
    /// - View name (title_view_color or accent color)
    pub fn format_title_styled(&self, view: &str) -> Vec<ratatui::text::Span<'static>> {
        use ratatui::text::Span;

        if view.is_empty() {
            let app_color = self.ui_text.title_app_color
                .clone()
                .map(|c| c.into())
                .unwrap_or_else(|| self.colors.text.clone().into());
            return vec![Span::styled(
                self.ui_text.app_name.clone(),
                Style::default().fg(app_color),
            )];
        }

        let mut spans = Vec::new();
        let template = &self.ui_text.title_format;

        // Colors for each component
        let app_color = self.ui_text.title_app_color
            .clone()
            .map(|c| c.into())
            .unwrap_or_else(|| self.colors.text.clone().into());

        let separator_color = self.ui_text.title_separator_color
            .clone()
            .map(|c| c.into())
            .unwrap_or_else(|| self.colors.text_dim.clone().into());

        let view_color = self.ui_text.title_view_color
            .clone()
            .map(|c| c.into())
            .unwrap_or_else(|| self.colors.accent.clone().into());

        // Parse the template and build spans
        let mut remaining = template.as_str();

        if let Some(app_pos) = remaining.find("{app}") {
            // Text before {app} is separator
            if app_pos > 0 {
                spans.push(Span::styled(
                    remaining[..app_pos].to_string(),
                    Style::default().fg(separator_color),
                ));
            }

            // Add app name
            spans.push(Span::styled(
                self.ui_text.app_name.clone(),
                Style::default().fg(app_color),
            ));

            remaining = &remaining[app_pos + 5..]; // Skip "{app}"

            // Check for {view}
            if let Some(view_pos) = remaining.find("{view}") {
                // Text between {app} and {view} is separator
                if view_pos > 0 {
                    spans.push(Span::styled(
                        remaining[..view_pos].to_string(),
                        Style::default().fg(separator_color),
                    ));
                }

                // Add view name
                spans.push(Span::styled(
                    view.to_string(),
                    Style::default().fg(view_color),
                ));

                remaining = &remaining[view_pos + 6..]; // Skip "{view}"

                // Any text after {view} is separator
                if !remaining.is_empty() {
                    spans.push(Span::styled(
                        remaining.to_string(),
                        Style::default().fg(separator_color),
                    ));
                }
            } else {
                // No {view} found, rest is separator
                if !remaining.is_empty() {
                    spans.push(Span::styled(
                        remaining.to_string(),
                        Style::default().fg(separator_color),
                    ));
                }
            }
        }

        // Handle case where template doesn't contain {app}
        if spans.is_empty() && remaining.contains("{view}") {
            if let Some(view_pos) = remaining.find("{view}") {
                if view_pos > 0 {
                    spans.push(Span::styled(
                        remaining[..view_pos].to_string(),
                        Style::default().fg(separator_color),
                    ));
                }
                spans.push(Span::styled(
                    view.to_string(),
                    Style::default().fg(view_color),
                ));
                let after = &remaining[view_pos + 6..];
                if !after.is_empty() {
                    spans.push(Span::styled(
                        after.to_string(),
                        Style::default().fg(separator_color),
                    ));
                }
            }
        }

        spans
    }
}

impl Default for ThemeStyles {
    fn default() -> Self {
        Self {
            border_type: BorderType::Rounded,
            bold_borders: false,
            spinners: SpinnerConfig::default(),
        }
    }
}

impl Default for SpinnerConfig {
    fn default() -> Self {
        Self {
            speed_ms: default_spinner_speed(),
            running: default_running_spinner(),
            completed: default_completed_icon(),
            failed: default_failed_icon(),
        }
    }
}

fn default_border_type() -> BorderType {
    BorderType::Rounded
}

fn default_prompt_border_color() -> SerializableColor {
    SerializableColor::Named("cyan".to_string())
}

fn default_modal_overlay_color() -> SerializableColor {
    SerializableColor::Named("black".to_string())
}

fn default_cursor_color() -> SerializableColor {
    SerializableColor::Named("white".to_string())
}

fn default_selection_color() -> SerializableColor {
    SerializableColor::Named("blue".to_string())
}

fn default_command_prefix_color() -> SerializableColor {
    SerializableColor::Named("cyan".to_string())
}

fn default_command_name_color() -> SerializableColor {
    SerializableColor::Named("yellow".to_string())
}

fn default_spinner_speed() -> u64 {
    100 // 100ms per frame = 10fps
}

fn default_running_spinner() -> SpinnerFrames {
    SpinnerFrames {
        frames: vec![
            "⠋".to_string(),
            "⠙".to_string(),
            "⠹".to_string(),
            "⠸".to_string(),
            "⠼".to_string(),
            "⠴".to_string(),
            "⠦".to_string(),
            "⠧".to_string(),
            "⠇".to_string(),
            "⠏".to_string(),
        ],
        colors: vec![],
    }
}

fn default_completed_icon() -> String {
    "✓".to_string()
}

fn default_failed_icon() -> String {
    "✗".to_string()
}

fn default_app_name() -> String {
    "kql-panopticon".to_string()
}

fn default_title_format() -> String {
    "{app} ({view})".to_string()
}

impl Default for UiText {
    fn default() -> Self {
        Self {
            app_name: default_app_name(),
            title_format: default_title_format(),
            title_app_color: None,
            title_separator_color: None,
            title_view_color: None,
        }
    }
}
