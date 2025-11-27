//! Configuration management for CRT terminal
//!
//! Loads config from ~/.config/crt/config.toml with sensible defaults.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
    /// Get the config directory path (~/.config/crt)
    pub fn config_dir() -> Option<PathBuf> {
        dirs::home_dir().map(|p| p.join(".config").join("crt"))
    }

    /// Get the config file path (~/.config/crt/config.toml)
    pub fn config_path() -> Option<PathBuf> {
        Self::config_dir().map(|p| p.join("config.toml"))
    }

    /// Load config from file, or return defaults if not found
    pub fn load() -> Self {
        let Some(path) = Self::config_path() else {
            log::info!("Could not determine config path, using defaults");
            return Self::default();
        };

        if !path.exists() {
            log::info!("Config file not found at {:?}, using defaults", path);
            return Self::default();
        }

        match std::fs::read_to_string(&path) {
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

    /// Get the themes directory path (~/.config/crt/themes)
    pub fn themes_dir() -> Option<PathBuf> {
        Self::config_dir().map(|p| p.join("themes"))
    }

    /// Get theme CSS content from ~/.config/crt/themes/{name}.css
    pub fn theme_css(&self) -> Option<String> {
        if let Some(themes_dir) = Self::themes_dir() {
            let theme_path = themes_dir.join(format!("{}.css", self.theme.name));
            match std::fs::read_to_string(&theme_path) {
                Ok(css) => {
                    log::info!("Loaded theme from {:?}", theme_path);
                    return Some(css);
                }
                Err(_) => {
                    log::warn!(
                        "Theme '{}' not found at {:?}. Install themes to ~/.config/crt/themes/",
                        self.theme.name,
                        theme_path
                    );
                }
            }
        }
        None
    }
}
