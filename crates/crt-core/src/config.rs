//! Configuration management for CRT Terminal
//!
//! Supports loading configuration from:
//! - `~/.config/crt/config.toml` (XDG on Linux/macOS)
//! - Command line arguments (override)

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Shell configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ShellConfig {
    /// Shell program to run (default: $SHELL or /bin/sh)
    pub program: Option<String>,
    /// Arguments to pass to the shell
    pub args: Vec<String>,
    /// Working directory
    pub working_directory: Option<PathBuf>,
}

impl Default for ShellConfig {
    fn default() -> Self {
        Self {
            program: None,
            args: vec![],
            working_directory: None,
        }
    }
}

/// Font configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FontConfig {
    /// Primary font family
    pub family: String,
    /// Fallback font families (tried in order if glyphs are missing)
    pub fallback: Vec<String>,
    /// Font size in points
    pub size: f32,
    /// Bold font family (optional override)
    pub bold: Option<String>,
    /// Italic font family (optional override)
    pub italic: Option<String>,
    /// Bold italic font family (optional override)
    pub bold_italic: Option<String>,
}

impl Default for FontConfig {
    fn default() -> Self {
        Self {
            family: "JetBrains Mono".to_string(),
            fallback: vec![
                "Fira Code".to_string(),
                "SF Mono".to_string(),
                "Menlo".to_string(),
                "Monaco".to_string(),
                "Consolas".to_string(),
                "monospace".to_string(),
            ],
            size: 14.0,
            bold: None,
            italic: None,
            bold_italic: None,
        }
    }
}

/// Window configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WindowConfig {
    /// Initial number of columns
    pub columns: usize,
    /// Initial number of rows
    pub rows: usize,
    /// Window title
    pub title: String,
    /// Enable window decorations
    pub decorations: bool,
    /// Window opacity (0.0 - 1.0)
    pub opacity: f32,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            columns: 80,
            rows: 24,
            title: "CRT Terminal".to_string(),
            decorations: true,
            opacity: 1.0,
        }
    }
}

/// Keybinding action
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum KeyAction {
    Copy,
    Paste,
    Clear,
    ScrollUp,
    ScrollDown,
    ScrollPageUp,
    ScrollPageDown,
    ScrollToTop,
    ScrollToBottom,
    IncreaseFontSize,
    DecreaseFontSize,
    ResetFontSize,
    ToggleFullscreen,
    Quit,
    NewWindow,
    NewTab,
    CloseTab,
    NextTab,
    PreviousTab,
}

/// Key modifier flags
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct KeyModifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    #[serde(rename = "super")]
    pub logo: bool, // Command on macOS, Super/Windows on other platforms
}

/// A single keybinding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Keybinding {
    pub key: String,
    #[serde(default)]
    pub modifiers: KeyModifiers,
    pub action: KeyAction,
}

/// Keybindings configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct KeybindingsConfig {
    pub bindings: Vec<Keybinding>,
}

impl Default for KeybindingsConfig {
    fn default() -> Self {
        Self {
            bindings: vec![
                // macOS-style defaults
                Keybinding {
                    key: "c".to_string(),
                    modifiers: KeyModifiers { logo: true, ..Default::default() },
                    action: KeyAction::Copy,
                },
                Keybinding {
                    key: "v".to_string(),
                    modifiers: KeyModifiers { logo: true, ..Default::default() },
                    action: KeyAction::Paste,
                },
                Keybinding {
                    key: "k".to_string(),
                    modifiers: KeyModifiers { logo: true, ..Default::default() },
                    action: KeyAction::Clear,
                },
                Keybinding {
                    key: "plus".to_string(),
                    modifiers: KeyModifiers { logo: true, ..Default::default() },
                    action: KeyAction::IncreaseFontSize,
                },
                Keybinding {
                    key: "minus".to_string(),
                    modifiers: KeyModifiers { logo: true, ..Default::default() },
                    action: KeyAction::DecreaseFontSize,
                },
                Keybinding {
                    key: "0".to_string(),
                    modifiers: KeyModifiers { logo: true, ..Default::default() },
                    action: KeyAction::ResetFontSize,
                },
                Keybinding {
                    key: "q".to_string(),
                    modifiers: KeyModifiers { logo: true, ..Default::default() },
                    action: KeyAction::Quit,
                },
            ],
        }
    }
}

/// Complete CRT configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub shell: ShellConfig,
    pub font: FontConfig,
    pub window: WindowConfig,
    pub keybindings: KeybindingsConfig,
    /// Path to theme CSS file
    pub theme: Option<PathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            shell: ShellConfig::default(),
            font: FontConfig::default(),
            window: WindowConfig::default(),
            keybindings: KeybindingsConfig::default(),
            theme: None,
        }
    }
}

impl Config {
    /// Get the default configuration directory
    pub fn config_dir() -> Option<PathBuf> {
        dirs::config_dir().map(|p| p.join("crt"))
    }

    /// Get the default configuration file path
    pub fn config_path() -> Option<PathBuf> {
        Self::config_dir().map(|p| p.join("config.toml"))
    }

    /// Load configuration from the default location
    pub fn load() -> Self {
        Self::config_path()
            .and_then(|path| Self::load_from(&path).ok())
            .unwrap_or_default()
    }

    /// Load configuration from a specific path
    pub fn load_from(path: &PathBuf) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| ConfigError::Io(e.to_string()))?;
        toml::from_str(&content)
            .map_err(|e| ConfigError::Parse(e.to_string()))
    }

    /// Save configuration to the default location
    pub fn save(&self) -> Result<(), ConfigError> {
        let path = Self::config_path()
            .ok_or_else(|| ConfigError::Io("Could not determine config directory".to_string()))?;
        self.save_to(&path)
    }

    /// Save configuration to a specific path
    pub fn save_to(&self, path: &PathBuf) -> Result<(), ConfigError> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| ConfigError::Io(e.to_string()))?;
        }

        let content = toml::to_string_pretty(self)
            .map_err(|e| ConfigError::Serialize(e.to_string()))?;
        std::fs::write(path, content)
            .map_err(|e| ConfigError::Io(e.to_string()))
    }

    /// Generate a default config file with comments
    pub fn generate_default_config() -> String {
        r#"# CRT Terminal Configuration
# Place this file at ~/.config/crt/config.toml

[shell]
# Shell program to run (uses $SHELL if not specified)
# program = "/bin/zsh"
# args = ["-l"]
# working_directory = "~"

[font]
# Primary font family
family = "JetBrains Mono"
# Fallback fonts (tried in order if glyphs are missing)
fallback = ["Fira Code", "SF Mono", "Menlo", "Monaco", "Consolas", "monospace"]
# Font size in points
size = 14.0
# Optional font style overrides
# bold = "JetBrains Mono Bold"
# italic = "JetBrains Mono Italic"
# bold_italic = "JetBrains Mono Bold Italic"

[window]
# Initial terminal size
columns = 80
rows = 24
# Window title
title = "CRT Terminal"
# Enable window decorations
decorations = true
# Window opacity (0.0 - 1.0)
opacity = 1.0

# Theme CSS file (relative to config dir or absolute)
# theme = "themes/synthwave.css"

# Keybindings
# Supported actions: copy, paste, clear, scroll_up, scroll_down,
#   scroll_page_up, scroll_page_down, scroll_to_top, scroll_to_bottom,
#   increase_font_size, decrease_font_size, reset_font_size,
#   toggle_fullscreen, quit, new_window, new_tab, close_tab,
#   next_tab, previous_tab

[[keybindings.bindings]]
key = "c"
modifiers = { super = true }
action = "copy"

[[keybindings.bindings]]
key = "v"
modifiers = { super = true }
action = "paste"

[[keybindings.bindings]]
key = "k"
modifiers = { super = true }
action = "clear"

[[keybindings.bindings]]
key = "plus"
modifiers = { super = true }
action = "increase_font_size"

[[keybindings.bindings]]
key = "minus"
modifiers = { super = true }
action = "decrease_font_size"

[[keybindings.bindings]]
key = "0"
modifiers = { super = true }
action = "reset_font_size"

[[keybindings.bindings]]
key = "q"
modifiers = { super = true }
action = "quit"
"#.to_string()
    }
}

/// Configuration error types
#[derive(Debug)]
pub enum ConfigError {
    Io(String),
    Parse(String),
    Serialize(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::Io(e) => write!(f, "IO error: {}", e),
            ConfigError::Parse(e) => write!(f, "Parse error: {}", e),
            ConfigError::Serialize(e) => write!(f, "Serialize error: {}", e),
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
        assert_eq!(config.window.columns, 80);
        assert_eq!(config.window.rows, 24);
        assert_eq!(config.font.size, 14.0);
    }

    #[test]
    fn test_config_serialize_roundtrip() {
        let config = Config::default();
        let toml_str = toml::to_string(&config).unwrap();
        let parsed: Config = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.window.columns, config.window.columns);
        assert_eq!(parsed.font.family, config.font.family);
    }

    #[test]
    fn test_generate_default_config() {
        let default_config = Config::generate_default_config();
        assert!(default_config.contains("[shell]"));
        assert!(default_config.contains("[font]"));
        assert!(default_config.contains("[window]"));
    }
}
