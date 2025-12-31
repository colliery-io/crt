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

    /// Get the shell integration assets directory path
    pub fn shell_assets_dir(&self) -> PathBuf {
        self.config_dir.join("shell")
    }
}

/// Shell configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[derive(Default)]
pub struct ShellConfig {
    /// Shell program to run (default: user's login shell or /bin/zsh)
    pub program: Option<String>,
    /// Arguments to pass to shell
    pub args: Vec<String>,
    /// Working directory (default: user's home)
    pub working_directory: Option<PathBuf>,
    /// Enable semantic prompt markers (OSC 133) for command success/fail detection
    /// When enabled, CRT injects shell hooks for bash/zsh to emit OSC 133 sequences.
    /// Not needed if using starship, oh-my-zsh, or other tools with OSC 133 support.
    pub semantic_prompts: bool,
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
            // Use nyancat in dev builds to test event-driven theming
            #[cfg(debug_assertions)]
            name: "nyancat".to_string(),
            #[cfg(not(debug_assertions))]
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
        Self::load_with_error().0
    }

    /// Load config from default path, returning any error message
    pub fn load_with_error() -> (Self, Option<String>) {
        match ConfigPaths::from_env_or_default() {
            Some(paths) => {
                let config_path = paths.config_path();
                Self::load_from_with_error(&config_path)
            }
            None => {
                log::info!("Could not determine config path, using defaults");
                (Self::default(), None)
            }
        }
    }

    /// Load config from a specific file path
    #[allow(dead_code)]
    pub fn load_from(path: &Path) -> Self {
        Self::load_from_with_error(path).0
    }

    /// Load config from a specific file path, returning any error message
    pub fn load_from_with_error(path: &Path) -> (Self, Option<String>) {
        if !path.exists() {
            log::info!("Config file not found at {:?}, using defaults", path);
            return (Self::default(), None);
        }

        match std::fs::read_to_string(path) {
            Ok(contents) => match toml::from_str(&contents) {
                Ok(config) => {
                    log::info!("Loaded config from {:?}", path);
                    (config, None)
                }
                Err(e) => {
                    let error_msg = format!("Config error: {}", e);
                    log::warn!("Failed to parse config {:?}: {}, using defaults", path, e);
                    (Self::default(), Some(error_msg))
                }
            },
            Err(e) => {
                let error_msg = format!("Config read error: {}", e);
                log::warn!("Failed to read config {:?}: {}, using defaults", path, e);
                (Self::default(), Some(error_msg))
            }
        }
    }

    /// Load config using specified paths
    #[allow(dead_code)]
    pub fn load_with_paths(paths: &ConfigPaths) -> Self {
        let config_path = paths.config_path();
        Self::load_from(&config_path)
    }

    /// Get the shell integration assets directory
    pub fn shell_assets_dir() -> Option<PathBuf> {
        ConfigPaths::from_env_or_default().map(|paths| paths.shell_assets_dir())
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
    }

    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert_eq!(config.window.columns, 80);
        assert_eq!(config.window.rows, 24);
        assert_eq!(config.font.size, 14.0);
        // Default theme differs between debug (nyancat) and release (synthwave)
        #[cfg(debug_assertions)]
        assert_eq!(config.theme.name, "nyancat");
        #[cfg(not(debug_assertions))]
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
    fn test_partial_config_uses_defaults() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");

        // Only specify window columns, everything else should be defaults
        fs::write(&config_path, "[window]\ncolumns = 200\n").unwrap();

        let config = Config::load_from(&config_path);
        assert_eq!(config.window.columns, 200);
        assert_eq!(config.window.rows, 24); // default
        assert_eq!(config.font.size, 14.0); // default
        // Default theme differs between debug (nyancat) and release (synthwave)
        #[cfg(debug_assertions)]
        assert_eq!(config.theme.name, "nyancat");
        #[cfg(not(debug_assertions))]
        assert_eq!(config.theme.name, "synthwave");
    }

    // ========== Cursor Style Tests ==========

    #[test]
    fn test_cursor_style_default() {
        let cursor = CursorConfig::default();
        assert_eq!(cursor.style, CursorStyle::Block);
        assert!(cursor.blink);
        assert_eq!(cursor.blink_interval_ms, 530);
    }

    #[test]
    fn test_cursor_style_serde() {
        // Block
        let config: CursorConfig = toml::from_str(r#"style = "block""#).unwrap();
        assert_eq!(config.style, CursorStyle::Block);

        // Bar
        let config: CursorConfig = toml::from_str(r#"style = "bar""#).unwrap();
        assert_eq!(config.style, CursorStyle::Bar);

        // Underline
        let config: CursorConfig = toml::from_str(r#"style = "underline""#).unwrap();
        assert_eq!(config.style, CursorStyle::Underline);
    }

    #[test]
    fn test_cursor_blink_config() {
        let config: CursorConfig = toml::from_str(
            r#"
            blink = false
            blink_interval_ms = 1000
            "#,
        )
        .unwrap();
        assert!(!config.blink);
        assert_eq!(config.blink_interval_ms, 1000);
    }

    // ========== Bell Config Tests ==========

    #[test]
    fn test_bell_config_default() {
        let bell = BellConfig::default();
        assert!(bell.visual);
        assert_eq!(bell.flash_duration_ms, 100);
        assert!((bell.flash_intensity - 0.3).abs() < 0.001);
    }

    #[test]
    fn test_bell_config_serde() {
        let config: BellConfig = toml::from_str(
            r#"
            visual = false
            flash_duration_ms = 200
            flash_intensity = 0.5
            "#,
        )
        .unwrap();
        assert!(!config.visual);
        assert_eq!(config.flash_duration_ms, 200);
        assert!((config.flash_intensity - 0.5).abs() < 0.001);
    }

    // ========== Font Config Tests ==========

    #[test]
    fn test_font_config_default() {
        let font = FontConfig::default();
        assert!(!font.family.is_empty());
        assert!(font.family.contains(&"JetBrains Mono".to_string()));
        assert_eq!(font.size, 14.0);
        assert!((font.line_height - 1.5).abs() < 0.001);
    }

    #[test]
    fn test_font_config_serde() {
        let config: FontConfig = toml::from_str(
            r#"
            family = ["Monaco", "Courier New"]
            size = 18.0
            line_height = 1.8
            "#,
        )
        .unwrap();
        assert_eq!(config.family, vec!["Monaco", "Courier New"]);
        assert_eq!(config.size, 18.0);
        assert!((config.line_height - 1.8).abs() < 0.001);
    }

    #[test]
    fn test_font_config_empty_family() {
        let config: FontConfig = toml::from_str(r#"family = []"#).unwrap();
        assert!(config.family.is_empty());
    }

    // ========== Shell Config Tests ==========

    #[test]
    fn test_shell_config_default() {
        let shell = ShellConfig::default();
        assert!(shell.program.is_none());
        assert!(shell.args.is_empty());
        assert!(shell.working_directory.is_none());
    }

    #[test]
    fn test_shell_config_serde() {
        let config: ShellConfig = toml::from_str(
            r#"
            program = "/bin/bash"
            args = ["-l", "-i"]
            working_directory = "/home/user"
            "#,
        )
        .unwrap();
        assert_eq!(config.program, Some("/bin/bash".to_string()));
        assert_eq!(config.args, vec!["-l", "-i"]);
        assert_eq!(config.working_directory, Some(PathBuf::from("/home/user")));
    }

    // ========== Window Config Tests ==========

    #[test]
    fn test_window_config_default() {
        let window = WindowConfig::default();
        assert_eq!(window.columns, 80);
        assert_eq!(window.rows, 24);
        assert_eq!(window.title, "CRT Terminal");
        assert!(!window.fullscreen);
    }

    #[test]
    fn test_window_config_serde() {
        let config: WindowConfig = toml::from_str(
            r#"
            columns = 160
            rows = 48
            title = "My Terminal"
            fullscreen = true
            "#,
        )
        .unwrap();
        assert_eq!(config.columns, 160);
        assert_eq!(config.rows, 48);
        assert_eq!(config.title, "My Terminal");
        assert!(config.fullscreen);
    }

    // ========== Theme Config Tests ==========

    #[test]
    fn test_theme_config_default() {
        let theme = ThemeConfig::default();
        #[cfg(debug_assertions)]
        assert_eq!(theme.name, "nyancat");
        #[cfg(not(debug_assertions))]
        assert_eq!(theme.name, "synthwave");
    }

    #[test]
    fn test_theme_config_serde() {
        let config: ThemeConfig = toml::from_str(r#"name = "dracula""#).unwrap();
        assert_eq!(config.name, "dracula");
    }

    // ========== KeyAction Tests ==========

    #[test]
    fn test_key_action_serde() {
        // Test snake_case serialization
        let binding: Keybinding = toml::from_str(
            r#"
            key = "t"
            mods = ["super"]
            action = "new_tab"
            "#,
        )
        .unwrap();
        assert_eq!(binding.action, KeyAction::NewTab);

        let binding: Keybinding = toml::from_str(
            r#"
            key = "w"
            mods = ["super"]
            action = "close_tab"
            "#,
        )
        .unwrap();
        assert_eq!(binding.action, KeyAction::CloseTab);

        let binding: Keybinding = toml::from_str(
            r#"
            key = "equal"
            mods = ["super"]
            action = "increase_font_size"
            "#,
        )
        .unwrap();
        assert_eq!(binding.action, KeyAction::IncreaseFontSize);
    }

    #[test]
    fn test_keybinding_with_multiple_mods() {
        let binding: Keybinding = toml::from_str(
            r#"
            key = "["
            mods = ["super", "shift"]
            action = "prev_tab"
            "#,
        )
        .unwrap();
        assert_eq!(binding.key, "[");
        assert_eq!(binding.mods, vec!["super", "shift"]);
        assert_eq!(binding.action, KeyAction::PrevTab);
    }

    #[test]
    fn test_keybinding_no_mods() {
        let binding: Keybinding = toml::from_str(
            r#"
            key = "F1"
            action = "toggle_fullscreen"
            "#,
        )
        .unwrap();
        assert_eq!(binding.key, "F1");
        assert!(binding.mods.is_empty());
        assert_eq!(binding.action, KeyAction::ToggleFullscreen);
    }

    #[test]
    fn test_keybindings_config_default() {
        let keybindings = KeybindingsConfig::default();
        assert!(!keybindings.bindings.is_empty());

        // Check for expected default bindings
        let has_new_tab = keybindings
            .bindings
            .iter()
            .any(|b| b.action == KeyAction::NewTab);
        assert!(has_new_tab);

        let has_copy = keybindings
            .bindings
            .iter()
            .any(|b| b.action == KeyAction::Copy);
        assert!(has_copy);
    }

    // ========== Error Handling Tests ==========

    #[test]
    fn test_load_with_error_returns_error_message() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");

        fs::write(&config_path, "invalid { toml content").unwrap();

        let (config, error) = Config::load_from_with_error(&config_path);

        // Should return defaults
        assert_eq!(config.window.columns, 80);

        // Should have error message
        assert!(error.is_some());
        let error_msg = error.unwrap();
        assert!(error_msg.contains("Config error"));
    }

    #[test]
    fn test_load_with_error_no_error_on_valid() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");

        fs::write(&config_path, "[window]\ncolumns = 100\n").unwrap();

        let (config, error) = Config::load_from_with_error(&config_path);

        assert_eq!(config.window.columns, 100);
        assert!(error.is_none());
    }

    #[test]
    fn test_load_with_error_missing_file() {
        let (config, error) = Config::load_from_with_error(Path::new("/nonexistent/config.toml"));

        // Should return defaults
        assert_eq!(config.window.columns, 80);
        // No error for missing file (intentional)
        assert!(error.is_none());
    }

    // ========== Full Config Integration Tests ==========

    #[test]
    fn test_full_config_serde() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");

        fs::write(
            &config_path,
            r#"
[shell]
program = "/bin/zsh"
args = ["-l"]

[font]
family = ["Fira Code"]
size = 16.0
line_height = 1.4

[window]
columns = 100
rows = 30
title = "Test"
fullscreen = false

[theme]
name = "dracula"

[cursor]
style = "bar"
blink = false
blink_interval_ms = 600

[bell]
visual = true
flash_duration_ms = 150
flash_intensity = 0.4

[[keybindings.bindings]]
key = "n"
mods = ["super"]
action = "new_tab"
"#,
        )
        .unwrap();

        let config = Config::load_from(&config_path);

        assert_eq!(config.shell.program, Some("/bin/zsh".to_string()));
        assert_eq!(config.shell.args, vec!["-l"]);
        assert_eq!(config.font.family, vec!["Fira Code"]);
        assert_eq!(config.font.size, 16.0);
        assert_eq!(config.window.columns, 100);
        assert_eq!(config.window.rows, 30);
        assert_eq!(config.theme.name, "dracula");
        assert_eq!(config.cursor.style, CursorStyle::Bar);
        assert!(!config.cursor.blink);
        assert!(config.bell.visual);
        assert_eq!(config.keybindings.bindings.len(), 1);
        assert_eq!(config.keybindings.bindings[0].action, KeyAction::NewTab);
    }

    #[test]
    fn test_config_unknown_fields_ignored() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");

        // Include an unknown field
        fs::write(
            &config_path,
            r#"
[window]
columns = 90
unknown_field = "should be ignored"
"#,
        )
        .unwrap();

        let config = Config::load_from(&config_path);
        assert_eq!(config.window.columns, 90);
    }

    #[test]
    fn test_config_type_mismatch_uses_default() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");

        // columns should be a number, not a string
        fs::write(&config_path, r#"[window]\ncolumns = "not a number"\n"#).unwrap();

        let (config, error) = Config::load_from_with_error(&config_path);

        // Should fall back to defaults due to parse error
        assert_eq!(config.window.columns, 80);
        assert!(error.is_some());
    }
}
