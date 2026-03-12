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
            if path.extension().is_some_and(|ext| ext == "css")
                && let Some(stem) = path.file_stem().and_then(|s| s.to_str())
            {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Minimal valid CSS theme for testing
    const MINIMAL_CSS: &str = r#"
        :terminal {
            color: #ffffff;
            background: #000000;
        }
    "#;

    fn setup_themes_dir(themes: &[(&str, &str)]) -> TempDir {
        let dir = TempDir::new().unwrap();
        for (name, css) in themes {
            fs::write(dir.path().join(format!("{}.css", name)), css).unwrap();
        }
        dir
    }

    #[test]
    fn new_empty_dir() {
        let dir = TempDir::new().unwrap();
        let registry = ThemeRegistry::new(dir.path().to_path_buf(), "default".to_string());
        assert!(registry.list_themes().is_empty());
    }

    #[test]
    fn new_nonexistent_dir() {
        let registry =
            ThemeRegistry::new(PathBuf::from("/nonexistent/themes"), "default".to_string());
        assert!(registry.list_themes().is_empty());
    }

    #[test]
    fn loads_css_files() {
        let dir = setup_themes_dir(&[("alpha", MINIMAL_CSS), ("beta", MINIMAL_CSS)]);
        let registry = ThemeRegistry::new(dir.path().to_path_buf(), "alpha".to_string());
        let names = registry.list_themes();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"alpha"));
        assert!(names.contains(&"beta"));
    }

    #[test]
    fn ignores_non_css_files() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("valid.css"), MINIMAL_CSS).unwrap();
        fs::write(dir.path().join("readme.txt"), "not a theme").unwrap();
        fs::write(dir.path().join("data.json"), "{}").unwrap();
        let registry = ThemeRegistry::new(dir.path().to_path_buf(), "valid".to_string());
        assert_eq!(registry.list_themes(), vec!["valid"]);
    }

    #[test]
    fn get_theme_by_name() {
        let dir = setup_themes_dir(&[("test", MINIMAL_CSS)]);
        let registry = ThemeRegistry::new(dir.path().to_path_buf(), "test".to_string());
        assert!(registry.get_theme("test").is_some());
        assert!(registry.get_theme("nonexistent").is_none());
    }

    #[test]
    fn get_default_theme_when_present() {
        let dir = setup_themes_dir(&[("mydefault", MINIMAL_CSS)]);
        let registry = ThemeRegistry::new(dir.path().to_path_buf(), "mydefault".to_string());
        let (name, _theme) = registry.get_default_theme();
        assert_eq!(name, "mydefault");
    }

    #[test]
    fn get_default_theme_falls_back_when_missing() {
        let dir = setup_themes_dir(&[("other", MINIMAL_CSS)]);
        let registry = ThemeRegistry::new(dir.path().to_path_buf(), "missing".to_string());
        let (name, _theme) = registry.get_default_theme();
        // Falls back to whatever theme is available
        assert_eq!(name, "other");
    }

    #[test]
    fn get_default_theme_uses_builtin_when_no_themes() {
        let dir = TempDir::new().unwrap();
        let registry = ThemeRegistry::new(dir.path().to_path_buf(), "anything".to_string());
        let (name, _theme) = registry.get_default_theme();
        assert_eq!(name, "default");
    }

    #[test]
    fn default_theme_name_returns_configured() {
        let dir = TempDir::new().unwrap();
        let registry = ThemeRegistry::new(dir.path().to_path_buf(), "synthwave".to_string());
        assert_eq!(registry.default_theme_name(), "synthwave");
    }

    #[test]
    fn list_themes_sorted_alphabetically() {
        let dir = setup_themes_dir(&[("zeta", MINIMAL_CSS), ("alpha", MINIMAL_CSS), ("mid", MINIMAL_CSS)]);
        let registry = ThemeRegistry::new(dir.path().to_path_buf(), "alpha".to_string());
        assert_eq!(registry.list_themes(), vec!["alpha", "mid", "zeta"]);
    }

    #[test]
    fn reload_picks_up_new_themes() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("first.css"), MINIMAL_CSS).unwrap();
        let mut registry = ThemeRegistry::new(dir.path().to_path_buf(), "first".to_string());
        assert_eq!(registry.list_themes().len(), 1);

        // Add a new theme
        fs::write(dir.path().join("second.css"), MINIMAL_CSS).unwrap();
        registry.reload_all();
        assert_eq!(registry.list_themes().len(), 2);
    }

    #[test]
    fn reload_removes_deleted_themes() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("temp.css"), MINIMAL_CSS).unwrap();
        let mut registry = ThemeRegistry::new(dir.path().to_path_buf(), "temp".to_string());
        assert_eq!(registry.list_themes().len(), 1);

        fs::remove_file(dir.path().join("temp.css")).unwrap();
        registry.reload_all();
        assert!(registry.list_themes().is_empty());
    }

    #[test]
    fn malformed_css_skipped() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("good.css"), MINIMAL_CSS).unwrap();
        // Not valid theme CSS, but won't panic — just gets default/empty theme or skips
        fs::write(dir.path().join("bad.css"), "this is not css { at all }}}").unwrap();
        let registry = ThemeRegistry::new(dir.path().to_path_buf(), "good".to_string());
        // Should at least have the good theme (bad may parse or fail gracefully)
        assert!(registry.get_theme("good").is_some());
    }
}
