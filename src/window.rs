//! Window state management
//!
//! Per-window state including shells, GPU resources, and interaction state.

use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::hash::Hasher;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crt_core::{AnsiColor, CellFlags, SemanticZone, ShellEvent, ShellTerminal, Size, SpawnOptions};
use crt_renderer::GlyphStyle;
use crt_theme::{AnsiPalette, EventOverride, Theme};
use winit::window::Window;

use crate::gpu::{SharedGpuState, WindowGpuState};
use crate::input::{detect_urls_in_line, merge_wrapped_urls};

/// Unique identifier for a terminal tab
pub type TabId = u64;

/// Map alacritty_terminal AnsiColor to RGBA array using theme palette
fn ansi_color_to_rgba(
    color: AnsiColor,
    palette: &AnsiPalette,
    default_fg: [f32; 4],
    default_bg: [f32; 4],
) -> [f32; 4] {
    use crt_core::AnsiColor::*;
    use crt_core::NamedColor;

    match color {
        // Named colors (0-7 normal, 8-15 bright)
        Named(named) => {
            let c = match named {
                NamedColor::Black => palette.black,
                NamedColor::Red => palette.red,
                NamedColor::Green => palette.green,
                NamedColor::Yellow => palette.yellow,
                NamedColor::Blue => palette.blue,
                NamedColor::Magenta => palette.magenta,
                NamedColor::Cyan => palette.cyan,
                NamedColor::White => palette.white,
                NamedColor::BrightBlack => palette.bright_black,
                NamedColor::BrightRed => palette.bright_red,
                NamedColor::BrightGreen => palette.bright_green,
                NamedColor::BrightYellow => palette.bright_yellow,
                NamedColor::BrightBlue => palette.bright_blue,
                NamedColor::BrightMagenta => palette.bright_magenta,
                NamedColor::BrightCyan => palette.bright_cyan,
                NamedColor::BrightWhite => palette.bright_white,
                // Foreground/Background use their actual theme colors
                NamedColor::Foreground => return default_fg,
                NamedColor::Background => return default_bg,
                // Dim variants use regular colors
                NamedColor::DimBlack => palette.black,
                NamedColor::DimRed => palette.red,
                NamedColor::DimGreen => palette.green,
                NamedColor::DimYellow => palette.yellow,
                NamedColor::DimBlue => palette.blue,
                NamedColor::DimMagenta => palette.magenta,
                NamedColor::DimCyan => palette.cyan,
                NamedColor::DimWhite => palette.white,
                // Cursor color - use foreground as default
                NamedColor::Cursor => return default_fg,
                // Bright foreground
                NamedColor::BrightForeground => palette.bright_white,
                NamedColor::DimForeground => palette.white,
            };
            c.to_array()
        }
        // Indexed colors (0-255)
        Indexed(idx) => {
            if idx < 16 {
                // First 16 are the base ANSI palette
                palette.get(idx).to_array()
            } else {
                // Extended colors (16-255): check for theme override first
                if let Some(color) = palette.get_extended(idx) {
                    color.to_array()
                } else {
                    // Fall back to calculated standard 256-color palette
                    AnsiPalette::calculate_extended(idx).to_array()
                }
            }
        }
        // Direct RGB color
        Spec(rgb) => [
            rgb.r as f32 / 255.0,
            rgb.g as f32 / 255.0,
            rgb.b as f32 / 255.0,
            1.0,
        ],
    }
}

/// Per-window state containing window handle, GPU state, shells, and interaction state
pub struct WindowState {
    pub window: Arc<Window>,
    pub gpu: WindowGpuState,
    // Map of tab_id -> shell (each window has its own tabs)
    pub shells: HashMap<TabId, ShellTerminal>,
    // Content hash to skip reshaping when unchanged (per tab)
    pub content_hashes: HashMap<TabId, u64>,
    // Window-specific sizing
    pub cols: usize,
    pub rows: usize,
    pub scale_factor: f32,
    // User font scale multiplier (1.0 = default)
    pub font_scale: f32,
    // Rendering state (dirty, frame_count, occluded, focused, cached)
    pub render: RenderState,
    // Interaction state (cursor, mouse, selection, URLs)
    pub interaction: InteractionState,
    // UI overlay state (search, bell, context menu)
    pub ui: UiState,
    // Custom window title (None = use default "CRT Terminal")
    pub custom_title: Option<String>,
    // Per-window theme
    pub theme: Theme,
    pub theme_name: String,
}

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

/// Event type that triggered an override
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverrideEventType {
    Bell,
    CommandSuccess,
    CommandFail,
    FocusGained,
    FocusLost,
}

impl From<ShellEvent> for OverrideEventType {
    fn from(event: ShellEvent) -> Self {
        match event {
            ShellEvent::Bell => OverrideEventType::Bell,
            ShellEvent::CommandSuccess => OverrideEventType::CommandSuccess,
            ShellEvent::CommandFail(_) => OverrideEventType::CommandFail,
        }
    }
}

/// Active theme override state (triggered by events like bell, command success/fail)
///
/// Stores a temporary theme override with timing for duration-based effects.
#[derive(Debug, Clone)]
pub struct ActiveOverride {
    /// The event type that triggered this override
    pub event_type: OverrideEventType,
    /// The override properties from the theme
    pub properties: EventOverride,
    /// When the override was triggered
    pub triggered_at: Instant,
}

#[allow(dead_code)]
impl ActiveOverride {
    /// Create a new active override from an event
    pub fn new(event_type: OverrideEventType, properties: EventOverride) -> Self {
        Self {
            event_type,
            properties,
            triggered_at: Instant::now(),
        }
    }

    /// Get the duration of this override in milliseconds
    pub fn duration_ms(&self) -> u32 {
        self.properties.duration_ms
    }

    /// Check if the override is still active
    pub fn is_active(&self) -> bool {
        let duration = Duration::from_millis(self.properties.duration_ms as u64);
        self.triggered_at.elapsed() < duration
    }

    /// Get the remaining intensity (1.0 at start, fades to 0.0 at end)
    ///
    /// Uses a smooth ease-out curve for natural fading.
    pub fn intensity(&self) -> f32 {
        let duration = Duration::from_millis(self.properties.duration_ms as u64);
        let elapsed = self.triggered_at.elapsed();

        if elapsed >= duration {
            return 0.0;
        }

        let progress = elapsed.as_secs_f32() / duration.as_secs_f32();
        // Ease-out: 1 - progress^2 gives smooth fade
        1.0 - (progress * progress)
    }

    /// Get the elapsed time since the override was triggered
    pub fn elapsed(&self) -> Duration {
        self.triggered_at.elapsed()
    }
}

/// Manager for active theme overrides
///
/// Handles multiple simultaneous overrides with priority and duration tracking.
#[derive(Debug, Clone, Default)]
pub struct OverrideState {
    /// Currently active overrides (may have multiple from different events)
    pub active: Vec<ActiveOverride>,
    /// Track which effects are currently patched (e.g., "starfield", "particles", "sprite")
    patched_effects: HashSet<String>,
}

#[allow(dead_code)]
impl OverrideState {
    /// Add a new override, potentially replacing an existing one of the same type
    pub fn add(&mut self, event_type: OverrideEventType, properties: EventOverride) {
        // Remove any existing override of the same type
        self.active.retain(|o| o.event_type != event_type);

        // Add the new override
        self.active
            .push(ActiveOverride::new(event_type, properties));
    }

    /// Check if any override is active
    pub fn has_active(&self) -> bool {
        self.active.iter().any(|o| o.is_active())
    }

    /// Update state, removing expired overrides
    ///
    /// Returns true if any overrides were removed (caller may want to reset theme)
    pub fn update(&mut self) -> bool {
        let before_len = self.active.len();
        self.active.retain(|o| o.is_active());
        let removed = self.active.len() < before_len;

        if removed {
            log::debug!("Theme overrides expired, {} remaining", self.active.len());
        }

        removed
    }

    /// Check if an effect is currently patched
    pub fn is_patched(&self, effect: &str) -> bool {
        self.patched_effects.contains(effect)
    }

    /// Mark an effect as patched
    pub fn set_patched(&mut self, effect: &str) {
        self.patched_effects.insert(effect.to_string());
    }

    /// Clear the patched state for an effect
    pub fn clear_patched(&mut self, effect: &str) {
        self.patched_effects.remove(effect);
    }

    /// Get the most recent active override for sprite patching
    pub fn get_sprite_patch(&self) -> Option<&crt_theme::SpritePatch> {
        self.active
            .iter()
            .filter(|o| o.is_active())
            .filter_map(|o| o.properties.sprite_patch.as_ref())
            .next_back()
    }

    /// Get the most recent active override for sprite overlay
    pub fn get_sprite_overlay(&self) -> Option<&crt_theme::SpriteOverlay> {
        self.active
            .iter()
            .filter(|o| o.is_active())
            .filter_map(|o| o.properties.sprite_overlay.as_ref())
            .next_back()
    }

    /// Get blended foreground color from active overrides
    pub fn get_foreground(&self) -> Option<crt_theme::Color> {
        // Return the foreground from the most recent active override that has one
        self.active
            .iter()
            .filter(|o| o.is_active())
            .filter_map(|o| o.properties.foreground)
            .next_back()
    }

    /// Get blended background from active overrides
    pub fn get_background(&self) -> Option<crt_theme::LinearGradient> {
        self.active
            .iter()
            .filter(|o| o.is_active())
            .filter_map(|o| o.properties.background)
            .next_back()
    }

    /// Get cursor color from active overrides
    pub fn get_cursor_color(&self) -> Option<crt_theme::Color> {
        self.active
            .iter()
            .filter(|o| o.is_active())
            .filter_map(|o| o.properties.cursor_color)
            .next_back()
    }

    /// Get cursor shape from active overrides
    pub fn get_cursor_shape(&self) -> Option<crt_theme::CursorShape> {
        self.active
            .iter()
            .filter(|o| o.is_active())
            .filter_map(|o| o.properties.cursor_shape)
            .next_back()
    }

    /// Get text shadow from active overrides
    pub fn get_text_shadow(&self) -> Option<crt_theme::TextShadow> {
        self.active
            .iter()
            .filter(|o| o.is_active())
            .filter_map(|o| o.properties.text_shadow)
            .next_back()
    }

    /// Get flash color from active overrides
    pub fn get_flash_color(&self) -> Option<crt_theme::Color> {
        self.active
            .iter()
            .filter(|o| o.is_active())
            .filter_map(|o| o.properties.flash_color)
            .next_back()
    }

    /// Get flash intensity from active overrides
    pub fn get_flash_intensity(&self) -> Option<f32> {
        self.active
            .iter()
            .filter(|o| o.is_active())
            .filter_map(|o| o.properties.flash_intensity)
            .next_back()
    }

    /// Get the effective flash for rendering (color and faded intensity)
    ///
    /// Returns (color, current_intensity) where current_intensity accounts for
    /// both the configured intensity and the override's fade-out progress.
    pub fn get_effective_flash(&self) -> Option<(crt_theme::Color, f32)> {
        // Find the most recent active override with flash properties
        self.active
            .iter()
            .rfind(|o| o.is_active() && o.properties.flash_color.is_some())
            .map(|o| {
                let color = o.properties.flash_color.unwrap();
                let base_intensity = o.properties.flash_intensity.unwrap_or(0.5);
                let fade = o.intensity(); // Uses ease-out curve
                (color, base_intensity * fade)
            })
    }

    /// Get the most recent active starfield patch
    pub fn get_starfield_patch(&self) -> Option<&crt_theme::StarfieldPatch> {
        self.active
            .iter()
            .filter(|o| o.is_active())
            .filter_map(|o| o.properties.starfield_patch.as_ref())
            .next_back()
    }

    /// Get the most recent active particle patch
    pub fn get_particle_patch(&self) -> Option<&crt_theme::ParticlePatch> {
        self.active
            .iter()
            .filter(|o| o.is_active())
            .filter_map(|o| o.properties.particle_patch.as_ref())
            .next_back()
    }

    /// Get the most recent active grid patch
    pub fn get_grid_patch(&self) -> Option<&crt_theme::GridPatch> {
        self.active
            .iter()
            .filter(|o| o.is_active())
            .filter_map(|o| o.properties.grid_patch.as_ref())
            .next_back()
    }

    /// Get the most recent active rain patch
    pub fn get_rain_patch(&self) -> Option<&crt_theme::RainPatch> {
        self.active
            .iter()
            .filter(|o| o.is_active())
            .filter_map(|o| o.properties.rain_patch.as_ref())
            .next_back()
    }

    /// Get the most recent active matrix patch
    pub fn get_matrix_patch(&self) -> Option<&crt_theme::MatrixPatch> {
        self.active
            .iter()
            .filter(|o| o.is_active())
            .filter_map(|o| o.properties.matrix_patch.as_ref())
            .next_back()
    }

    /// Get the most recent active shape patch
    pub fn get_shape_patch(&self) -> Option<&crt_theme::ShapePatch> {
        self.active
            .iter()
            .filter(|o| o.is_active())
            .filter_map(|o| o.properties.shape_patch.as_ref())
            .next_back()
    }

    /// Clear all overrides
    pub fn clear(&mut self) {
        self.active.clear();
    }

    /// Clear overrides of a specific event type
    pub fn clear_event(&mut self, event_type: OverrideEventType) {
        let before_len = self.active.len();
        self.active.retain(|o| o.event_type != event_type);
        if self.active.len() < before_len {
            log::debug!("Cleared {:?} override", event_type);
        }
    }
}

/// Search match position in terminal
#[derive(Debug, Clone, Copy)]
pub struct SearchMatch {
    /// Line number (grid-relative: negative = history, 0+ = visible)
    pub line: i32,
    /// Starting column
    pub start_col: usize,
    /// Ending column (exclusive)
    pub end_col: usize,
}

/// Search state for find-in-terminal functionality
#[derive(Debug, Clone, Default)]
pub struct SearchState {
    /// Whether search mode is active
    pub active: bool,
    /// Current search query
    pub query: String,
    /// All matches found
    pub matches: Vec<SearchMatch>,
    /// Index of current/focused match
    pub current_match: usize,
}

/// Context menu item
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContextMenuItem {
    Copy,
    Paste,
    SelectAll,
    Separator,
    /// Parent item that shows submenu on hover
    Themes,
    /// Individual theme (shown in submenu)
    Theme(String),
}

impl ContextMenuItem {
    /// Get the display label for this menu item
    pub fn label(&self) -> String {
        match self {
            ContextMenuItem::Copy => "Copy".to_string(),
            ContextMenuItem::Paste => "Paste".to_string(),
            ContextMenuItem::SelectAll => "Select All".to_string(),
            ContextMenuItem::Separator => String::new(),
            ContextMenuItem::Themes => "Theme".to_string(),
            ContextMenuItem::Theme(name) => name.clone(),
        }
    }

    /// Get the keyboard shortcut hint (or arrow indicator for submenu)
    pub fn shortcut(&self) -> &'static str {
        #[cfg(target_os = "macos")]
        match self {
            ContextMenuItem::Copy => "Cmd+C",
            ContextMenuItem::Paste => "Cmd+V",
            ContextMenuItem::SelectAll => "Cmd+A",
            ContextMenuItem::Themes => "\u{25B6}", // Right-pointing triangle for submenu
            ContextMenuItem::Separator | ContextMenuItem::Theme(_) => "",
        }
        #[cfg(not(target_os = "macos"))]
        match self {
            ContextMenuItem::Copy => "Ctrl+C",
            ContextMenuItem::Paste => "Ctrl+V",
            ContextMenuItem::SelectAll => "Ctrl+A",
            ContextMenuItem::Themes => "\u{25B6}", // Right-pointing triangle for submenu
            ContextMenuItem::Separator | ContextMenuItem::Theme(_) => "",
        }
    }

    /// Returns true if this is a submenu parent
    pub fn has_submenu(&self) -> bool {
        matches!(self, ContextMenuItem::Themes)
    }

    /// Returns true if this is a separator item
    pub fn is_separator(&self) -> bool {
        matches!(self, ContextMenuItem::Separator)
    }

    /// Returns true if this is a selectable (non-separator) item
    pub fn is_selectable(&self) -> bool {
        !self.is_separator()
    }

    /// Get the base edit menu items
    pub fn edit_items() -> Vec<ContextMenuItem> {
        vec![
            ContextMenuItem::Copy,
            ContextMenuItem::Paste,
            ContextMenuItem::SelectAll,
        ]
    }
}

/// Interaction state (cursor, mouse, selection, URLs)
///
/// Groups state related to user interaction and mouse handling.
#[derive(Default)]
pub struct InteractionState {
    /// Current cursor position in pixels
    pub cursor_position: (f32, f32),
    /// Last click time for double-click detection
    pub last_click_time: Option<Instant>,
    /// Last clicked tab for double-click detection
    pub last_click_tab: Option<TabId>,
    /// Whether mouse button is currently pressed
    pub mouse_pressed: bool,
    /// Click count for multi-click selection (1=single, 2=word, 3=line)
    pub selection_click_count: u8,
    /// Last selection click time for multi-click detection
    pub last_selection_click_time: Option<Instant>,
    /// Last selection click position (col, line) for multi-click detection
    pub last_selection_click_pos: Option<(usize, usize)>,
    /// Detected URLs in current viewport
    pub detected_urls: Vec<crate::input::DetectedUrl>,
    /// Index of currently hovered URL (for hover underline effect)
    pub hovered_url_index: Option<usize>,
}

/// Render state (dirty tracking, frame count, visibility)
///
/// Groups state related to rendering decisions and caching.
#[derive(Default)]
pub struct RenderState {
    /// Whether the window needs redrawing
    pub dirty: bool,
    /// Frame counter for periodic operations
    pub frame_count: u32,
    /// Window is occluded (hidden, minimized, or fully covered)
    pub occluded: bool,
    /// Window has keyboard focus (effects animate only when focused)
    pub focused: bool,
    /// Cached decorations from last content update
    pub cached: CachedRenderState,
    /// Paste operation just occurred - normalize INVERSE flags on next render
    pub paste_pending: bool,
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

/// Context menu state
#[derive(Debug, Clone, Default)]
pub struct ContextMenu {
    /// Whether the context menu is visible
    pub visible: bool,
    /// Position of the menu (top-left corner)
    pub x: f32,
    pub y: f32,
    /// Currently hovered item index (from mouse)
    pub hovered_item: Option<usize>,
    /// Currently focused item index (from keyboard navigation)
    pub focused_item: Option<usize>,
    /// Menu dimensions (computed during render)
    pub width: f32,
    pub height: f32,
    pub item_height: f32,
    /// Available themes for theme picker section
    pub themes: Vec<String>,
    /// Currently active theme name
    pub current_theme: String,
    /// Whether the theme submenu is visible
    pub submenu_visible: bool,
    /// Submenu position (top-left corner)
    pub submenu_x: f32,
    pub submenu_y: f32,
    /// Submenu dimensions
    pub submenu_width: f32,
    pub submenu_height: f32,
    /// Hovered item in submenu
    pub submenu_hovered_item: Option<usize>,
}

impl ContextMenu {
    /// Build the main menu items (Themes shown as a parent item, not expanded)
    pub fn items(&self) -> Vec<ContextMenuItem> {
        let mut items = ContextMenuItem::edit_items();
        if !self.themes.is_empty() {
            items.push(ContextMenuItem::Separator);
            items.push(ContextMenuItem::Themes);
        }
        items
    }

    /// Build the theme submenu items
    pub fn theme_items(&self) -> Vec<ContextMenuItem> {
        self.themes
            .iter()
            .map(|name| ContextMenuItem::Theme(name.clone()))
            .collect()
    }

    /// Show the context menu at the given position
    pub fn show(&mut self, x: f32, y: f32) {
        self.visible = true;
        self.x = x;
        self.y = y;
        self.hovered_item = None;
        self.focused_item = Some(0); // Focus first item for keyboard accessibility
        self.submenu_visible = false;
        self.submenu_hovered_item = None;
    }

    /// Hide the context menu
    pub fn hide(&mut self) {
        self.visible = false;
        self.hovered_item = None;
        self.focused_item = None;
        self.submenu_visible = false;
        self.submenu_hovered_item = None;
    }

    /// Move focus to the next selectable item (wraps around, skips separators)
    pub fn focus_next(&mut self) {
        if !self.visible {
            return;
        }
        let items = self.items();
        let item_count = items.len();
        if item_count == 0 {
            return;
        }

        let start = self.focused_item.unwrap_or(0);
        for i in 1..=item_count {
            let idx = (start + i) % item_count;
            if items[idx].is_selectable() {
                self.focused_item = Some(idx);
                break;
            }
        }
        // Clear hover when using keyboard
        self.hovered_item = None;
    }

    /// Move focus to the previous selectable item (wraps around, skips separators)
    pub fn focus_prev(&mut self) {
        if !self.visible {
            return;
        }
        let items = self.items();
        let item_count = items.len();
        if item_count == 0 {
            return;
        }

        let start = self.focused_item.unwrap_or(0);
        for i in 1..=item_count {
            let idx = (start + item_count - i) % item_count;
            if items[idx].is_selectable() {
                self.focused_item = Some(idx);
                break;
            }
        }
        // Clear hover when using keyboard
        self.hovered_item = None;
    }

    /// Get the currently focused item
    pub fn get_focused_item(&self) -> Option<ContextMenuItem> {
        let items = self.items();
        self.focused_item.and_then(|idx| items.get(idx).cloned())
    }

    /// Check if a point is inside the main menu
    pub fn contains(&self, x: f32, y: f32) -> bool {
        if !self.visible {
            return false;
        }
        x >= self.x && x <= self.x + self.width && y >= self.y && y <= self.y + self.height
    }

    /// Check if a point is inside the submenu
    pub fn contains_submenu(&self, x: f32, y: f32) -> bool {
        if !self.submenu_visible {
            return false;
        }
        x >= self.submenu_x
            && x <= self.submenu_x + self.submenu_width
            && y >= self.submenu_y
            && y <= self.submenu_y + self.submenu_height
    }

    /// Get the menu item at the given position (main menu only)
    pub fn item_at(&self, x: f32, y: f32) -> Option<ContextMenuItem> {
        if !self.contains(x, y) {
            return None;
        }
        let items = self.items();
        let rel_y = y - self.y;
        let index = (rel_y / self.item_height) as usize;
        items.get(index).cloned()
    }

    /// Get the submenu item at the given position
    pub fn submenu_item_at(&self, x: f32, y: f32) -> Option<ContextMenuItem> {
        if !self.contains_submenu(x, y) {
            return None;
        }
        let items = self.theme_items();
        let rel_y = y - self.submenu_y;
        let index = (rel_y / self.item_height) as usize;
        items.get(index).cloned()
    }

    /// Check if a theme name is the currently active theme
    pub fn is_current_theme(&self, name: &str) -> bool {
        self.current_theme == name
    }

    /// Update hover state based on mouse position
    pub fn update_hover(&mut self, x: f32, y: f32) {
        if !self.visible {
            self.hovered_item = None;
            self.submenu_hovered_item = None;
            return;
        }

        // Check submenu first
        if self.submenu_visible && self.contains_submenu(x, y) {
            let rel_y = y - self.submenu_y;
            self.submenu_hovered_item = Some((rel_y / self.item_height) as usize);
            // Keep main menu item hovered (the Themes item)
            return;
        } else {
            self.submenu_hovered_item = None;
        }

        // Check main menu
        if self.contains(x, y) {
            let rel_y = y - self.y;
            self.hovered_item = Some((rel_y / self.item_height) as usize);
        } else {
            self.hovered_item = None;
            // Hide submenu when mouse leaves both menus
            if !self.contains_submenu(x, y) {
                self.submenu_visible = false;
            }
        }
    }

    /// Get the index of the Themes item in the main menu
    pub fn themes_item_index(&self) -> Option<usize> {
        self.items()
            .iter()
            .position(|item| matches!(item, ContextMenuItem::Themes))
    }
}

/// Cursor position info returned from text buffer update
#[derive(Debug, Clone, Copy)]
pub struct CursorInfo {
    /// X position in pixels
    pub x: f32,
    /// Y position in pixels
    pub y: f32,
    /// Cell width in pixels
    pub cell_width: f32,
    /// Cell height in pixels
    pub cell_height: f32,
    /// Whether the cursor should be visible (false if terminal hid it via escape sequence)
    pub visible: bool,
    /// Cursor shape requested by the terminal/application
    pub shape: crt_core::CursorShape,
}

/// Text decoration (underline, strikethrough, or background)
#[derive(Debug, Clone, Copy)]
pub struct TextDecoration {
    /// X position in pixels
    pub x: f32,
    /// Y position in pixels
    pub y: f32,
    /// Cell width in pixels
    pub cell_width: f32,
    /// Cell height in pixels
    pub cell_height: f32,
    /// Decoration color
    pub color: [f32; 4],
    /// Decoration type
    pub kind: DecorationKind,
}

/// Types of text decoration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecorationKind {
    /// Cell background color
    Background,
    /// Underline decoration
    Underline,
    /// Strikethrough decoration
    Strikethrough,
}

/// Result from text buffer update
pub struct TextBufferUpdateResult {
    pub cursor: CursorInfo,
    pub decorations: Vec<TextDecoration>,
}

/// Collected cell data for single-pass processing
/// Avoids multiple terminal.renderable_content() calls
#[derive(Clone, Copy)]
struct CollectedCell {
    col: usize,
    grid_line: i32,
    c: char,
    flags: CellFlags,
    fg: AnsiColor,
    bg: AnsiColor,
}

/// Cached rendering state that persists across frames
#[derive(Default)]
pub struct CachedRenderState {
    /// Cached decorations from last content update
    pub decorations: Vec<TextDecoration>,
    /// Cached cursor info
    pub cursor: Option<CursorInfo>,
    /// Reusable line text buffer for URL detection (cleared and reused each update)
    line_texts: std::collections::BTreeMap<i32, String>,
    /// Reusable cell collection buffer (cleared and reused each update)
    collected_cells: Vec<CollectedCell>,
}

impl WindowState {
    /// Set the theme for this window, updating all GPU resources
    pub fn set_theme(&mut self, name: &str, theme: Theme) {
        self.theme_name = name.to_string();
        self.theme = theme.clone();

        // Update effect pipeline with new theme
        self.gpu.effect_pipeline.set_theme(theme.clone());

        // Update tab bar theme
        self.gpu.tab_bar.set_theme(theme.tabs);

        // Update cursor colors
        self.gpu.terminal_vello.set_cursor_color([
            theme.cursor_color.r,
            theme.cursor_color.g,
            theme.cursor_color.b,
            theme.cursor_color.a,
        ]);
        self.gpu
            .terminal_vello
            .set_cursor_glow(theme.cursor_glow.map(|g| {
                (
                    [g.color.r, g.color.g, g.color.b, g.color.a],
                    g.radius,
                    g.intensity,
                )
            }));

        // Mark window as needing redraw
        self.render.dirty = true;
    }

    /// Update text buffer for this window's active shell
    ///
    /// Returns cursor position and decorations if content changed, None otherwise
    pub fn update_text_buffer(
        &mut self,
        shared_gpu: &SharedGpuState,
    ) -> Option<TextBufferUpdateResult> {
        let active_tab_id = self.gpu.tab_bar.active_tab_id();
        let shell = active_tab_id.and_then(|id| self.shells.get(&id));

        shell?;
        let shell = shell.unwrap();
        let terminal = shell.terminal();

        // Compute content hash to avoid re-rendering unchanged content
        let mut hasher = DefaultHasher::new();
        let content = terminal.renderable_content();
        hasher.write_i32(content.cursor.point.line.0);
        hasher.write_usize(content.cursor.point.column.0);
        // Include cursor shape in hash - programs like Claude Code change cursor style
        let cursor_shape_discriminant = match content.cursor.shape {
            crt_core::CursorShape::Block => 0u8,
            crt_core::CursorShape::Underline => 1u8,
            crt_core::CursorShape::Beam => 2u8,
            crt_core::CursorShape::HollowBlock => 3u8,
            crt_core::CursorShape::Hidden => 4u8,
        };
        hasher.write_u8(cursor_shape_discriminant);
        // Include cursor visibility mode
        hasher.write_u8(if terminal.cursor_mode_visible() { 1 } else { 0 });
        for cell in content.display_iter {
            hasher.write_u32(cell.c as u32);
        }
        let content_hash = hasher.finish();

        // Check if content changed
        let tab_id = active_tab_id.unwrap();
        let prev_hash = self.content_hashes.get(&tab_id).copied().unwrap_or(0);
        log::debug!(
            "update_text_buffer: prev_hash={}, content_hash={}, will_render={}",
            prev_hash,
            content_hash,
            prev_hash == 0 || content_hash != prev_hash
        );
        if content_hash == prev_hash && prev_hash != 0 {
            return None; // No changes
        }
        self.content_hashes.insert(tab_id, content_hash);

        // Get content offset (excluding tab bar)
        let (offset_x, offset_y) = self.gpu.tab_bar.content_offset();

        // Re-read content since we consumed it above for hashing
        let content = terminal.renderable_content();
        self.gpu.grid_renderer.clear();
        self.gpu.output_grid_renderer.clear();

        let cell_width = self.gpu.glyph_cache.cell_width();
        let line_height = self.gpu.glyph_cache.line_height();
        let padding = 10.0 * self.scale_factor;

        // Get display offset to convert grid lines to viewport lines
        let display_offset = terminal.display_offset() as i32;

        // Cursor info
        let cursor = content.cursor;
        let cursor_point = cursor.point;
        // Check cursor visibility via TermMode::SHOW_CURSOR (CSI ?25h/l)
        let cursor_visible = terminal.cursor_mode_visible();

        // Compute cursor position (adjust for scroll offset)
        let cursor_viewport_line = cursor_point.line.0 + display_offset;
        let cursor_x = offset_x + padding + (cursor_point.column.0 as f32 * cell_width);
        let cursor_y = offset_y + padding + (cursor_viewport_line as f32 * line_height);

        // Single pass: collect cells AND build line text for URL detection
        // This avoids a second terminal.renderable_content() call
        // Reuse cached collections to avoid per-update allocations
        // Clear line_texts values (keeping keys to reuse String allocations)
        for s in self.render.cached.line_texts.values_mut() {
            s.clear();
        }
        self.render.cached.collected_cells.clear();

        let mut inverse_count = 0;
        let mut total_cells = 0;
        let mut line1_cells = 0;
        for cell in content.display_iter {
            let viewport_line = cell.point.line.0 + display_offset;
            self.render
                .cached
                .line_texts
                .entry(viewport_line)
                .or_default()
                .push(cell.c);

            // Track cells on line 1 for debugging paste issue
            if cell.point.line.0 == 1 {
                line1_cells += 1;
                if line1_cells <= 20 {
                    log::debug!(
                        "Line1 cell: col={}, char='{}', inverse={}, fg={:?}, bg={:?}",
                        cell.point.column.0,
                        cell.c,
                        cell.flags.contains(CellFlags::INVERSE),
                        cell.fg,
                        cell.bg
                    );
                }
            }

            // Track INVERSE cells for debugging
            if cell.flags.contains(CellFlags::INVERSE) {
                inverse_count += 1;
                if inverse_count <= 5 {
                    log::debug!(
                        "INVERSE cell: line={}, col={}, char='{}', fg={:?}, bg={:?}",
                        cell.point.line.0,
                        cell.point.column.0,
                        cell.c,
                        cell.fg,
                        cell.bg
                    );
                }
            }
            total_cells += 1;

            // Collect cell data for rendering pass
            self.render.cached.collected_cells.push(CollectedCell {
                col: cell.point.column.0,
                grid_line: cell.point.line.0,
                c: cell.c,
                flags: cell.flags,
                fg: cell.fg,
                bg: cell.bg,
            });
        }
        if inverse_count > 0 {
            log::info!(
                "Collected {} cells, {} with INVERSE flag",
                total_cells,
                inverse_count
            );
        }

        // Normalize INVERSE flag on paste to fix visual boundary issue
        // zsh enables INVERSE mid-line for paste highlighting, creating an ugly
        // discontinuity at the cursor position. Clear INVERSE from all cells on
        // lines with mixed INVERSE states during paste operations only.
        if self.render.paste_pending {
            // Count INVERSE vs non-INVERSE cells per line (skip whitespace)
            let mut line_stats: HashMap<i32, (usize, usize)> = HashMap::new();
            let mut total_inverse = 0usize;
            for cell in &self.render.cached.collected_cells {
                if cell.c == ' ' || cell.c == '\0' {
                    continue;
                }
                let entry = line_stats.entry(cell.grid_line).or_insert((0, 0));
                if cell.flags.contains(CellFlags::INVERSE) {
                    entry.0 += 1;
                    total_inverse += 1;
                } else {
                    entry.1 += 1;
                }
            }

            // Find lines with mixed INVERSE states
            let mixed_lines: HashSet<i32> = line_stats
                .iter()
                .filter(|(_, (inv, non_inv))| *inv > 0 && *non_inv > 0)
                .map(|(line, _)| *line)
                .collect();

            if !mixed_lines.is_empty() {
                // Found mixed lines - normalize by adding INVERSE to all cells
                // This keeps the reverse highlight look that zsh paste provides
                log::info!(
                    "Paste: adding INVERSE to all cells on {} lines with mixed states",
                    mixed_lines.len()
                );
                for cell in &mut self.render.cached.collected_cells {
                    if mixed_lines.contains(&cell.grid_line) {
                        cell.flags.insert(CellFlags::INVERSE);
                    }
                }
                self.render.paste_pending = false;
            } else if total_inverse == 0 {
                // No INVERSE cells yet - PTY hasn't responded, keep waiting
                log::debug!("Paste: waiting for PTY response (no INVERSE cells yet)");
            } else {
                // INVERSE exists but no mixed lines - nothing to normalize, clear flag
                self.render.paste_pending = false;
            }
        }

        // Detect URLs before rendering so we can underline them with text color
        self.interaction.detected_urls.clear();
        for (viewport_line, line_text) in &self.render.cached.line_texts {
            let urls = detect_urls_in_line(line_text, *viewport_line as usize);
            self.interaction.detected_urls.extend(urls);
        }
        // Merge URLs that wrap across multiple lines
        merge_wrapped_urls(
            &mut self.interaction.detected_urls,
            &self.render.cached.line_texts,
            self.cols,
        );

        // Check if shell supports OSC 133 semantic zones
        let has_semantic_zones = terminal.has_semantic_zones();

        // Collect decorations (backgrounds, underline, strikethrough)
        let mut decorations: Vec<TextDecoration> = Vec::new();

        // Get theme colors
        let palette = &self.gpu.effect_pipeline.theme().palette;
        let default_fg = self.gpu.effect_pipeline.theme().foreground.to_array();
        // Use the bottom of the gradient as default background (typically the darker color)
        let default_bg = self
            .gpu
            .effect_pipeline
            .theme()
            .background
            .bottom
            .to_array();

        // Render cells from collected data (avoids second terminal.renderable_content() call)
        for cell in &self.render.cached.collected_cells {
            let col = cell.col;
            // Convert grid line to viewport line (grid lines can be negative for history)
            let grid_line = cell.grid_line;
            let viewport_line = grid_line + display_offset;

            let x = offset_x + padding + (col as f32 * cell_width);
            let y = offset_y + padding + (viewport_line as f32 * line_height);

            let flags = cell.flags;

            // Handle INVERSE flag - swap foreground and background colors
            let (fg_ansi, bg_ansi) = if flags.contains(CellFlags::INVERSE) {
                (cell.bg, cell.fg)
            } else {
                (cell.fg, cell.bg)
            };

            // Get foreground color
            let mut fg_color = ansi_color_to_rgba(fg_ansi, palette, default_fg, default_bg);

            // Apply DIM flag by reducing alpha
            if flags.contains(CellFlags::DIM) {
                fg_color[3] *= 0.5;
            }

            // Get background color and add decoration if non-default
            // Skip spacer cells (for wide characters) and hidden cells - they shouldn't have
            // their own background decorations as this causes visual artifacts
            let is_spacer =
                flags.intersects(CellFlags::WIDE_CHAR_SPACER | CellFlags::LEADING_WIDE_CHAR_SPACER);
            let is_hidden = flags.contains(CellFlags::HIDDEN);

            if !is_spacer && !is_hidden {
                let bg_color = ansi_color_to_rgba(bg_ansi, palette, default_fg, default_bg);
                if bg_color != default_bg {
                    decorations.push(TextDecoration {
                        x,
                        y,
                        cell_width,
                        cell_height: line_height,
                        color: bg_color,
                        kind: DecorationKind::Background,
                    });
                }
            }

            // Collect underline decorations
            if flags.intersects(CellFlags::UNDERLINE | CellFlags::DOUBLE_UNDERLINE) {
                decorations.push(TextDecoration {
                    x,
                    y,
                    cell_width,
                    cell_height: line_height,
                    color: fg_color,
                    kind: DecorationKind::Underline,
                });
            }

            // Collect strikethrough decorations
            if flags.contains(CellFlags::STRIKEOUT) {
                decorations.push(TextDecoration {
                    x,
                    y,
                    cell_width,
                    cell_height: line_height,
                    color: fg_color,
                    kind: DecorationKind::Strikethrough,
                });
            }

            // Check if this cell is part of the hovered URL and add underline (supports multi-line URLs)
            let in_hovered_url = if let Some(hovered_idx) = self.interaction.hovered_url_index
                && let Some(url) = self.interaction.detected_urls.get(hovered_idx)
            {
                let vp_line = viewport_line as usize;
                vp_line >= url.line && vp_line <= url.end_line && {
                    if url.line == url.end_line {
                        // Single-line URL
                        col >= url.start_col && col < url.end_col
                    } else if vp_line == url.line {
                        // First line of multi-line URL
                        col >= url.start_col
                    } else if vp_line == url.end_line {
                        // Last line of multi-line URL
                        col < url.end_col
                    } else {
                        // Middle line - entire line is part of URL
                        true
                    }
                }
            } else {
                false
            };
            if in_hovered_url {
                decorations.push(TextDecoration {
                    x,
                    y,
                    cell_width,
                    cell_height: line_height,
                    color: fg_color,
                    kind: DecorationKind::Underline,
                });
            }

            // Check if this cell is part of a search match and add background highlight
            if self.ui.search.active && !self.ui.search.matches.is_empty() {
                let highlight_style = &self.gpu.effect_pipeline.theme().highlight;
                for (match_idx, search_match) in self.ui.search.matches.iter().enumerate() {
                    // Compare against grid_line (search matches use grid-relative coordinates)
                    if search_match.line == grid_line
                        && col >= search_match.start_col
                        && col < search_match.end_col
                    {
                        // Use brighter color for current match, theme color for others
                        let highlight_color = if match_idx == self.ui.search.current_match {
                            highlight_style.current_background.to_array()
                        } else {
                            highlight_style.background.to_array()
                        };
                        decorations.push(TextDecoration {
                            x,
                            y,
                            cell_width,
                            cell_height: line_height,
                            color: highlight_color,
                            kind: DecorationKind::Background,
                        });
                        break; // Only one match highlight per cell
                    }
                }
            }

            let c = cell.c;
            if c == ' ' {
                continue;
            }

            // Get style from cell flags
            let style = GlyphStyle::new(
                flags.contains(CellFlags::BOLD),
                flags.contains(CellFlags::ITALIC),
            );

            if let Some(glyph) = self.gpu.glyph_cache.position_char_styled(c, x, y, style) {
                // Route to appropriate renderer based on semantic zones (OSC 133)
                // - Prompt/Input zones -> grid_renderer (with glow effect)
                // - Output/Unknown zones -> output_grid_renderer (flat, no glow)
                let use_glow = if has_semantic_zones {
                    // Use deterministic zone-based routing
                    let zone = terminal.get_line_zone(grid_line);
                    matches!(zone, SemanticZone::Prompt | SemanticZone::Input)
                } else {
                    // Fallback heuristic: cursor line and line above (for multi-line prompts)
                    viewport_line >= cursor_viewport_line - 1
                        && viewport_line <= cursor_viewport_line
                };

                if use_glow {
                    self.gpu.grid_renderer.push_glyphs(&[glyph], fg_color);
                } else {
                    self.gpu
                        .output_grid_renderer
                        .push_glyphs(&[glyph], fg_color);
                }
            }
        }

        self.gpu.glyph_cache.flush(&shared_gpu.queue);

        Some(TextBufferUpdateResult {
            cursor: CursorInfo {
                x: cursor_x,
                y: cursor_y,
                cell_width,
                cell_height: line_height,
                visible: cursor_visible,
                shape: cursor.shape,
            },
            decorations,
        })
    }

    /// Create a shell for a new tab with spawn options
    pub fn create_shell_for_tab(&mut self, tab_id: u64, options: SpawnOptions) {
        let size = Size::new(self.cols, self.rows);
        log::info!(
            "Spawning shell for tab {} with semantic_prompts={}",
            tab_id,
            options.semantic_prompts
        );
        let result = ShellTerminal::with_options(size, options);

        match result {
            Ok(shell) => {
                log::info!("Shell spawned for tab {}", tab_id);
                self.shells.insert(tab_id, shell);
                self.content_hashes.insert(tab_id, 0);
            }
            Err(e) => {
                log::error!("Failed to spawn shell for tab {}: {}", tab_id, e);
            }
        }
    }

    /// Get the current working directory of the active tab's shell
    pub fn active_shell_cwd(&self) -> Option<std::path::PathBuf> {
        let tab_id = self.gpu.tab_bar.active_tab_id()?;
        let shell = self.shells.get(&tab_id)?;
        shell.working_directory()
    }

    /// Remove shell for a closed tab
    pub fn remove_shell_for_tab(&mut self, tab_id: u64) {
        self.shells.remove(&tab_id);
        self.content_hashes.remove(&tab_id);
        log::info!("Removed shell for tab {}", tab_id);
    }

    /// Force redraw of active tab by clearing its content hash
    pub fn force_active_tab_redraw(&mut self) {
        if let Some(tab_id) = self.gpu.tab_bar.active_tab_id() {
            self.content_hashes.insert(tab_id, 0);
            self.render.dirty = true;
        }
    }
}
