//! Theme registry for managing available themes
//!
//! Scans the themes directory and caches parsed themes for runtime switching.

use crt_theme::Theme;
use std::collections::HashMap;
use std::path::PathBuf;

/// Registry of available themes, loaded from the themes directory
pub struct ThemeRegistry {
    /// Cached parsed themes by name (without .css extension)
    themes: HashMap<String, Theme>,
    /// Path to the themes directory
    themes_dir: PathBuf,
    /// Default theme name from config
    default_theme: String,
}

impl ThemeRegistry {
    /// Create a new theme registry and scan for available themes
    pub fn new(themes_dir: PathBuf, default_theme: String) -> Self {
        let mut registry = Self {
            themes: HashMap::new(),
            themes_dir,
            default_theme,
        };
        registry.scan_themes();
        registry
    }

    /// Scan the themes directory and load all available themes
    pub fn scan_themes(&mut self) {
        self.themes.clear();

        let entries = match std::fs::read_dir(&self.themes_dir) {
            Ok(entries) => entries,
            Err(e) => {
                log::warn!(
                    "Failed to read themes directory {:?}: {}",
                    self.themes_dir,
                    e
                );
                return;
            }
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "css") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    let name = stem.to_string();
                    match self.load_theme_from_path(&path) {
                        Ok(theme) => {
                            log::info!("Loaded theme: {}", name);
                            self.themes.insert(name, theme);
                        }
                        Err(e) => {
                            log::warn!("Failed to load theme {:?}: {}", path, e);
                        }
                    }
                }
            }
        }

        log::info!(
            "Theme registry loaded {} themes from {:?}",
            self.themes.len(),
            self.themes_dir
        );
    }

    /// Load a theme from a CSS file path
    fn load_theme_from_path(&self, path: &std::path::Path) -> Result<Theme, String> {
        let css = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        Theme::from_css_with_base(&css, &self.themes_dir).map_err(|e| e.to_string())
    }

    /// Get list of available theme names, sorted alphabetically
    pub fn list_themes(&self) -> Vec<&str> {
        let mut names: Vec<_> = self.themes.keys().map(|s| s.as_str()).collect();
        names.sort();
        names
    }

    /// Get a theme by name
    pub fn get_theme(&self, name: &str) -> Option<&Theme> {
        self.themes.get(name)
    }

    /// Get the default theme (from config or fallback)
    pub fn get_default_theme(&self) -> (&str, Theme) {
        if let Some(theme) = self.themes.get(&self.default_theme) {
            (&self.default_theme, theme.clone())
        } else if let Some((name, theme)) = self.themes.iter().next() {
            log::warn!(
                "Default theme '{}' not found, using '{}'",
                self.default_theme,
                name
            );
            (name.as_str(), theme.clone())
        } else {
            log::warn!("No themes found, using built-in default");
            ("default", Theme::default())
        }
    }

    /// Get the default theme name
    pub fn default_theme_name(&self) -> &str {
        &self.default_theme
    }

    /// Reload all themes from disk
    pub fn reload_all(&mut self) {
        self.scan_themes();
    }
}
