//! Particle backdrop effect - floating shapes with configurable behavior
//!
//! Renders many small particles (dots, circles, stars, hearts, sparkles)
//! that float, drift, rise, or fall across the screen.
//!
//! ## CSS Properties
//!
//! - `--particles-enabled: true|false`
//! - `--particles-count: <number>` (number of particles)
//! - `--particles-shape: dot|circle|star|heart|sparkle`
//! - `--particles-color: <color>`
//! - `--particles-size: <number>` (base size in pixels)
//! - `--particles-speed: <number>` (movement speed)
//! - `--particles-behavior: float|drift|rise|fall`
//! - `--particles-glow-radius: <number>` (glow spread)
//! - `--particles-glow-intensity: <number>` (0-1)

use std::f64::consts::PI;

use vello::kurbo::{Affine, BezPath, Circle, Point, Rect};
use vello::peniko::{Brush, Color, Fill};
use vello::Scene;

use super::{BackdropEffect, EffectConfig};

/// Particle shape type
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ParticleShape {
    /// Simple filled dot
    #[default]
    Dot,
    /// Circle outline
    Circle,
    /// Five-pointed star
    Star,
    /// Heart shape
    Heart,
    /// Four-pointed sparkle
    Sparkle,
}

impl ParticleShape {
    fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "dot" => Some(Self::Dot),
            "circle" => Some(Self::Circle),
            "star" => Some(Self::Star),
            "heart" => Some(Self::Heart),
            "sparkle" => Some(Self::Sparkle),
            _ => None,
        }
    }
}

/// Particle behavior/movement pattern
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ParticleBehavior {
    /// Random gentle floating motion
    #[default]
    Float,
    /// Drift in one direction (horizontal)
    Drift,
    /// Rise upward
    Rise,
    /// Fall downward
    Fall,
}

impl ParticleBehavior {
    fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "float" => Some(Self::Float),
            "drift" => Some(Self::Drift),
            "rise" => Some(Self::Rise),
            "fall" => Some(Self::Fall),
            _ => None,
        }
    }
}

/// Individual particle data
#[derive(Debug, Clone, Copy)]
struct Particle {
    /// Base X position (0-1 normalized)
    x: f64,
    /// Base Y position (0-1 normalized)
    y: f64,
    /// Size multiplier
    size: f64,
    /// Brightness/alpha multiplier
    brightness: f64,
    /// Phase offset for oscillation
    phase: f64,
    /// Rotation angle
    rotation: f64,
    /// Individual speed multiplier
    speed: f64,
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

/// Particle effect with configurable shapes and behaviors
pub struct ParticleEffect {
    /// Whether the effect is enabled
    enabled: bool,

    /// Particle color
    color: Rgba,

    /// Number of particles
    count: usize,

    /// Particle shape
    shape: ParticleShape,

    /// Movement behavior
    behavior: ParticleBehavior,

    /// Base particle size in pixels
    size: f64,

    /// Movement speed multiplier
    speed: f64,

    /// Glow radius in pixels
    glow_radius: f64,

    /// Glow intensity (0-1)
    glow_intensity: f64,

    /// All particles
    particles: Vec<Particle>,

    /// Current time for animation
    time: f64,

    /// Seed for reproducible positions
    seed: u64,

    /// Whether particles need regeneration
    needs_regeneration: bool,
}

impl Default for ParticleEffect {
    fn default() -> Self {
        Self {
            enabled: false,
            color: Rgba::new(255, 200, 220, 200), // Soft pink
            count: 50,
            shape: ParticleShape::Dot,
            behavior: ParticleBehavior::Float,
            size: 4.0,
            speed: 0.5,
            glow_radius: 0.0,
            glow_intensity: 0.0,
            particles: Vec::new(),
            time: 0.0,
            seed: 98765,
            needs_regeneration: true,
        }
    }
}

impl ParticleEffect {
    /// Create a new particle effect with default settings
    pub fn new() -> Self {
        Self::default()
    }

    /// Hash-based pseudo-random number generator
    fn random(&self, index: usize) -> f64 {
        let mut x = (index as u64).wrapping_add(self.seed);
        x ^= x >> 33;
        x = x.wrapping_mul(0xff51afd7ed558ccd);
        x ^= x >> 33;
        x = x.wrapping_mul(0xc4ceb9fe1a85ec53);
        x ^= x >> 33;
        (x as f64) / (u64::MAX as f64)
    }

    /// Generate particles
    fn generate_particles(&mut self) {
        self.particles.clear();
        self.particles.reserve(self.count);

        for i in 0..self.count {
            let x = self.random(i * 6);
            let y = self.random(i * 6 + 1);
            let size = 0.5 + self.random(i * 6 + 2) * 1.0; // 0.5-1.5
            let brightness = 0.4 + self.random(i * 6 + 3) * 0.6; // 0.4-1.0
            let phase = self.random(i * 6 + 4) * PI * 2.0;
            let rotation = self.random(i * 6 + 5) * PI * 2.0;
            let speed = 0.6 + self.random(i * 6 + 6) * 0.8; // 0.6-1.4

            self.particles.push(Particle {
                x,
                y,
                size,
                brightness,
                phase,
                rotation,
                speed,
            });
        }

        self.needs_regeneration = false;
    }

    /// Calculate particle position based on behavior
    fn particle_position(&self, particle: &Particle, width: f64, height: f64) -> (f64, f64, f64) {
        let speed = self.speed * particle.speed;
        let time = self.time;

        match self.behavior {
            ParticleBehavior::Float => {
                // Gentle floating with oscillation
                let base_x = particle.x * width;
                let base_y = particle.y * height;

                // Oscillate in a figure-8 pattern
                let osc_x = (time * speed * 0.5 + particle.phase).sin() * 30.0;
                let osc_y = (time * speed * 0.3 + particle.phase * 1.5).sin() * 20.0;

                let x = base_x + osc_x;
                let y = base_y + osc_y;

                // Gentle rotation over time
                let rotation = particle.rotation + time * speed * 0.2;

                (x, y, rotation)
            }
            ParticleBehavior::Drift => {
                // Horizontal drift with slight vertical oscillation
                let x_speed = speed * 40.0;
                let x = (particle.x * width + time * x_speed) % (width + 40.0) - 20.0;
                let y = particle.y * height + (time * speed + particle.phase).sin() * 15.0;
                let rotation = particle.rotation + time * speed * 0.1;

                (x, y, rotation)
            }
            ParticleBehavior::Rise => {
                // Rise upward with slight horizontal sway
                let y_speed = speed * 50.0;
                let base_y = particle.y * height;
                let y = (base_y - time * y_speed) % height;
                let y = if y < 0.0 { y + height } else { y };

                let x = particle.x * width + (time * speed * 0.5 + particle.phase).sin() * 20.0;
                let rotation = particle.rotation + time * speed * 0.3;

                (x, y, rotation)
            }
            ParticleBehavior::Fall => {
                // Fall downward with slight horizontal sway
                let y_speed = speed * 50.0;
                let base_y = particle.y * height;
                let y = (base_y + time * y_speed) % height;

                let x = particle.x * width + (time * speed * 0.5 + particle.phase).sin() * 20.0;
                let rotation = particle.rotation + time * speed * 0.3;

                (x, y, rotation)
            }
        }
    }

    /// Draw a star shape
    fn draw_star(center: Point, size: f64, rotation: f64) -> BezPath {
        let mut path = BezPath::new();
        let points = 5;
        let outer_radius = size;
        let inner_radius = size * 0.4;

        for i in 0..(points * 2) {
            let angle = rotation + (i as f64 * PI / points as f64) - PI / 2.0;
            let radius = if i % 2 == 0 { outer_radius } else { inner_radius };
            let x = center.x + angle.cos() * radius;
            let y = center.y + angle.sin() * radius;

            if i == 0 {
                path.move_to(Point::new(x, y));
            } else {
                path.line_to(Point::new(x, y));
            }
        }
        path.close_path();
        path
    }

    /// Draw a heart shape
    fn draw_heart(center: Point, size: f64, rotation: f64) -> BezPath {
        let mut path = BezPath::new();

        // Heart shape using bezier curves
        let s = size * 0.8;

        // Transform points by rotation
        let rotate = |x: f64, y: f64| -> Point {
            let cos_r = rotation.cos();
            let sin_r = rotation.sin();
            Point::new(
                center.x + x * cos_r - y * sin_r,
                center.y + x * sin_r + y * cos_r,
            )
        };

        path.move_to(rotate(0.0, -s * 0.3));
        // Left curve
        path.curve_to(
            rotate(-s * 0.5, -s * 0.8),
            rotate(-s, -s * 0.3),
            rotate(-s * 0.5, s * 0.2),
        );
        // Bottom point
        path.line_to(rotate(0.0, s));
        // Right curve
        path.line_to(rotate(s * 0.5, s * 0.2));
        path.curve_to(
            rotate(s, -s * 0.3),
            rotate(s * 0.5, -s * 0.8),
            rotate(0.0, -s * 0.3),
        );
        path.close_path();
        path
    }

    /// Draw a sparkle (4-pointed star)
    fn draw_sparkle(center: Point, size: f64, rotation: f64) -> BezPath {
        let mut path = BezPath::new();
        let outer = size;
        let inner = size * 0.2;

        for i in 0..8 {
            let angle = rotation + (i as f64 * PI / 4.0);
            let radius = if i % 2 == 0 { outer } else { inner };
            let x = center.x + angle.cos() * radius;
            let y = center.y + angle.sin() * radius;

            if i == 0 {
                path.move_to(Point::new(x, y));
            } else {
                path.line_to(Point::new(x, y));
            }
        }
        path.close_path();
        path
    }

    /// Draw a particle at the given position
    fn draw_particle(&self, scene: &mut Scene, x: f64, y: f64, size: f64, rotation: f64, color: Color) {
        let center = Point::new(x, y);

        // Draw glow if enabled
        if self.glow_radius > 0.0 && self.glow_intensity > 0.0 {
            let [r, g, b, a] = color.components;
            let glow_layers = 3;

            for i in (0..glow_layers).rev() {
                let t = (i + 1) as f64 / glow_layers as f64;
                let glow_size = size + self.glow_radius * t;
                let glow_alpha = (a as f64 * self.glow_intensity * (1.0 - t) * 0.4) as f32;

                if glow_alpha > 0.001 {
                    let glow_color = Color::new([r, g, b, glow_alpha]);
                    let circle = Circle::new(center, glow_size);
                    scene.fill(Fill::NonZero, Affine::IDENTITY, &Brush::Solid(glow_color), None, &circle);
                }
            }
        }

        // Draw the shape
        match self.shape {
            ParticleShape::Dot => {
                let circle = Circle::new(center, size);
                scene.fill(Fill::NonZero, Affine::IDENTITY, &Brush::Solid(color), None, &circle);
            }
            ParticleShape::Circle => {
                let circle = Circle::new(center, size);
                let stroke = vello::kurbo::Stroke::new(size * 0.3);
                scene.stroke(&stroke, Affine::IDENTITY, &Brush::Solid(color), None, &circle);
            }
            ParticleShape::Star => {
                let path = Self::draw_star(center, size, rotation);
                scene.fill(Fill::NonZero, Affine::IDENTITY, &Brush::Solid(color), None, &path);
            }
            ParticleShape::Heart => {
                let path = Self::draw_heart(center, size, rotation);
                scene.fill(Fill::NonZero, Affine::IDENTITY, &Brush::Solid(color), None, &path);
            }
            ParticleShape::Sparkle => {
                let path = Self::draw_sparkle(center, size, rotation);
                scene.fill(Fill::NonZero, Affine::IDENTITY, &Brush::Solid(color), None, &path);
            }
        }
    }
}

impl BackdropEffect for ParticleEffect {
    fn effect_type(&self) -> &'static str {
        "particles"
    }

    fn update(&mut self, _dt: f32, time: f32) {
        self.time = time as f64;

        if self.needs_regeneration {
            self.generate_particles();
        }
    }

    fn render(&self, scene: &mut Scene, bounds: Rect) {
        if !self.enabled || self.particles.is_empty() {
            return;
        }

        let width = bounds.width();
        let height = bounds.height();

        for particle in &self.particles {
            let (x, y, rotation) = self.particle_position(particle, width, height);

            // Skip if off screen
            let margin = self.size * 2.0;
            if x < -margin || x > width + margin || y < -margin || y > height + margin {
                continue;
            }

            // Calculate size and color
            let size = self.size * particle.size;
            let alpha = (self.color.a as f64 * particle.brightness) as u8;
            let color = self.color.with_alpha(alpha);

            self.draw_particle(scene, x, y, size, rotation, color);
        }
    }

    fn configure(&mut self, config: &EffectConfig) {
        if let Some(enabled) = config.get_bool("enabled") {
            self.enabled = enabled;
        }

        if let Some(count) = config.get_usize("count") {
            if count != self.count {
                self.count = count.clamp(1, 500);
                self.needs_regeneration = true;
            }
        }

        if let Some(shape_str) = config.get("shape") {
            if let Some(shape) = ParticleShape::from_str(shape_str) {
                self.shape = shape;
            }
        }

        if let Some(behavior_str) = config.get("behavior") {
            if let Some(behavior) = ParticleBehavior::from_str(behavior_str) {
                self.behavior = behavior;
            }
        }

        if let Some(size) = config.get_f64("size") {
            self.size = size.clamp(1.0, 50.0);
        }

        if let Some(speed) = config.get_f64("speed") {
            self.speed = speed.max(0.0);
        }

        if let Some(glow_radius) = config.get_f64("glow-radius") {
            self.glow_radius = glow_radius.max(0.0);
        }

        if let Some(glow_intensity) = config.get_f64("glow-intensity") {
            self.glow_intensity = glow_intensity.clamp(0.0, 1.0);
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
    fn test_default_particles_disabled() {
        let particles = ParticleEffect::default();
        assert!(!particles.is_enabled());
    }

    #[test]
    fn test_shape_parsing() {
        assert_eq!(ParticleShape::from_str("dot"), Some(ParticleShape::Dot));
        assert_eq!(ParticleShape::from_str("STAR"), Some(ParticleShape::Star));
        assert_eq!(ParticleShape::from_str("Heart"), Some(ParticleShape::Heart));
        assert_eq!(ParticleShape::from_str("sparkle"), Some(ParticleShape::Sparkle));
    }

    #[test]
    fn test_behavior_parsing() {
        assert_eq!(ParticleBehavior::from_str("float"), Some(ParticleBehavior::Float));
        assert_eq!(ParticleBehavior::from_str("RISE"), Some(ParticleBehavior::Rise));
        assert_eq!(ParticleBehavior::from_str("Fall"), Some(ParticleBehavior::Fall));
    }
}
