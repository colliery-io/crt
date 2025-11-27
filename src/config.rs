//! Configuration management for CRT terminal
//!
//! Loads config from ~/.config/crt/config.toml with sensible defaults.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Tab bar position
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TabPosition {
    #[default]
    Top,
    Bottom,
    Left,
    Right,
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
    /// Tab bar position
    pub tab_position: TabPosition,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            columns: 80,
            rows: 24,
            title: "CRT Terminal".to_string(),
            fullscreen: false,
            tab_position: TabPosition::default(),
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

    /// Save config to file (creates directory if needed)
    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let Some(dir) = Self::config_dir() else {
            return Err("Could not determine config directory".into());
        };

        std::fs::create_dir_all(&dir)?;

        let path = dir.join("config.toml");
        let contents = toml::to_string_pretty(self)?;
        std::fs::write(&path, contents)?;

        log::info!("Saved config to {:?}", path);
        Ok(())
    }

    /// Get shell program (or default to login shell)
    pub fn shell_program(&self) -> String {
        if let Some(ref program) = self.shell.program {
            return program.clone();
        }

        // Try to get user's login shell from SHELL env var
        if let Ok(shell) = std::env::var("SHELL") {
            return shell;
        }

        // Fallback to /bin/zsh on macOS, /bin/bash elsewhere
        #[cfg(target_os = "macos")]
        {
            "/bin/zsh".to_string()
        }
        #[cfg(not(target_os = "macos"))]
        {
            "/bin/bash".to_string()
        }
    }

    /// Get the themes directory path (~/.config/crt/themes)
    pub fn themes_dir() -> Option<PathBuf> {
        Self::config_dir().map(|p| p.join("themes"))
    }

    /// Get theme CSS content
    /// Tries to load from ~/.config/crt/themes/{name}.css first,
    /// falls back to embedded defaults for "synthwave" and "minimal"
    pub fn theme_css(&self) -> Option<String> {
        // Try to load from themes directory first
        if let Some(themes_dir) = Self::themes_dir() {
            let theme_path = themes_dir.join(format!("{}.css", self.theme.name));
            if let Ok(css) = std::fs::read_to_string(&theme_path) {
                log::info!("Loaded theme from {:?}", theme_path);
                return Some(css);
            }
        }

        // Fall back to embedded defaults
        match self.theme.name.as_str() {
            "synthwave" => {
                log::info!("Using embedded synthwave theme");
                Some(BUILTIN_SYNTHWAVE_THEME.to_string())
            }
            "minimal" => {
                log::info!("Using embedded minimal theme");
                Some(BUILTIN_MINIMAL_THEME.to_string())
            }
            _ => {
                log::warn!("Theme '{}' not found in ~/.config/crt/themes/", self.theme.name);
                None
            }
        }
    }
}

/// Builtin synthwave theme CSS
const BUILTIN_SYNTHWAVE_THEME: &str = r#"/* Synthwave Theme - The Extra AF Terminal Experience */

:terminal {
    /* Typography */
    font-family: "MesloLGS NF", "Fira Code", monospace;
    font-size: 14;
    line-height: 1.5;
    font-bold: "MesloLGS NF Bold";
    font-italic: "MesloLGS NF Italic";
    font-bold-italic: "MesloLGS NF Bold Italic";
    ligatures: true;

    /* Base colors - teal text like synthwave */
    color: #61e2fe;
    background: linear-gradient(to bottom, #1a0a2e, #16213e);
    cursor-color: #00ffff;

    /* Text glow effect - strong teal glow */
    text-shadow: 0 0 20px rgba(97, 226, 254, 1.0);
}

:terminal::selection {
    background: #44475a;
    color: #f8f8f2;
}

:terminal::highlight {
    background: #e6db74;
    color: #1a1a1a;
}

:terminal::backdrop {
    /* Perspective grid */
    --grid-enabled: true;
    --grid-color: rgba(255, 0, 255, 0.3);
    --grid-spacing: 8;
    --grid-line-width: 0.02;
    --grid-perspective: 2;
    --grid-horizon: 0.35;
    --grid-animation-speed: 0.5;
}

/* ANSI 16-color palette */
:terminal::palette {
    /* Normal colors */
    --color-0: #0d0d0d;   /* black */
    --color-1: #ff5555;   /* red */
    --color-2: #50fa7b;   /* green */
    --color-3: #f1fa8c;   /* yellow */
    --color-4: #bd93f9;   /* blue */
    --color-5: #ff79c6;   /* magenta */
    --color-6: #8be9fd;   /* cyan */
    --color-7: #f8f8f2;   /* white */

    /* Bright colors */
    --color-8: #4d4d4d;   /* bright black */
    --color-9: #ff6e6e;   /* bright red */
    --color-10: #69ff94;  /* bright green */
    --color-11: #ffffa5;  /* bright yellow */
    --color-12: #d6acff;  /* bright blue */
    --color-13: #ff92df;  /* bright magenta */
    --color-14: #a4ffff;  /* bright cyan */
    --color-15: #ffffff;  /* bright white */
}

/* Tab bar styling */
:terminal::tab-bar {
    background: #1a0a2e;
    border-color: #4a3a5e;
    height: 36px;
    padding: 4px;
}

:tab {
    background: #2a1a3e;
    color: #8888aa;
    border-radius: 4px;
    min-width: 80px;
    max-width: 200px;
}

:tab.active {
    background: #3a2a4e;
    color: #ffd700;
    accent-color: #ffd700;
    text-shadow: 0 0 15px rgba(255, 215, 0, 0.9);
}

:terminal::tab-close {
    color: #666688;
    --hover-background: #ff5555;
    --hover-color: #ffffff;
}
"#;

/// Builtin minimal theme CSS
const BUILTIN_MINIMAL_THEME: &str = r#"/* Minimal Theme - Clean and simple */

:terminal {
    /* Typography */
    font-family: "MesloLGS NF", "Fira Code", monospace;
    font-size: 14;
    line-height: 1.5;
    ligatures: true;

    /* Base colors - clean white on dark */
    color: #c8c8c8;
    background: #1a1a1a;
    cursor-color: #ffffff;
}

:terminal::selection {
    background: #44475a;
    color: #f8f8f2;
}

:terminal::highlight {
    background: #e6db74;
    color: #1a1a1a;
}

:terminal::backdrop {
    /* No grid effect */
    --grid-enabled: false;
}

/* ANSI 16-color palette */
:terminal::palette {
    /* Normal colors */
    --color-0: #0d0d0d;   /* black */
    --color-1: #ff5555;   /* red */
    --color-2: #50fa7b;   /* green */
    --color-3: #f1fa8c;   /* yellow */
    --color-4: #bd93f9;   /* blue */
    --color-5: #ff79c6;   /* magenta */
    --color-6: #8be9fd;   /* cyan */
    --color-7: #f8f8f2;   /* white */

    /* Bright colors */
    --color-8: #4d4d4d;   /* bright black */
    --color-9: #ff6e6e;   /* bright red */
    --color-10: #69ff94;  /* bright green */
    --color-11: #ffffa5;  /* bright yellow */
    --color-12: #d6acff;  /* bright blue */
    --color-13: #ff92df;  /* bright magenta */
    --color-14: #a4ffff;  /* bright cyan */
    --color-15: #ffffff;  /* bright white */
}

/* Tab bar styling */
:terminal::tab-bar {
    background: #1a1a1a;
    border-color: #333333;
    height: 36px;
    padding: 4px;
}

:tab {
    background: #2a2a2a;
    color: #888888;
    border-radius: 4px;
    min-width: 80px;
    max-width: 200px;
}

:tab.active {
    background: #3a3a3a;
    color: #ffffff;
    accent-color: #ffffff;
}

:terminal::tab-close {
    color: #666666;
    --hover-background: #ff5555;
    --hover-color: #ffffff;
}
"#;

/// Generate a default config file with comments
pub fn generate_default_config() -> String {
    r#"# CRT Terminal Configuration
# Location: ~/.config/crt/config.toml

[shell]
# Shell program to run (default: uses $SHELL environment variable)
# program = "/bin/zsh"
# args = ["-l"]  # Login shell
# working_directory = "~"

[font]
# Font family (for future system font support)
family = "MesloLGS NF"
# Base font size in points
size = 14.0
# Line height multiplier
line_height = 1.5

[window]
# Initial terminal size
columns = 80
rows = 24
# Window title
title = "CRT Terminal"
# Start in fullscreen
fullscreen = false
# Tab bar position (top, bottom, left, right)
tab_position = "top"

[theme]
# Theme name - looks for ~/.config/crt/themes/{name}.css
# Falls back to embedded "synthwave" or "minimal" if file not found
name = "synthwave"

# Keybindings - you can customize or add new bindings
# Supported modifiers: "super" (Cmd on macOS), "shift", "control", "alt"
# Supported actions: new_tab, close_tab, next_tab, prev_tab,
#                    select_tab1-9, increase_font_size, decrease_font_size,
#                    reset_font_size, toggle_fullscreen, copy, paste, quit

# Example: Override Cmd+T to do something else
# [[keybindings.bindings]]
# key = "t"
# mods = ["super"]
# action = "new_tab"
"#
    .to_string()
}
