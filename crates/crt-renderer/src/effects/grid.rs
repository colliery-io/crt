//! Grid backdrop effect - perspective grid with animation
//!
//! Renders a retro-style perspective grid below a configurable horizon line.
//! Vertical lines converge toward a vanishing point, horizontal lines
//! use perspective spacing, and the whole grid animates/scrolls.
//!
//! ## CSS Properties
//!
//! - `--grid-enabled: true|false`
//! - `--grid-color: <color>`
//! - `--grid-spacing: <number>` (grid density)
//! - `--grid-line-width: <number>`
//! - `--grid-perspective: <number>` (0 = flat, higher = more perspective)
//! - `--grid-horizon: <number>` (0 = top, 1 = bottom)
//! - `--grid-intensity: <number>` (0-1 opacity)
//! - `--grid-animation-speed: <number>`

use vello::kurbo::{Affine, Line, Rect, Stroke};
use vello::peniko::{Brush, Color};
use vello::Scene;

use super::{BackdropEffect, EffectConfig};

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

    fn to_peniko_color(&self) -> Color {
        Color::from_rgba8(self.r, self.g, self.b, self.a)
    }

    fn with_alpha(&self, alpha: u8) -> Color {
        Color::from_rgba8(self.r, self.g, self.b, alpha)
    }
}

/// Perspective grid effect
pub struct GridEffect {
    /// Whether the effect is enabled
    enabled: bool,

    /// Grid line color (stored as RGBA)
    color: Rgba,

    /// Grid spacing/density
    spacing: f64,

    /// Line width in pixels
    line_width: f64,

    /// Perspective amount (0 = flat, higher = more perspective)
    perspective: f64,

    /// Horizon position (0 = top, 1 = bottom)
    horizon: f64,

    /// Overall intensity/opacity (0-1)
    intensity: f64,

    /// Animation speed multiplier
    animation_speed: f64,

    /// Current time offset for animation
    time_offset: f64,
}

impl Default for GridEffect {
    fn default() -> Self {
        Self {
            enabled: false,
            color: Rgba::new(0, 255, 255, 40), // Cyan with low alpha
            spacing: 8.0,
            line_width: 1.0,
            perspective: 2.0,
            horizon: 0.3,
            intensity: 1.0,
            animation_speed: 0.5,
            time_offset: 0.0,
        }
    }
}

impl GridEffect {
    /// Create a new grid effect with default settings
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a grid effect with custom color
    pub fn with_color(mut self, r: u8, g: u8, b: u8, a: u8) -> Self {
        self.color = Rgba::new(r, g, b, a);
        self
    }

    /// Set enabled state
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Calculate perspective-adjusted Y position
    /// Maps a linear t (0-1) to perspective-adjusted screen position
    fn perspective_y(&self, t: f64, horizon_y: f64, bottom_y: f64) -> f64 {
        let grid_height = bottom_y - horizon_y;
        let perspective_t = t.powf(self.perspective);
        horizon_y + perspective_t * grid_height
    }

    /// Calculate fade factor based on distance from horizon
    fn distance_fade(&self, t: f64) -> f64 {
        // Fade near horizon and slightly at distance
        let horizon_fade = (t * 6.67).min(1.0); // Fade in over first ~15%
        let distance_fade = 1.0 - t * 0.3;
        horizon_fade * distance_fade * self.intensity
    }
}

impl BackdropEffect for GridEffect {
    fn effect_type(&self) -> &'static str {
        "grid"
    }

    fn update(&mut self, _dt: f32, time: f32) {
        self.time_offset = time as f64 * self.animation_speed;
    }

    fn render(&self, scene: &mut Scene, bounds: Rect) {
        if !self.enabled || self.intensity <= 0.0 {
            return;
        }

        let width = bounds.width();
        let height = bounds.height();
        let horizon_y = bounds.y0 + height * self.horizon;
        let bottom_y = bounds.y1;
        let grid_height = bottom_y - horizon_y;

        if grid_height <= 0.0 {
            return;
        }

        let center_x = bounds.x0 + width / 2.0;

        // Number of horizontal lines based on spacing
        let h_line_count = (self.spacing * 2.0) as usize;

        // Draw horizontal lines with perspective spacing
        for i in 0..h_line_count {
            // Calculate base position with animation offset
            let base_t = (i as f64 / h_line_count as f64 + self.time_offset * 0.1) % 1.0;

            // Apply perspective to get actual Y position
            let y = self.perspective_y(base_t, horizon_y, bottom_y);

            if y < horizon_y || y > bottom_y {
                continue;
            }

            // Calculate fade and line properties
            let t = (y - horizon_y) / grid_height;
            let fade = self.distance_fade(t);

            if fade <= 0.01 {
                continue;
            }

            // Adjust color alpha based on fade
            let alpha = (self.color.a as f64 / 255.0 * fade * 255.0) as u8;
            let line_color = self.color.with_alpha(alpha);

            // Line width increases with perspective (closer = wider)
            let adjusted_width = self.line_width * (0.5 + t * 1.5);

            let line = Line::new((bounds.x0, y), (bounds.x1, y));
            let stroke = Stroke::new(adjusted_width);

            scene.stroke(&stroke, Affine::IDENTITY, &Brush::Solid(line_color), None, &line);
        }

        // Number of vertical lines
        let v_line_count = (self.spacing * 2.0) as usize;
        let half_count = v_line_count / 2;

        // Draw vertical lines converging to vanishing point at horizon center
        for i in 0..=v_line_count {
            // Position from -half to +half, normalized
            let x_offset = (i as f64 - half_count as f64) / half_count as f64;

            // Start position at bottom of screen
            let bottom_x = center_x + x_offset * width * 0.5;

            // End position converges toward center at horizon
            // Lines closer to center converge less
            let convergence = x_offset.abs().powf(0.5); // Square root for smoother convergence
            let horizon_x = center_x + x_offset * width * 0.1 * convergence;

            // Draw line from horizon to bottom
            let line = Line::new((horizon_x, horizon_y), (bottom_x, bottom_y));

            // Fade lines that are further from center
            let center_fade = 1.0 - x_offset.abs() * 0.3;
            let alpha = (self.color.a as f64 / 255.0 * self.intensity * center_fade * 255.0) as u8;
            let line_color = self.color.with_alpha(alpha);

            let stroke = Stroke::new(self.line_width);
            scene.stroke(&stroke, Affine::IDENTITY, &Brush::Solid(line_color), None, &line);
        }
    }

    fn configure(&mut self, config: &EffectConfig) {
        if let Some(enabled) = config.get_bool("enabled") {
            self.enabled = enabled;
        }

        if let Some(spacing) = config.get_f64("spacing") {
            self.spacing = spacing;
        }

        if let Some(line_width) = config.get_f64("line-width") {
            self.line_width = line_width;
        }

        if let Some(perspective) = config.get_f64("perspective") {
            self.perspective = perspective;
        }

        if let Some(horizon) = config.get_f64("horizon") {
            self.horizon = horizon.clamp(0.0, 1.0);
        }

        if let Some(intensity) = config.get_f64("intensity") {
            self.intensity = intensity.clamp(0.0, 1.0);
        }

        if let Some(speed) = config.get_f64("animation-speed") {
            self.animation_speed = speed;
        }

        // Parse color - for now just support basic format
        // Full color parsing will be in theme parser task
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
/// Full parsing handled by theme parser
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
    fn test_default_grid_is_disabled() {
        let grid = GridEffect::default();
        assert!(!grid.is_enabled());
    }

    #[test]
    fn test_configure_enables_grid() {
        let mut grid = GridEffect::default();
        let mut config = EffectConfig::new();
        config.insert("enabled", "true");

        grid.configure(&config);

        assert!(grid.is_enabled());
    }

    #[test]
    fn test_parse_hex_color() {
        let color = parse_simple_color("#ff00ff").unwrap();
        assert_eq!(color.r, 255);
        assert_eq!(color.g, 0);
        assert_eq!(color.b, 255);
        assert_eq!(color.a, 255);
    }

    #[test]
    fn test_parse_rgba_color() {
        let color = parse_simple_color("rgba(255, 128, 0, 0.5)").unwrap();
        assert_eq!(color.r, 255);
        assert_eq!(color.g, 128);
        assert_eq!(color.b, 0);
        assert_eq!(color.a, 127); // 0.5 * 255 = 127
    }

    #[test]
    fn test_perspective_y_calculation() {
        let grid = GridEffect {
            perspective: 2.0,
            ..Default::default()
        };

        // At t=0, should be at horizon
        let y0 = grid.perspective_y(0.0, 100.0, 500.0);
        assert!((y0 - 100.0).abs() < 0.01);

        // At t=1, should be at bottom
        let y1 = grid.perspective_y(1.0, 100.0, 500.0);
        assert!((y1 - 500.0).abs() < 0.01);

        // At t=0.5 with perspective, should be closer to horizon than linear
        let y_mid = grid.perspective_y(0.5, 100.0, 500.0);
        let linear_mid = 300.0; // (100 + 500) / 2
        assert!(y_mid < linear_mid, "Perspective should compress toward horizon");
    }
}
