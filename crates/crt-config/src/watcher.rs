//! Configuration and Theme Hot-Reload
//!
//! Watches for changes to config.toml and theme files, sending reload events
//! through a channel for the application to handle.

use notify::{
    Config as NotifyConfig, Event, RecommendedWatcher, RecursiveMode, Watcher,
    event::ModifyKind,
};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use crate::{Config, ConfigError};

/// Events emitted by the configuration watcher
#[derive(Debug, Clone)]
pub enum ConfigEvent {
    /// Configuration file changed, contains new config
    ConfigReloaded(Config),
    /// Theme file changed, contains new CSS content
    ThemeReloaded(String),
    /// Error occurred during reload
    ReloadError(String),
}

/// Watches configuration and theme files for changes
pub struct ConfigWatcher {
    _watcher: RecommendedWatcher,
    receiver: Receiver<ConfigEvent>,
}

impl ConfigWatcher {
    /// Create a new configuration watcher
    ///
    /// Watches:
    /// - ~/.crt/config.toml for configuration changes
    /// - ~/.crt/themes/ for theme file changes
    /// - Current theme file specifically
    pub fn new(config: &Config) -> Result<Self, ConfigError> {
        let (tx, rx) = mpsc::channel();

        let config_dir = Config::config_dir()?;
        let config_file = Config::config_file_path()?;
        let themes_dir = Config::themes_dir()?;

        // Clone paths for the watcher closure
        let config_file_clone = config_file.clone();
        let themes_dir_clone = themes_dir.clone();
        let current_theme = config.general.theme.clone();

        // Track last event times for debouncing
        let debounce_duration = Duration::from_millis(100);
        let mut last_config_event: Option<Instant> = None;
        let mut last_theme_event: Option<Instant> = None;

        let tx_clone = tx.clone();

        let mut watcher = RecommendedWatcher::new(
            move |result: Result<Event, notify::Error>| {
                match result {
                    Ok(event) => {
                        // Only handle modify events (writes)
                        if !matches!(event.kind, notify::EventKind::Modify(ModifyKind::Data(_))) {
                            return;
                        }

                        for path in &event.paths {
                            // Check if config file changed
                            if path == &config_file_clone {
                                // Debounce
                                let now = Instant::now();
                                if let Some(last) = last_config_event {
                                    if now.duration_since(last) < debounce_duration {
                                        continue;
                                    }
                                }
                                last_config_event = Some(now);

                                log::info!("Config file changed, reloading...");
                                match Config::load() {
                                    Ok(new_config) => {
                                        let _ = tx_clone.send(ConfigEvent::ConfigReloaded(new_config));
                                    }
                                    Err(e) => {
                                        log::error!("Failed to reload config: {}", e);
                                        let _ = tx_clone.send(ConfigEvent::ReloadError(e.to_string()));
                                    }
                                }
                            }

                            // Check if a theme file changed
                            if path.starts_with(&themes_dir_clone) {
                                if let Some(file_name) = path.file_stem() {
                                    let theme_name = file_name.to_string_lossy();

                                    // Only reload if it's the current theme
                                    if theme_name == current_theme {
                                        // Debounce
                                        let now = Instant::now();
                                        if let Some(last) = last_theme_event {
                                            if now.duration_since(last) < debounce_duration {
                                                continue;
                                            }
                                        }
                                        last_theme_event = Some(now);

                                        log::info!("Theme file changed, reloading...");
                                        match std::fs::read_to_string(path) {
                                            Ok(css) => {
                                                let _ = tx_clone.send(ConfigEvent::ThemeReloaded(css));
                                            }
                                            Err(e) => {
                                                log::error!("Failed to reload theme: {}", e);
                                                let _ = tx_clone.send(ConfigEvent::ReloadError(e.to_string()));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("Watch error: {:?}", e);
                    }
                }
            },
            NotifyConfig::default().with_poll_interval(Duration::from_secs(1)),
        ).map_err(|e| ConfigError::WatchError(e.to_string()))?;

        // Watch config directory (which includes config.toml)
        if config_dir.exists() {
            watcher.watch(&config_dir, RecursiveMode::NonRecursive)
                .map_err(|e| ConfigError::WatchError(e.to_string()))?;
            log::info!("Watching config directory: {:?}", config_dir);
        }

        // Watch themes directory
        if themes_dir.exists() {
            watcher.watch(&themes_dir, RecursiveMode::Recursive)
                .map_err(|e| ConfigError::WatchError(e.to_string()))?;
            log::info!("Watching themes directory: {:?}", themes_dir);
        }

        Ok(Self {
            _watcher: watcher,
            receiver: rx,
        })
    }

    /// Try to receive a config event without blocking
    pub fn try_recv(&self) -> Option<ConfigEvent> {
        self.receiver.try_recv().ok()
    }

    /// Get all pending events
    pub fn drain_events(&self) -> Vec<ConfigEvent> {
        let mut events = Vec::new();
        while let Ok(event) = self.receiver.try_recv() {
            events.push(event);
        }
        events
    }
}

/// Builder for creating a ConfigWatcher with custom options
pub struct ConfigWatcherBuilder {
    debounce_ms: u64,
    watch_themes: bool,
}

impl Default for ConfigWatcherBuilder {
    fn default() -> Self {
        Self {
            debounce_ms: 100,
            watch_themes: true,
        }
    }
}

impl ConfigWatcherBuilder {
    /// Create a new builder with default options
    pub fn new() -> Self {
        Self::default()
    }

    /// Set debounce duration in milliseconds
    pub fn debounce_ms(mut self, ms: u64) -> Self {
        self.debounce_ms = ms;
        self
    }

    /// Enable or disable theme watching
    pub fn watch_themes(mut self, watch: bool) -> Self {
        self.watch_themes = watch;
        self
    }

    /// Build the watcher
    pub fn build(self, config: &Config) -> Result<ConfigWatcher, ConfigError> {
        // For now, just use the standard constructor
        // Future: apply builder options
        ConfigWatcher::new(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_event_debug() {
        let config = Config::default();
        let event = ConfigEvent::ConfigReloaded(config);
        let debug_str = format!("{:?}", event);
        assert!(debug_str.contains("ConfigReloaded"));
    }

    #[test]
    fn test_reload_error_event() {
        let event = ConfigEvent::ReloadError("test error".to_string());
        match event {
            ConfigEvent::ReloadError(msg) => assert_eq!(msg, "test error"),
            _ => panic!("Expected ReloadError"),
        }
    }

    #[test]
    fn test_theme_reloaded_event() {
        let css = ":terminal { color: #fff; }".to_string();
        let event = ConfigEvent::ThemeReloaded(css.clone());
        match event {
            ConfigEvent::ThemeReloaded(content) => assert_eq!(content, css),
            _ => panic!("Expected ThemeReloaded"),
        }
    }

    #[test]
    fn test_watcher_builder_default() {
        let builder = ConfigWatcherBuilder::new();
        assert_eq!(builder.debounce_ms, 100);
        assert!(builder.watch_themes);
    }

    #[test]
    fn test_watcher_builder_fluent() {
        let builder = ConfigWatcherBuilder::new()
            .debounce_ms(200)
            .watch_themes(false);
        assert_eq!(builder.debounce_ms, 200);
        assert!(!builder.watch_themes);
    }
}
