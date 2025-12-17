//! Theme discovery and loading

use super::types::Theme;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Manages theme discovery and loading
pub struct ThemeManager {
    themes_dir: PathBuf,
    loaded_themes: HashMap<String, Theme>,
}

impl ThemeManager {
    /// Create a new theme manager
    pub fn new() -> Result<Self> {
        let themes_dir = dirs::home_dir()
            .context("Failed to get home directory")?
            .join(".kql-panopticon")
            .join("themes");

        // Create themes directory if it doesn't exist
        if !themes_dir.exists() {
            fs::create_dir_all(&themes_dir)
                .context("Failed to create themes directory")?;
        }

        Ok(Self {
            themes_dir,
            loaded_themes: HashMap::new(),
        })
    }

    /// Discover all available themes (built-in + user)
    pub fn discover(&mut self) -> Result<Vec<String>> {
        self.loaded_themes.clear();

        // Load built-in themes
        for name in Theme::builtin_names() {
            if let Some(theme) = Theme::builtin(name) {
                self.loaded_themes.insert(name.to_string(), theme);
            }
        }

        // Discover user themes from YAML files
        if self.themes_dir.exists() {
            for entry in fs::read_dir(&self.themes_dir)? {
                let entry = entry?;
                let path = entry.path();

                // Only process .yaml and .yml files
                if let Some(ext) = path.extension() {
                    if ext == "yaml" || ext == "yml" {
                        match self.load_theme_file(&path) {
                            Ok(theme) => {
                                let name = theme.name.clone();
                                self.loaded_themes.insert(name, theme);
                            }
                            Err(e) => {
                                log::warn!("Failed to load theme from {:?}: {}", path, e);
                            }
                        }
                    }
                }
            }
        }

        let mut names: Vec<String> = self.loaded_themes.keys().cloned().collect();
        names.sort();
        Ok(names)
    }

    /// Load a theme from a YAML file
    fn load_theme_file(&self, path: &PathBuf) -> Result<Theme> {
        let contents = fs::read_to_string(path)
            .with_context(|| format!("Failed to read theme file: {:?}", path))?;

        let theme: Theme = serde_yaml::from_str(&contents)
            .with_context(|| format!("Failed to parse theme YAML: {:?}", path))?;

        Ok(theme)
    }

    /// Get a theme by name
    pub fn get(&self, name: &str) -> Option<&Theme> {
        self.loaded_themes.get(name)
    }

    /// List all available theme names
    pub fn list(&self) -> Vec<String> {
        let mut names: Vec<String> = self.loaded_themes.keys().cloned().collect();
        names.sort();
        names
    }

    /// Get themes grouped by category
    pub fn list_by_category(&self) -> HashMap<super::types::ThemeCategory, Vec<String>> {
        use super::types::ThemeCategory;
        let mut grouped: HashMap<ThemeCategory, Vec<String>> = HashMap::new();

        for (name, theme) in &self.loaded_themes {
            grouped
                .entry(theme.category)
                .or_insert_with(Vec::new)
                .push(name.clone());
        }

        // Sort theme names within each category
        for themes in grouped.values_mut() {
            themes.sort();
        }

        grouped
    }

    /// Check if a theme exists
    pub fn exists(&self, name: &str) -> bool {
        self.loaded_themes.contains_key(name)
    }

    /// Get the themes directory path
    pub fn themes_dir(&self) -> &PathBuf {
        &self.themes_dir
    }

    /// Create a sample theme file for users to customize
    pub fn create_sample_theme(&self) -> Result<()> {
        let sample_path = self.themes_dir.join("sample.yaml");

        if sample_path.exists() {
            return Ok(());
        }

        let sample = r#"name: sample
category: other  # Options: light, dark, other

# UI Text customization
ui_text:
  app_name: "kql-panopticon"        # Application name
  title_format: "{app} ({view})"    # Title template: {app} and {view} are replaced
                                    # Examples: "{app} > {view}", "{app} - {view}", "{app} | {view}"

  # Title component colors (optional - omit to use defaults)
  # title_app_color: cyan           # Color for app name (default: text color)
  # title_separator_color: gray     # Color for separators (default: text_dim)
  # title_view_color: yellow        # Color for view name (default: accent)

colors:
  # Core colors
  primary:
    r: 100
    g: 200
    b: 255
  secondary:
    r: 200
    g: 100
    b: 255
  accent:
    r: 255
    g: 200
    b: 100
  background: black
  text: white
  text_dim: gray

  # Status colors
  success: green
  error: red
  warning: yellow

  # Per-element colors (optional - will use defaults if omitted)
  prompt_border: cyan           # Color for input prompt border
  modal_overlay: black          # Background color for modal windows
  cursor: white                 # Text cursor color
  selection: blue               # Text selection highlight color

  # Command syntax highlighting (optional - will use defaults if omitted)
  command_prefix: cyan          # Color for ':' prefix (default: cyan)
  command_name: yellow          # Color for command name like 'ws', 'theme' (default: yellow)

styles:
  border_type: rounded  # Options: rounded, square, thick, double
  bold_borders: false

  # Spinner/animation configuration (optional)
  spinners:
    speed_ms: 100  # Milliseconds per frame (100 = 10fps)

    # Running status spinner (animated)
    running:
      frames:
        - "⠋"
        - "⠙"
        - "⠹"
        - "⠸"
        - "⠼"
        - "⠴"
        - "⠦"
        - "⠧"
        - "⠇"
        - "⠏"
      # Optional: custom color for each frame (omit to use status color)
      # colors:
      #   - { r: 255, g: 100, b: 100 }
      #   - { r: 255, g: 150, b: 100 }
      #   - yellow

    # Completed status icon (static)
    completed: "✓"

    # Failed status icon (static)
    failed: "✗"

# Example spinner alternatives:
# Dots: ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]
# Arc: ["◜", "◠", "◝", "◞", "◡", "◟"]
# Braille: ["⣾", "⣽", "⣻", "⢿", "⡿", "⣟", "⣯", "⣷"]
# Clock: ["🕐", "🕑", "🕒", "🕓", "🕔", "🕕", "🕖", "🕗", "🕘", "🕙", "🕚", "🕛"]
# Arrows: ["←", "↖", "↑", "↗", "→", "↘", "↓", "↙"]
"#;

        fs::write(&sample_path, sample)
            .with_context(|| format!("Failed to write sample theme: {:?}", sample_path))?;

        Ok(())
    }
}

impl Default for ThemeManager {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| Self {
            themes_dir: PathBuf::from("."),
            loaded_themes: HashMap::new(),
        })
    }
}
