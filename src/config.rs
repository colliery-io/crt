//! Configuration management for CRT terminal
//!
//! Loads config from ~/.config/crt/config.toml with sensible defaults.
//! Paths can be overridden via:
//! - `CRT_CONFIG_DIR` environment variable
//! - `ConfigPaths` for programmatic control (useful for testing)

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Configuration paths that can be overridden for testing
#[derive(Debug, Clone)]
pub struct ConfigPaths {
    /// Base directory for all config files
    pub config_dir: PathBuf,
}

impl ConfigPaths {
    /// Create from a specific directory
    pub fn new(config_dir: PathBuf) -> Self {
        Self { config_dir }
    }

    /// Get paths from environment or default
    pub fn from_env_or_default() -> Option<Self> {
        // First check environment variable
        if let Ok(dir) = std::env::var("CRT_CONFIG_DIR") {
            let path = PathBuf::from(dir);
            if path.is_absolute() {
                return Some(Self::new(path));
            }
            log::warn!(
                "CRT_CONFIG_DIR must be an absolute path, ignoring: {:?}",
                path
            );
        }

        // Fall back to default
        dirs::home_dir().map(|p| Self::new(p.join(".config").join("crt")))
    }

    /// Get the config file path
    pub fn config_path(&self) -> PathBuf {
        self.config_dir.join("config.toml")
    }

    /// Get the themes directory path
    pub fn themes_dir(&self) -> PathBuf {
        self.config_dir.join("themes")
    }

    /// Get a specific theme file path
    pub fn theme_path(&self, theme_name: &str) -> PathBuf {
        self.themes_dir().join(format!("{}.css", theme_name))
    }
}

/// Shell configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ShellConfig {
    /// Shell program to run (default: user's login shell or /bin/zsh)
    pub program: Option<String>,
    /// Arguments to pass to shell
    pub args: Vec<String>,
    /// Working directory (default: user's home)
    pub working_directory: Option<PathBuf>,
}

impl Default for ShellConfig {
    fn default() -> Self {
        Self {
            program: None, // Will use login shell
            args: vec![],
            working_directory: None,
        }
    }
}

/// Font configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FontConfig {
    /// Font family names in order of preference (fallback chain)
    /// First available font will be used; falls back to embedded font if none found
    pub family: Vec<String>,
    /// Base font size in points
    pub size: f32,
    /// Line height multiplier
    pub line_height: f32,
}

impl Default for FontConfig {
    fn default() -> Self {
        Self {
            family: vec![
                "MesloLGS NF".to_string(),
                "JetBrains Mono".to_string(),
                "Fira Code".to_string(),
                "SF Mono".to_string(),
                "Menlo".to_string(),
            ],
            size: 14.0,
            line_height: 1.5,
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
    /// Start in fullscreen mode
    pub fullscreen: bool,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            columns: 80,
            rows: 24,
            title: "CRT Terminal".to_string(),
            fullscreen: false,
        }
    }
}

/// Theme configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ThemeConfig {
    /// Theme name (looks for ~/.config/crt/themes/{name}.css)
    pub name: String,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            name: "synthwave".to_string(),
        }
    }
}

/// Cursor shape style
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CursorStyle {
    #[default]
    Block,
    Bar,
    Underline,
}

/// Cursor configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CursorConfig {
    /// Cursor shape style
    pub style: CursorStyle,
    /// Whether cursor blinks
    pub blink: bool,
    /// Blink interval in milliseconds
    pub blink_interval_ms: u64,
}

impl Default for CursorConfig {
    fn default() -> Self {
        Self {
            style: CursorStyle::default(),
            blink: true,
            blink_interval_ms: 530,
        }
    }
}

/// Bell configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct BellConfig {
    /// Enable visual bell (screen flash)
    pub visual: bool,
    /// Flash duration in milliseconds
    pub flash_duration_ms: u64,
    /// Flash intensity (0.0 to 1.0)
    pub flash_intensity: f32,
}

impl Default for BellConfig {
    fn default() -> Self {
        Self {
            visual: true,
            flash_duration_ms: 100,
            flash_intensity: 0.3,
        }
    }
}

/// Keybinding action
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum KeyAction {
    NewTab,
    CloseTab,
    NextTab,
    PrevTab,
    SelectTab1,
    SelectTab2,
    SelectTab3,
    SelectTab4,
    SelectTab5,
    SelectTab6,
    SelectTab7,
    SelectTab8,
    SelectTab9,
    IncreaseFontSize,
    DecreaseFontSize,
    ResetFontSize,
    ToggleFullscreen,
    Copy,
    Paste,
    Quit,
}

/// Single keybinding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Keybinding {
    /// Key (e.g., "t", "w", "1", "equal", "minus")
    pub key: String,
    /// Modifiers (e.g., ["super"], ["super", "shift"])
    #[serde(default)]
    pub mods: Vec<String>,
    /// Action to perform
    pub action: KeyAction,
}

/// Keybindings configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct KeybindingsConfig {
    /// List of keybindings
    pub bindings: Vec<Keybinding>,
}

impl Default for KeybindingsConfig {
    fn default() -> Self {
        Self {
            bindings: vec![
                // Tab management
                Keybinding {
                    key: "t".to_string(),
                    mods: vec!["super".to_string()],
                    action: KeyAction::NewTab,
                },
                Keybinding {
                    key: "w".to_string(),
                    mods: vec!["super".to_string()],
                    action: KeyAction::CloseTab,
                },
                Keybinding {
                    key: "[".to_string(),
                    mods: vec!["super".to_string(), "shift".to_string()],
                    action: KeyAction::PrevTab,
                },
                Keybinding {
                    key: "]".to_string(),
                    mods: vec!["super".to_string(), "shift".to_string()],
                    action: KeyAction::NextTab,
                },
                // Tab selection
                Keybinding {
                    key: "1".to_string(),
                    mods: vec!["super".to_string()],
                    action: KeyAction::SelectTab1,
                },
                Keybinding {
                    key: "2".to_string(),
                    mods: vec!["super".to_string()],
                    action: KeyAction::SelectTab2,
                },
                Keybinding {
                    key: "3".to_string(),
                    mods: vec!["super".to_string()],
                    action: KeyAction::SelectTab3,
                },
                Keybinding {
                    key: "4".to_string(),
                    mods: vec!["super".to_string()],
                    action: KeyAction::SelectTab4,
                },
                Keybinding {
                    key: "5".to_string(),
                    mods: vec!["super".to_string()],
                    action: KeyAction::SelectTab5,
                },
                Keybinding {
                    key: "6".to_string(),
                    mods: vec!["super".to_string()],
                    action: KeyAction::SelectTab6,
                },
                Keybinding {
                    key: "7".to_string(),
                    mods: vec!["super".to_string()],
                    action: KeyAction::SelectTab7,
                },
                Keybinding {
                    key: "8".to_string(),
                    mods: vec!["super".to_string()],
                    action: KeyAction::SelectTab8,
                },
                Keybinding {
                    key: "9".to_string(),
                    mods: vec!["super".to_string()],
                    action: KeyAction::SelectTab9,
                },
                // Font size
                Keybinding {
                    key: "equal".to_string(),
                    mods: vec!["super".to_string()],
                    action: KeyAction::IncreaseFontSize,
                },
                Keybinding {
                    key: "minus".to_string(),
                    mods: vec!["super".to_string()],
                    action: KeyAction::DecreaseFontSize,
                },
                Keybinding {
                    key: "0".to_string(),
                    mods: vec!["super".to_string()],
                    action: KeyAction::ResetFontSize,
                },
                // Other
                Keybinding {
                    key: "q".to_string(),
                    mods: vec!["super".to_string()],
                    action: KeyAction::Quit,
                },
                Keybinding {
                    key: "c".to_string(),
                    mods: vec!["super".to_string()],
                    action: KeyAction::Copy,
                },
                Keybinding {
                    key: "v".to_string(),
                    mods: vec!["super".to_string()],
                    action: KeyAction::Paste,
                },
            ],
        }
    }
}

/// Complete configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Config {
    pub shell: ShellConfig,
    pub font: FontConfig,
    pub window: WindowConfig,
    pub theme: ThemeConfig,
    pub cursor: CursorConfig,
    pub bell: BellConfig,
    pub keybindings: KeybindingsConfig,
}

impl Config {
    /// Get the config directory path (~/.config/crt or from CRT_CONFIG_DIR)
    pub fn config_dir() -> Option<PathBuf> {
        ConfigPaths::from_env_or_default().map(|p| p.config_dir)
    }

    /// Load config from default path (respects CRT_CONFIG_DIR env var)
    pub fn load() -> Self {
        match ConfigPaths::from_env_or_default() {
            Some(paths) => Self::load_with_paths(&paths),
            None => {
                log::info!("Could not determine config path, using defaults");
                Self::default()
            }
        }
    }

    /// Load config from a specific file path
    pub fn load_from(path: &Path) -> Self {
        if !path.exists() {
            log::info!("Config file not found at {:?}, using defaults", path);
            return Self::default();
        }

        match std::fs::read_to_string(path) {
            Ok(contents) => match toml::from_str(&contents) {
                Ok(config) => {
                    log::info!("Loaded config from {:?}", path);
                    config
                }
                Err(e) => {
                    log::warn!("Failed to parse config {:?}: {}, using defaults", path, e);
                    Self::default()
                }
            },
            Err(e) => {
                log::warn!("Failed to read config {:?}: {}, using defaults", path, e);
                Self::default()
            }
        }
    }

    /// Load config using specified paths
    pub fn load_with_paths(paths: &ConfigPaths) -> Self {
        let config_path = paths.config_path();
        Self::load_from(&config_path)
    }

    /// Get theme CSS with paths from environment or default
    pub fn theme_css_with_path(&self) -> Option<(String, PathBuf)> {
        ConfigPaths::from_env_or_default().and_then(|paths| self.theme_css_with_paths(&paths))
    }

    /// Get theme CSS content and the theme file's directory using specified paths
    /// Returns (css_content, theme_directory) for resolving relative paths
    pub fn theme_css_with_paths(&self, paths: &ConfigPaths) -> Option<(String, PathBuf)> {
        let themes_dir = paths.themes_dir();
        let theme_path = paths.theme_path(&self.theme.name);

        match std::fs::read_to_string(&theme_path) {
            Ok(css) => {
                log::info!("Loaded theme from {:?}", theme_path);
                Some((css, themes_dir))
            }
            Err(_) => {
                log::warn!(
                    "Theme '{}' not found at {:?}. Install themes to {:?}",
                    self.theme.name,
                    theme_path,
                    themes_dir
                );
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_config_paths_new() {
        let paths = ConfigPaths::new(PathBuf::from("/test/config"));
        assert_eq!(paths.config_dir, PathBuf::from("/test/config"));
        assert_eq!(
            paths.config_path(),
            PathBuf::from("/test/config/config.toml")
        );
        assert_eq!(paths.themes_dir(), PathBuf::from("/test/config/themes"));
        assert_eq!(
            paths.theme_path("synthwave"),
            PathBuf::from("/test/config/themes/synthwave.css")
        );
    }

    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert_eq!(config.window.columns, 80);
        assert_eq!(config.window.rows, 24);
        assert_eq!(config.font.size, 14.0);
        assert_eq!(config.theme.name, "synthwave");
    }

    #[test]
    fn test_load_from_missing_file() {
        let config = Config::load_from(Path::new("/nonexistent/config.toml"));
        // Should return defaults
        assert_eq!(config.window.columns, 80);
    }

    #[test]
    fn test_load_from_valid_file() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");

        fs::write(
            &config_path,
            r#"
[window]
columns = 120
rows = 40
title = "Test Terminal"

[font]
size = 16.0

[theme]
name = "test-theme"
"#,
        )
        .unwrap();

        let config = Config::load_from(&config_path);
        assert_eq!(config.window.columns, 120);
        assert_eq!(config.window.rows, 40);
        assert_eq!(config.window.title, "Test Terminal");
        assert_eq!(config.font.size, 16.0);
        assert_eq!(config.theme.name, "test-theme");
    }

    #[test]
    fn test_load_with_paths() {
        let temp_dir = TempDir::new().unwrap();
        let paths = ConfigPaths::new(temp_dir.path().to_path_buf());

        fs::write(
            paths.config_path(),
            r#"
[window]
columns = 100
"#,
        )
        .unwrap();

        let config = Config::load_with_paths(&paths);
        assert_eq!(config.window.columns, 100);
    }

    #[test]
    fn test_load_invalid_toml() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");

        fs::write(&config_path, "this is { not valid toml").unwrap();

        let config = Config::load_from(&config_path);
        // Should return defaults
        assert_eq!(config.window.columns, 80);
    }

    #[test]
    fn test_theme_css_with_paths() {
        let temp_dir = TempDir::new().unwrap();
        let paths = ConfigPaths::new(temp_dir.path().to_path_buf());

        // Create themes directory and theme file
        fs::create_dir_all(paths.themes_dir()).unwrap();
        fs::write(
            paths.theme_path("test-theme"),
            ":root { --background: #000; }",
        )
        .unwrap();

        let mut config = Config::default();
        config.theme.name = "test-theme".to_string();

        let result = config.theme_css_with_paths(&paths);
        assert!(result.is_some());
        let (css, themes_dir) = result.unwrap();
        assert!(css.contains("--background"));
        assert_eq!(themes_dir, paths.themes_dir());
    }

    #[test]
    fn test_theme_css_missing() {
        let temp_dir = TempDir::new().unwrap();
        let paths = ConfigPaths::new(temp_dir.path().to_path_buf());

        let config = Config::default();
        let result = config.theme_css_with_paths(&paths);
        assert!(result.is_none());
    }

    #[test]
    fn test_partial_config_uses_defaults() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");

        // Only specify window columns, everything else should be defaults
        fs::write(&config_path, "[window]\ncolumns = 200\n").unwrap();

        let config = Config::load_from(&config_path);
        assert_eq!(config.window.columns, 200);
        assert_eq!(config.window.rows, 24); // default
        assert_eq!(config.font.size, 14.0); // default
        assert_eq!(config.theme.name, "synthwave"); // default
    }
}
