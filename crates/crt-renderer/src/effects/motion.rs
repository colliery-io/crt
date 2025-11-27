//! Motion behaviors for backdrop effects
//!
//! Provides reusable motion patterns that can be composed with
//! positional effects like ShapeEffect and SpriteEffect.
//!
//! ## Available Behaviors
//!
//! - `None` - Static position, no movement
//! - `Bounce` - DVD-logo style, bounces off edges
//! - `Scroll` - Moves in direction, wraps around
//! - `Float` - Gentle random drift
//! - `Orbit` - Circular path around center

use vello::kurbo::{Rect, Vec2};

/// Motion behavior for positional effects
#[derive(Debug, Clone)]
pub enum MotionBehavior {
    /// No movement, static position
    None,

    /// Bounce off screen edges (DVD-logo style)
    Bounce {
        /// Current velocity in pixels per second
        velocity: Vec2,
    },

    /// Scroll in a direction, wrap around edges
    Scroll {
        /// Direction and speed in pixels per second
        direction: Vec2,
    },

    /// Gentle random floating/drifting
    Float {
        /// Random seed for deterministic noise
        seed: u32,
        /// Amplitude of drift in pixels
        amplitude: f64,
        /// Speed multiplier
        speed: f64,
    },

    /// Circular orbit around a center point
    Orbit {
        /// Center point (relative to bounds, 0-1)
        center: Vec2,
        /// Radius in pixels
        radius: f64,
        /// Angular speed in radians per second
        speed: f64,
        /// Current angle in radians
        angle: f64,
    },
}

impl Default for MotionBehavior {
    fn default() -> Self {
        Self::None
    }
}

impl MotionBehavior {
    /// Create a bounce behavior with given velocity
    pub fn bounce(vx: f64, vy: f64) -> Self {
        Self::Bounce {
            velocity: Vec2::new(vx, vy),
        }
    }

    /// Create a bounce behavior from speed and angle
    pub fn bounce_angled(speed: f64, angle_degrees: f64) -> Self {
        let angle_rad = angle_degrees.to_radians();
        Self::Bounce {
            velocity: Vec2::new(angle_rad.cos() * speed, angle_rad.sin() * speed),
        }
    }

    /// Create a scroll behavior with given direction
    pub fn scroll(dx: f64, dy: f64) -> Self {
        Self::Scroll {
            direction: Vec2::new(dx, dy),
        }
    }

    /// Create a scroll behavior from speed and angle
    pub fn scroll_angled(speed: f64, angle_degrees: f64) -> Self {
        let angle_rad = angle_degrees.to_radians();
        Self::Scroll {
            direction: Vec2::new(angle_rad.cos() * speed, angle_rad.sin() * speed),
        }
    }

    /// Create a float behavior
    pub fn float(seed: u32, amplitude: f64, speed: f64) -> Self {
        Self::Float {
            seed,
            amplitude,
            speed,
        }
    }

    /// Create an orbit behavior
    pub fn orbit(center_x: f64, center_y: f64, radius: f64, speed: f64) -> Self {
        Self::Orbit {
            center: Vec2::new(center_x, center_y),
            radius,
            speed,
            angle: 0.0,
        }
    }

    /// Update position based on motion behavior
    ///
    /// # Arguments
    /// * `position` - Current position to update
    /// * `size` - Size of the moving object (for collision)
    /// * `bounds` - Screen/container bounds
    /// * `dt` - Delta time in seconds
    /// * `time` - Total elapsed time in seconds (for float behavior)
    pub fn update(
        &mut self,
        position: &mut Vec2,
        size: Vec2,
        bounds: Rect,
        dt: f32,
        time: f32,
    ) {
        match self {
            Self::None => {}

            Self::Bounce { velocity } => {
                // Update position
                position.x += velocity.x * dt as f64;
                position.y += velocity.y * dt as f64;

                // Check left/right bounds
                if position.x <= bounds.x0 {
                    position.x = bounds.x0;
                    velocity.x = velocity.x.abs(); // Bounce right
                } else if position.x + size.x >= bounds.x1 {
                    position.x = bounds.x1 - size.x;
                    velocity.x = -velocity.x.abs(); // Bounce left
                }

                // Check top/bottom bounds
                if position.y <= bounds.y0 {
                    position.y = bounds.y0;
                    velocity.y = velocity.y.abs(); // Bounce down
                } else if position.y + size.y >= bounds.y1 {
                    position.y = bounds.y1 - size.y;
                    velocity.y = -velocity.y.abs(); // Bounce up
                }
            }

            Self::Scroll { direction } => {
                // Update position
                position.x += direction.x * dt as f64;
                position.y += direction.y * dt as f64;

                let width = bounds.x1 - bounds.x0;
                let height = bounds.y1 - bounds.y0;

                // Wrap around horizontally
                if position.x > bounds.x1 {
                    position.x = bounds.x0 - size.x;
                } else if position.x + size.x < bounds.x0 {
                    position.x = bounds.x1;
                }

                // Wrap around vertically
                if position.y > bounds.y1 {
                    position.y = bounds.y0 - size.y;
                } else if position.y + size.y < bounds.y0 {
                    position.y = bounds.y1;
                }

                // Keep within bounds range for wrapping
                let _ = (width, height); // suppress unused warning
            }

            Self::Float {
                seed,
                amplitude,
                speed,
            } => {
                // Use simple smooth noise based on time
                let t = time as f64 * *speed;
                let s = *seed as f64;

                // Two independent sine waves with different frequencies for organic motion
                let noise_x = (t * 0.7 + s * 0.1).sin() * 0.6
                    + (t * 1.3 + s * 0.3).sin() * 0.4;
                let noise_y = (t * 0.9 + s * 0.2).sin() * 0.6
                    + (t * 1.1 + s * 0.4).sin() * 0.4;

                // Calculate center of bounds
                let center_x = (bounds.x0 + bounds.x1) / 2.0;
                let center_y = (bounds.y0 + bounds.y1) / 2.0;

                // Position drifts around center
                position.x = center_x + noise_x * *amplitude - size.x / 2.0;
                position.y = center_y + noise_y * *amplitude - size.y / 2.0;
            }

            Self::Orbit {
                center,
                radius,
                speed,
                angle,
            } => {
                // Update angle
                *angle += *speed * dt as f64;

                // Keep angle in reasonable range
                if *angle > std::f64::consts::TAU {
                    *angle -= std::f64::consts::TAU;
                } else if *angle < 0.0 {
                    *angle += std::f64::consts::TAU;
                }

                // Calculate center in absolute coordinates
                let bounds_width = bounds.x1 - bounds.x0;
                let bounds_height = bounds.y1 - bounds.y0;
                let abs_center_x = bounds.x0 + center.x * bounds_width;
                let abs_center_y = bounds.y0 + center.y * bounds_height;

                // Calculate position on orbit
                position.x = abs_center_x + angle.cos() * *radius - size.x / 2.0;
                position.y = abs_center_y + angle.sin() * *radius - size.y / 2.0;
            }
        }
    }

    /// Check if this behavior requires animation updates
    pub fn is_animated(&self) -> bool {
        !matches!(self, Self::None)
    }

    /// Get the behavior type name
    pub fn behavior_type(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Bounce { .. } => "bounce",
            Self::Scroll { .. } => "scroll",
            Self::Float { .. } => "float",
            Self::Orbit { .. } => "orbit",
        }
    }

    /// Parse motion behavior from string
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "bounce" => Self::bounce(100.0, 75.0), // Default diagonal bounce
            "scroll" => Self::scroll(50.0, 0.0),   // Default scroll right
            "float" => Self::float(42, 50.0, 0.5), // Default gentle float
            "orbit" => Self::orbit(0.5, 0.5, 100.0, 1.0), // Default center orbit
            _ => Self::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_bounds() -> Rect {
        Rect::new(0.0, 0.0, 800.0, 600.0)
    }

    fn test_size() -> Vec2 {
        Vec2::new(50.0, 50.0)
    }

    #[test]
    fn test_bounce_reflects_at_right_edge() {
        let mut behavior = MotionBehavior::bounce(100.0, 0.0);
        let mut pos = Vec2::new(740.0, 300.0);
        let size = test_size();
        let bounds = test_bounds();

        // Move right until hitting edge
        behavior.update(&mut pos, size, bounds, 0.2, 0.0);

        // Should have bounced and velocity reversed
        if let MotionBehavior::Bounce { velocity } = behavior {
            assert!(velocity.x < 0.0, "Velocity should be negative after right bounce");
        }
    }

    #[test]
    fn test_bounce_reflects_at_left_edge() {
        let mut behavior = MotionBehavior::bounce(-100.0, 0.0);
        let mut pos = Vec2::new(10.0, 300.0);
        let size = test_size();
        let bounds = test_bounds();

        behavior.update(&mut pos, size, bounds, 0.2, 0.0);

        if let MotionBehavior::Bounce { velocity } = behavior {
            assert!(velocity.x > 0.0, "Velocity should be positive after left bounce");
        }
    }

    #[test]
    fn test_bounce_reflects_at_bottom_edge() {
        let mut behavior = MotionBehavior::bounce(0.0, 100.0);
        let mut pos = Vec2::new(400.0, 540.0);
        let size = test_size();
        let bounds = test_bounds();

        behavior.update(&mut pos, size, bounds, 0.2, 0.0);

        if let MotionBehavior::Bounce { velocity } = behavior {
            assert!(velocity.y < 0.0, "Velocity should be negative after bottom bounce");
        }
    }

    #[test]
    fn test_scroll_wraps_horizontally() {
        let mut behavior = MotionBehavior::scroll(1000.0, 0.0);
        let mut pos = Vec2::new(750.0, 300.0);
        let size = test_size();
        let bounds = test_bounds();

        // Scroll past right edge
        behavior.update(&mut pos, size, bounds, 0.1, 0.0);

        assert!(pos.x < 100.0, "Position should wrap to left side");
    }

    #[test]
    fn test_float_stays_bounded() {
        let mut behavior = MotionBehavior::float(42, 100.0, 1.0);
        let mut pos = Vec2::new(400.0, 300.0);
        let size = test_size();
        let bounds = test_bounds();

        // Run several updates
        for i in 0..100 {
            behavior.update(&mut pos, size, bounds, 0.1, i as f32 * 0.1);
        }

        // Position should stay roughly in the middle area
        assert!(pos.x >= -100.0 && pos.x <= 900.0, "X should be reasonable: {}", pos.x);
        assert!(pos.y >= -100.0 && pos.y <= 700.0, "Y should be reasonable: {}", pos.y);
    }

    #[test]
    fn test_orbit_maintains_radius() {
        let mut behavior = MotionBehavior::orbit(0.5, 0.5, 100.0, 1.0);
        let mut pos = Vec2::new(0.0, 0.0);
        let size = Vec2::new(0.0, 0.0); // Point size for easy distance calc
        let bounds = test_bounds();

        // Update to different angles
        behavior.update(&mut pos, size, bounds, 0.0, 0.0);
        let center = Vec2::new(400.0, 300.0);
        let dist = ((pos.x - center.x).powi(2) + (pos.y - center.y).powi(2)).sqrt();

        assert!(
            (dist - 100.0).abs() < 1.0,
            "Distance from center should be ~100: {}",
            dist
        );
    }

    #[test]
    fn test_none_does_not_move() {
        let mut behavior = MotionBehavior::None;
        let mut pos = Vec2::new(100.0, 200.0);
        let original = pos;

        behavior.update(&mut pos, test_size(), test_bounds(), 1.0, 1.0);

        assert_eq!(pos.x, original.x);
        assert_eq!(pos.y, original.y);
    }

    #[test]
    fn test_from_str() {
        assert!(matches!(MotionBehavior::from_str("bounce"), MotionBehavior::Bounce { .. }));
        assert!(matches!(MotionBehavior::from_str("scroll"), MotionBehavior::Scroll { .. }));
        assert!(matches!(MotionBehavior::from_str("float"), MotionBehavior::Float { .. }));
        assert!(matches!(MotionBehavior::from_str("orbit"), MotionBehavior::Orbit { .. }));
        assert!(matches!(MotionBehavior::from_str("none"), MotionBehavior::None));
        assert!(matches!(MotionBehavior::from_str("invalid"), MotionBehavior::None));
    }
}
