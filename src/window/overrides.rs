//! Theme override state management.
//!
//! Handles event-triggered temporary theme overrides (bell, command success/fail, focus).

use std::collections::HashSet;
use std::time::{Duration, Instant};

use crt_core::ShellEvent;
use crt_theme::EventOverride;

use super::types::EffectId;

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

/// Macro for override getters that return `Option<T>` where T: Copy.
/// Finds the most recent active override with the given field set.
macro_rules! override_copy_getter {
    ($name:ident, $field:ident, $ty:ty) => {
        pub fn $name(&self) -> Option<$ty> {
            self.active
                .iter()
                .filter(|o| o.is_active())
                .filter_map(|o| o.properties.$field)
                .next_back()
        }
    };
}

/// Macro for override getters that return `Option<&T>` (reference to non-Copy field).
macro_rules! override_ref_getter {
    ($name:ident, $field:ident, $ty:ty) => {
        pub fn $name(&self) -> Option<&$ty> {
            self.active
                .iter()
                .filter(|o| o.is_active())
                .filter_map(|o| o.properties.$field.as_ref())
                .next_back()
        }
    };
}

/// Manager for active theme overrides
///
/// Handles multiple simultaneous overrides with priority and duration tracking.
#[derive(Debug, Clone, Default)]
pub struct OverrideState {
    /// Currently active overrides (may have multiple from different events)
    pub active: Vec<ActiveOverride>,
    /// Track which effects are currently patched
    patched_effects: HashSet<EffectId>,
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
    pub fn is_patched(&self, effect: EffectId) -> bool {
        self.patched_effects.contains(&effect)
    }

    /// Mark an effect as patched
    pub fn set_patched(&mut self, effect: EffectId) {
        self.patched_effects.insert(effect);
    }

    /// Clear the patched state for an effect
    pub fn clear_patched(&mut self, effect: EffectId) {
        self.patched_effects.remove(&effect);
    }

    // Copy getters — return Option<T> where T: Copy
    override_copy_getter!(get_foreground, foreground, crt_theme::Color);
    override_copy_getter!(get_background, background, crt_theme::LinearGradient);
    override_copy_getter!(get_cursor_color, cursor_color, crt_theme::Color);
    override_copy_getter!(get_cursor_shape, cursor_shape, crt_theme::CursorShape);
    override_copy_getter!(get_text_shadow, text_shadow, crt_theme::TextShadow);
    override_copy_getter!(get_flash_color, flash_color, crt_theme::Color);
    override_copy_getter!(get_flash_intensity, flash_intensity, f32);

    // Ref getters — return Option<&T> for non-Copy patch types
    override_ref_getter!(get_sprite_patch, sprite_patch, crt_theme::SpritePatch);
    override_ref_getter!(get_sprite_overlay, sprite_overlay, crt_theme::SpriteOverlay);
    override_ref_getter!(get_starfield_patch, starfield_patch, crt_theme::StarfieldPatch);
    override_ref_getter!(get_particle_patch, particle_patch, crt_theme::ParticlePatch);
    override_ref_getter!(get_grid_patch, grid_patch, crt_theme::GridPatch);
    override_ref_getter!(get_rain_patch, rain_patch, crt_theme::RainPatch);
    override_ref_getter!(get_matrix_patch, matrix_patch, crt_theme::MatrixPatch);
    override_ref_getter!(get_shape_patch, shape_patch, crt_theme::ShapePatch);

    /// Get the effective flash for rendering (color and faded intensity)
    ///
    /// Returns (color, current_intensity) where current_intensity accounts for
    /// both the configured intensity and the override's fade-out progress.
    pub fn get_effective_flash(&self) -> Option<(crt_theme::Color, f32)> {
        self.active
            .iter()
            .rfind(|o| o.is_active() && o.properties.flash_color.is_some())
            .map(|o| {
                let color = o.properties.flash_color.unwrap();
                let base_intensity = o.properties.flash_intensity.unwrap_or(0.5);
                let fade = o.intensity();
                (color, base_intensity * fade)
            })
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
