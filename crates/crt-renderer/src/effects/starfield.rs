//! Starfield backdrop effect - parallax star layers with motion
//!
//! Renders multiple layers of stars moving at different speeds to create
//! a parallax depth effect. Stars can move outward from center (warp effect)
//! or in a specific direction.
//!
//! ## CSS Properties
//!
//! - `--starfield-enabled: true|false`
//! - `--starfield-density: <number>` (stars per layer, default 100)
//! - `--starfield-layers: <number>` (number of parallax layers, 1-5)
//! - `--starfield-speed: <number>` (base movement speed)
//! - `--starfield-color: <color>` (base star color)
//! - `--starfield-direction: warp|up|down|left|right` (movement direction)
//! - `--starfield-glow-radius: <number>` (glow spread, 0 = no glow)
//! - `--starfield-glow-intensity: <number>` (0-1 glow brightness)
//! - `--starfield-twinkle: true|false` (enable star twinkling)
//! - `--starfield-twinkle-speed: <number>` (twinkle animation speed)
//! - `--starfield-min-size: <number>` (minimum star size in pixels)
//! - `--starfield-max-size: <number>` (maximum star size in pixels)

use vello::kurbo::{Affine, Circle, Point, Rect};
use vello::peniko::{Brush, Color};
use vello::Scene;

use super::{BackdropEffect, EffectConfig};

/// Direction stars drift
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum StarDirection {
    /// Stars don't move (static field)
    #[default]
    Static,
    /// Stars drift upward
    Up,
    /// Stars drift downward
    Down,
    /// Stars drift left
    Left,
    /// Stars drift right
    Right,
}

impl StarDirection {
    fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "static" | "none" => Some(Self::Static),
            "up" => Some(Self::Up),
            "down" => Some(Self::Down),
            "left" => Some(Self::Left),
            "right" => Some(Self::Right),
            _ => None,
        }
    }
}

/// Individual star data
#[derive(Debug, Clone, Copy)]
struct Star {
    /// Base position (0-1 normalized)
    x: f64,
    y: f64,
    /// Size multiplier (0-1)
    size: f64,
    /// Brightness multiplier (0-1)
    brightness: f64,
    /// Layer this star belongs to (0 = furthest, higher = closer)
    layer: usize,
    /// Twinkle phase offset
    twinkle_phase: f64,
}

/// RGBA color stored as u8 components
#[derive(Debug, Clone, Copy)]
struct Rgba {
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

impl Rgba {
    fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    fn with_alpha(&self, alpha: u8) -> Color {
        Color::from_rgba8(self.r, self.g, self.b, alpha)
    }
}

/// Starfield effect with parallax layers
pub struct StarfieldEffect {
    /// Whether the effect is enabled
    enabled: bool,

    /// Star color
    color: Rgba,

    /// Number of stars per layer
    density: usize,

    /// Number of parallax layers (1-5)
    layer_count: usize,

    /// Base movement speed
    speed: f64,

    /// Movement direction
    direction: StarDirection,

    /// Glow radius in pixels (0 = no glow)
    glow_radius: f64,

    /// Glow intensity (0-1)
    glow_intensity: f64,

    /// Enable twinkling
    twinkle: bool,

    /// Twinkle animation speed
    twinkle_speed: f64,

    /// Minimum star size in pixels
    min_size: f64,

    /// Maximum star size in pixels
    max_size: f64,

    /// All stars across all layers
    stars: Vec<Star>,

    /// Current time for animation
    time: f64,

    /// Seed for reproducible star positions
    seed: u64,

    /// Whether stars need regeneration
    needs_regeneration: bool,
}

impl Default for StarfieldEffect {
    fn default() -> Self {
        Self {
            enabled: false,
            color: Rgba::new(255, 255, 255, 255), // White stars
            density: 100,
            layer_count: 3,
            speed: 0.3,
            direction: StarDirection::Static,
            glow_radius: 0.0,
            glow_intensity: 0.0,
            twinkle: true,
            twinkle_speed: 2.0,
            min_size: 1.0,
            max_size: 3.0,
            stars: Vec::new(),
            time: 0.0,
            seed: 12345,
            needs_regeneration: true,
        }
    }
}

impl StarfieldEffect {
    /// Create a new starfield effect with default settings
    pub fn new() -> Self {
        Self::default()
    }

    /// Hash-based pseudo-random number generator for better distribution
    fn random(&self, index: usize) -> f64 {
        // Use multiple rounds of mixing for better distribution
        let mut x = (index as u64).wrapping_add(self.seed);
        x ^= x >> 33;
        x = x.wrapping_mul(0xff51afd7ed558ccd);
        x ^= x >> 33;
        x = x.wrapping_mul(0xc4ceb9fe1a85ec53);
        x ^= x >> 33;
        // Normalize to 0-1
        (x as f64) / (u64::MAX as f64)
    }

    /// Generate stars for all layers
    fn generate_stars(&mut self) {
        self.stars.clear();

        let total_stars = self.density * self.layer_count;
        self.stars.reserve(total_stars);

        for i in 0..total_stars {
            let layer = i % self.layer_count;

            // Generate pseudo-random values using index-based seeding
            let x = self.random(i * 5);
            let y = self.random(i * 5 + 1);
            let size = self.random(i * 5 + 2);
            let brightness = 0.5 + self.random(i * 5 + 3) * 0.5; // 0.5-1.0
            let twinkle_phase = self.random(i * 5 + 4) * std::f64::consts::TAU;

            self.stars.push(Star {
                x,
                y,
                size,
                brightness,
                layer,
                twinkle_phase,
            });
        }

        self.needs_regeneration = false;
    }

    /// Calculate animated star position
    fn star_position(&self, star: &Star, width: f64, height: f64) -> (f64, f64) {
        // Layer speed multiplier: closer layers move faster for parallax
        let layer_speed = 0.2 + (star.layer as f64 / self.layer_count as f64) * 0.8;
        let speed = self.speed * layer_speed;

        match self.direction {
            StarDirection::Static => {
                // Stars don't move
                let x = star.x * width;
                let y = star.y * height;
                (x, y)
            }
            StarDirection::Up => {
                let x = star.x * width;
                let y_base = star.y * height;
                let y = (y_base - self.time * speed * 50.0) % height;
                let y = if y < 0.0 { y + height } else { y };
                (x, y)
            }
            StarDirection::Down => {
                let x = star.x * width;
                let y_base = star.y * height;
                let y = (y_base + self.time * speed * 50.0) % height;
                (x, y)
            }
            StarDirection::Left => {
                let x_base = star.x * width;
                let x = (x_base - self.time * speed * 50.0) % width;
                let x = if x < 0.0 { x + width } else { x };
                let y = star.y * height;
                (x, y)
            }
            StarDirection::Right => {
                let x_base = star.x * width;
                let x = (x_base + self.time * speed * 50.0) % width;
                let y = star.y * height;
                (x, y)
            }
        }
    }

    /// Draw a star with optional glow
    fn draw_star(&self, scene: &mut Scene, x: f64, y: f64, radius: f64, color: Color) {
        let center = Point::new(x, y);

        // Draw glow layers if enabled
        if self.glow_radius > 0.0 && self.glow_intensity > 0.0 {
            let [r, g, b, a] = color.components;
            let glow_layers = 3;

            for i in (0..glow_layers).rev() {
                let t = (i + 1) as f64 / glow_layers as f64;
                let glow_r = radius + self.glow_radius * t;
                let glow_alpha = (a as f64 * self.glow_intensity * (1.0 - t) * 0.4) as f32;

                if glow_alpha > 0.001 {
                    let glow_color = Color::new([r, g, b, glow_alpha]);
                    let circle = Circle::new(center, glow_r);
                    scene.fill(
                        vello::peniko::Fill::NonZero,
                        Affine::IDENTITY,
                        &Brush::Solid(glow_color),
                        None,
                        &circle,
                    );
                }
            }
        }

        // Draw core star
        let circle = Circle::new(center, radius);
        scene.fill(
            vello::peniko::Fill::NonZero,
            Affine::IDENTITY,
            &Brush::Solid(color),
            None,
            &circle,
        );
    }
}

impl BackdropEffect for StarfieldEffect {
    fn effect_type(&self) -> &'static str {
        "starfield"
    }

    fn update(&mut self, _dt: f32, time: f32) {
        self.time = time as f64;

        // Regenerate stars if needed
        if self.needs_regeneration {
            self.generate_stars();
        }
    }

    fn render(&self, scene: &mut Scene, bounds: Rect) {
        if !self.enabled || self.stars.is_empty() {
            return;
        }

        let width = bounds.width();
        let height = bounds.height();

        // Render stars from back layer to front (back layers = dimmer/smaller)
        for layer in 0..self.layer_count {
            for star in self.stars.iter().filter(|s| s.layer == layer) {
                // Get animated position
                let (x, y) = self.star_position(star, width, height);

                // Calculate star size based on layer (closer = larger) and random factor
                let layer_factor = 0.4 + (star.layer as f64 / self.layer_count as f64) * 0.6;
                let base_size = self.min_size + star.size * (self.max_size - self.min_size);
                let size = base_size * layer_factor;

                // Calculate brightness - back layers are dimmer
                let layer_brightness = 0.3 + (star.layer as f64 / self.layer_count as f64) * 0.7;
                let mut brightness = star.brightness * layer_brightness;

                // Optional twinkling
                if self.twinkle {
                    let twinkle = (self.time * self.twinkle_speed + star.twinkle_phase).sin();
                    brightness *= 0.6 + twinkle * 0.4; // Vary between 0.2 and 1.0
                }

                // Apply brightness to alpha
                let alpha = (self.color.a as f64 * brightness) as u8;
                if alpha < 5 {
                    continue; // Skip nearly invisible stars
                }
                let color = self.color.with_alpha(alpha);

                self.draw_star(scene, x, y, size, color);
            }
        }
    }

    fn configure(&mut self, config: &EffectConfig) {
        if let Some(enabled) = config.get_bool("enabled") {
            self.enabled = enabled;
        }

        if let Some(density) = config.get_usize("density") {
            if density != self.density {
                self.density = density.clamp(10, 1000);
                self.needs_regeneration = true;
            }
        }

        if let Some(layers) = config.get_usize("layers") {
            if layers != self.layer_count {
                self.layer_count = layers.clamp(1, 5);
                self.needs_regeneration = true;
            }
        }

        if let Some(speed) = config.get_f64("speed") {
            self.speed = speed.max(0.0);
        }

        if let Some(direction_str) = config.get("direction") {
            if let Some(direction) = StarDirection::from_str(direction_str) {
                self.direction = direction;
            }
        }

        if let Some(glow_radius) = config.get_f64("glow-radius") {
            self.glow_radius = glow_radius.max(0.0);
        }

        if let Some(glow_intensity) = config.get_f64("glow-intensity") {
            self.glow_intensity = glow_intensity.clamp(0.0, 1.0);
        }

        if let Some(twinkle) = config.get_bool("twinkle") {
            self.twinkle = twinkle;
        }

        if let Some(twinkle_speed) = config.get_f64("twinkle-speed") {
            self.twinkle_speed = twinkle_speed.max(0.0);
        }

        if let Some(min_size) = config.get_f64("min-size") {
            self.min_size = min_size.max(0.5);
        }

        if let Some(max_size) = config.get_f64("max-size") {
            self.max_size = max_size.max(self.min_size);
        }

        // Parse color
        if let Some(color_str) = config.get("color") {
            if let Some(color) = parse_simple_color(color_str) {
                self.color = color;
            }
        }
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }
}

/// Simple color parser for common formats
fn parse_simple_color(s: &str) -> Option<Rgba> {
    let s = s.trim();

    // Handle rgba(r, g, b, a)
    if s.starts_with("rgba(") && s.ends_with(')') {
        let inner = &s[5..s.len() - 1];
        let parts: Vec<&str> = inner.split(',').map(|p| p.trim()).collect();
        if parts.len() == 4 {
            let r: u8 = parts[0].parse().ok()?;
            let g: u8 = parts[1].parse().ok()?;
            let b: u8 = parts[2].parse().ok()?;
            let a: f64 = parts[3].parse().ok()?;
            return Some(Rgba::new(r, g, b, (a * 255.0) as u8));
        }
    }

    // Handle rgb(r, g, b)
    if s.starts_with("rgb(") && s.ends_with(')') {
        let inner = &s[4..s.len() - 1];
        let parts: Vec<&str> = inner.split(',').map(|p| p.trim()).collect();
        if parts.len() == 3 {
            let r: u8 = parts[0].parse().ok()?;
            let g: u8 = parts[1].parse().ok()?;
            let b: u8 = parts[2].parse().ok()?;
            return Some(Rgba::new(r, g, b, 255));
        }
    }

    // Handle #RRGGBB or #RRGGBBAA
    if s.starts_with('#') {
        let hex = &s[1..];
        match hex.len() {
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                return Some(Rgba::new(r, g, b, 255));
            }
            8 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
                return Some(Rgba::new(r, g, b, a));
            }
            _ => {}
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_starfield_is_disabled() {
        let starfield = StarfieldEffect::default();
        assert!(!starfield.is_enabled());
    }

    #[test]
    fn test_configure_enables_starfield() {
        let mut starfield = StarfieldEffect::default();
        let mut config = EffectConfig::new();
        config.insert("enabled", "true");

        starfield.configure(&config);

        assert!(starfield.is_enabled());
    }

    #[test]
    fn test_star_generation() {
        let mut starfield = StarfieldEffect::default();
        starfield.density = 50;
        starfield.layer_count = 2;
        starfield.generate_stars();

        assert_eq!(starfield.stars.len(), 100); // 50 * 2 layers
    }

    #[test]
    fn test_direction_parsing() {
        assert_eq!(StarDirection::from_str("static"), Some(StarDirection::Static));
        assert_eq!(StarDirection::from_str("UP"), Some(StarDirection::Up));
        assert_eq!(StarDirection::from_str("down"), Some(StarDirection::Down));
        assert_eq!(StarDirection::from_str("LEFT"), Some(StarDirection::Left));
        assert_eq!(StarDirection::from_str("Right"), Some(StarDirection::Right));
        assert_eq!(StarDirection::from_str("invalid"), None);
    }
}
