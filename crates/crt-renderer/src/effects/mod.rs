//! Vello-powered backdrop effects system
//!
//! Provides a trait-based effect system for rendering animated backgrounds.
//! Effects include grids, starfields, rain, matrix code, particles, shapes, and sprites.
//!
//! ## Architecture
//!
//! Each effect implements `BackdropEffect` and renders to a shared Vello scene.
//! The `EffectsRenderer` manages multiple effects, updates animation state,
//! and composites the final result to a texture.
//!
//! ## Usage
//!
//! Effects are configured via CSS custom properties in the `::backdrop` selector:
//!
//! ```css
//! :terminal::backdrop {
//!     --starfield-enabled: true;
//!     --starfield-density: 100;
//!     --starfield-speed: 0.5;
//! }
//! ```

pub mod grid;
pub mod motion;
pub mod renderer;
pub mod starfield;

pub use grid::GridEffect;
pub use motion::MotionBehavior;
pub use renderer::EffectsRenderer;
pub use starfield::StarfieldEffect;

use vello::kurbo::{Rect, Vec2};
use std::collections::HashMap;
use vello::Scene;

/// Configuration passed to effects from parsed CSS properties
#[derive(Debug, Clone, Default)]
pub struct EffectConfig {
    /// Raw CSS property values keyed by property name (without --)
    pub properties: HashMap<String, String>,
}

impl EffectConfig {
    /// Create a new empty config
    pub fn new() -> Self {
        Self::default()
    }

    /// Get a property value as a string
    pub fn get(&self, key: &str) -> Option<&str> {
        self.properties.get(key).map(|s| s.as_str())
    }

    /// Get a property as a boolean (true/false)
    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.get(key).and_then(|v| match v.to_lowercase().as_str() {
            "true" | "yes" | "1" => Some(true),
            "false" | "no" | "0" => Some(false),
            _ => None,
        })
    }

    /// Get a property as an f64
    pub fn get_f64(&self, key: &str) -> Option<f64> {
        self.get(key).and_then(|v| v.parse().ok())
    }

    /// Get a property as an f32
    pub fn get_f32(&self, key: &str) -> Option<f32> {
        self.get(key).and_then(|v| v.parse().ok())
    }

    /// Get a property as a u32
    pub fn get_u32(&self, key: &str) -> Option<u32> {
        self.get(key).and_then(|v| v.parse().ok())
    }

    /// Get a property as a usize
    pub fn get_usize(&self, key: &str) -> Option<usize> {
        self.get(key).and_then(|v| v.parse().ok())
    }

    /// Insert a property
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.properties.insert(key.into(), value.into());
    }
}

/// Trait for backdrop effects that render to a Vello scene
///
/// Effects are updated each frame with delta time and total elapsed time,
/// then render their content to a shared Vello scene.
pub trait BackdropEffect: Send + Sync {
    /// Unique identifier for this effect type (e.g., "grid", "starfield")
    fn effect_type(&self) -> &'static str;

    /// Update animation state
    ///
    /// # Arguments
    /// * `dt` - Delta time since last frame in seconds
    /// * `time` - Total elapsed time in seconds
    fn update(&mut self, dt: f32, time: f32);

    /// Render to the Vello scene
    ///
    /// # Arguments
    /// * `scene` - The Vello scene to render into
    /// * `bounds` - The rendering bounds (typically window size)
    fn render(&self, scene: &mut Scene, bounds: Rect);

    /// Configure the effect from CSS properties
    ///
    /// Called when theme is loaded or hot-reloaded.
    /// The config contains properties prefixed with the effect type,
    /// e.g., for "grid" effect: grid-enabled, grid-color, etc.
    fn configure(&mut self, config: &EffectConfig);

    /// Check if the effect is enabled
    fn is_enabled(&self) -> bool;
}

/// Position with velocity for motion behaviors
#[derive(Debug, Clone, Copy, Default)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}

impl Position {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    pub fn to_vec2(&self) -> Vec2 {
        Vec2::new(self.x, self.y)
    }
}

impl From<Vec2> for Position {
    fn from(v: Vec2) -> Self {
        Self { x: v.x, y: v.y }
    }
}

impl From<Position> for Vec2 {
    fn from(p: Position) -> Self {
        Vec2::new(p.x, p.y)
    }
}
