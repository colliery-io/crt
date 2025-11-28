//! Configuration file watcher for hot-reloading
//!
//! Watches ~/.config/crt/config.toml and theme files for changes.

use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::sync::mpsc::{Receiver, channel};
use std::time::Instant;

use crate::config::Config;

/// Debounce window in milliseconds
const DEBOUNCE_MS: u128 = 100;

/// Events from the config watcher
#[derive(Debug, Clone)]
pub enum ConfigEvent {
    /// config.toml was modified
    ConfigChanged,
    /// Theme CSS file was modified
    ThemeChanged,
}

/// Watches config directory for changes
pub struct ConfigWatcher {
    _watcher: RecommendedWatcher,
    receiver: Receiver<ConfigEvent>,
    last_config_event: Option<Instant>,
    last_theme_event: Option<Instant>,
}

impl ConfigWatcher {
    /// Create a new config watcher
    pub fn new() -> Option<Self> {
        let config_dir = Config::config_dir()?;
        let config_path = config_dir.join("config.toml");

        let (tx, rx) = channel();

        // Clone paths for the closure
        let config_path_clone = config_path.clone();
        let themes_dir = config_dir.join("themes");

        let mut watcher = notify::recommended_watcher(move |res: Result<Event, _>| {
            if let Ok(event) = res {
                if event.kind.is_modify() || event.kind.is_create() {
                    for path in &event.paths {
                        if path == &config_path_clone {
                            let _ = tx.send(ConfigEvent::ConfigChanged);
                        } else if path.starts_with(&themes_dir)
                            && path.extension().map(|e| e == "css").unwrap_or(false)
                        {
                            let _ = tx.send(ConfigEvent::ThemeChanged);
                        }
                    }
                }
            }
        })
        .ok()?;

        // Watch the config directory
        watcher.watch(&config_dir, RecursiveMode::Recursive).ok()?;

        log::info!("Watching {:?} for config changes", config_dir);

        Some(Self {
            _watcher: watcher,
            receiver: rx,
            last_config_event: None,
            last_theme_event: None,
        })
    }

    /// Poll for config events (non-blocking) with debouncing
    pub fn poll(&mut self) -> Option<ConfigEvent> {
        while let Ok(event) = self.receiver.try_recv() {
            let now = Instant::now();
            match event {
                ConfigEvent::ConfigChanged => {
                    // Check if we should debounce this event
                    if let Some(last) = self.last_config_event {
                        if now.duration_since(last).as_millis() < DEBOUNCE_MS {
                            continue; // Skip this event, too soon after last one
                        }
                    }
                    self.last_config_event = Some(now);
                    return Some(ConfigEvent::ConfigChanged);
                }
                ConfigEvent::ThemeChanged => {
                    // Check if we should debounce this event
                    if let Some(last) = self.last_theme_event {
                        if now.duration_since(last).as_millis() < DEBOUNCE_MS {
                            continue; // Skip this event, too soon after last one
                        }
                    }
                    self.last_theme_event = Some(now);
                    return Some(ConfigEvent::ThemeChanged);
                }
            }
        }
        None
    }
}
