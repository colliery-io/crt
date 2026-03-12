//! Matrix-style falling code effect
//!
//! Renders columns of falling characters represented as simple geometric shapes.
//! Uses bright head and fading trail for the classic matrix rain look.

use super::{BackdropEffect, EffectConfig};
use vello::Scene;
use vello::kurbo::{Affine, BezPath, Circle, Point, Rect, Shape, Stroke};
use vello::peniko::{Brush, Color};

/// A single column of falling characters
#[derive(Debug, Clone)]
struct Column {
    /// X position as fraction of width (0-1)
    x: f64,
    /// Current Y position of the head as fraction of height (0-1, can go > 1)
    head_y: f64,
    /// Character shapes in the trail (seed values for deterministic shapes)
    char_seeds: Vec<u32>,
    /// Trail length as fraction of height
    trail_length: f64,
    /// Speed multiplier for this column
    speed: f64,
    /// Whether this column is active
    active: bool,
    /// Delay before respawning (in seconds)
    respawn_delay: f64,
}

/// Matrix falling code effect
pub struct MatrixEffect {
    enabled: bool,
    color: Color,
    columns: Vec<Column>,
    char_width: f64,
    char_height: f64,
    base_speed: f64,
    density: f64,
    time: f64,
    num_columns: usize,
    needs_regeneration: bool,
    /// Custom charset - each unique character maps to a unique shape
    charset: Vec<char>,
}

impl MatrixEffect {
    pub fn new() -> Self {
        Self {
            enabled: false,
            color: Color::from_rgba8(0, 255, 70, 255), // Classic matrix green
            columns: Vec::new(),
            char_width: 10.0,
            char_height: 14.0,
            base_speed: 0.3, // Fraction of height per second
            density: 1.0,
            time: 0.0,
            num_columns: 100,
            needs_regeneration: true,
            charset: Vec::new(), // Empty = use default shapes
        }
    }

    /// Murmur3-style hash for random number generation
    fn hash(mut h: u32) -> u32 {
        h ^= h >> 16;
        h = h.wrapping_mul(0x85ebca6b);
        h ^= h >> 13;
        h = h.wrapping_mul(0xc2b2ae35);
        h ^= h >> 16;
        h
    }

    /// Get a random float [0, 1) from a seed
    fn rand(seed: u32) -> f64 {
        (Self::hash(seed) as f64) / (u32::MAX as f64)
    }

    /// Draw a matrix-style character glyph
    /// If charset is provided, renders stylized letter shapes
    /// Otherwise uses random geometric shapes
    fn draw_char(center: Point, width: f64, height: f64, seed: u32, charset: &[char]) -> BezPath {
        let mut path = BezPath::new();

        let w = width * 0.7;
        let h = height * 0.8;
        let x = center.x - w / 2.0;
        let y = center.y - h / 2.0;

        // If charset provided, draw stylized letter shapes
        if !charset.is_empty() {
            let char_idx = Self::hash(seed) as usize % charset.len();
            let ch = charset[char_idx].to_ascii_uppercase();

            match ch {
                'A' => {
                    // A shape: /\ with bar
                    path.move_to(Point::new(x, y + h));
                    path.line_to(Point::new(x + w / 2.0, y));
                    path.line_to(Point::new(x + w, y + h));
                    path.move_to(Point::new(x + w * 0.2, y + h * 0.6));
                    path.line_to(Point::new(x + w * 0.8, y + h * 0.6));
                }
                'B' => {
                    // B shape
                    path.move_to(Point::new(x, y));
                    path.line_to(Point::new(x, y + h));
                    path.line_to(Point::new(x + w * 0.7, y + h));
                    path.line_to(Point::new(x + w, y + h * 0.75));
                    path.line_to(Point::new(x + w * 0.7, y + h * 0.5));
                    path.line_to(Point::new(x, y + h * 0.5));
                    path.move_to(Point::new(x + w * 0.7, y + h * 0.5));
                    path.line_to(Point::new(x + w, y + h * 0.25));
                    path.line_to(Point::new(x + w * 0.7, y));
                    path.line_to(Point::new(x, y));
                }
                'C' => {
                    // C shape: arc
                    path.move_to(Point::new(x + w, y + h * 0.2));
                    path.line_to(Point::new(x + w * 0.3, y));
                    path.line_to(Point::new(x, y + h * 0.3));
                    path.line_to(Point::new(x, y + h * 0.7));
                    path.line_to(Point::new(x + w * 0.3, y + h));
                    path.line_to(Point::new(x + w, y + h * 0.8));
                }
                'D' => {
                    // D shape
                    path.move_to(Point::new(x, y));
                    path.line_to(Point::new(x, y + h));
                    path.line_to(Point::new(x + w * 0.6, y + h));
                    path.line_to(Point::new(x + w, y + h * 0.5));
                    path.line_to(Point::new(x + w * 0.6, y));
                    path.close_path();
                }
                'E' => {
                    // E shape
                    path.move_to(Point::new(x + w, y));
                    path.line_to(Point::new(x, y));
                    path.line_to(Point::new(x, y + h));
                    path.line_to(Point::new(x + w, y + h));
                    path.move_to(Point::new(x, y + h * 0.5));
                    path.line_to(Point::new(x + w * 0.7, y + h * 0.5));
                }
                'F' => {
                    // F shape
                    path.move_to(Point::new(x + w, y));
                    path.line_to(Point::new(x, y));
                    path.line_to(Point::new(x, y + h));
                    path.move_to(Point::new(x, y + h * 0.5));
                    path.line_to(Point::new(x + w * 0.7, y + h * 0.5));
                }
                'G' => {
                    // G shape: C with bar
                    path.move_to(Point::new(x + w, y + h * 0.2));
                    path.line_to(Point::new(x + w * 0.3, y));
                    path.line_to(Point::new(x, y + h * 0.3));
                    path.line_to(Point::new(x, y + h * 0.7));
                    path.line_to(Point::new(x + w * 0.3, y + h));
                    path.line_to(Point::new(x + w, y + h * 0.8));
                    path.line_to(Point::new(x + w, y + h * 0.5));
                    path.line_to(Point::new(x + w * 0.5, y + h * 0.5));
                }
                'H' => {
                    // H shape
                    path.move_to(Point::new(x, y));
                    path.line_to(Point::new(x, y + h));
                    path.move_to(Point::new(x + w, y));
                    path.line_to(Point::new(x + w, y + h));
                    path.move_to(Point::new(x, y + h * 0.5));
                    path.line_to(Point::new(x + w, y + h * 0.5));
                }
                'I' => {
                    // I shape
                    path.move_to(Point::new(x + w * 0.2, y));
                    path.line_to(Point::new(x + w * 0.8, y));
                    path.move_to(Point::new(x + w * 0.5, y));
                    path.line_to(Point::new(x + w * 0.5, y + h));
                    path.move_to(Point::new(x + w * 0.2, y + h));
                    path.line_to(Point::new(x + w * 0.8, y + h));
                }
                'T' => {
                    // T shape
                    path.move_to(Point::new(x, y));
                    path.line_to(Point::new(x + w, y));
                    path.move_to(Point::new(x + w * 0.5, y));
                    path.line_to(Point::new(x + w * 0.5, y + h));
                }
                'U' => {
                    // U shape
                    path.move_to(Point::new(x, y));
                    path.line_to(Point::new(x, y + h * 0.7));
                    path.line_to(Point::new(x + w * 0.3, y + h));
                    path.line_to(Point::new(x + w * 0.7, y + h));
                    path.line_to(Point::new(x + w, y + h * 0.7));
                    path.line_to(Point::new(x + w, y));
                }
                'X' => {
                    // X shape
                    path.move_to(Point::new(x, y));
                    path.line_to(Point::new(x + w, y + h));
                    path.move_to(Point::new(x + w, y));
                    path.line_to(Point::new(x, y + h));
                }
                '0' | 'O' => {
                    // O/0 shape
                    path.move_to(Point::new(x + w * 0.3, y));
                    path.line_to(Point::new(x, y + h * 0.3));
                    path.line_to(Point::new(x, y + h * 0.7));
                    path.line_to(Point::new(x + w * 0.3, y + h));
                    path.line_to(Point::new(x + w * 0.7, y + h));
                    path.line_to(Point::new(x + w, y + h * 0.7));
                    path.line_to(Point::new(x + w, y + h * 0.3));
                    path.line_to(Point::new(x + w * 0.7, y));
                    path.close_path();
                }
                '1' => {
                    // 1 shape
                    path.move_to(Point::new(x + w * 0.3, y + h * 0.2));
                    path.line_to(Point::new(x + w * 0.5, y));
                    path.line_to(Point::new(x + w * 0.5, y + h));
                    path.move_to(Point::new(x + w * 0.2, y + h));
                    path.line_to(Point::new(x + w * 0.8, y + h));
                }
                _ => {
                    // Default: use character code to pick a shape
                    let shape = (ch as u32) % 4;
                    match shape {
                        0 => {
                            path.move_to(Point::new(x, y));
                            path.line_to(Point::new(x + w, y + h));
                        }
                        1 => {
                            path.move_to(Point::new(x + w / 2.0, y));
                            path.line_to(Point::new(x + w / 2.0, y + h));
                        }
                        2 => {
                            path.move_to(Point::new(x, y + h / 2.0));
                            path.line_to(Point::new(x + w, y + h / 2.0));
                        }
                        _ => {
                            path.move_to(Point::new(x + w, y));
                            path.line_to(Point::new(x, y + h));
                        }
                    }
                }
            }
        } else {
            // No charset: use random geometric shapes
            let char_type = Self::hash(seed) % 8;
            match char_type {
                0 => {
                    path.move_to(Point::new(x + w * 0.2, y));
                    path.line_to(Point::new(x + w * 0.2, y + h));
                    path.move_to(Point::new(x + w * 0.8, y));
                    path.line_to(Point::new(x + w * 0.8, y + h));
                }
                1 => {
                    path.move_to(Point::new(x, y));
                    path.line_to(Point::new(x + w, y));
                    path.line_to(Point::new(x + w, y + h));
                    path.line_to(Point::new(x, y + h));
                    path.close_path();
                }
                2 => {
                    path.move_to(Point::new(x + w / 2.0, y));
                    path.line_to(Point::new(x + w / 2.0, y + h));
                    path.move_to(Point::new(x, y + h / 2.0));
                    path.line_to(Point::new(x + w, y + h / 2.0));
                }
                3 => {
                    path.move_to(Point::new(x + w / 2.0, y));
                    path.line_to(Point::new(x + w, y + h));
                    path.line_to(Point::new(x, y + h));
                    path.close_path();
                }
                4 => {
                    path.move_to(Point::new(x, y));
                    path.line_to(Point::new(x + w, y));
                    path.move_to(Point::new(x, y + h / 2.0));
                    path.line_to(Point::new(x + w, y + h / 2.0));
                    path.move_to(Point::new(x, y + h));
                    path.line_to(Point::new(x + w, y + h));
                }
                5 => {
                    path.move_to(Point::new(x, y));
                    path.line_to(Point::new(x, y + h));
                    path.line_to(Point::new(x + w, y + h));
                }
                6 => {
                    path.move_to(Point::new(x, y));
                    path.line_to(Point::new(x + w, y + h));
                    path.move_to(Point::new(x + w, y));
                    path.line_to(Point::new(x, y + h));
                }
                _ => {
                    let dot_r = w * 0.15;
                    for dx in [0.25, 0.75] {
                        for dy in [0.25, 0.75] {
                            let cx = x + w * dx;
                            let cy = y + h * dy;
                            path.move_to(Point::new(cx + dot_r, cy));
                            path.extend(Circle::new(Point::new(cx, cy), dot_r).path_elements(0.1));
                        }
                    }
                }
            }
        }

        path
    }

    /// Initialize columns
    fn generate_columns(&mut self) {
        self.columns.clear();
        self.columns.reserve(self.num_columns);

        for i in 0..self.num_columns {
            let seed = i as u32;
            let x = i as f64 / self.num_columns as f64;

            // Stagger initial positions
            let initial_y = -Self::rand(seed) * 0.5;
            let trail_len = 0.15 + Self::rand(seed.wrapping_add(1000)) * 0.25; // 15-40% of screen
            let speed = 0.7 + Self::rand(seed.wrapping_add(2000)) * 0.6;

            let num_chars = 20; // Approximate max trail characters
            let mut char_seeds = Vec::with_capacity(num_chars);
            for j in 0..num_chars {
                char_seeds.push(seed.wrapping_add(j as u32 * 100));
            }

            self.columns.push(Column {
                x,
                head_y: initial_y,
                char_seeds,
                trail_length: trail_len,
                speed,
                active: true,
                respawn_delay: 0.0,
            });
        }

        self.needs_regeneration = false;
    }
}

impl Default for MatrixEffect {
    fn default() -> Self {
        Self::new()
    }
}

impl BackdropEffect for MatrixEffect {
    fn effect_type(&self) -> &'static str {
        "matrix"
    }

    fn update(&mut self, dt: f32, time: f32) {
        if !self.enabled {
            return;
        }

        if self.needs_regeneration {
            self.generate_columns();
        }

        self.time = time as f64;
        let dt = dt as f64;

        for column in &mut self.columns {
            let idx = (column.x * 1000.0) as u32;

            if column.active {
                // Move the head down (using normalized coordinates)
                column.head_y += self.base_speed * column.speed * dt;

                // Occasionally change a random character in the trail
                let change_seed = (self.time * 1000.0) as u32 + idx;
                if Self::rand(change_seed) < 0.1 && !column.char_seeds.is_empty() {
                    let char_idx = Self::hash(change_seed.wrapping_add(500)) as usize
                        % column.char_seeds.len();
                    column.char_seeds[char_idx] = change_seed.wrapping_add(1000);
                }

                // Check if column has gone off screen
                if column.head_y > 1.0 + column.trail_length {
                    column.active = false;
                    column.respawn_delay = Self::rand(idx + (self.time * 100.0) as u32) * 2.0;
                }
            } else {
                // Count down respawn delay
                column.respawn_delay -= dt;
                if column.respawn_delay <= 0.0 {
                    // Respawn at top
                    let seed = idx + (self.time * 1000.0) as u32;
                    column.head_y = -Self::rand(seed) * 0.2;
                    column.trail_length = 0.15 + Self::rand(seed.wrapping_add(1000)) * 0.25;
                    column.speed = 0.7 + Self::rand(seed.wrapping_add(2000)) * 0.6;
                    column.active = true;

                    // Regenerate character seeds
                    for (j, char_seed) in column.char_seeds.iter_mut().enumerate() {
                        *char_seed = seed.wrapping_add(j as u32 * 100);
                    }
                }
            }
        }
    }

    fn render(&self, scene: &mut Scene, bounds: Rect) {
        if !self.enabled || self.columns.is_empty() {
            return;
        }

        let width = bounds.width();
        let height = bounds.height();
        let [r, g, b, _] = self.color.components;

        // Calculate character spacing based on height
        let char_spacing = self.char_height / height;

        for column in &self.columns {
            if !column.active && column.respawn_delay > 0.5 {
                continue;
            }

            let x_px = column.x * width;
            let head_y_px = column.head_y * height;

            // Render trail characters (from head backwards)
            let num_chars = (column.trail_length / char_spacing) as usize;
            for i in 0..num_chars.min(column.char_seeds.len()) {
                let char_y_px = head_y_px - (i as f64 * self.char_height);

                // Skip if off screen
                if char_y_px < -self.char_height || char_y_px > height + self.char_height {
                    continue;
                }

                // Calculate alpha based on position in trail
                let alpha = if i == 0 {
                    255 // Head is full brightness
                } else {
                    let fade = 1.0 - (i as f64 / num_chars as f64);
                    (fade * fade * 200.0) as u8 // Quadratic falloff
                };

                if alpha == 0 {
                    continue;
                }

                // Head character is white/bright, trail is colored
                let color = if i == 0 {
                    // Bright white-ish head
                    Color::new([
                        (0.78 + r * 0.22).min(1.0),
                        1.0,
                        (0.78 + b * 0.22).min(1.0),
                        1.0,
                    ])
                } else {
                    Color::new([r, g, b, alpha as f32 / 255.0])
                };

                // Draw the character shape
                let center = Point::new(x_px, char_y_px + self.char_height / 2.0);
                let char_seed = column.char_seeds[i];
                let path = Self::draw_char(
                    center,
                    self.char_width,
                    self.char_height,
                    char_seed,
                    &self.charset,
                );

                let stroke = Stroke::new(1.5);
                scene.stroke(&stroke, Affine::IDENTITY, &Brush::Solid(color), None, &path);
            }
        }
    }

    fn configure(&mut self, config: &EffectConfig) {
        // Note: EffectsRenderer strips the "matrix-" prefix before calling configure
        if let Some(enabled) = config.get_bool("enabled") {
            self.enabled = enabled;
            if enabled {
                self.needs_regeneration = true;
            }
        }

        if let Some(color_str) = config.get("color") {
            if let Some(color) = parse_color(color_str) {
                self.color = color;
            }
        }

        if let Some(density) = config.get_f64("density") {
            let new_density = density.clamp(0.1, 3.0);
            if (new_density - self.density).abs() > 0.01 {
                self.density = new_density;
                self.num_columns = (100.0 * self.density) as usize;
                self.needs_regeneration = true;
            }
        }

        if let Some(speed) = config.get_f64("speed") {
            self.base_speed = (speed / 25.0).clamp(0.05, 1.0); // Convert to fraction of screen height/sec
        }

        if let Some(font_size) = config.get_f64("font-size") {
            self.char_height = font_size.clamp(8.0, 32.0);
            self.char_width = font_size * 0.7;
        }

        if let Some(charset_str) = config.get("charset") {
            let chars: Vec<char> = charset_str.trim_matches('"').chars().collect();
            if !chars.is_empty() {
                self.charset = chars;
            }
        }
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }
}

/// Parse a CSS color string into a Color
fn parse_color(s: &str) -> Option<Color> {
    let s = s.trim();

    // Handle #RRGGBB and #RRGGBBAA
    if s.starts_with('#') {
        let hex = &s[1..];
        match hex.len() {
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                return Some(Color::from_rgba8(r, g, b, 255));
            }
            8 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
                return Some(Color::from_rgba8(r, g, b, a));
            }
            _ => return None,
        }
    }

    // Handle rgb(r, g, b) and rgba(r, g, b, a)
    if s.starts_with("rgb") {
        let inner = s
            .trim_start_matches("rgba(")
            .trim_start_matches("rgb(")
            .trim_end_matches(')');
        let parts: Vec<&str> = inner.split(',').map(|p| p.trim()).collect();

        if parts.len() >= 3 {
            let r: u8 = parts[0].parse().ok()?;
            let g: u8 = parts[1].parse().ok()?;
            let b: u8 = parts[2].parse().ok()?;
            let a: u8 = if parts.len() >= 4 {
                let a_float: f32 = parts[3].parse().ok()?;
                (a_float * 255.0) as u8
            } else {
                255
            };
            return Some(Color::from_rgba8(r, g, b, a));
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Construction and defaults ────────────────────────────────

    #[test]
    fn default_matrix_is_disabled() {
        let m = MatrixEffect::new();
        assert!(!m.is_enabled());
        assert_eq!(m.effect_type(), "matrix");
    }

    #[test]
    fn default_columns_empty_before_generation() {
        let m = MatrixEffect::new();
        assert!(m.columns.is_empty());
        assert!(m.needs_regeneration);
    }

    // ── Hash and rand helpers ────────────────────────────────────

    #[test]
    fn hash_is_deterministic() {
        assert_eq!(MatrixEffect::hash(42), MatrixEffect::hash(42));
        // Different inputs produce different outputs
        assert_ne!(MatrixEffect::hash(0), MatrixEffect::hash(1));
    }

    #[test]
    fn rand_produces_zero_to_one() {
        for seed in 0..100 {
            let val = MatrixEffect::rand(seed);
            assert!(val >= 0.0 && val < 1.0, "rand({}) = {} out of range", seed, val);
        }
    }

    // ── Column generation ────────────────────────────────────────

    #[test]
    fn generate_columns_creates_correct_count() {
        let mut m = MatrixEffect::new();
        m.num_columns = 50;
        m.generate_columns();
        assert_eq!(m.columns.len(), 50);
        assert!(!m.needs_regeneration);
    }

    #[test]
    fn generated_columns_are_active() {
        let mut m = MatrixEffect::new();
        m.num_columns = 10;
        m.generate_columns();
        for col in &m.columns {
            assert!(col.active);
            assert_eq!(col.respawn_delay, 0.0);
        }
    }

    #[test]
    fn generated_columns_have_spread_x_positions() {
        let mut m = MatrixEffect::new();
        m.num_columns = 10;
        m.generate_columns();
        for (i, col) in m.columns.iter().enumerate() {
            let expected_x = i as f64 / 10.0;
            assert!((col.x - expected_x).abs() < 1e-10);
        }
    }

    #[test]
    fn generated_columns_have_staggered_starts() {
        let mut m = MatrixEffect::new();
        m.num_columns = 20;
        m.generate_columns();
        // All should start above or at the top (head_y <= 0)
        for col in &m.columns {
            assert!(col.head_y <= 0.0, "Column starts at {} (should be <= 0)", col.head_y);
        }
    }

    #[test]
    fn generated_columns_have_char_seeds() {
        let mut m = MatrixEffect::new();
        m.num_columns = 5;
        m.generate_columns();
        for col in &m.columns {
            assert_eq!(col.char_seeds.len(), 20); // approximate max trail characters
        }
    }

    #[test]
    fn generated_columns_have_varied_speeds() {
        let mut m = MatrixEffect::new();
        m.num_columns = 20;
        m.generate_columns();
        let speeds: Vec<f64> = m.columns.iter().map(|c| c.speed).collect();
        // All speeds should be in range [0.7, 1.3]
        for &s in &speeds {
            assert!(s >= 0.7 && s <= 1.3, "Speed {} out of expected range", s);
        }
        // Not all the same
        assert!(speeds.windows(2).any(|w| (w[0] - w[1]).abs() > 0.001));
    }

    // ── Update behavior ──────────────────────────────────────────

    #[test]
    fn update_noop_when_disabled() {
        let mut m = MatrixEffect::new();
        m.num_columns = 5;
        m.generate_columns();
        let positions_before: Vec<f64> = m.columns.iter().map(|c| c.head_y).collect();
        m.update(0.016, 1.0); // disabled, should not move
        let positions_after: Vec<f64> = m.columns.iter().map(|c| c.head_y).collect();
        assert_eq!(positions_before, positions_after);
    }

    #[test]
    fn update_moves_columns_downward() {
        let mut m = MatrixEffect::new();
        m.enabled = true;
        m.num_columns = 5;
        m.generate_columns();
        let positions_before: Vec<f64> = m.columns.iter().map(|c| c.head_y).collect();
        m.update(0.1, 1.0);
        for (i, col) in m.columns.iter().enumerate() {
            assert!(
                col.head_y > positions_before[i],
                "Column {} didn't move down",
                i
            );
        }
    }

    #[test]
    fn update_triggers_regeneration_when_needed() {
        let mut m = MatrixEffect::new();
        m.enabled = true;
        assert!(m.needs_regeneration);
        assert!(m.columns.is_empty());
        m.update(0.016, 0.016);
        // After update, columns should be generated
        assert!(!m.needs_regeneration);
        assert!(!m.columns.is_empty());
    }

    #[test]
    fn column_deactivates_when_off_screen() {
        let mut m = MatrixEffect::new();
        m.enabled = true;
        m.num_columns = 1;
        m.base_speed = 100.0; // Very fast — will fly off screen quickly
        m.generate_columns();
        m.columns[0].head_y = 0.9; // Near bottom
        m.columns[0].trail_length = 0.05; // Short trail
        // Update with large dt to push past 1.0 + trail_length
        m.update(1.0, 1.0);
        assert!(!m.columns[0].active);
        assert!(m.columns[0].respawn_delay > 0.0);
    }

    #[test]
    fn column_respawns_after_delay() {
        let mut m = MatrixEffect::new();
        m.enabled = true;
        m.num_columns = 1;
        m.generate_columns();
        // Manually deactivate with short delay
        m.columns[0].active = false;
        m.columns[0].respawn_delay = 0.01;
        m.update(0.1, 1.0); // dt > respawn_delay
        assert!(m.columns[0].active);
        assert!(m.columns[0].head_y <= 0.0); // Respawned at top
    }

    // ── Configure behavior ───────────────────────────────────────

    #[test]
    fn configure_enables_effect() {
        let mut m = MatrixEffect::new();
        let mut config = EffectConfig::new();
        config.insert("enabled", "true");
        m.configure(&config);
        assert!(m.is_enabled());
        assert!(m.needs_regeneration); // enabling triggers regeneration
    }

    #[test]
    fn configure_density_changes_column_count() {
        let mut m = MatrixEffect::new();
        let mut config = EffectConfig::new();
        config.insert("density", "2.0");
        m.configure(&config);
        assert_eq!(m.num_columns, 200); // 100 * 2.0
        assert!(m.needs_regeneration);
    }

    #[test]
    fn configure_density_clamps_to_range() {
        let mut m = MatrixEffect::new();
        // Too high
        let mut config = EffectConfig::new();
        config.insert("density", "10.0");
        m.configure(&config);
        assert_eq!(m.num_columns, 300); // 100 * 3.0 (clamped)

        // Too low
        let mut config = EffectConfig::new();
        config.insert("density", "0.01");
        m.configure(&config);
        assert_eq!(m.num_columns, 10); // 100 * 0.1 (clamped)
    }

    #[test]
    fn configure_speed() {
        let mut m = MatrixEffect::new();
        let mut config = EffectConfig::new();
        config.insert("speed", "12.5"); // 12.5 / 25.0 = 0.5
        m.configure(&config);
        assert!((m.base_speed - 0.5).abs() < 0.01);
    }

    #[test]
    fn configure_font_size() {
        let mut m = MatrixEffect::new();
        let mut config = EffectConfig::new();
        config.insert("font-size", "20.0");
        m.configure(&config);
        assert!((m.char_height - 20.0).abs() < 0.01);
        assert!((m.char_width - 14.0).abs() < 0.01); // 20 * 0.7
    }

    #[test]
    fn configure_charset() {
        let mut m = MatrixEffect::new();
        let mut config = EffectConfig::new();
        config.insert("charset", "ABC");
        m.configure(&config);
        assert_eq!(m.charset, vec!['A', 'B', 'C']);
    }

    #[test]
    fn configure_color_hex() {
        let mut m = MatrixEffect::new();
        let mut config = EffectConfig::new();
        config.insert("color", "#ff0000");
        m.configure(&config);
        let [r, g, b, a] = m.color.components;
        assert!((r - 1.0).abs() < 0.01);
        assert!(g.abs() < 0.01);
        assert!(b.abs() < 0.01);
        assert!((a - 1.0).abs() < 0.01);
    }

    // ── parse_color tests ────────────────────────────────────────

    #[test]
    fn parse_color_hex6() {
        let c = parse_color("#ff5500").unwrap();
        let [r, g, b, a] = c.components;
        assert!((r - 1.0).abs() < 0.01);
        assert!((g - 85.0 / 255.0).abs() < 0.01);
        assert!(b.abs() < 0.01);
        assert!((a - 1.0).abs() < 0.01);
    }

    #[test]
    fn parse_color_hex8() {
        let c = parse_color("#ff550080").unwrap();
        let [_, _, _, a] = c.components;
        assert!((a - 128.0 / 255.0).abs() < 0.02);
    }

    #[test]
    fn parse_color_rgb() {
        let c = parse_color("rgb(255, 0, 128)").unwrap();
        let [r, g, b, _] = c.components;
        assert!((r - 1.0).abs() < 0.01);
        assert!(g.abs() < 0.01);
        assert!((b - 128.0 / 255.0).abs() < 0.01);
    }

    #[test]
    fn parse_color_rgba() {
        let c = parse_color("rgba(0, 255, 0, 0.5)").unwrap();
        let [r, g, _, a] = c.components;
        assert!(r.abs() < 0.01);
        assert!((g - 1.0).abs() < 0.01);
        assert!((a - 127.0 / 255.0).abs() < 0.02);
    }

    #[test]
    fn parse_color_invalid() {
        assert!(parse_color("not-a-color").is_none());
        assert!(parse_color("#xyz").is_none());
        assert!(parse_color("").is_none());
    }

    // ── draw_char tests ──────────────────────────────────────────

    #[test]
    fn draw_char_without_charset_produces_path() {
        let center = Point::new(50.0, 50.0);
        for seed in 0..8 {
            let path = MatrixEffect::draw_char(center, 10.0, 14.0, seed, &[]);
            // Should produce non-empty path elements
            assert!(path.elements().len() > 0, "Seed {} produced empty path", seed);
        }
    }

    #[test]
    fn draw_char_with_charset_uses_char_shapes() {
        let center = Point::new(50.0, 50.0);
        let charset = vec!['A', 'B', 'C'];
        let path = MatrixEffect::draw_char(center, 10.0, 14.0, 0, &charset);
        assert!(path.elements().len() > 0);
    }

    #[test]
    fn draw_char_deterministic() {
        let center = Point::new(50.0, 50.0);
        let p1 = MatrixEffect::draw_char(center, 10.0, 14.0, 42, &[]);
        let p2 = MatrixEffect::draw_char(center, 10.0, 14.0, 42, &[]);
        assert_eq!(p1.elements().len(), p2.elements().len());
    }

    // ── Render behavior ──────────────────────────────────────────

    #[test]
    fn render_noop_when_disabled() {
        let m = MatrixEffect::new();
        let mut scene = Scene::new();
        let bounds = Rect::new(0.0, 0.0, 800.0, 600.0);
        m.render(&mut scene, bounds); // Should not panic
    }

    #[test]
    fn render_noop_when_no_columns() {
        let mut m = MatrixEffect::new();
        m.enabled = true;
        // columns is empty
        let mut scene = Scene::new();
        let bounds = Rect::new(0.0, 0.0, 800.0, 600.0);
        m.render(&mut scene, bounds); // Should not panic
    }

    #[test]
    fn render_with_active_columns() {
        let mut m = MatrixEffect::new();
        m.enabled = true;
        m.num_columns = 5;
        m.generate_columns();
        // Move columns into visible range
        for col in &mut m.columns {
            col.head_y = 0.5;
        }
        let mut scene = Scene::new();
        let bounds = Rect::new(0.0, 0.0, 800.0, 600.0);
        m.render(&mut scene, bounds); // Should render without panic
    }
}
