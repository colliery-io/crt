//! Effect configuration from theme settings.
//!
//! Translates theme CSS properties into effect renderer configuration.

use crt_renderer::{EffectConfig, EffectsRenderer};
use crt_theme::Theme;

/// Configure backdrop effects from theme settings
pub(crate) fn configure_effects_from_theme(effects_renderer: &mut EffectsRenderer, theme: &Theme) {
    let mut config = EffectConfig::new();

    // First, explicitly disable ALL effects to ensure clean state when switching themes.
    // This prevents effects from the previous theme persisting when the new theme
    // doesn't define them.
    config.insert("grid-enabled", "false");
    config.insert("starfield-enabled", "false");
    config.insert("rain-enabled", "false");
    config.insert("particles-enabled", "false");
    config.insert("matrix-enabled", "false");
    config.insert("shape-enabled", "false");
    config.insert("sprite-enabled", "false");

    // Now configure effects that are defined in the new theme (will override the disables above)

    // Grid effect configuration from theme
    if let Some(ref grid) = theme.grid {
        config.insert("grid-enabled", if grid.enabled { "true" } else { "false" });
        // Convert Color to rgba() string
        let c = grid.color;
        config.insert(
            "grid-color",
            format!(
                "rgba({}, {}, {}, {})",
                (c.r * 255.0) as u8,
                (c.g * 255.0) as u8,
                (c.b * 255.0) as u8,
                c.a
            ),
        );
        config.insert("grid-spacing", grid.spacing.to_string());
        config.insert("grid-line-width", grid.line_width.to_string());
        config.insert("grid-perspective", grid.perspective.to_string());
        config.insert("grid-horizon", grid.horizon.to_string());
        config.insert("grid-animation-speed", grid.animation_speed.to_string());
        config.insert("grid-glow-radius", grid.glow_radius.to_string());
        config.insert("grid-glow-intensity", grid.glow_intensity.to_string());
        config.insert("grid-vanishing-spread", grid.vanishing_spread.to_string());
        config.insert("grid-curved", if grid.curved { "true" } else { "false" });
    }

    // Starfield effect configuration from theme
    if let Some(ref starfield) = theme.starfield {
        config.insert(
            "starfield-enabled",
            if starfield.enabled { "true" } else { "false" },
        );
        // Convert Color to rgba() string
        let c = starfield.color;
        config.insert(
            "starfield-color",
            format!(
                "rgba({}, {}, {}, {})",
                (c.r * 255.0) as u8,
                (c.g * 255.0) as u8,
                (c.b * 255.0) as u8,
                c.a
            ),
        );
        config.insert("starfield-density", starfield.density.to_string());
        config.insert("starfield-layers", starfield.layers.to_string());
        config.insert("starfield-speed", starfield.speed.to_string());
        config.insert(
            "starfield-direction",
            starfield.direction.as_str().to_string(),
        );
        config.insert("starfield-glow-radius", starfield.glow_radius.to_string());
        config.insert(
            "starfield-glow-intensity",
            starfield.glow_intensity.to_string(),
        );
        config.insert(
            "starfield-twinkle",
            if starfield.twinkle { "true" } else { "false" },
        );
        config.insert(
            "starfield-twinkle-speed",
            starfield.twinkle_speed.to_string(),
        );
        config.insert("starfield-min-size", starfield.min_size.to_string());
        config.insert("starfield-max-size", starfield.max_size.to_string());
    }

    // Rain effect configuration from theme
    if let Some(ref rain) = theme.rain {
        config.insert("rain-enabled", if rain.enabled { "true" } else { "false" });
        let c = rain.color;
        config.insert(
            "rain-color",
            format!(
                "rgba({}, {}, {}, {})",
                (c.r * 255.0) as u8,
                (c.g * 255.0) as u8,
                (c.b * 255.0) as u8,
                c.a
            ),
        );
        config.insert("rain-density", rain.density.to_string());
        config.insert("rain-speed", rain.speed.to_string());
        config.insert("rain-angle", rain.angle.to_string());
        config.insert("rain-length", rain.length.to_string());
        config.insert("rain-thickness", rain.thickness.to_string());
        config.insert("rain-glow-radius", rain.glow_radius.to_string());
        config.insert("rain-glow-intensity", rain.glow_intensity.to_string());
    }

    // Particle effect configuration from theme
    if let Some(ref particles) = theme.particles {
        config.insert(
            "particles-enabled",
            if particles.enabled { "true" } else { "false" },
        );
        let c = particles.color;
        config.insert(
            "particles-color",
            format!(
                "rgba({}, {}, {}, {})",
                (c.r * 255.0) as u8,
                (c.g * 255.0) as u8,
                (c.b * 255.0) as u8,
                c.a
            ),
        );
        config.insert("particles-count", particles.count.to_string());
        config.insert("particles-shape", particles.shape.as_str().to_string());
        config.insert(
            "particles-behavior",
            particles.behavior.as_str().to_string(),
        );
        config.insert("particles-size", particles.size.to_string());
        config.insert("particles-speed", particles.speed.to_string());
        config.insert("particles-glow-radius", particles.glow_radius.to_string());
        config.insert(
            "particles-glow-intensity",
            particles.glow_intensity.to_string(),
        );
    }

    // Matrix effect configuration from theme
    if let Some(ref matrix) = theme.matrix {
        config.insert(
            "matrix-enabled",
            if matrix.enabled { "true" } else { "false" },
        );
        let c = matrix.color;
        config.insert(
            "matrix-color",
            format!(
                "rgba({}, {}, {}, {})",
                (c.r * 255.0) as u8,
                (c.g * 255.0) as u8,
                (c.b * 255.0) as u8,
                c.a
            ),
        );
        config.insert("matrix-density", matrix.density.to_string());
        config.insert("matrix-speed", matrix.speed.to_string());
        config.insert("matrix-font-size", matrix.font_size.to_string());
        config.insert("matrix-charset", matrix.charset.clone());
    }

    // Shape effect configuration from theme
    if let Some(ref shape) = theme.shape {
        config.insert(
            "shape-enabled",
            if shape.enabled { "true" } else { "false" },
        );
        config.insert("shape-type", shape.shape_type.as_str().to_string());
        config.insert("shape-size", shape.size.to_string());
        if let Some(ref fill) = shape.fill {
            config.insert(
                "shape-fill",
                format!(
                    "rgba({}, {}, {}, {})",
                    (fill.r * 255.0) as u8,
                    (fill.g * 255.0) as u8,
                    (fill.b * 255.0) as u8,
                    fill.a
                ),
            );
        } else {
            config.insert("shape-fill", "none".to_string());
        }
        if let Some(ref stroke) = shape.stroke {
            config.insert(
                "shape-stroke",
                format!(
                    "rgba({}, {}, {}, {})",
                    (stroke.r * 255.0) as u8,
                    (stroke.g * 255.0) as u8,
                    (stroke.b * 255.0) as u8,
                    stroke.a
                ),
            );
        } else {
            config.insert("shape-stroke", "none".to_string());
        }
        config.insert("shape-stroke-width", shape.stroke_width.to_string());
        config.insert("shape-glow-radius", shape.glow_radius.to_string());
        if let Some(ref glow_color) = shape.glow_color {
            config.insert(
                "shape-glow-color",
                format!(
                    "rgba({}, {}, {}, {})",
                    (glow_color.r * 255.0) as u8,
                    (glow_color.g * 255.0) as u8,
                    (glow_color.b * 255.0) as u8,
                    glow_color.a
                ),
            );
        }
        config.insert("shape-rotation", shape.rotation.as_str().to_string());
        config.insert("shape-rotation-speed", shape.rotation_speed.to_string());
        config.insert("shape-motion", shape.motion.as_str().to_string());
        config.insert("shape-motion-speed", shape.motion_speed.to_string());
        config.insert("shape-polygon-sides", shape.polygon_sides.to_string());
    }

    // Note: Sprite effect is disabled at the top of this function.
    // Sprite rendering uses raw wgpu SpriteRenderer (in render.rs) to avoid vello memory issues.

    effects_renderer.configure(&config);
}
