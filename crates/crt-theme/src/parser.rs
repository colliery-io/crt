//! CSS Theme Parser
//!
//! Parses CSS-like theme files into Theme structs.

use std::collections::HashMap;
use thiserror::Error;

use crate::{Color, Theme, TextShadow, LinearGradient, GridEffect};

#[derive(Error, Debug)]
pub enum ThemeParseError {
    #[error("CSS parse error: {0}")]
    CssError(String),

    #[error("Invalid color: {0}")]
    InvalidColor(String),

    #[error("Invalid gradient: {0}")]
    InvalidGradient(String),

    #[error("Missing required property: {0}")]
    MissingProperty(String),
}

/// Parse a hex color (#rgb, #rrggbb, #rrggbbaa)
pub fn parse_hex_color(hex: &str) -> Result<Color, ThemeParseError> {
    let hex = hex.trim_start_matches('#');

    match hex.len() {
        3 => {
            let r = u8::from_str_radix(&hex[0..1].repeat(2), 16)
                .map_err(|_| ThemeParseError::InvalidColor(hex.to_string()))?;
            let g = u8::from_str_radix(&hex[1..2].repeat(2), 16)
                .map_err(|_| ThemeParseError::InvalidColor(hex.to_string()))?;
            let b = u8::from_str_radix(&hex[2..3].repeat(2), 16)
                .map_err(|_| ThemeParseError::InvalidColor(hex.to_string()))?;
            Ok(Color::rgb(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0))
        }
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16)
                .map_err(|_| ThemeParseError::InvalidColor(hex.to_string()))?;
            let g = u8::from_str_radix(&hex[2..4], 16)
                .map_err(|_| ThemeParseError::InvalidColor(hex.to_string()))?;
            let b = u8::from_str_radix(&hex[4..6], 16)
                .map_err(|_| ThemeParseError::InvalidColor(hex.to_string()))?;
            Ok(Color::rgb(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0))
        }
        8 => {
            let r = u8::from_str_radix(&hex[0..2], 16)
                .map_err(|_| ThemeParseError::InvalidColor(hex.to_string()))?;
            let g = u8::from_str_radix(&hex[2..4], 16)
                .map_err(|_| ThemeParseError::InvalidColor(hex.to_string()))?;
            let b = u8::from_str_radix(&hex[4..6], 16)
                .map_err(|_| ThemeParseError::InvalidColor(hex.to_string()))?;
            let a = u8::from_str_radix(&hex[6..8], 16)
                .map_err(|_| ThemeParseError::InvalidColor(hex.to_string()))?;
            Ok(Color::rgba(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, a as f32 / 255.0))
        }
        _ => Err(ThemeParseError::InvalidColor(hex.to_string())),
    }
}

/// Parse rgb(r, g, b) or rgba(r, g, b, a)
pub fn parse_rgb_color(input: &str) -> Result<Color, ThemeParseError> {
    let input = input.trim();

    let (is_rgba, inner) = if input.starts_with("rgba(") && input.ends_with(')') {
        (true, &input[5..input.len()-1])
    } else if input.starts_with("rgb(") && input.ends_with(')') {
        (false, &input[4..input.len()-1])
    } else {
        return Err(ThemeParseError::InvalidColor(input.to_string()));
    };

    let parts: Vec<&str> = inner.split(',').map(|s| s.trim()).collect();

    if is_rgba && parts.len() != 4 {
        return Err(ThemeParseError::InvalidColor(input.to_string()));
    }
    if !is_rgba && parts.len() != 3 {
        return Err(ThemeParseError::InvalidColor(input.to_string()));
    }

    let r: f32 = parts[0].parse().map_err(|_| ThemeParseError::InvalidColor(input.to_string()))?;
    let g: f32 = parts[1].parse().map_err(|_| ThemeParseError::InvalidColor(input.to_string()))?;
    let b: f32 = parts[2].parse().map_err(|_| ThemeParseError::InvalidColor(input.to_string()))?;

    // Normalize 0-255 to 0-1
    let r = if r > 1.0 { r / 255.0 } else { r };
    let g = if g > 1.0 { g / 255.0 } else { g };
    let b = if b > 1.0 { b / 255.0 } else { b };

    if is_rgba {
        let a: f32 = parts[3].parse().map_err(|_| ThemeParseError::InvalidColor(input.to_string()))?;
        Ok(Color::rgba(r, g, b, a))
    } else {
        Ok(Color::rgb(r, g, b))
    }
}

/// Parse any CSS color value
pub fn parse_color(value: &str) -> Result<Color, ThemeParseError> {
    let value = value.trim();

    if value.starts_with('#') {
        parse_hex_color(value)
    } else if value.starts_with("rgb") {
        parse_rgb_color(value)
    } else {
        Err(ThemeParseError::InvalidColor(value.to_string()))
    }
}

/// Parse linear-gradient(to bottom, color1, color2)
pub fn parse_linear_gradient(value: &str) -> Result<LinearGradient, ThemeParseError> {
    let value = value.trim();

    if !value.starts_with("linear-gradient(") || !value.ends_with(')') {
        return Err(ThemeParseError::InvalidGradient(value.to_string()));
    }

    let inner = &value[16..value.len()-1];
    let parts: Vec<&str> = inner.splitn(3, ',').map(|s| s.trim()).collect();

    if parts.len() < 2 {
        return Err(ThemeParseError::InvalidGradient(value.to_string()));
    }

    let (top_str, bottom_str) = if parts[0].starts_with("to ") {
        if parts.len() < 3 {
            return Err(ThemeParseError::InvalidGradient(value.to_string()));
        }
        (parts[1], parts[2])
    } else {
        (parts[0], parts[1])
    };

    // Strip percentage suffixes
    let top_color = top_str.split_whitespace().next().unwrap_or(top_str);
    let bottom_color = bottom_str.split_whitespace().next().unwrap_or(bottom_str);

    let top = parse_color(top_color)?;
    let bottom = parse_color(bottom_color)?;

    Ok(LinearGradient { top, bottom })
}

/// Parse text-shadow: offset-x offset-y blur-radius color
pub fn parse_text_shadow(value: &str) -> Result<TextShadow, ThemeParseError> {
    let value = value.trim();

    // Handle rgba() colors which contain spaces
    let color_start = value.find("rgb").or_else(|| value.find('#'));

    let (radius, color) = if let Some(idx) = color_start {
        let prefix = &value[..idx].trim();
        let parts: Vec<&str> = prefix.split_whitespace().collect();
        let radius_str = parts.last().unwrap_or(&"8px").trim_end_matches("px");
        let radius: f32 = radius_str.parse().unwrap_or(8.0);

        let color_str = &value[idx..];
        let color = parse_color(color_str)?;
        (radius, color)
    } else {
        return Err(ThemeParseError::InvalidColor(format!("text-shadow: {}", value)));
    };

    Ok(TextShadow {
        color,
        radius,
        intensity: color.a,
    })
}

/// Simple CSS parser using regex-like string parsing
/// (cssparser is too complex for our subset)
pub fn parse_theme(css: &str) -> Result<Theme, ThemeParseError> {
    let mut theme = Theme::minimal();

    // Strip all CSS comments from the entire input first
    // This prevents comments from breaking selector matching
    let css = strip_comments(css);
    let css = css.trim();

    // Find all rule blocks
    let mut pos = 0;
    while pos < css.len() {
        // Find selector
        let Some(brace_start) = css[pos..].find('{') else { break };
        let selector = css[pos..pos + brace_start].trim();

        // Find matching close brace
        let Some(brace_end) = css[pos + brace_start..].find('}') else { break };
        let block = &css[pos + brace_start + 1..pos + brace_start + brace_end];

        // Parse properties
        let props = parse_properties(block);

        // Apply to theme based on selector
        apply_properties(&mut theme, selector, &props)?;

        pos = pos + brace_start + brace_end + 1;
    }

    Ok(theme)
}

fn parse_properties(block: &str) -> HashMap<String, String> {
    let mut props = HashMap::new();

    // Strip CSS comments first
    let block = strip_comments(block);

    for line in block.split(';') {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        if let Some(colon_pos) = line.find(':') {
            let name = line[..colon_pos].trim().to_string();
            let value = line[colon_pos + 1..].trim().to_string();
            props.insert(name, value);
        }
    }

    props
}

/// Strip CSS comments (/* ... */) from a string
fn strip_comments(input: &str) -> String {
    let mut result = String::new();
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '/' && chars.peek() == Some(&'*') {
            // Start of comment - skip until */
            chars.next(); // consume *
            loop {
                match chars.next() {
                    Some('*') if chars.peek() == Some(&'/') => {
                        chars.next(); // consume /
                        break;
                    }
                    Some(_) => continue,
                    None => break,
                }
            }
        } else {
            result.push(c);
        }
    }

    result
}

fn apply_properties(theme: &mut Theme, selector: &str, props: &HashMap<String, String>) -> Result<(), ThemeParseError> {
    let selector = selector.trim();

    match selector {
        ":terminal" | "terminal" => {
            // Typography
            if let Some(font) = props.get("font-family") {
                theme.typography.font_family = font.split(',')
                    .map(|s| s.trim().trim_matches('"').trim_matches('\'').to_string())
                    .collect();
            }
            if let Some(size) = props.get("font-size") {
                theme.typography.font_size = size.trim_end_matches("px").parse().unwrap_or(14.0);
            }
            if let Some(height) = props.get("line-height") {
                theme.typography.line_height = height.parse().unwrap_or(1.3);
            }

            // Colors
            if let Some(color) = props.get("color") {
                theme.foreground = parse_color(color)?;
            }
            if let Some(bg) = props.get("background") {
                if bg.starts_with("linear-gradient") {
                    theme.background = parse_linear_gradient(bg)?;
                } else {
                    let color = parse_color(bg)?;
                    theme.background = LinearGradient { top: color, bottom: color };
                }
            }

            // Text shadow / glow
            if let Some(shadow) = props.get("text-shadow") {
                theme.text_shadow = Some(parse_text_shadow(shadow)?);
            }

            // ANSI palette
            if let Some(c) = props.get("--color-black") { theme.palette.black = parse_color(c)?; }
            if let Some(c) = props.get("--color-red") { theme.palette.red = parse_color(c)?; }
            if let Some(c) = props.get("--color-green") { theme.palette.green = parse_color(c)?; }
            if let Some(c) = props.get("--color-yellow") { theme.palette.yellow = parse_color(c)?; }
            if let Some(c) = props.get("--color-blue") { theme.palette.blue = parse_color(c)?; }
            if let Some(c) = props.get("--color-magenta") { theme.palette.magenta = parse_color(c)?; }
            if let Some(c) = props.get("--color-cyan") { theme.palette.cyan = parse_color(c)?; }
            if let Some(c) = props.get("--color-white") { theme.palette.white = parse_color(c)?; }
            if let Some(c) = props.get("--color-bright-black") { theme.palette.bright_black = parse_color(c)?; }
            if let Some(c) = props.get("--color-bright-red") { theme.palette.bright_red = parse_color(c)?; }
            if let Some(c) = props.get("--color-bright-green") { theme.palette.bright_green = parse_color(c)?; }
            if let Some(c) = props.get("--color-bright-yellow") { theme.palette.bright_yellow = parse_color(c)?; }
            if let Some(c) = props.get("--color-bright-blue") { theme.palette.bright_blue = parse_color(c)?; }
            if let Some(c) = props.get("--color-bright-magenta") { theme.palette.bright_magenta = parse_color(c)?; }
            if let Some(c) = props.get("--color-bright-cyan") { theme.palette.bright_cyan = parse_color(c)?; }
            if let Some(c) = props.get("--color-bright-white") { theme.palette.bright_white = parse_color(c)?; }

            // Font variants
            if let Some(f) = props.get("--font-bold") {
                theme.typography.font_bold = Some(f.trim_matches('"').to_string());
            }
            if let Some(f) = props.get("--font-italic") {
                theme.typography.font_italic = Some(f.trim_matches('"').to_string());
            }
            if let Some(f) = props.get("--font-bold-italic") {
                theme.typography.font_bold_italic = Some(f.trim_matches('"').to_string());
            }
        }

        ":terminal::selection" | "terminal::selection" => {
            if let Some(bg) = props.get("background") {
                theme.selection.background = parse_color(bg)?;
            }
            if let Some(fg) = props.get("color") {
                theme.selection.foreground = parse_color(fg)?;
            }
        }

        ":terminal::highlight" | "terminal::highlight" => {
            if let Some(bg) = props.get("background") {
                theme.highlight.background = parse_color(bg)?;
            }
            if let Some(fg) = props.get("color") {
                theme.highlight.foreground = parse_color(fg)?;
            }
        }

        ":terminal::cursor" | "terminal::cursor" => {
            if let Some(bg) = props.get("background") {
                theme.cursor_color = parse_color(bg)?;
            }
        }

        ":terminal::backdrop" | "terminal::backdrop" => {
            let mut grid = theme.grid.unwrap_or(GridEffect {
                enabled: false,
                ..Default::default()
            });

            // Track if any grid property was set (for auto-enable)
            let mut has_grid_props = false;

            if let Some(c) = props.get("--grid-color") {
                grid.color = parse_color(c)?;
                has_grid_props = true;
            }
            if let Some(v) = props.get("--grid-spacing") {
                grid.spacing = v.parse().unwrap_or(8.0);
                has_grid_props = true;
            }
            if let Some(v) = props.get("--grid-line-width") {
                grid.line_width = v.parse().unwrap_or(0.02);
            }
            if let Some(v) = props.get("--grid-perspective") {
                grid.perspective = v.parse().unwrap_or(2.0);
            }
            if let Some(v) = props.get("--grid-horizon") {
                grid.horizon = v.parse().unwrap_or(0.35);
            }
            if let Some(v) = props.get("--grid-animation-speed") {
                grid.animation_speed = v.parse().unwrap_or(0.5);
            }

            // Parse --grid-enabled explicitly (this overrides auto-enable)
            if let Some(v) = props.get("--grid-enabled") {
                grid.enabled = v.trim() == "true";
            } else if has_grid_props {
                // Auto-enable if grid properties were set but no explicit enabled flag
                grid.enabled = true;
            }

            if grid.enabled {
                theme.grid = Some(grid);
            }
        }

        ":terminal::tab-bar" | "terminal::tab-bar" => {
            if let Some(bg) = props.get("background") {
                theme.tabs.bar.background = parse_color(bg)?;
            }
            if let Some(c) = props.get("border-color") {
                theme.tabs.bar.border_color = parse_color(c)?;
            }
            if let Some(v) = props.get("height") {
                theme.tabs.bar.height = v.trim_end_matches("px").parse().unwrap_or(36.0);
            }
            if let Some(v) = props.get("padding") {
                theme.tabs.bar.padding = v.trim_end_matches("px").parse().unwrap_or(4.0);
            }
        }

        ":terminal::tab" | "terminal::tab" | ":tab" | "tab" => {
            if let Some(bg) = props.get("background") {
                theme.tabs.tab.background = parse_color(bg)?;
            }
            if let Some(fg) = props.get("color") {
                theme.tabs.tab.foreground = parse_color(fg)?;
            }
            if let Some(v) = props.get("border-radius") {
                theme.tabs.tab.border_radius = v.trim_end_matches("px").parse().unwrap_or(4.0);
            }
            if let Some(v) = props.get("padding-x") {
                theme.tabs.tab.padding_x = v.trim_end_matches("px").parse().unwrap_or(12.0);
            }
            if let Some(v) = props.get("padding-y") {
                theme.tabs.tab.padding_y = v.trim_end_matches("px").parse().unwrap_or(6.0);
            }
            if let Some(v) = props.get("min-width") {
                theme.tabs.tab.min_width = v.trim_end_matches("px").parse().unwrap_or(80.0);
            }
            if let Some(v) = props.get("max-width") {
                theme.tabs.tab.max_width = v.trim_end_matches("px").parse().unwrap_or(200.0);
            }
            if let Some(shadow) = props.get("text-shadow") {
                theme.tabs.tab.text_shadow = Some(parse_text_shadow(shadow)?);
            }
        }

        ":terminal::tab-active" | "terminal::tab-active" | ":tab.active" | "tab.active" => {
            if let Some(bg) = props.get("background") {
                theme.tabs.active.background = parse_color(bg)?;
            }
            if let Some(fg) = props.get("color") {
                theme.tabs.active.foreground = parse_color(fg)?;
            }
            if let Some(c) = props.get("accent-color") {
                theme.tabs.active.accent = parse_color(c)?;
            }
            if let Some(shadow) = props.get("text-shadow") {
                theme.tabs.active.text_shadow = Some(parse_text_shadow(shadow)?);
            }
        }

        ":terminal::tab-close" | "terminal::tab-close" => {
            if let Some(bg) = props.get("background") {
                theme.tabs.close.background = parse_color(bg)?;
            }
            if let Some(fg) = props.get("color") {
                theme.tabs.close.foreground = parse_color(fg)?;
            }
            if let Some(bg) = props.get("--hover-background") {
                theme.tabs.close.hover_background = parse_color(bg)?;
            }
            if let Some(fg) = props.get("--hover-color") {
                theme.tabs.close.hover_foreground = parse_color(fg)?;
            }
            if let Some(v) = props.get("width") {
                theme.tabs.close.size = v.trim_end_matches("px").parse().unwrap_or(16.0);
            }
        }

        _ => {
            // Ignore unknown selectors
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hex_color() {
        let c = parse_hex_color("#ff5555").unwrap();
        assert!((c.r - 1.0).abs() < 0.01);
        assert!((c.g - 0.333).abs() < 0.01);

        let c = parse_hex_color("#fff").unwrap();
        assert!((c.r - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_parse_hex_color_teal() {
        // Test the specific teal color from synthwave.css
        let c = parse_hex_color("#61e2fe").unwrap();
        // #61 = 97, #e2 = 226, #fe = 254
        assert!((c.r - 97.0/255.0).abs() < 0.01, "red: expected {}, got {}", 97.0/255.0, c.r);
        assert!((c.g - 226.0/255.0).abs() < 0.01, "green: expected {}, got {}", 226.0/255.0, c.g);
        assert!((c.b - 254.0/255.0).abs() < 0.01, "blue: expected {}, got {}", 254.0/255.0, c.b);
    }

    #[test]
    fn test_parse_rgb_color() {
        let c = parse_rgb_color("rgb(255, 85, 85)").unwrap();
        assert!((c.r - 1.0).abs() < 0.01);

        let c = parse_rgb_color("rgba(0, 255, 255, 0.6)").unwrap();
        assert!((c.g - 1.0).abs() < 0.01);
        assert!((c.a - 0.6).abs() < 0.01);
    }

    #[test]
    fn test_parse_gradient() {
        let g = parse_linear_gradient("linear-gradient(to bottom, #1a0a2e, #16213e)").unwrap();
        assert!(g.top.r < 0.2);
    }

    #[test]
    fn test_parse_simple_theme() {
        let css = r#"
            :terminal {
                color: #c8c8c8;
                background: #1a1a1a;
            }
        "#;

        let theme = parse_theme(css).unwrap();
        assert!((theme.foreground.r - 0.784).abs() < 0.01);
    }

    #[test]
    fn test_parse_full_theme() {
        let css = r#"
            :terminal {
                font-family: "JetBrains Mono", monospace;
                font-size: 14px;
                color: #c8c8c8;
                background: linear-gradient(to bottom, #1a0a2e, #16213e);
                text-shadow: 0 0 8px rgba(0, 255, 255, 0.6);
                --color-red: #ff5555;
            }

            :terminal::selection {
                background: #44475a;
                color: #f8f8f2;
            }

            :terminal::backdrop {
                --grid-color: rgba(255, 0, 255, 0.15);
                --grid-spacing: 8;
            }

            :terminal::cursor {
                background: #00ffff;
            }
        "#;

        let theme = parse_theme(css).unwrap();
        assert_eq!(theme.typography.font_family[0], "JetBrains Mono");
        assert!(theme.text_shadow.is_some());
        assert!(theme.grid.is_some());
        assert!((theme.cursor_color.g - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_strip_comments_basic() {
        let input = "/* comment */ color: red";
        let result = strip_comments(input);
        assert_eq!(result.trim(), "color: red");
    }

    #[test]
    fn test_strip_comments_multiline() {
        let input = r#"
            /* Typography */
            font-family: monospace;
            /* Base colors */
            color: #fff;
        "#;
        let result = strip_comments(input);
        assert!(result.contains("font-family: monospace"));
        assert!(result.contains("color: #fff"));
        assert!(!result.contains("Typography"));
        assert!(!result.contains("Base colors"));
    }

    #[test]
    fn test_parse_theme_with_comments() {
        // This is the key test - CSS with inline comments
        let css = r#"
            :terminal {
                /* Typography */
                font-family: "JetBrains Mono", monospace;
                font-size: 14;

                /* Base colors - teal text */
                color: #61e2fe;
                background: linear-gradient(to bottom, #1a0a2e, #16213e);

                /* Text glow effect */
                text-shadow: 0 0 15px rgba(97, 226, 254, 0.9);
            }
        "#;

        let theme = parse_theme(css).unwrap();

        // Verify foreground color is teal (#61e2fe)
        assert!((theme.foreground.r - 97.0/255.0).abs() < 0.01,
            "foreground.r: expected {}, got {}", 97.0/255.0, theme.foreground.r);
        assert!((theme.foreground.g - 226.0/255.0).abs() < 0.01,
            "foreground.g: expected {}, got {}", 226.0/255.0, theme.foreground.g);
        assert!((theme.foreground.b - 254.0/255.0).abs() < 0.01,
            "foreground.b: expected {}, got {}", 254.0/255.0, theme.foreground.b);

        // Verify text_shadow is parsed
        assert!(theme.text_shadow.is_some(), "text_shadow should be Some");
        let shadow = theme.text_shadow.unwrap();
        assert!((shadow.radius - 15.0).abs() < 0.1, "shadow radius: expected 15, got {}", shadow.radius);
        assert!((shadow.intensity - 0.9).abs() < 0.01, "shadow intensity: expected 0.9, got {}", shadow.intensity);

        // Verify font-family
        assert_eq!(theme.typography.font_family[0], "JetBrains Mono");
    }

    #[test]
    fn test_parse_text_shadow_with_rgba() {
        let shadow = parse_text_shadow("0 0 15px rgba(97, 226, 254, 0.9)").unwrap();
        assert!((shadow.radius - 15.0).abs() < 0.1);
        assert!((shadow.color.r - 97.0/255.0).abs() < 0.01);
        assert!((shadow.color.g - 226.0/255.0).abs() < 0.01);
        assert!((shadow.color.b - 254.0/255.0).abs() < 0.01);
        assert!((shadow.color.a - 0.9).abs() < 0.01);
    }

    #[test]
    fn test_theme_to_uniforms_with_glow() {
        let css = r#"
            :terminal {
                color: #61e2fe;
                background: #1a1a1a;
                text-shadow: 0 0 15px rgba(97, 226, 254, 0.9);
            }
        "#;

        let theme = parse_theme(css).unwrap();
        let uniforms = theme.to_uniforms(800.0, 600.0, 0.0);

        // Verify glow values are passed to uniforms
        assert!((uniforms.glow_radius - 15.0).abs() < 0.1,
            "glow_radius: expected 15, got {}", uniforms.glow_radius);
        assert!((uniforms.glow_intensity - 0.9).abs() < 0.01,
            "glow_intensity: expected 0.9, got {}", uniforms.glow_intensity);

        // Verify glow color
        assert!((uniforms.glow_color[0] - 97.0/255.0).abs() < 0.01);
        assert!((uniforms.glow_color[1] - 226.0/255.0).abs() < 0.01);
        assert!((uniforms.glow_color[2] - 254.0/255.0).abs() < 0.01);

        // Verify text color
        assert!((uniforms.text_color[0] - 97.0/255.0).abs() < 0.01,
            "text_color[0]: expected {}, got {}", 97.0/255.0, uniforms.text_color[0]);
    }

    #[test]
    fn test_theme_to_uniforms_no_glow() {
        let css = r#"
            :terminal {
                color: #c8c8c8;
                background: #1a1a1a;
            }
        "#;

        let theme = parse_theme(css).unwrap();
        let uniforms = theme.to_uniforms(800.0, 600.0, 0.0);

        // Without text-shadow, glow should be disabled (radius/intensity = 0)
        assert!((uniforms.glow_radius - 0.0).abs() < 0.01,
            "glow_radius should be 0 without text-shadow, got {}", uniforms.glow_radius);
        assert!((uniforms.glow_intensity - 0.0).abs() < 0.01,
            "glow_intensity should be 0 without text-shadow, got {}", uniforms.glow_intensity);
    }

    #[test]
    fn test_parse_real_synthwave_css() {
        // Parse the actual synthwave.css file structure
        let css = r#"/* Synthwave Theme - The Extra AF Terminal Experience - DEBUG */

:terminal {
    /* Typography */
    font-family: "JetBrains Mono", "Fira Code", monospace;
    font-size: 14;
    line-height: 1.3;
    font-bold: "JetBrains Mono Bold";
    font-italic: "JetBrains Mono Italic";
    font-bold-italic: "JetBrains Mono Bold Italic";
    ligatures: true;

    /* Base colors - teal text like synthwave.rs example */
    color: #61e2fe;
    background: linear-gradient(to bottom, #1a0a2e, #16213e);
    cursor-color: #00ffff;

    /* Text glow effect - stronger teal glow */
    text-shadow: 0 0 15px rgba(97, 226, 254, 0.9);
}

:terminal::selection {
    background: #44475a;
    color: #f8f8f2;
}

:terminal::highlight {
    background: #e6db74;
    color: #1a1a1a;
}

:terminal::backdrop {
    /* Perspective grid */
    --grid-enabled: true;
    --grid-color: rgba(255, 0, 255, 0.3);
    --grid-spacing: 8;
    --grid-line-width: 0.02;
    --grid-perspective: 2;
    --grid-horizon: 0.35;
    --grid-animation-speed: 0.5;
}

/* ANSI 16-color palette */
:terminal::palette {
    /* Normal colors */
    --color-0: #0d0d0d;   /* black */
    --color-1: #ff5555;   /* red */
    --color-2: #50fa7b;   /* green */
    --color-3: #f1fa8c;   /* yellow */
    --color-4: #bd93f9;   /* blue */
    --color-5: #ff79c6;   /* magenta */
    --color-6: #8be9fd;   /* cyan */
    --color-7: #f8f8f2;   /* white */

    /* Bright colors */
    --color-8: #4d4d4d;   /* bright black */
    --color-9: #ff6e6e;   /* bright red */
    --color-10: #69ff94;  /* bright green */
    --color-11: #ffffa5;  /* bright yellow */
    --color-12: #d6acff;  /* bright blue */
    --color-13: #ff92df;  /* bright magenta */
    --color-14: #a4ffff;  /* bright cyan */
    --color-15: #ffffff;  /* bright white */
}
"#;

        let theme = parse_theme(css).unwrap();

        // Verify foreground color is teal (#61e2fe)
        println!("Foreground: r={}, g={}, b={}", theme.foreground.r, theme.foreground.g, theme.foreground.b);
        assert!((theme.foreground.r - 97.0/255.0).abs() < 0.01,
            "foreground.r: expected {}, got {}", 97.0/255.0, theme.foreground.r);
        assert!((theme.foreground.g - 226.0/255.0).abs() < 0.01,
            "foreground.g: expected {}, got {}", 226.0/255.0, theme.foreground.g);

        // Verify text_shadow is parsed
        println!("text_shadow: {:?}", theme.text_shadow);
        assert!(theme.text_shadow.is_some(), "text_shadow should be Some, but it's None");
        let shadow = theme.text_shadow.unwrap();
        assert!((shadow.radius - 15.0).abs() < 0.1, "shadow radius: expected 15, got {}", shadow.radius);

        // Verify grid is parsed
        println!("grid: {:?}", theme.grid);
        assert!(theme.grid.is_some(), "grid should be Some");

        // Verify uniforms are correct
        let uniforms = theme.to_uniforms(800.0, 600.0, 0.0);
        println!("Uniforms - glow_radius: {}, glow_intensity: {}", uniforms.glow_radius, uniforms.glow_intensity);
        println!("Uniforms - text_color: {:?}", uniforms.text_color);
        assert!(uniforms.glow_radius > 0.0, "glow_radius should be > 0");
        assert!(uniforms.glow_intensity > 0.0, "glow_intensity should be > 0");
    }

    #[test]
    fn test_parse_tab_styling() {
        let css = r#"
            :terminal {
                color: #c8c8c8;
                background: #1a1a1a;
            }

            :terminal::tab-bar {
                background: #1a1a2e;
                border-color: #2a2a3e;
                height: 40px;
                padding: 8px;
            }

            :terminal::tab {
                background: #2a2a3e;
                color: #888899;
                border-radius: 6px;
                min-width: 100px;
                max-width: 250px;
            }

            :terminal::tab-active {
                background: #3a3a4e;
                color: #f8f8f2;
                accent-color: #ff00ff;
            }

            :terminal::tab-close {
                color: #666677;
                --hover-background: #ff5555;
                --hover-color: #ffffff;
                width: 20px;
            }
        "#;

        let theme = parse_theme(css).unwrap();

        // Tab bar
        assert!((theme.tabs.bar.height - 40.0).abs() < 0.1);
        assert!((theme.tabs.bar.padding - 8.0).abs() < 0.1);

        // Tab
        assert!((theme.tabs.tab.border_radius - 6.0).abs() < 0.1);
        assert!((theme.tabs.tab.min_width - 100.0).abs() < 0.1);
        assert!((theme.tabs.tab.max_width - 250.0).abs() < 0.1);

        // Active tab - verify accent color is magenta (#ff00ff)
        assert!((theme.tabs.active.accent.r - 1.0).abs() < 0.01);
        assert!((theme.tabs.active.accent.g - 0.0).abs() < 0.01);
        assert!((theme.tabs.active.accent.b - 1.0).abs() < 0.01);

        // Close button
        assert!((theme.tabs.close.size - 20.0).abs() < 0.1);
        assert!((theme.tabs.close.hover_background.r - 1.0).abs() < 0.01); // #ff5555 red
    }
}
