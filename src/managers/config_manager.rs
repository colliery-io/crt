//! Configuration and theme management
//!
//! This module provides centralized management of application configuration
//! and themes, including hot-reload support via file watching.

use crate::config::Config;
use crate::watcher::{ConfigEvent, ConfigWatcher};
use crt_theme::Theme;
use std::path::PathBuf;

/// Events emitted by the ConfigManager when configuration changes
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigChange {
    /// Configuration file was reloaded successfully
    ConfigReloaded,
    /// Theme file was reloaded successfully
    ThemeReloaded,
    /// An error occurred during reload
    Error(String),
}

/// Manages application configuration and themes
///
/// This struct centralizes all configuration management including:
/// - Loading and reloading config.toml
/// - Loading and reloading theme CSS files
/// - File watching for hot-reload support
pub struct ConfigManager {
    /// The current configuration
    config: Config,
    /// The current theme
    theme: Theme,
    /// File watcher for hot-reload (optional)
    watcher: Option<ConfigWatcher>,
}

impl std::fmt::Debug for ConfigManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConfigManager")
            .field("config", &self.config)
            .field("theme", &"<Theme>")
            .field("watcher", &self.watcher.as_ref().map(|_| "<ConfigWatcher>"))
            .finish()
    }
}

impl ConfigManager {
    /// Create a new ConfigManager with the given config and theme
    pub fn new(config: Config, theme: Theme) -> Self {
        Self {
            config,
            theme,
            watcher: None,
        }
    }

    /// Create a ConfigManager by loading from default paths
    ///
    /// Loads config from ~/.config/crt/config.toml and theme based on config.
    pub fn load_default() -> Self {
        let config = Config::load();
        let theme = Self::load_theme_from_config(&config);

        Self {
            config,
            theme,
            watcher: None,
        }
    }

    /// Start watching config files for changes
    ///
    /// Creates a file watcher that monitors ~/.config/crt for changes
    /// to config.toml and theme CSS files.
    pub fn start_watching(&mut self) {
        if self.watcher.is_none() {
            self.watcher = ConfigWatcher::new();
            if self.watcher.is_some() {
                log::info!("Config file watching enabled");
            } else {
                log::warn!("Failed to start config file watcher");
            }
        }
    }

    /// Stop watching config files
    pub fn stop_watching(&mut self) {
        self.watcher = None;
    }

    /// Check if file watching is active
    pub fn is_watching(&self) -> bool {
        self.watcher.is_some()
    }

    /// Poll for configuration changes (non-blocking)
    ///
    /// Returns `Some(ConfigChange)` if a change was detected, `None` otherwise.
    /// This should be called regularly (e.g., in the event loop).
    pub fn poll_changes(&mut self) -> Option<ConfigChange> {
        let watcher = self.watcher.as_mut()?;

        match watcher.poll() {
            Some(ConfigEvent::ConfigChanged) => Some(self.reload_config()),
            Some(ConfigEvent::ThemeChanged) => Some(self.reload_theme()),
            None => None,
        }
    }

    /// Reload configuration from disk
    ///
    /// Returns `ConfigChange::ConfigReloaded` on success, or
    /// `ConfigChange::Error` if parsing fails. Note that Config::load
    /// returns defaults on error, so this rarely fails entirely.
    pub fn reload_config(&mut self) -> ConfigChange {
        self.config = Config::load();

        // Also reload theme since config may reference a different theme
        let old_theme_name = self.theme_name().to_string();
        let new_theme = Self::load_theme_from_config(&self.config);
        self.theme = new_theme;

        if self.theme_name() != old_theme_name {
            log::info!(
                "Theme changed from '{}' to '{}'",
                old_theme_name,
                self.theme_name()
            );
        }

        ConfigChange::ConfigReloaded
    }

    /// Reload theme from disk
    ///
    /// Reloads the theme CSS file specified in the current config.
    pub fn reload_theme(&mut self) -> ConfigChange {
        self.theme = Self::load_theme_from_config(&self.config);
        ConfigChange::ThemeReloaded
    }

    /// Get the current configuration
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Get the current theme
    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    /// Get the theme name from config
    pub fn theme_name(&self) -> &str {
        &self.config.theme.name
    }

    /// Get the config directory path
    pub fn config_dir() -> Option<PathBuf> {
        Config::config_dir()
    }

    /// Get the themes directory path
    pub fn themes_dir() -> Option<PathBuf> {
        Config::themes_dir()
    }

    /// Load theme based on config settings
    fn load_theme_from_config(config: &Config) -> Theme {
        match config.theme_css_with_path() {
            Some((css, base_dir)) => {
                Theme::from_css_with_base(&css, &base_dir).unwrap_or_else(|e| {
                    log::warn!("Failed to parse theme '{}': {:?}", config.theme.name, e);
                    Theme::default()
                })
            }
            None => {
                log::warn!("Theme '{}' not found, using default", config.theme.name);
                Theme::default()
            }
        }
    }
}

impl Default for ConfigManager {
    fn default() -> Self {
        Self::load_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let config = Config::default();
        let theme = Theme::default();
        let manager = ConfigManager::new(config, theme);

        assert!(!manager.is_watching());
    }

    #[test]
    fn test_load_default() {
        // This will load from disk or use defaults
        let manager = ConfigManager::load_default();
        assert!(!manager.is_watching());
    }

    #[test]
    fn test_default_trait() {
        let manager = ConfigManager::default();
        assert!(!manager.is_watching());
    }

    #[test]
    fn test_config_access() {
        let config = Config::default();
        let theme = Theme::default();
        let manager = ConfigManager::new(config, theme);

        // Verify we can access config
        let _ = manager.config().font.size;
        let _ = manager.config().window.columns;
    }

    #[test]
    fn test_theme_access() {
        let config = Config::default();
        let theme = Theme::default();
        let manager = ConfigManager::new(config, theme);

        // Verify we can access theme
        let _ = manager.theme().background;
        let _ = manager.theme().foreground;
    }

    #[test]
    fn test_theme_name() {
        let config = Config::default();
        let theme = Theme::default();
        let manager = ConfigManager::new(config, theme);

        // Default theme name from config
        assert_eq!(manager.theme_name(), "synthwave");
    }

    #[test]
    fn test_start_stop_watching() {
        let config = Config::default();
        let theme = Theme::default();
        let mut manager = ConfigManager::new(config, theme);

        // Initially not watching
        assert!(!manager.is_watching());

        // Start watching (may fail if config dir doesn't exist, which is fine)
        manager.start_watching();

        // Stop watching
        manager.stop_watching();
        assert!(!manager.is_watching());
    }

    #[test]
    fn test_poll_without_watcher() {
        let config = Config::default();
        let theme = Theme::default();
        let mut manager = ConfigManager::new(config, theme);

        // Polling without watcher should return None
        assert!(manager.poll_changes().is_none());
    }

    #[test]
    fn test_reload_config() {
        let config = Config::default();
        let theme = Theme::default();
        let mut manager = ConfigManager::new(config, theme);

        let result = manager.reload_config();
        assert_eq!(result, ConfigChange::ConfigReloaded);
    }

    #[test]
    fn test_reload_theme() {
        let config = Config::default();
        let theme = Theme::default();
        let mut manager = ConfigManager::new(config, theme);

        let result = manager.reload_theme();
        assert_eq!(result, ConfigChange::ThemeReloaded);
    }

    #[test]
    fn test_config_change_debug() {
        let change = ConfigChange::ConfigReloaded;
        let debug_str = format!("{:?}", change);
        assert!(debug_str.contains("ConfigReloaded"));

        let error = ConfigChange::Error("test error".to_string());
        let debug_str = format!("{:?}", error);
        assert!(debug_str.contains("test error"));
    }

    #[test]
    fn test_config_change_eq() {
        assert_eq!(ConfigChange::ConfigReloaded, ConfigChange::ConfigReloaded);
        assert_eq!(ConfigChange::ThemeReloaded, ConfigChange::ThemeReloaded);
        assert_ne!(ConfigChange::ConfigReloaded, ConfigChange::ThemeReloaded);

        let err1 = ConfigChange::Error("error".to_string());
        let err2 = ConfigChange::Error("error".to_string());
        let err3 = ConfigChange::Error("different".to_string());
        assert_eq!(err1, err2);
        assert_ne!(err1, err3);
    }

    #[test]
    fn test_static_methods() {
        // These may return None depending on environment, but shouldn't panic
        let _ = ConfigManager::config_dir();
        let _ = ConfigManager::themes_dir();
    }
}
