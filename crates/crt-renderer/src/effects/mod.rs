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
pub mod matrix;
pub mod motion;
pub mod particles;
pub mod rain;
pub mod renderer;
pub mod shape;
pub mod sprite;
pub mod starfield;

pub use grid::GridEffect;
pub use matrix::MatrixEffect;
pub use motion::MotionBehavior;
pub use particles::ParticleEffect;
pub use rain::RainEffect;
pub use renderer::EffectsRenderer;
pub use shape::ShapeEffect;
pub use sprite::SpriteEffect;
pub use starfield::StarfieldEffect;

use std::collections::HashMap;
use vello::Scene;
use vello::kurbo::{Rect, Vec2};

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

    /// Prepare GPU resources for effects that need persistent textures.
    ///
    /// Called before rendering when GPU resources are available.
    /// Effects can use `renderer.register_texture()` to pre-upload textures
    /// that bypass vello's atlas system, preventing memory growth.
    ///
    /// Default implementation does nothing.
    fn prepare_gpu_resources(
        &mut self,
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
        _renderer: &mut vello::Renderer,
    ) {
        // Default: no GPU resources needed
    }

    /// Check if GPU resources need to be prepared or updated.
    ///
    /// Returns true if `prepare_gpu_resources` should be called.
    /// Default implementation returns false.
    fn needs_gpu_resources(&self) -> bool {
        false
    }

    /// Cleanup GPU resources when effect is disabled or removed.
    ///
    /// Called to unregister textures and free GPU memory.
    /// Default implementation does nothing.
    fn cleanup_gpu_resources(&mut self, _renderer: &mut vello::Renderer) {
        // Default: no cleanup needed
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effect_config_new_is_empty() {
        let config = EffectConfig::new();
        assert!(config.properties.is_empty());
    }

    #[test]
    fn effect_config_insert_and_get() {
        let mut config = EffectConfig::new();
        config.insert("color", "#ff0000");
        assert_eq!(config.get("color"), Some("#ff0000"));
        assert_eq!(config.get("missing"), None);
    }

    #[test]
    fn effect_config_get_bool() {
        let mut config = EffectConfig::new();
        config.insert("a", "true");
        config.insert("b", "false");
        config.insert("c", "yes");
        config.insert("d", "no");
        config.insert("e", "1");
        config.insert("f", "0");
        config.insert("g", "invalid");

        assert_eq!(config.get_bool("a"), Some(true));
        assert_eq!(config.get_bool("b"), Some(false));
        assert_eq!(config.get_bool("c"), Some(true));
        assert_eq!(config.get_bool("d"), Some(false));
        assert_eq!(config.get_bool("e"), Some(true));
        assert_eq!(config.get_bool("f"), Some(false));
        assert_eq!(config.get_bool("g"), None);
        assert_eq!(config.get_bool("missing"), None);
    }

    #[test]
    fn effect_config_get_f64() {
        let mut config = EffectConfig::new();
        config.insert("speed", "1.5");
        config.insert("bad", "not_a_number");
        assert_eq!(config.get_f64("speed"), Some(1.5));
        assert_eq!(config.get_f64("bad"), None);
        assert_eq!(config.get_f64("missing"), None);
    }

    #[test]
    fn effect_config_get_f32() {
        let mut config = EffectConfig::new();
        config.insert("val", "3.14");
        assert!((config.get_f32("val").unwrap() - 3.14).abs() < 0.001);
    }

    #[test]
    fn effect_config_get_u32() {
        let mut config = EffectConfig::new();
        config.insert("count", "42");
        config.insert("negative", "-1");
        assert_eq!(config.get_u32("count"), Some(42));
        assert_eq!(config.get_u32("negative"), None);
    }

    #[test]
    fn effect_config_get_usize() {
        let mut config = EffectConfig::new();
        config.insert("size", "1024");
        assert_eq!(config.get_usize("size"), Some(1024));
    }

    #[test]
    fn effect_config_overwrite_key() {
        let mut config = EffectConfig::new();
        config.insert("key", "old");
        config.insert("key", "new");
        assert_eq!(config.get("key"), Some("new"));
    }

    #[test]
    fn position_new_and_to_vec2() {
        let p = Position::new(1.5, 2.5);
        assert_eq!(p.x, 1.5);
        assert_eq!(p.y, 2.5);
        let v = p.to_vec2();
        assert_eq!(v.x, 1.5);
        assert_eq!(v.y, 2.5);
    }

    #[test]
    fn position_from_vec2() {
        let v = Vec2::new(3.0, 4.0);
        let p = Position::from(v);
        assert_eq!(p.x, 3.0);
        assert_eq!(p.y, 4.0);
    }

    #[test]
    fn vec2_from_position() {
        let p = Position::new(5.0, 6.0);
        let v: Vec2 = p.into();
        assert_eq!(v.x, 5.0);
        assert_eq!(v.y, 6.0);
    }
}
