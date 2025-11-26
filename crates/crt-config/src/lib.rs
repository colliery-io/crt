//! CRT Configuration Management
//!
//! Handles loading and managing configuration from ~/.crt/config.toml
//! Supports hot-reloading and default config generation.

pub mod keybindings;
pub mod themes;
pub mod watcher;

pub use keybindings::{Action, Key, Keybinding, Keybindings, Modifiers};
pub use themes::{BundledTheme, get_bundled_theme, bundled_theme_names};
pub use watcher::{ConfigEvent, ConfigWatcher, ConfigWatcherBuilder};

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::fs;

/// Default configuration directory name
const CONFIG_DIR_NAME: &str = ".crt";
/// Default configuration file name
const CONFIG_FILE_NAME: &str = "config.toml";
/// Default themes directory name
const THEMES_DIR_NAME: &str = "themes";

/// Tab bar position options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum TabPosition {
    #[default]
    Top,
    Bottom,
    Left,
    Right,
}

/// Cursor style options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum CursorStyle {
    #[default]
    Block,
    HollowBlock,
    Underline,
    Beam,
}

/// General configuration section
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    /// Theme name (built-in or file in ~/.crt/themes/)
    #[serde(default = "default_theme")]
    pub theme: String,

    /// Shell to spawn
    #[serde(default = "default_shell")]
    pub shell: String,
}

fn default_theme() -> String {
    "synthwave".to_string()
}

fn default_shell() -> String {
    std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string())
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            shell: default_shell(),
        }
    }
}

/// Window configuration section
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowConfig {
    /// Font size in pixels
    #[serde(default = "default_font_size")]
    pub font_size: f32,

    /// Initial terminal columns
    #[serde(default = "default_cols")]
    pub initial_cols: usize,

    /// Initial terminal rows
    #[serde(default = "default_rows")]
    pub initial_rows: usize,

    /// Padding around terminal content in pixels
    #[serde(default = "default_padding")]
    pub padding: u32,
}

fn default_font_size() -> f32 {
    14.0
}

fn default_cols() -> usize {
    80
}

fn default_rows() -> usize {
    24
}

fn default_padding() -> u32 {
    10
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            font_size: default_font_size(),
            initial_cols: default_cols(),
            initial_rows: default_rows(),
            padding: default_padding(),
        }
    }
}

/// Cursor configuration section
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorConfig {
    /// Cursor style
    #[serde(default)]
    pub style: CursorStyle,

    /// Enable cursor blinking
    #[serde(default = "default_cursor_blink")]
    pub blink: bool,

    /// Blink interval in milliseconds
    #[serde(default = "default_blink_interval")]
    pub blink_interval_ms: u32,
}

fn default_cursor_blink() -> bool {
    true
}

fn default_blink_interval() -> u32 {
    500
}

impl Default for CursorConfig {
    fn default() -> Self {
        Self {
            style: CursorStyle::default(),
            blink: default_cursor_blink(),
            blink_interval_ms: default_blink_interval(),
        }
    }
}

/// Tabs configuration section
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TabsConfig {
    /// Enable tab bar
    #[serde(default = "default_tabs_enabled")]
    pub enabled: bool,

    /// Tab bar position
    #[serde(default)]
    pub position: TabPosition,
}

fn default_tabs_enabled() -> bool {
    true
}

impl Default for TabsConfig {
    fn default() -> Self {
        Self {
            enabled: default_tabs_enabled(),
            position: TabPosition::default(),
        }
    }
}

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    /// General settings
    #[serde(default)]
    pub general: GeneralConfig,

    /// Window settings
    #[serde(default)]
    pub window: WindowConfig,

    /// Cursor settings
    #[serde(default)]
    pub cursor: CursorConfig,

    /// Tab settings
    #[serde(default)]
    pub tabs: TabsConfig,
}

impl Config {
    /// Load configuration from file, creating default if it doesn't exist
    pub fn load() -> Result<Self, ConfigError> {
        let config_path = Self::config_file_path()?;

        if !config_path.exists() {
            log::info!("Config file not found, creating default at {:?}", config_path);
            Self::create_default_config()?;
        }

        let content = fs::read_to_string(&config_path)
            .map_err(|e| ConfigError::ReadError(config_path.clone(), e))?;

        let config: Config = toml::from_str(&content)
            .map_err(|e| ConfigError::ParseError(config_path.clone(), e))?;

        log::info!("Loaded configuration from {:?}", config_path);
        Ok(config)
    }

    /// Load configuration from a specific path
    pub fn load_from(path: &PathBuf) -> Result<Self, ConfigError> {
        let content = fs::read_to_string(path)
            .map_err(|e| ConfigError::ReadError(path.clone(), e))?;

        let config: Config = toml::from_str(&content)
            .map_err(|e| ConfigError::ParseError(path.clone(), e))?;

        Ok(config)
    }

    /// Get the configuration directory path (~/.crt/)
    pub fn config_dir() -> Result<PathBuf, ConfigError> {
        let home = dirs::home_dir().ok_or(ConfigError::NoHomeDirectory)?;
        Ok(home.join(CONFIG_DIR_NAME))
    }

    /// Get the configuration file path (~/.crt/config.toml)
    pub fn config_file_path() -> Result<PathBuf, ConfigError> {
        Ok(Self::config_dir()?.join(CONFIG_FILE_NAME))
    }

    /// Get the themes directory path (~/.crt/themes/)
    pub fn themes_dir() -> Result<PathBuf, ConfigError> {
        Ok(Self::config_dir()?.join(THEMES_DIR_NAME))
    }

    /// Create the default configuration file and directory structure
    pub fn create_default_config() -> Result<(), ConfigError> {
        let config_dir = Self::config_dir()?;
        let themes_dir = Self::themes_dir()?;
        let config_path = Self::config_file_path()?;

        // Create directories
        fs::create_dir_all(&config_dir)
            .map_err(|e| ConfigError::CreateDirError(config_dir.clone(), e))?;
        fs::create_dir_all(&themes_dir)
            .map_err(|e| ConfigError::CreateDirError(themes_dir.clone(), e))?;

        // Generate default config
        let default_config = Config::default();
        let toml_content = toml::to_string_pretty(&default_config)
            .map_err(ConfigError::SerializeError)?;

        // Add header comment
        let content = format!(
            "# CRT Terminal Configuration\n\
             # https://github.com/colliery/crt\n\
             #\n\
             # Theme can be a built-in name (synthwave, minimal, dracula, solarized)\n\
             # or a filename in ~/.crt/themes/\n\
             \n\
             {toml_content}"
        );

        fs::write(&config_path, content)
            .map_err(|e| ConfigError::WriteError(config_path.clone(), e))?;

        log::info!("Created default configuration at {:?}", config_path);
        Ok(())
    }

    /// Get the path to a user theme file if it exists
    pub fn user_theme_path(&self) -> Result<Option<PathBuf>, ConfigError> {
        let themes_dir = Self::themes_dir()?;
        let theme_file = themes_dir.join(format!("{}.css", self.general.theme));

        if theme_file.exists() {
            Ok(Some(theme_file))
        } else {
            Ok(None)
        }
    }

    /// Resolve and load the theme CSS content
    ///
    /// Priority: user theme (~/.crt/themes/) > bundled theme
    pub fn resolve_theme_css(&self) -> Result<String, ConfigError> {
        // First check for user theme
        if let Some(theme_path) = self.user_theme_path()? {
            let css = fs::read_to_string(&theme_path)
                .map_err(|e| ConfigError::ReadError(theme_path.clone(), e))?;
            log::info!("Loaded user theme from {:?}", theme_path);
            return Ok(css);
        }

        // Fall back to bundled theme
        if let Some(bundled) = themes::get_bundled_theme(&self.general.theme) {
            log::info!("Using bundled theme: {}", bundled.name);
            return Ok(bundled.css.to_string());
        }

        // Default to synthwave if theme not found
        log::warn!(
            "Theme '{}' not found, falling back to synthwave",
            self.general.theme
        );
        Ok(themes::SYNTHWAVE.css.to_string())
    }
}

/// Configuration errors
#[derive(Debug)]
pub enum ConfigError {
    /// Home directory not found
    NoHomeDirectory,
    /// Failed to read config file
    ReadError(PathBuf, std::io::Error),
    /// Failed to parse config file
    ParseError(PathBuf, toml::de::Error),
    /// Failed to serialize config
    SerializeError(toml::ser::Error),
    /// Failed to write config file
    WriteError(PathBuf, std::io::Error),
    /// Failed to create directory
    CreateDirError(PathBuf, std::io::Error),
    /// Failed to set up file watcher
    WatchError(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::NoHomeDirectory => write!(f, "Could not determine home directory"),
            ConfigError::ReadError(path, e) => write!(f, "Failed to read {:?}: {}", path, e),
            ConfigError::ParseError(path, e) => write!(f, "Failed to parse {:?}: {}", path, e),
            ConfigError::SerializeError(e) => write!(f, "Failed to serialize config: {}", e),
            ConfigError::WriteError(path, e) => write!(f, "Failed to write {:?}: {}", path, e),
            ConfigError::CreateDirError(path, e) => write!(f, "Failed to create {:?}: {}", path, e),
            ConfigError::WatchError(e) => write!(f, "Failed to watch files: {}", e),
        }
    }
}

impl std::error::Error for ConfigError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.general.theme, "synthwave");
        assert_eq!(config.window.font_size, 14.0);
        assert_eq!(config.window.initial_cols, 80);
        assert_eq!(config.window.initial_rows, 24);
        assert!(config.tabs.enabled);
        assert_eq!(config.tabs.position, TabPosition::Top);
    }

    #[test]
    fn test_serialize_deserialize() {
        let config = Config::default();
        let toml_str = toml::to_string(&config).unwrap();
        let parsed: Config = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.general.theme, config.general.theme);
        assert_eq!(parsed.window.font_size, config.window.font_size);
    }

    #[test]
    fn test_partial_config() {
        let partial = r#"
            [general]
            theme = "custom"
        "#;
        let config: Config = toml::from_str(partial).unwrap();
        assert_eq!(config.general.theme, "custom");
        // Other fields should have defaults
        assert_eq!(config.window.font_size, 14.0);
    }

    #[test]
    fn test_resolve_bundled_theme() {
        let config = Config::default();
        let css = config.resolve_theme_css().unwrap();
        // Default theme is synthwave, should contain terminal styling
        assert!(css.contains(":terminal"));
    }

    #[test]
    fn test_resolve_fallback_theme() {
        let mut config = Config::default();
        config.general.theme = "nonexistent".to_string();
        // Should fall back to synthwave
        let css = config.resolve_theme_css().unwrap();
        assert!(css.contains(":terminal"));
    }
}
