//! UI overlay state types.
//!
//! Groups transient UI state that overlays the terminal content:
//! search, bell, context menu, zoom indicator, toast, window rename, and theme overrides.

use std::time::{Duration, Instant};

use super::interaction::{ContextMenu, SearchState};
use super::overrides::OverrideState;

/// Window rename input state
#[derive(Debug, Clone, Default)]
pub struct WindowRenameState {
    /// Whether rename mode is active
    pub active: bool,
    /// Current input text
    pub input: String,
}

impl WindowRenameState {
    /// Start renaming with current window title
    pub fn start(&mut self, current_title: &str) {
        self.active = true;
        self.input = current_title.to_string();
    }

    /// Cancel renaming
    pub fn cancel(&mut self) {
        self.active = false;
        self.input.clear();
    }

    /// Confirm and return the new title
    pub fn confirm(&mut self) -> Option<String> {
        if self.active {
            self.active = false;
            let title = std::mem::take(&mut self.input);
            if title.is_empty() {
                None // Empty means reset to default
            } else {
                Some(title)
            }
        } else {
            None
        }
    }
}

/// Bell visual flash state
#[derive(Debug, Clone)]
pub struct BellState {
    /// When the bell was triggered (None if not active)
    pub triggered_at: Option<Instant>,
    /// Duration of the visual flash
    pub flash_duration: Duration,
    /// Flash intensity multiplier (from config)
    pub intensity: f32,
    /// Whether visual bell is enabled
    pub enabled: bool,
}

impl Default for BellState {
    fn default() -> Self {
        Self {
            triggered_at: None,
            flash_duration: Duration::from_millis(100),
            intensity: 0.3,
            enabled: true,
        }
    }
}

impl BellState {
    /// Create from config
    pub fn from_config(config: &crate::config::BellConfig) -> Self {
        Self {
            triggered_at: None,
            flash_duration: Duration::from_millis(config.flash_duration_ms),
            intensity: config.flash_intensity,
            enabled: config.visual,
        }
    }

    /// Trigger the bell (start visual flash)
    pub fn trigger(&mut self) {
        if self.enabled {
            self.triggered_at = Some(Instant::now());
        }
    }

    /// Get the current flash intensity (0.0 = no flash, up to configured intensity)
    pub fn flash_intensity(&self) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        match self.triggered_at {
            Some(triggered) => {
                let elapsed = triggered.elapsed();
                if elapsed >= self.flash_duration {
                    0.0
                } else {
                    // Fade out linearly, scaled by intensity
                    let progress =
                        1.0 - (elapsed.as_secs_f32() / self.flash_duration.as_secs_f32());
                    progress * self.intensity
                }
            }
            None => 0.0,
        }
    }

    /// Check if flash is still active
    pub fn is_active(&self) -> bool {
        self.flash_intensity() > 0.0
    }
}

/// UI overlay state (search, bell, context menu, zoom indicator, toast)
///
/// Groups transient UI state that overlays the terminal content.
#[derive(Default)]
pub struct UiState {
    /// Search state for find-in-terminal functionality
    pub search: SearchState,
    /// Bell state for visual flash
    pub bell: BellState,
    /// Context menu state
    pub context_menu: ContextMenu,
    /// Zoom indicator state (shows current zoom level temporarily)
    pub zoom_indicator: ZoomIndicator,
    /// Copy indicator state (shows brief "Copied!" feedback)
    pub copy_indicator: CopyIndicator,
    /// Toast notification for errors and status messages
    pub toast: Toast,
    /// Window rename input state
    pub window_rename: WindowRenameState,
    /// Active theme overrides from events (bell, command success/fail, focus)
    pub overrides: OverrideState,
    /// Pending theme change from context menu (processed by main loop)
    pub pending_theme: Option<String>,
}

/// Toast notification for errors and status messages
#[derive(Debug, Clone, Default)]
pub struct Toast {
    /// Message to display
    pub message: String,
    /// When the toast was triggered
    pub triggered_at: Option<Instant>,
    /// Display duration
    pub display_duration: Duration,
    /// Toast type (affects styling)
    pub toast_type: ToastType,
}

/// Type of toast notification
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum ToastType {
    #[default]
    Info,
    #[allow(dead_code)]
    Warning,
    Error,
}

impl Toast {
    /// Show a toast message
    pub fn show(&mut self, message: impl Into<String>, toast_type: ToastType) {
        self.message = message.into();
        self.triggered_at = Some(Instant::now());
        self.display_duration = Duration::from_secs(5);
        self.toast_type = toast_type;
    }

    /// Check if the toast should be visible
    pub fn is_visible(&self) -> bool {
        self.triggered_at
            .map(|t| t.elapsed() < self.display_duration)
            .unwrap_or(false)
    }

    /// Get the opacity (fades out over last 1s)
    pub fn opacity(&self) -> f32 {
        let Some(triggered_at) = self.triggered_at else {
            return 0.0;
        };
        let elapsed = triggered_at.elapsed();
        if elapsed >= self.display_duration {
            return 0.0;
        }
        let fade_start = self.display_duration.saturating_sub(Duration::from_secs(1));
        if elapsed < fade_start {
            1.0
        } else {
            let fade_elapsed = elapsed - fade_start;
            let fade_duration = Duration::from_secs(1);
            1.0 - (fade_elapsed.as_secs_f32() / fade_duration.as_secs_f32())
        }
    }
}

/// Zoom indicator state for font size feedback
#[derive(Debug, Clone, Default)]
pub struct ZoomIndicator {
    /// When the zoom was last changed (None if not showing)
    pub triggered_at: Option<Instant>,
    /// Current zoom scale (1.0 = 100%)
    pub scale: f32,
    /// Display duration before fade out
    pub display_duration: Duration,
}

impl ZoomIndicator {
    /// Trigger the zoom indicator with the current scale
    pub fn trigger(&mut self, scale: f32) {
        self.triggered_at = Some(Instant::now());
        self.scale = scale;
        self.display_duration = Duration::from_millis(1500);
    }

    /// Check if the indicator should be visible
    pub fn is_visible(&self) -> bool {
        self.triggered_at
            .map(|t| t.elapsed() < self.display_duration)
            .unwrap_or(false)
    }

    /// Get the opacity (fades out over last 500ms)
    pub fn opacity(&self) -> f32 {
        let Some(triggered_at) = self.triggered_at else {
            return 0.0;
        };
        let elapsed = triggered_at.elapsed();
        if elapsed >= self.display_duration {
            return 0.0;
        }
        let fade_start = self
            .display_duration
            .saturating_sub(Duration::from_millis(500));
        if elapsed < fade_start {
            1.0
        } else {
            let fade_elapsed = elapsed - fade_start;
            let fade_duration = Duration::from_millis(500);
            1.0 - (fade_elapsed.as_secs_f32() / fade_duration.as_secs_f32())
        }
    }
}

/// Copy indicator state (shows brief "Copied!" feedback)
#[derive(Debug, Clone, Default)]
pub struct CopyIndicator {
    /// When copy was triggered (None if not showing)
    pub triggered_at: Option<Instant>,
    /// Display duration before fade out
    pub display_duration: Duration,
}

impl CopyIndicator {
    /// Trigger the copy indicator
    pub fn trigger(&mut self) {
        self.triggered_at = Some(Instant::now());
        self.display_duration = Duration::from_millis(800);
    }

    /// Check if the indicator should be visible
    pub fn is_visible(&self) -> bool {
        self.triggered_at
            .map(|t| t.elapsed() < self.display_duration)
            .unwrap_or(false)
    }

    /// Get the opacity (fades out over last 300ms)
    pub fn opacity(&self) -> f32 {
        let Some(triggered_at) = self.triggered_at else {
            return 0.0;
        };
        let elapsed = triggered_at.elapsed();
        if elapsed >= self.display_duration {
            return 0.0;
        }
        let fade_start = self
            .display_duration
            .saturating_sub(Duration::from_millis(300));
        if elapsed < fade_start {
            1.0
        } else {
            let fade_elapsed = elapsed - fade_start;
            let fade_duration = Duration::from_millis(300);
            1.0 - (fade_elapsed.as_secs_f32() / fade_duration.as_secs_f32())
        }
    }
}
