//! Shape backdrop effect - geometric primitives with motion and rotation
//!
//! Renders a single geometric shape (rect, circle, ellipse, triangle, star, heart, polygon)
//! with configurable fill, stroke, glow, rotation, and motion behavior.
//!
//! ## CSS Properties
//!
//! - `--shape-enabled: true|false`
//! - `--shape-type: rect|circle|ellipse|triangle|star|heart|polygon`
//! - `--shape-size: <number>` (size in pixels)
//! - `--shape-fill: <color>` (fill color, "none" to disable)
//! - `--shape-stroke: <color>` (stroke color)
//! - `--shape-stroke-width: <number>` (stroke width in pixels)
//! - `--shape-glow-radius: <number>` (glow spread in pixels)
//! - `--shape-glow-color: <color>` (glow color, defaults to fill)
//! - `--shape-rotation: none|spin|wobble`
//! - `--shape-rotation-speed: <number>` (rotation speed)
//! - `--shape-motion: none|bounce|scroll|float|orbit`
//! - `--shape-motion-speed: <number>` (motion speed)
//! - `--shape-polygon-sides: <number>` (sides for polygon, 3-12)

use std::f64::consts::PI;

use vello::kurbo::{Affine, BezPath, Circle, Ellipse, Point, Rect};
use vello::peniko::{Brush, Color, Fill};
use vello::Scene;

use super::{BackdropEffect, EffectConfig};

/// Shape type for the effect
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ShapeType {
    /// Rectangle
    #[default]
    Rect,
    /// Circle
    Circle,
    /// Ellipse (2:1 aspect ratio)
    Ellipse,
    /// Triangle (equilateral)
    Triangle,
    /// Five-pointed star
    Star,
    /// Heart shape
    Heart,
    /// Regular polygon (configurable sides)
    Polygon,
}

impl ShapeType {
    fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "rect" | "rectangle" => Some(Self::Rect),
            "circle" => Some(Self::Circle),
            "ellipse" => Some(Self::Ellipse),
            "triangle" => Some(Self::Triangle),
            "star" => Some(Self::Star),
            "heart" => Some(Self::Heart),
            "polygon" => Some(Self::Polygon),
            _ => None,
        }
    }
}

/// Rotation behavior
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum RotationBehavior {
    /// No rotation
    #[default]
    None,
    /// Continuous spin in one direction
    Spin,
    /// Oscillating wobble back and forth
    Wobble,
}

impl RotationBehavior {
    fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "none" => Some(Self::None),
            "spin" => Some(Self::Spin),
            "wobble" => Some(Self::Wobble),
            _ => None,
        }
    }
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

    fn to_color(&self) -> Color {
        Color::from_rgba8(self.r, self.g, self.b, self.a)
    }

    fn with_alpha(&self, alpha: u8) -> Color {
        Color::from_rgba8(self.r, self.g, self.b, alpha)
    }
}

/// Motion type for deterministic position calculation
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum MotionType {
    /// No movement, static center
    #[default]
    None,
    /// Bounce off edges (simulated via sin/cos)
    Bounce,
    /// Scroll across screen
    Scroll,
    /// Gentle floating motion
    Float,
    /// Circular orbit
    Orbit,
}

impl MotionType {
    fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "none" => Some(Self::None),
            "bounce" => Some(Self::Bounce),
            "scroll" => Some(Self::Scroll),
            "float" => Some(Self::Float),
            "orbit" => Some(Self::Orbit),
            _ => None,
        }
    }
}

/// Single geometric shape effect with motion and rotation
pub struct ShapeEffect {
    /// Whether the effect is enabled
    enabled: bool,

    /// Shape type
    shape_type: ShapeType,

    /// Shape size in pixels
    size: f64,

    /// Fill color (None = no fill)
    fill: Option<Rgba>,

    /// Stroke color (None = no stroke)
    stroke: Option<Rgba>,

    /// Stroke width in pixels
    stroke_width: f64,

    /// Glow radius in pixels
    glow_radius: f64,

    /// Glow color (defaults to fill color)
    glow_color: Option<Rgba>,

    /// Rotation behavior
    rotation: RotationBehavior,

    /// Rotation speed (radians per second)
    rotation_speed: f64,

    /// Motion type
    motion_type: MotionType,

    /// Motion speed multiplier
    motion_speed: f64,

    /// Number of sides for polygon
    polygon_sides: usize,

    /// Current time for animation
    time: f64,
}

impl Default for ShapeEffect {
    fn default() -> Self {
        Self {
            enabled: false,
            shape_type: ShapeType::Circle,
            size: 100.0,
            fill: Some(Rgba::new(255, 100, 150, 200)),
            stroke: None,
            stroke_width: 2.0,
            glow_radius: 0.0,
            glow_color: None,
            rotation: RotationBehavior::None,
            rotation_speed: 1.0,
            motion_type: MotionType::Bounce,
            motion_speed: 1.0,
            polygon_sides: 6,
            time: 0.0,
        }
    }
}

impl ShapeEffect {
    /// Create a new shape effect with default settings
    pub fn new() -> Self {
        Self::default()
    }

    /// Calculate current rotation based on behavior and time
    fn calculate_rotation(&self) -> f64 {
        match self.rotation {
            RotationBehavior::None => 0.0,
            RotationBehavior::Spin => self.time * self.rotation_speed,
            RotationBehavior::Wobble => {
                // Oscillate between -45 and +45 degrees
                (self.time * self.rotation_speed).sin() * (PI / 4.0)
            }
        }
    }

    /// Calculate position based on motion type and time (deterministic)
    fn calculate_position(&self, bounds: Rect) -> Point {
        let width = bounds.width();
        let height = bounds.height();
        let cx = width / 2.0;
        let cy = height / 2.0;
        let speed = self.motion_speed as f64;
        let t = self.time * speed;

        let (x, y) = match self.motion_type {
            MotionType::None => (cx, cy),
            MotionType::Bounce => {
                // Simulate bounce using triangle wave functions
                // This creates a back-and-forth motion that looks like bouncing
                let period_x = 2.0 * (width - self.size);
                let period_y = 2.0 * (height - self.size);

                // Different speeds for x and y create diagonal movement
                let tx = t * 100.0;
                let ty = t * 75.0;

                // Triangle wave: goes 0 -> 1 -> 0 -> 1 ...
                let triangle = |v: f64, period: f64| -> f64 {
                    if period <= 0.0 { return 0.5; }
                    let p = v % period;
                    let half = period / 2.0;
                    if p < half { p / half } else { 2.0 - p / half }
                };

                let x = self.size / 2.0 + triangle(tx, period_x) * (width - self.size);
                let y = self.size / 2.0 + triangle(ty, period_y) * (height - self.size);
                (x, y)
            }
            MotionType::Scroll => {
                // Scroll diagonally, wrap around
                let x = (t * 80.0) % (width + self.size) - self.size / 2.0;
                let y = (t * 40.0) % (height + self.size) - self.size / 2.0;
                (x, y)
            }
            MotionType::Float => {
                // Gentle floating using multiple sine waves
                let x = cx + (t * 0.5).sin() * 80.0 + (t * 0.7).sin() * 40.0;
                let y = cy + (t * 0.4).sin() * 60.0 + (t * 0.6).sin() * 30.0;
                (x, y)
            }
            MotionType::Orbit => {
                // Circular orbit around center
                let radius = width.min(height) * 0.3;
                let x = cx + (t * 0.8).cos() * radius;
                let y = cy + (t * 0.8).sin() * radius;
                (x, y)
            }
        };

        Point::new(bounds.x0 + x, bounds.y0 + y)
    }

    /// Draw a rectangle
    fn draw_rect(&self, scene: &mut Scene, center: Point, angle: f64) {
        let half = self.size / 2.0;
        let rect = Rect::new(-half, -half, half, half);
        let transform = Affine::translate(center.to_vec2()) * Affine::rotate(angle);

        self.render_shape_with_glow(scene, transform, |scene, transform, brush| {
            scene.fill(Fill::NonZero, transform, brush, None, &rect);
        }, |scene, transform, brush, stroke| {
            scene.stroke(stroke, transform, brush, None, &rect);
        });
    }

    /// Draw a circle
    fn draw_circle(&self, scene: &mut Scene, center: Point, _angle: f64) {
        let circle = Circle::new(center, self.size / 2.0);

        self.render_shape_with_glow(scene, Affine::IDENTITY, |scene, transform, brush| {
            scene.fill(Fill::NonZero, transform, brush, None, &circle);
        }, |scene, transform, brush, stroke| {
            scene.stroke(stroke, transform, brush, None, &circle);
        });
    }

    /// Draw an ellipse
    fn draw_ellipse(&self, scene: &mut Scene, center: Point, angle: f64) {
        let ellipse = Ellipse::new(center, (self.size / 2.0, self.size / 4.0), angle);

        self.render_shape_with_glow(scene, Affine::IDENTITY, |scene, transform, brush| {
            scene.fill(Fill::NonZero, transform, brush, None, &ellipse);
        }, |scene, transform, brush, stroke| {
            scene.stroke(stroke, transform, brush, None, &ellipse);
        });
    }

    /// Draw a triangle
    fn draw_triangle(&self, scene: &mut Scene, center: Point, angle: f64) {
        let path = self.build_polygon_path(3, angle);
        let transform = Affine::translate(center.to_vec2());

        self.render_shape_with_glow(scene, transform, |scene, transform, brush| {
            scene.fill(Fill::NonZero, transform, brush, None, &path);
        }, |scene, transform, brush, stroke| {
            scene.stroke(stroke, transform, brush, None, &path);
        });
    }

    /// Draw a star
    fn draw_star(&self, scene: &mut Scene, center: Point, angle: f64) {
        let path = self.build_star_path(5, angle);
        let transform = Affine::translate(center.to_vec2());

        self.render_shape_with_glow(scene, transform, |scene, transform, brush| {
            scene.fill(Fill::NonZero, transform, brush, None, &path);
        }, |scene, transform, brush, stroke| {
            scene.stroke(stroke, transform, brush, None, &path);
        });
    }

    /// Draw a heart
    fn draw_heart(&self, scene: &mut Scene, center: Point, angle: f64) {
        let path = self.build_heart_path(angle);
        let transform = Affine::translate(center.to_vec2());

        self.render_shape_with_glow(scene, transform, |scene, transform, brush| {
            scene.fill(Fill::NonZero, transform, brush, None, &path);
        }, |scene, transform, brush, stroke| {
            scene.stroke(stroke, transform, brush, None, &path);
        });
    }

    /// Draw a polygon
    fn draw_polygon(&self, scene: &mut Scene, center: Point, angle: f64) {
        let path = self.build_polygon_path(self.polygon_sides, angle);
        let transform = Affine::translate(center.to_vec2());

        self.render_shape_with_glow(scene, transform, |scene, transform, brush| {
            scene.fill(Fill::NonZero, transform, brush, None, &path);
        }, |scene, transform, brush, stroke| {
            scene.stroke(stroke, transform, brush, None, &path);
        });
    }

    /// Render shape with glow, fill, and stroke
    fn render_shape_with_glow<F, S>(&self, scene: &mut Scene, transform: Affine, fill_fn: F, stroke_fn: S)
    where
        F: Fn(&mut Scene, Affine, &Brush),
        S: Fn(&mut Scene, Affine, &Brush, &vello::kurbo::Stroke),
    {
        // Draw glow layers
        if self.glow_radius > 0.0 {
            let glow_color = self.glow_color.unwrap_or_else(|| {
                self.fill.unwrap_or(Rgba::new(255, 255, 255, 255))
            });

            let glow_layers = 4;
            for i in (1..=glow_layers).rev() {
                let t = i as f64 / glow_layers as f64;
                let scale = 1.0 + (self.glow_radius / self.size) * t;
                let alpha = ((1.0 - t) * 0.3 * glow_color.a as f64) as u8;

                if alpha > 0 {
                    let glow_brush = Brush::Solid(glow_color.with_alpha(alpha));
                    let glow_transform = transform * Affine::scale(scale);
                    fill_fn(scene, glow_transform, &glow_brush);
                }
            }
        }

        // Draw fill
        if let Some(fill_color) = self.fill {
            let brush = Brush::Solid(fill_color.to_color());
            fill_fn(scene, transform, &brush);
        }

        // Draw stroke
        if let Some(stroke_color) = self.stroke {
            let brush = Brush::Solid(stroke_color.to_color());
            let stroke = vello::kurbo::Stroke::new(self.stroke_width);
            stroke_fn(scene, transform, &brush, &stroke);
        }
    }

    /// Build a regular polygon path
    fn build_polygon_path(&self, sides: usize, rotation: f64) -> BezPath {
        let mut path = BezPath::new();
        let radius = self.size / 2.0;

        for i in 0..sides {
            let angle = rotation + (i as f64 * 2.0 * PI / sides as f64) - PI / 2.0;
            let x = angle.cos() * radius;
            let y = angle.sin() * radius;

            if i == 0 {
                path.move_to(Point::new(x, y));
            } else {
                path.line_to(Point::new(x, y));
            }
        }
        path.close_path();
        path
    }

    /// Build a star path
    fn build_star_path(&self, points: usize, rotation: f64) -> BezPath {
        let mut path = BezPath::new();
        let outer_radius = self.size / 2.0;
        let inner_radius = outer_radius * 0.4;

        for i in 0..(points * 2) {
            let angle = rotation + (i as f64 * PI / points as f64) - PI / 2.0;
            let radius = if i % 2 == 0 { outer_radius } else { inner_radius };
            let x = angle.cos() * radius;
            let y = angle.sin() * radius;

            if i == 0 {
                path.move_to(Point::new(x, y));
            } else {
                path.line_to(Point::new(x, y));
            }
        }
        path.close_path();
        path
    }

    /// Build a heart path
    fn build_heart_path(&self, rotation: f64) -> BezPath {
        let mut path = BezPath::new();
        let s = self.size / 2.0;

        // Transform points by rotation
        let rotate = |x: f64, y: f64| -> Point {
            let cos_r = rotation.cos();
            let sin_r = rotation.sin();
            Point::new(x * cos_r - y * sin_r, x * sin_r + y * cos_r)
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
}

impl BackdropEffect for ShapeEffect {
    fn effect_type(&self) -> &'static str {
        "shape"
    }

    fn update(&mut self, _dt: f32, time: f32) {
        self.time = time as f64;
    }

    fn render(&self, scene: &mut Scene, bounds: Rect) {
        if !self.enabled {
            return;
        }

        // Calculate position deterministically based on time
        let center = self.calculate_position(bounds);

        // Calculate rotation
        let angle = self.calculate_rotation();

        // Draw the shape
        match self.shape_type {
            ShapeType::Rect => self.draw_rect(scene, center, angle),
            ShapeType::Circle => self.draw_circle(scene, center, angle),
            ShapeType::Ellipse => self.draw_ellipse(scene, center, angle),
            ShapeType::Triangle => self.draw_triangle(scene, center, angle),
            ShapeType::Star => self.draw_star(scene, center, angle),
            ShapeType::Heart => self.draw_heart(scene, center, angle),
            ShapeType::Polygon => self.draw_polygon(scene, center, angle),
        }
    }

    fn configure(&mut self, config: &EffectConfig) {
        if let Some(enabled) = config.get_bool("enabled") {
            self.enabled = enabled;
        }

        if let Some(type_str) = config.get("type") {
            if let Some(shape_type) = ShapeType::from_str(type_str) {
                self.shape_type = shape_type;
            }
        }

        if let Some(size) = config.get_f64("size") {
            self.size = size.clamp(10.0, 1000.0);
        }

        if let Some(fill_str) = config.get("fill") {
            if fill_str.to_lowercase() == "none" {
                self.fill = None;
            } else if let Some(color) = parse_simple_color(fill_str) {
                self.fill = Some(color);
            }
        }

        if let Some(stroke_str) = config.get("stroke") {
            if stroke_str.to_lowercase() == "none" {
                self.stroke = None;
            } else if let Some(color) = parse_simple_color(stroke_str) {
                self.stroke = Some(color);
            }
        }

        if let Some(stroke_width) = config.get_f64("stroke-width") {
            self.stroke_width = stroke_width.clamp(0.5, 20.0);
        }

        if let Some(glow_radius) = config.get_f64("glow-radius") {
            self.glow_radius = glow_radius.max(0.0);
        }

        if let Some(glow_color_str) = config.get("glow-color") {
            if let Some(color) = parse_simple_color(glow_color_str) {
                self.glow_color = Some(color);
            }
        }

        if let Some(rotation_str) = config.get("rotation") {
            if let Some(rotation) = RotationBehavior::from_str(rotation_str) {
                self.rotation = rotation;
            }
        }

        if let Some(rotation_speed) = config.get_f64("rotation-speed") {
            self.rotation_speed = rotation_speed;
        }

        if let Some(motion_str) = config.get("motion") {
            if let Some(motion) = MotionType::from_str(motion_str) {
                self.motion_type = motion;
            }
        }

        if let Some(motion_speed) = config.get_f64("motion-speed") {
            self.motion_speed = motion_speed.max(0.0);
        }

        if let Some(sides) = config.get_usize("polygon-sides") {
            self.polygon_sides = sides.clamp(3, 12);
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
    fn test_default_shape_disabled() {
        let shape = ShapeEffect::default();
        assert!(!shape.is_enabled());
    }

    #[test]
    fn test_shape_type_parsing() {
        assert_eq!(ShapeType::from_str("rect"), Some(ShapeType::Rect));
        assert_eq!(ShapeType::from_str("CIRCLE"), Some(ShapeType::Circle));
        assert_eq!(ShapeType::from_str("Star"), Some(ShapeType::Star));
        assert_eq!(ShapeType::from_str("heart"), Some(ShapeType::Heart));
        assert_eq!(ShapeType::from_str("polygon"), Some(ShapeType::Polygon));
    }

    #[test]
    fn test_rotation_parsing() {
        assert_eq!(RotationBehavior::from_str("none"), Some(RotationBehavior::None));
        assert_eq!(RotationBehavior::from_str("SPIN"), Some(RotationBehavior::Spin));
        assert_eq!(RotationBehavior::from_str("Wobble"), Some(RotationBehavior::Wobble));
    }

    #[test]
    fn test_color_parsing() {
        let hex = parse_simple_color("#ff0000");
        assert!(hex.is_some());
        let c = hex.unwrap();
        assert_eq!(c.r, 255);
        assert_eq!(c.g, 0);
        assert_eq!(c.b, 0);

        let rgba = parse_simple_color("rgba(100, 150, 200, 0.5)");
        assert!(rgba.is_some());
        let c = rgba.unwrap();
        assert_eq!(c.r, 100);
        assert_eq!(c.g, 150);
        assert_eq!(c.b, 200);
        assert_eq!(c.a, 127);
    }
}
