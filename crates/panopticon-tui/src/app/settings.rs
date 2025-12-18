//! Application settings persistence

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Application settings that can be persisted
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Settings {
    /// Selected theme name
    pub theme_name: String,
    /// Force black background for theme
    #[serde(default)]
    pub override_terminal_background: bool,

    // ===== Execution Settings =====
    /// Directory for pack execution outputs
    /// Default: "./output"
    #[serde(default = "default_output_dir")]
    pub output_directory: String,

    /// Directories to search for packs (in priority order)
    /// Default: ["./packs", "~/.kql-panopticon/packs"]
    #[serde(default = "default_pack_dirs")]
    pub pack_directories: Vec<String>,
}

/// Default output directory for pack execution results
fn default_output_dir() -> String {
    "./output".to_string()
}

/// Default pack directories to search
fn default_pack_dirs() -> Vec<String> {
    let mut dirs = vec!["./packs".to_string()];

    // Add user's home directory pack location if available
    if let Some(home) = dirs::home_dir() {
        if let Some(path_str) = home.join(".kql-panopticon/packs").to_str() {
            dirs.push(path_str.to_string());
        }
    }

    dirs
}

impl Settings {
    /// Get the settings file path
    fn settings_path() -> Result<PathBuf> {
        let path = dirs::home_dir()
            .context("Failed to get home directory")?
            .join(".kql-panopticon")
            .join("settings.yaml");
        Ok(path)
    }

    /// Load settings from disk, or return defaults if file doesn't exist
    pub fn load() -> Result<Self> {
        let path = Self::settings_path()?;

        if !path.exists() {
            return Ok(Self::default());
        }

        let contents = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read settings from {:?}", path))?;

        let settings: Settings = serde_yaml::from_str(&contents)
            .with_context(|| format!("Failed to parse settings YAML: {:?}", path))?;

        Ok(settings)
    }

    /// Save settings to disk
    pub fn save(&self) -> Result<()> {
        let path = Self::settings_path()?;

        // Create parent directory if it doesn't exist
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("Failed to create settings directory: {:?}", parent))?;
            }
        }

        let contents = serde_yaml::to_string(self)
            .context("Failed to serialize settings")?;

        fs::write(&path, contents)
            .with_context(|| format!("Failed to write settings to {:?}", path))?;

        Ok(())
    }

    /// Apply these settings to the app state
    pub fn apply_to<'a>(&self, app: &mut super::App<'a>) -> Result<()> {
        // Apply theme
        app.set_theme(&self.theme_name)
            .map_err(|e| anyhow::anyhow!(e))?;

        // Apply override_terminal_background
        app.theme.override_terminal_background = self.override_terminal_background;

        Ok(())
    }

    /// Read current settings from app state
    pub fn read_from<'a>(app: &super::App<'a>) -> Self {
        Self {
            theme_name: app.theme.name.clone(),
            override_terminal_background: app.theme.override_terminal_background,
            output_directory: app.initial_settings.output_directory.clone(),
            pack_directories: app.pack_dirs.clone(),
        }
    }

    /// Check if these settings differ from another set
    pub fn has_changed_from(&self, other: &Settings) -> bool {
        self != other
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme_name: "default".to_string(),
            override_terminal_background: false,
            output_directory: default_output_dir(),
            pack_directories: default_pack_dirs(),
        }
    }
}
