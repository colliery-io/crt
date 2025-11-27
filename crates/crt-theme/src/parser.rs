//! CSS Theme Parser using lightningcss
//!
//! Parses CSS theme files into Theme structs using lightningcss for proper
//! CSS parsing including support for calc(), color functions, and @keyframes.

use std::collections::HashMap;
use thiserror::Error;

use lightningcss::stylesheet::{ParserOptions, StyleSheet};
use lightningcss::rules::CssRule;
use lightningcss::properties::Property;
use lightningcss::values::color::CssColor;
use lightningcss::traits::ToCss;
use lightningcss::printer::PrinterOptions;

use crate::{
    BackgroundImage, BackgroundPosition, BackgroundRepeat, BackgroundSize,
    Color, GridEffect, LinearGradient, MatrixEffect, ParticleBehavior, ParticleEffect,
    ParticleShape, RainEffect, ShapeEffect, ShapeMotion, ShapeRotation, ShapeType,
    StarDirection, StarfieldEffect, TextShadow, Theme,
};

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

/// Helper to get PrinterOptions (since it doesn't implement Copy)
fn opts() -> PrinterOptions<'static> {
    PrinterOptions::default()
}

/// Convert lightningcss CssColor to our Color type
#[allow(dead_code)]
fn css_color_to_color(css_color: &CssColor) -> Result<Color, ThemeParseError> {
    match css_color {
        CssColor::RGBA(rgba) => Ok(Color::rgba(
            rgba.red as f32 / 255.0,
            rgba.green as f32 / 255.0,
            rgba.blue as f32 / 255.0,
            rgba.alpha as f32 / 255.0,
        )),
        CssColor::CurrentColor => Ok(Color::rgb(1.0, 1.0, 1.0)), // Default to white
        _ => {
            // For other color types, convert to string and parse
            let css_str = css_color.to_css_string(opts())
                .map_err(|e| ThemeParseError::InvalidColor(format!("{:?}", e)))?;
            parse_color_string(&css_str)
        }
    }
}

/// Parse a color from a CSS string value
fn parse_color_string(value: &str) -> Result<Color, ThemeParseError> {
    let value = value.trim();

    if value.starts_with('#') {
        parse_hex_color(value)
    } else if value.starts_with("rgb") {
        parse_rgb_color(value)
    } else {
        // Try named colors
        parse_named_color(value).ok_or_else(|| ThemeParseError::InvalidColor(value.to_string()))
    }
}

/// Parse CSS named colors
fn parse_named_color(name: &str) -> Option<Color> {
    let (r, g, b) = match name.to_lowercase().as_str() {
        // Basic colors
        "black" => (0, 0, 0),
        "white" => (255, 255, 255),
        "red" => (255, 0, 0),
        "green" => (0, 128, 0),
        "blue" => (0, 0, 255),
        "yellow" => (255, 255, 0),
        "cyan" | "aqua" => (0, 255, 255),
        "magenta" | "fuchsia" => (255, 0, 255),
        // Extended colors
        "gold" => (255, 215, 0),
        "orange" => (255, 165, 0),
        "pink" => (255, 192, 203),
        "purple" => (128, 0, 128),
        "gray" | "grey" => (128, 128, 128),
        "silver" => (192, 192, 192),
        "navy" => (0, 0, 128),
        "teal" => (0, 128, 128),
        "olive" => (128, 128, 0),
        "maroon" => (128, 0, 0),
        "lime" => (0, 255, 0),
        "coral" => (255, 127, 80),
        "hotpink" => (255, 105, 180),
        "deeppink" => (255, 20, 147),
        "crimson" => (220, 20, 60),
        "tomato" => (255, 99, 71),
        "orangered" => (255, 69, 0),
        "indianred" => (205, 92, 92),
        "transparent" => return Some(Color::rgba(0.0, 0.0, 0.0, 0.0)),
        _ => return None,
    };
    Some(Color::rgb(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0))
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
            Ok(Color::rgb(
                r as f32 / 255.0,
                g as f32 / 255.0,
                b as f32 / 255.0,
            ))
        }
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16)
                .map_err(|_| ThemeParseError::InvalidColor(hex.to_string()))?;
            let g = u8::from_str_radix(&hex[2..4], 16)
                .map_err(|_| ThemeParseError::InvalidColor(hex.to_string()))?;
            let b = u8::from_str_radix(&hex[4..6], 16)
                .map_err(|_| ThemeParseError::InvalidColor(hex.to_string()))?;
            Ok(Color::rgb(
                r as f32 / 255.0,
                g as f32 / 255.0,
                b as f32 / 255.0,
            ))
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
            Ok(Color::rgba(
                r as f32 / 255.0,
                g as f32 / 255.0,
                b as f32 / 255.0,
                a as f32 / 255.0,
            ))
        }
        _ => Err(ThemeParseError::InvalidColor(hex.to_string())),
    }
}

/// Parse rgb(r, g, b) or rgba(r, g, b, a)
pub fn parse_rgb_color(input: &str) -> Result<Color, ThemeParseError> {
    let input = input.trim();

    let (is_rgba, inner) = if input.starts_with("rgba(") && input.ends_with(')') {
        (true, &input[5..input.len() - 1])
    } else if input.starts_with("rgb(") && input.ends_with(')') {
        (false, &input[4..input.len() - 1])
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

    let r: f32 = parts[0]
        .parse()
        .map_err(|_| ThemeParseError::InvalidColor(input.to_string()))?;
    let g: f32 = parts[1]
        .parse()
        .map_err(|_| ThemeParseError::InvalidColor(input.to_string()))?;
    let b: f32 = parts[2]
        .parse()
        .map_err(|_| ThemeParseError::InvalidColor(input.to_string()))?;

    // Normalize 0-255 to 0-1
    let r = if r > 1.0 { r / 255.0 } else { r };
    let g = if g > 1.0 { g / 255.0 } else { g };
    let b = if b > 1.0 { b / 255.0 } else { b };

    if is_rgba {
        let a: f32 = parts[3]
            .parse()
            .map_err(|_| ThemeParseError::InvalidColor(input.to_string()))?;
        Ok(Color::rgba(r, g, b, a))
    } else {
        Ok(Color::rgb(r, g, b))
    }
}

/// Parse any CSS color value (string fallback)
pub fn parse_color(value: &str) -> Result<Color, ThemeParseError> {
    parse_color_string(value)
}

/// Parse linear-gradient from string
pub fn parse_linear_gradient(value: &str) -> Result<LinearGradient, ThemeParseError> {
    let value = value.trim();

    if !value.starts_with("linear-gradient(") || !value.ends_with(')') {
        return Err(ThemeParseError::InvalidGradient(value.to_string()));
    }

    let inner = &value[16..value.len() - 1];
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
        return Err(ThemeParseError::InvalidColor(format!(
            "text-shadow: {}",
            value
        )));
    };

    Ok(TextShadow {
        color,
        radius,
        intensity: color.a,
    })
}

/// Parse background-size from CSS string
pub fn parse_background_size(value: &str) -> BackgroundSize {
    let value = value.trim().to_lowercase();
    match value.as_str() {
        "cover" => BackgroundSize::Cover,
        "contain" => BackgroundSize::Contain,
        "auto" | "auto auto" => BackgroundSize::Auto,
        _ => {
            // Try to parse as fixed dimensions (e.g., "100px 200px")
            let parts: Vec<&str> = value.split_whitespace().collect();
            if parts.len() >= 2 {
                let w = parts[0].trim_end_matches("px").parse().unwrap_or(0);
                let h = parts[1].trim_end_matches("px").parse().unwrap_or(0);
                if w > 0 && h > 0 {
                    return BackgroundSize::Fixed(w, h);
                }
            }
            BackgroundSize::Cover
        }
    }
}

/// Parse background-position from CSS string
pub fn parse_background_position(value: &str) -> BackgroundPosition {
    let value = value.trim().to_lowercase();
    match value.as_str() {
        "center" | "center center" | "50% 50%" => BackgroundPosition::Center,
        "top" | "center top" | "top center" | "50% 0%" => BackgroundPosition::Top,
        "bottom" | "center bottom" | "bottom center" | "50% 100%" => BackgroundPosition::Bottom,
        "left" | "left center" | "center left" | "0% 50%" => BackgroundPosition::Left,
        "right" | "right center" | "center right" | "100% 50%" => BackgroundPosition::Right,
        "top left" | "left top" | "0% 0%" => BackgroundPosition::TopLeft,
        "top right" | "right top" | "100% 0%" => BackgroundPosition::TopRight,
        "bottom left" | "left bottom" | "0% 100%" => BackgroundPosition::BottomLeft,
        "bottom right" | "right bottom" | "100% 100%" => BackgroundPosition::BottomRight,
        _ => {
            // Try to parse as percentage values
            let parts: Vec<&str> = value.split_whitespace().collect();
            if parts.len() >= 2 {
                let x = parts[0].trim_end_matches('%').parse::<f32>().ok();
                let y = parts[1].trim_end_matches('%').parse::<f32>().ok();
                if let (Some(x), Some(y)) = (x, y) {
                    return BackgroundPosition::Percent(x / 100.0, y / 100.0);
                }
            }
            BackgroundPosition::Center
        }
    }
}

/// Parse background-repeat from CSS string
pub fn parse_background_repeat(value: &str) -> BackgroundRepeat {
    let value = value.trim().to_lowercase();
    match value.as_str() {
        "no-repeat" => BackgroundRepeat::NoRepeat,
        "repeat" | "repeat repeat" => BackgroundRepeat::Repeat,
        "repeat-x" | "repeat no-repeat" => BackgroundRepeat::RepeatX,
        "repeat-y" | "no-repeat repeat" => BackgroundRepeat::RepeatY,
        _ => BackgroundRepeat::NoRepeat,
    }
}

/// Collected properties from a CSS rule
struct RuleProperties {
    standard: HashMap<String, String>,
    custom: HashMap<String, String>,
}

/// Extract properties from a style rule's declarations
fn extract_properties(rule: &lightningcss::rules::style::StyleRule) -> RuleProperties {
    let mut standard = HashMap::new();
    let mut custom = HashMap::new();

    for decl in &rule.declarations.declarations {
        match decl {
            Property::Custom(prop) => {
                let name = prop.name.as_ref().to_string();
                // Manually serialize token list
                let mut value_parts = Vec::new();
                for token_or_value in &prop.value.0 {
                    use lightningcss::properties::custom::TokenOrValue;
                    match token_or_value {
                        TokenOrValue::Token(token) => {
                            if let Ok(s) = token.to_css_string(opts()) {
                                value_parts.push(s);
                            }
                        }
                        TokenOrValue::Color(color) => {
                            if let Ok(s) = color.to_css_string(opts()) {
                                value_parts.push(s);
                            }
                        }
                        TokenOrValue::Length(len) => {
                            if let Ok(s) = len.to_css_string(opts()) {
                                value_parts.push(s);
                            }
                        }
                        _ => {
                            // Other token types - skip for now
                        }
                    }
                }
                if !value_parts.is_empty() {
                    custom.insert(name, value_parts.join("").trim().to_string());
                }
            }
            Property::Color(color) => {
                if let Ok(css_str) = color.to_css_string(opts()) {
                    standard.insert("color".to_string(), css_str);
                }
            }
            Property::BackgroundColor(color) => {
                if let Ok(css_str) = color.to_css_string(opts()) {
                    standard.insert("background-color".to_string(), css_str);
                }
            }
            Property::Background(backgrounds) => {
                // Handle background shorthand - check for gradients and images
                for bg in backgrounds.iter() {
                    use lightningcss::values::image::Image;
                    match &bg.image {
                        Image::Gradient(gradient) => {
                            if let Ok(css_str) = gradient.to_css_string(opts()) {
                                standard.insert("background".to_string(), css_str);
                            }
                        }
                        Image::Url(url) => {
                            // Extract URL for background image
                            standard.insert("background-image".to_string(), url.url.to_string());
                        }
                        Image::None => {
                            // No image, check color
                            if let Ok(css_str) = bg.color.to_css_string(opts()) {
                                if !standard.contains_key("background") {
                                    standard.insert("background".to_string(), css_str);
                                }
                            }
                        }
                        _ => {}
                    }
                    // Also extract background-size, position, repeat from shorthand
                    if let Ok(css_str) = bg.size.to_css_string(opts()) {
                        if css_str != "auto" && css_str != "auto auto" {
                            standard.insert("background-size".to_string(), css_str);
                        }
                    }
                    if let Ok(css_str) = bg.position.to_css_string(opts()) {
                        if css_str != "0% 0%" {
                            standard.insert("background-position".to_string(), css_str);
                        }
                    }
                    if let Ok(css_str) = bg.repeat.to_css_string(opts()) {
                        if css_str != "repeat" {
                            standard.insert("background-repeat".to_string(), css_str);
                        }
                    }
                }
            }
            Property::BackgroundImage(images) => {
                // Handle background-image property directly
                use lightningcss::values::image::Image;
                for img in images.iter() {
                    match img {
                        Image::Url(url) => {
                            standard.insert("background-image".to_string(), url.url.to_string());
                        }
                        _ => {}
                    }
                }
            }
            Property::BackgroundSize(sizes) => {
                if let Some(size) = sizes.first() {
                    if let Ok(css_str) = size.to_css_string(opts()) {
                        standard.insert("background-size".to_string(), css_str);
                    }
                }
            }
            Property::BackgroundPosition(positions) => {
                if let Some(pos) = positions.first() {
                    if let Ok(css_str) = pos.to_css_string(opts()) {
                        standard.insert("background-position".to_string(), css_str);
                    }
                }
            }
            Property::BackgroundRepeat(repeats) => {
                if let Some(repeat) = repeats.first() {
                    if let Ok(css_str) = repeat.to_css_string(opts()) {
                        standard.insert("background-repeat".to_string(), css_str);
                    }
                }
            }
            Property::FontFamily(families) => {
                let names: Vec<String> = families
                    .iter()
                    .filter_map(|f| match f {
                        lightningcss::properties::font::FontFamily::FamilyName(name) => {
                            // Use ToCss to get the name - includes quotes if needed
                            name.to_css_string(opts()).ok()
                                .map(|s| s.trim_matches('"').to_string())
                        }
                        lightningcss::properties::font::FontFamily::Generic(g) => {
                            Some(format!("{:?}", g).to_lowercase())
                        }
                    })
                    .collect();
                standard.insert("font-family".to_string(), names.join(", "));
            }
            Property::FontSize(size) => {
                if let Ok(css_str) = size.to_css_string(opts()) {
                    standard.insert("font-size".to_string(), css_str);
                }
            }
            Property::LineHeight(height) => {
                if let Ok(css_str) = height.to_css_string(opts()) {
                    standard.insert("line-height".to_string(), css_str);
                }
            }
            Property::BorderRadius(radius, _) => {
                if let Ok(css_str) = radius.to_css_string(opts()) {
                    standard.insert("border-radius".to_string(), css_str);
                }
            }
            Property::BorderColor(color) => {
                if let Ok(css_str) = color.to_css_string(opts()) {
                    standard.insert("border-color".to_string(), css_str);
                }
            }
            Property::TextShadow(shadows) => {
                // Take first shadow
                if let Some(shadow) = shadows.first() {
                    if let Ok(css_str) = shadow.to_css_string(opts()) {
                        standard.insert("text-shadow".to_string(), css_str);
                    }
                }
            }
            Property::Width(width) => {
                if let Ok(css_str) = width.to_css_string(opts()) {
                    standard.insert("width".to_string(), css_str);
                }
            }
            Property::Height(height) => {
                if let Ok(css_str) = height.to_css_string(opts()) {
                    standard.insert("height".to_string(), css_str);
                }
            }
            Property::MinWidth(width) => {
                if let Ok(css_str) = width.to_css_string(opts()) {
                    standard.insert("min-width".to_string(), css_str);
                }
            }
            Property::MaxWidth(width) => {
                if let Ok(css_str) = width.to_css_string(opts()) {
                    standard.insert("max-width".to_string(), css_str);
                }
            }
            Property::Padding(padding) => {
                if let Ok(css_str) = padding.to_css_string(opts()) {
                    standard.insert("padding".to_string(), css_str);
                }
            }
            Property::Unparsed(unparsed) => {
                // Handle unparsed properties as fallback
                if let Ok(name) = unparsed.property_id.to_css_string(opts()) {
                    // Manually serialize token list
                    let mut value_parts = Vec::new();
                    for token_or_value in &unparsed.value.0 {
                        use lightningcss::properties::custom::TokenOrValue;
                        match token_or_value {
                            TokenOrValue::Token(token) => {
                                if let Ok(s) = token.to_css_string(opts()) {
                                    value_parts.push(s);
                                }
                            }
                            TokenOrValue::Color(color) => {
                                if let Ok(s) = color.to_css_string(opts()) {
                                    value_parts.push(s);
                                }
                            }
                            TokenOrValue::Length(len) => {
                                if let Ok(s) = len.to_css_string(opts()) {
                                    value_parts.push(s);
                                }
                            }
                            _ => {
                                // Other token types - skip for now
                            }
                        }
                    }
                    if !value_parts.is_empty() {
                        standard.insert(name, value_parts.join("").trim().to_string());
                    }
                }
            }
            _ => {
                // For other properties, try to get their string representation
            }
        }
    }

    RuleProperties { standard, custom }
}

/// Get selector string from a style rule
fn get_selector_string(rule: &lightningcss::rules::style::StyleRule) -> String {
    rule.selectors
        .to_css_string(opts())
        .unwrap_or_default()
}

/// Parse CSS theme using lightningcss
pub fn parse_theme(css: &str) -> Result<Theme, ThemeParseError> {
    let stylesheet = StyleSheet::parse(css, ParserOptions::default())
        .map_err(|e| ThemeParseError::CssError(format!("{:?}", e)))?;

    let mut theme = Theme::minimal();

    for rule in &stylesheet.rules.0 {
        if let CssRule::Style(style_rule) = rule {
            let selector = get_selector_string(style_rule);
            let props = extract_properties(style_rule);

            apply_properties(&mut theme, &selector, &props.standard, &props.custom)?;
        }
    }

    Ok(theme)
}

/// Apply parsed properties to theme based on selector
fn apply_properties(
    theme: &mut Theme,
    selector: &str,
    standard: &HashMap<String, String>,
    custom: &HashMap<String, String>,
) -> Result<(), ThemeParseError> {
    let selector = selector.trim();

    match selector {
        ":terminal" | "terminal" => {
            apply_terminal_properties(theme, standard, custom)?;
        }
        ":terminal::selection" | "terminal::selection" => {
            apply_selection_properties(theme, standard)?;
        }
        ":terminal::highlight" | "terminal::highlight" => {
            apply_highlight_properties(theme, standard, custom)?;
        }
        ":terminal::cursor" | "terminal::cursor" => {
            apply_cursor_properties(theme, standard)?;
        }
        ":terminal::backdrop" | "terminal::backdrop" => {
            apply_backdrop_properties(theme, custom)?;
        }
        ":terminal::tab-bar" | "terminal::tab-bar" => {
            apply_tab_bar_properties(theme, standard)?;
        }
        ":terminal::tab" | "terminal::tab" | ":tab" | "tab" => {
            apply_tab_properties(theme, standard)?;
        }
        ":terminal::tab-active" | "terminal::tab-active" | ":tab.active" | "tab.active" => {
            apply_tab_active_properties(theme, standard, custom)?;
        }
        ":terminal::tab-close" | "terminal::tab-close" => {
            apply_tab_close_properties(theme, standard, custom)?;
        }
        ":terminal::palette" | "terminal::palette" => {
            apply_palette_properties(theme, custom)?;
        }
        _ => {
            // Ignore unknown selectors
        }
    }

    Ok(())
}

fn apply_terminal_properties(
    theme: &mut Theme,
    standard: &HashMap<String, String>,
    custom: &HashMap<String, String>,
) -> Result<(), ThemeParseError> {
    // Typography
    if let Some(font) = standard.get("font-family") {
        theme.typography.font_family = font
            .split(',')
            .map(|s| s.trim().trim_matches('"').trim_matches('\'').to_string())
            .collect();
    }
    if let Some(size) = standard.get("font-size") {
        theme.typography.font_size = size.trim_end_matches("px").parse().unwrap_or(14.0);
    }
    if let Some(height) = standard.get("line-height") {
        theme.typography.line_height = height.parse().unwrap_or(1.3);
    }

    // Colors
    if let Some(color) = standard.get("color") {
        theme.foreground = parse_color(color)?;
    }
    if let Some(bg) = standard.get("background") {
        if bg.contains("linear-gradient") {
            theme.background = parse_linear_gradient(bg)?;
        } else {
            let color = parse_color(bg)?;
            theme.background = LinearGradient {
                top: color,
                bottom: color,
            };
        }
    }

    // Text shadow / glow
    if let Some(shadow) = standard.get("text-shadow") {
        theme.text_shadow = Some(parse_text_shadow(shadow)?);
    }

    // Background image
    if let Some(url) = standard.get("background-image") {
        let size = standard.get("background-size")
            .map(|s| parse_background_size(s))
            .unwrap_or_default();
        let position = standard.get("background-position")
            .map(|s| parse_background_position(s))
            .unwrap_or_default();
        let repeat = standard.get("background-repeat")
            .map(|s| parse_background_repeat(s))
            .unwrap_or_default();
        let opacity = custom.get("--background-opacity")
            .and_then(|s| s.parse::<f32>().ok())
            .unwrap_or(1.0);

        theme.background_image = Some(BackgroundImage {
            path: Some(url.clone()),
            base_dir: None, // Set by Theme::from_css_with_base
            size,
            position,
            repeat,
            opacity,
        });
    }

    // ANSI palette colors - supports both --ansi-* and --color-* naming
    // --ansi-* is the preferred/documented format
    apply_ansi_palette(theme, custom)?;

    // Font variants
    if let Some(f) = custom.get("--font-bold") {
        theme.typography.font_bold = Some(f.trim_matches('"').to_string());
    }
    if let Some(f) = custom.get("--font-italic") {
        theme.typography.font_italic = Some(f.trim_matches('"').to_string());
    }
    if let Some(f) = custom.get("--font-bold-italic") {
        theme.typography.font_bold_italic = Some(f.trim_matches('"').to_string());
    }

    Ok(())
}

fn apply_selection_properties(
    theme: &mut Theme,
    standard: &HashMap<String, String>,
) -> Result<(), ThemeParseError> {
    if let Some(bg) = standard.get("background") {
        theme.selection.background = parse_color(bg)?;
    }
    if let Some(fg) = standard.get("color") {
        theme.selection.foreground = parse_color(fg)?;
    }
    Ok(())
}

fn apply_highlight_properties(
    theme: &mut Theme,
    standard: &HashMap<String, String>,
    custom: &HashMap<String, String>,
) -> Result<(), ThemeParseError> {
    if let Some(bg) = standard.get("background") {
        theme.highlight.background = parse_color(bg)?;
    }
    if let Some(fg) = standard.get("color") {
        theme.highlight.foreground = parse_color(fg)?;
    }
    if let Some(bg) = custom.get("--current-background") {
        theme.highlight.current_background = parse_color(bg)?;
    }
    Ok(())
}

fn apply_cursor_properties(
    theme: &mut Theme,
    standard: &HashMap<String, String>,
) -> Result<(), ThemeParseError> {
    if let Some(bg) = standard.get("background") {
        theme.cursor_color = parse_color(bg)?;
    }
    Ok(())
}

fn apply_backdrop_properties(
    theme: &mut Theme,
    custom: &HashMap<String, String>,
) -> Result<(), ThemeParseError> {
    let mut grid = theme.grid.unwrap_or(GridEffect {
        enabled: false,
        ..Default::default()
    });

    let mut has_grid_props = false;

    if let Some(c) = custom.get("--grid-color") {
        grid.color = parse_color(c)?;
        has_grid_props = true;
    }
    if let Some(v) = custom.get("--grid-spacing") {
        grid.spacing = v.parse().unwrap_or(8.0);
        has_grid_props = true;
    }
    if let Some(v) = custom.get("--grid-line-width") {
        grid.line_width = v.parse().unwrap_or(0.02);
    }
    if let Some(v) = custom.get("--grid-perspective") {
        grid.perspective = v.parse().unwrap_or(2.0);
    }
    if let Some(v) = custom.get("--grid-horizon") {
        grid.horizon = v.parse().unwrap_or(0.35);
    }
    if let Some(v) = custom.get("--grid-animation-speed") {
        grid.animation_speed = v.parse().unwrap_or(0.5);
    }
    if let Some(v) = custom.get("--grid-glow-radius") {
        grid.glow_radius = v.parse().unwrap_or(0.0);
    }
    if let Some(v) = custom.get("--grid-glow-intensity") {
        grid.glow_intensity = v.parse().unwrap_or(0.0);
    }
    if let Some(v) = custom.get("--grid-vanishing-spread") {
        grid.vanishing_spread = v.parse().unwrap_or(0.3);
    }
    if let Some(v) = custom.get("--grid-curved") {
        grid.curved = v.trim() == "true";
    }

    if let Some(v) = custom.get("--grid-enabled") {
        grid.enabled = v.trim() == "true";
    } else if has_grid_props {
        grid.enabled = true;
    }

    if grid.enabled {
        theme.grid = Some(grid);
    }

    // Parse starfield effect properties
    let mut starfield = theme.starfield.unwrap_or(StarfieldEffect {
        enabled: false,
        ..Default::default()
    });

    let mut has_starfield_props = false;

    if let Some(c) = custom.get("--starfield-color") {
        starfield.color = parse_color(c)?;
        has_starfield_props = true;
    }
    if let Some(v) = custom.get("--starfield-density") {
        starfield.density = v.parse().unwrap_or(100);
        has_starfield_props = true;
    }
    if let Some(v) = custom.get("--starfield-layers") {
        starfield.layers = v.parse().unwrap_or(3);
        has_starfield_props = true;
    }
    if let Some(v) = custom.get("--starfield-speed") {
        starfield.speed = v.parse().unwrap_or(0.3);
    }
    if let Some(v) = custom.get("--starfield-direction") {
        if let Some(dir) = StarDirection::from_str(v.trim()) {
            starfield.direction = dir;
        }
    }
    if let Some(v) = custom.get("--starfield-glow-radius") {
        starfield.glow_radius = v.parse().unwrap_or(0.0);
    }
    if let Some(v) = custom.get("--starfield-glow-intensity") {
        starfield.glow_intensity = v.parse().unwrap_or(0.0);
    }
    if let Some(v) = custom.get("--starfield-twinkle") {
        starfield.twinkle = v.trim() == "true";
    }
    if let Some(v) = custom.get("--starfield-twinkle-speed") {
        starfield.twinkle_speed = v.parse().unwrap_or(2.0);
    }
    if let Some(v) = custom.get("--starfield-min-size") {
        starfield.min_size = v.parse().unwrap_or(1.0);
    }
    if let Some(v) = custom.get("--starfield-max-size") {
        starfield.max_size = v.parse().unwrap_or(3.0);
    }

    if let Some(v) = custom.get("--starfield-enabled") {
        starfield.enabled = v.trim() == "true";
    } else if has_starfield_props {
        starfield.enabled = true;
    }

    if starfield.enabled {
        theme.starfield = Some(starfield);
    }

    // Parse rain effect properties
    let mut rain = theme.rain.unwrap_or(RainEffect {
        enabled: false,
        ..Default::default()
    });

    let mut has_rain_props = false;

    if let Some(c) = custom.get("--rain-color") {
        rain.color = parse_color(c)?;
        has_rain_props = true;
    }
    if let Some(v) = custom.get("--rain-density") {
        rain.density = v.parse().unwrap_or(150);
        has_rain_props = true;
    }
    if let Some(v) = custom.get("--rain-speed") {
        rain.speed = v.parse().unwrap_or(1.0);
    }
    if let Some(v) = custom.get("--rain-angle") {
        rain.angle = v.parse().unwrap_or(0.0);
    }
    if let Some(v) = custom.get("--rain-length") {
        rain.length = v.parse().unwrap_or(20.0);
    }
    if let Some(v) = custom.get("--rain-thickness") {
        rain.thickness = v.parse().unwrap_or(1.5);
    }
    if let Some(v) = custom.get("--rain-glow-radius") {
        rain.glow_radius = v.parse().unwrap_or(0.0);
    }
    if let Some(v) = custom.get("--rain-glow-intensity") {
        rain.glow_intensity = v.parse().unwrap_or(0.0);
    }

    if let Some(v) = custom.get("--rain-enabled") {
        rain.enabled = v.trim() == "true";
    } else if has_rain_props {
        rain.enabled = true;
    }

    if rain.enabled {
        theme.rain = Some(rain);
    }

    // Parse particle effect properties
    let mut particles = theme.particles.unwrap_or(ParticleEffect {
        enabled: false,
        ..Default::default()
    });

    let mut has_particle_props = false;

    if let Some(c) = custom.get("--particles-color") {
        particles.color = parse_color(c)?;
        has_particle_props = true;
    }
    if let Some(v) = custom.get("--particles-count") {
        particles.count = v.parse().unwrap_or(50);
        has_particle_props = true;
    }
    if let Some(v) = custom.get("--particles-shape") {
        if let Some(shape) = ParticleShape::from_str(v.trim()) {
            particles.shape = shape;
        }
    }
    if let Some(v) = custom.get("--particles-behavior") {
        if let Some(behavior) = ParticleBehavior::from_str(v.trim()) {
            particles.behavior = behavior;
        }
    }
    if let Some(v) = custom.get("--particles-size") {
        particles.size = v.parse().unwrap_or(4.0);
    }
    if let Some(v) = custom.get("--particles-speed") {
        particles.speed = v.parse().unwrap_or(0.5);
    }
    if let Some(v) = custom.get("--particles-glow-radius") {
        particles.glow_radius = v.parse().unwrap_or(0.0);
    }
    if let Some(v) = custom.get("--particles-glow-intensity") {
        particles.glow_intensity = v.parse().unwrap_or(0.0);
    }

    if let Some(v) = custom.get("--particles-enabled") {
        particles.enabled = v.trim() == "true";
    } else if has_particle_props {
        particles.enabled = true;
    }

    if particles.enabled {
        theme.particles = Some(particles);
    }

    // Parse matrix effect properties
    let mut matrix = theme.matrix.clone().unwrap_or(MatrixEffect {
        enabled: false,
        ..Default::default()
    });

    let has_matrix_props = custom.keys().any(|k| k.starts_with("--matrix-"));

    if let Some(v) = custom.get("--matrix-color") {
        matrix.color = parse_color(v)?;
    }
    if let Some(v) = custom.get("--matrix-density") {
        matrix.density = v.parse().unwrap_or(1.0);
    }
    if let Some(v) = custom.get("--matrix-speed") {
        matrix.speed = v.parse().unwrap_or(8.0);
    }
    if let Some(v) = custom.get("--matrix-font-size") {
        matrix.font_size = v.parse().unwrap_or(14.0);
    }
    if let Some(v) = custom.get("--matrix-charset") {
        matrix.charset = v.trim_matches('"').to_string();
    }

    if let Some(v) = custom.get("--matrix-enabled") {
        matrix.enabled = v.trim() == "true";
    } else if has_matrix_props {
        matrix.enabled = true;
    }

    if matrix.enabled {
        theme.matrix = Some(matrix);
    }

    // Parse shape effect properties
    let mut shape = theme.shape.clone().unwrap_or(ShapeEffect {
        enabled: false,
        ..Default::default()
    });

    let has_shape_props = custom.keys().any(|k| k.starts_with("--shape-"));

    if let Some(v) = custom.get("--shape-type") {
        if let Some(t) = ShapeType::from_str(v.trim()) {
            shape.shape_type = t;
        }
    }
    if let Some(v) = custom.get("--shape-size") {
        shape.size = v.parse().unwrap_or(100.0);
    }
    if let Some(v) = custom.get("--shape-fill") {
        if v.trim().to_lowercase() == "none" {
            shape.fill = None;
        } else {
            shape.fill = Some(parse_color(v)?);
        }
    }
    if let Some(v) = custom.get("--shape-stroke") {
        if v.trim().to_lowercase() == "none" {
            shape.stroke = None;
        } else {
            shape.stroke = Some(parse_color(v)?);
        }
    }
    if let Some(v) = custom.get("--shape-stroke-width") {
        shape.stroke_width = v.parse().unwrap_or(2.0);
    }
    if let Some(v) = custom.get("--shape-glow-radius") {
        shape.glow_radius = v.parse().unwrap_or(0.0);
    }
    if let Some(v) = custom.get("--shape-glow-color") {
        shape.glow_color = Some(parse_color(v)?);
    }
    if let Some(v) = custom.get("--shape-rotation") {
        if let Some(r) = ShapeRotation::from_str(v.trim()) {
            shape.rotation = r;
        }
    }
    if let Some(v) = custom.get("--shape-rotation-speed") {
        shape.rotation_speed = v.parse().unwrap_or(1.0);
    }
    if let Some(v) = custom.get("--shape-motion") {
        if let Some(m) = ShapeMotion::from_str(v.trim()) {
            shape.motion = m;
        }
    }
    if let Some(v) = custom.get("--shape-motion-speed") {
        shape.motion_speed = v.parse().unwrap_or(1.0);
    }
    if let Some(v) = custom.get("--shape-polygon-sides") {
        shape.polygon_sides = v.parse().unwrap_or(6);
    }

    if let Some(v) = custom.get("--shape-enabled") {
        shape.enabled = v.trim() == "true";
    } else if has_shape_props {
        shape.enabled = true;
    }

    if shape.enabled {
        theme.shape = Some(shape);
    }

    Ok(())
}

fn apply_tab_bar_properties(
    theme: &mut Theme,
    standard: &HashMap<String, String>,
) -> Result<(), ThemeParseError> {
    if let Some(bg) = standard.get("background") {
        theme.tabs.bar.background = parse_color(bg)?;
    }
    if let Some(c) = standard.get("border-color") {
        theme.tabs.bar.border_color = parse_color(c)?;
    }
    if let Some(v) = standard.get("height") {
        theme.tabs.bar.height = v.trim_end_matches("px").parse().unwrap_or(36.0);
    }
    if let Some(v) = standard.get("padding") {
        theme.tabs.bar.padding = v.trim_end_matches("px").parse().unwrap_or(4.0);
    }
    Ok(())
}

fn apply_tab_properties(
    theme: &mut Theme,
    standard: &HashMap<String, String>,
) -> Result<(), ThemeParseError> {
    if let Some(bg) = standard.get("background") {
        theme.tabs.tab.background = parse_color(bg)?;
    }
    if let Some(fg) = standard.get("color") {
        theme.tabs.tab.foreground = parse_color(fg)?;
    }
    if let Some(v) = standard.get("border-radius") {
        theme.tabs.tab.border_radius = v.trim_end_matches("px").parse().unwrap_or(4.0);
    }
    if let Some(v) = standard.get("padding-x") {
        theme.tabs.tab.padding_x = v.trim_end_matches("px").parse().unwrap_or(12.0);
    }
    if let Some(v) = standard.get("padding-y") {
        theme.tabs.tab.padding_y = v.trim_end_matches("px").parse().unwrap_or(6.0);
    }
    if let Some(v) = standard.get("min-width") {
        theme.tabs.tab.min_width = v.trim_end_matches("px").parse().unwrap_or(80.0);
    }
    if let Some(v) = standard.get("max-width") {
        theme.tabs.tab.max_width = v.trim_end_matches("px").parse().unwrap_or(200.0);
    }
    if let Some(shadow) = standard.get("text-shadow") {
        theme.tabs.tab.text_shadow = Some(parse_text_shadow(shadow)?);
    }
    Ok(())
}

fn apply_tab_active_properties(
    theme: &mut Theme,
    standard: &HashMap<String, String>,
    _custom: &HashMap<String, String>,
) -> Result<(), ThemeParseError> {
    if let Some(bg) = standard.get("background") {
        theme.tabs.active.background = parse_color(bg)?;
    }
    if let Some(fg) = standard.get("color") {
        theme.tabs.active.foreground = parse_color(fg)?;
    }
    if let Some(c) = standard.get("accent-color") {
        theme.tabs.active.accent = parse_color(c)?;
    }
    if let Some(shadow) = standard.get("text-shadow") {
        theme.tabs.active.text_shadow = Some(parse_text_shadow(shadow)?);
    }
    Ok(())
}

fn apply_tab_close_properties(
    theme: &mut Theme,
    standard: &HashMap<String, String>,
    custom: &HashMap<String, String>,
) -> Result<(), ThemeParseError> {
    if let Some(bg) = standard.get("background") {
        theme.tabs.close.background = parse_color(bg)?;
    }
    if let Some(fg) = standard.get("color") {
        theme.tabs.close.foreground = parse_color(fg)?;
    }
    if let Some(bg) = custom.get("--hover-background") {
        theme.tabs.close.hover_background = parse_color(bg)?;
    }
    if let Some(fg) = custom.get("--hover-color") {
        theme.tabs.close.hover_foreground = parse_color(fg)?;
    }
    if let Some(v) = standard.get("width") {
        theme.tabs.close.size = v.trim_end_matches("px").parse().unwrap_or(16.0);
    }
    Ok(())
}

/// Apply ANSI palette colors from custom properties
/// Supports multiple naming conventions:
/// - --ansi-black, --ansi-red, etc. (preferred)
/// - --ansi-bright-black, --ansi-bright-red, etc. (preferred)
/// - --color-black, --color-red, etc. (legacy)
/// - --color-bright-black, etc. (legacy)
fn apply_ansi_palette(
    theme: &mut Theme,
    custom: &HashMap<String, String>,
) -> Result<(), ThemeParseError> {
    // Helper to get color from either --ansi-* or --color-* format
    fn get_color<'a>(custom: &'a HashMap<String, String>, name: &str) -> Option<&'a String> {
        custom.get(&format!("--ansi-{}", name))
            .or_else(|| custom.get(&format!("--color-{}", name)))
    }

    // Normal colors (0-7)
    if let Some(c) = get_color(custom, "black") {
        theme.palette.black = parse_color(c)?;
    }
    if let Some(c) = get_color(custom, "red") {
        theme.palette.red = parse_color(c)?;
    }
    if let Some(c) = get_color(custom, "green") {
        theme.palette.green = parse_color(c)?;
    }
    if let Some(c) = get_color(custom, "yellow") {
        theme.palette.yellow = parse_color(c)?;
    }
    if let Some(c) = get_color(custom, "blue") {
        theme.palette.blue = parse_color(c)?;
    }
    if let Some(c) = get_color(custom, "magenta") {
        theme.palette.magenta = parse_color(c)?;
    }
    if let Some(c) = get_color(custom, "cyan") {
        theme.palette.cyan = parse_color(c)?;
    }
    if let Some(c) = get_color(custom, "white") {
        theme.palette.white = parse_color(c)?;
    }

    // Bright colors (8-15)
    if let Some(c) = get_color(custom, "bright-black") {
        theme.palette.bright_black = parse_color(c)?;
    }
    if let Some(c) = get_color(custom, "bright-red") {
        theme.palette.bright_red = parse_color(c)?;
    }
    if let Some(c) = get_color(custom, "bright-green") {
        theme.palette.bright_green = parse_color(c)?;
    }
    if let Some(c) = get_color(custom, "bright-yellow") {
        theme.palette.bright_yellow = parse_color(c)?;
    }
    if let Some(c) = get_color(custom, "bright-blue") {
        theme.palette.bright_blue = parse_color(c)?;
    }
    if let Some(c) = get_color(custom, "bright-magenta") {
        theme.palette.bright_magenta = parse_color(c)?;
    }
    if let Some(c) = get_color(custom, "bright-cyan") {
        theme.palette.bright_cyan = parse_color(c)?;
    }
    if let Some(c) = get_color(custom, "bright-white") {
        theme.palette.bright_white = parse_color(c)?;
    }

    Ok(())
}

fn apply_palette_properties(
    theme: &mut Theme,
    custom: &HashMap<String, String>,
) -> Result<(), ThemeParseError> {
    // Base 16 colors (--color-0 through --color-15)
    if let Some(c) = custom.get("--color-0") {
        theme.palette.black = parse_color(c)?;
    }
    if let Some(c) = custom.get("--color-1") {
        theme.palette.red = parse_color(c)?;
    }
    if let Some(c) = custom.get("--color-2") {
        theme.palette.green = parse_color(c)?;
    }
    if let Some(c) = custom.get("--color-3") {
        theme.palette.yellow = parse_color(c)?;
    }
    if let Some(c) = custom.get("--color-4") {
        theme.palette.blue = parse_color(c)?;
    }
    if let Some(c) = custom.get("--color-5") {
        theme.palette.magenta = parse_color(c)?;
    }
    if let Some(c) = custom.get("--color-6") {
        theme.palette.cyan = parse_color(c)?;
    }
    if let Some(c) = custom.get("--color-7") {
        theme.palette.white = parse_color(c)?;
    }
    if let Some(c) = custom.get("--color-8") {
        theme.palette.bright_black = parse_color(c)?;
    }
    if let Some(c) = custom.get("--color-9") {
        theme.palette.bright_red = parse_color(c)?;
    }
    if let Some(c) = custom.get("--color-10") {
        theme.palette.bright_green = parse_color(c)?;
    }
    if let Some(c) = custom.get("--color-11") {
        theme.palette.bright_yellow = parse_color(c)?;
    }
    if let Some(c) = custom.get("--color-12") {
        theme.palette.bright_blue = parse_color(c)?;
    }
    if let Some(c) = custom.get("--color-13") {
        theme.palette.bright_magenta = parse_color(c)?;
    }
    if let Some(c) = custom.get("--color-14") {
        theme.palette.bright_cyan = parse_color(c)?;
    }
    if let Some(c) = custom.get("--color-15") {
        theme.palette.bright_white = parse_color(c)?;
    }

    // Extended colors (--color-16 through --color-255)
    // These override the standard 256-color palette calculations
    for idx in 16u8..=255 {
        let key = format!("--color-{}", idx);
        if let Some(c) = custom.get(&key) {
            let color = parse_color(c)?;
            theme.palette.set_extended(idx, color);
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
    fn test_parse_rgb_color() {
        let c = parse_rgb_color("rgb(255, 85, 85)").unwrap();
        assert!((c.r - 1.0).abs() < 0.01);

        let c = parse_rgb_color("rgba(0, 255, 255, 0.6)").unwrap();
        assert!((c.g - 1.0).abs() < 0.01);
        assert!((c.a - 0.6).abs() < 0.01);
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
    fn test_parse_theme_with_comments() {
        let css = r#"
            :terminal {
                /* Typography */
                font-family: "JetBrains Mono", monospace;
                font-size: 14px;

                /* Base colors - teal text */
                color: #61e2fe;
                background: #1a1a1a;
            }
        "#;

        let theme = parse_theme(css).unwrap();
        assert!((theme.foreground.r - 97.0 / 255.0).abs() < 0.01);
        assert!((theme.foreground.g - 226.0 / 255.0).abs() < 0.01);
    }

    #[test]
    fn test_parse_gradient() {
        let g = parse_linear_gradient("linear-gradient(to bottom, #1a0a2e, #16213e)").unwrap();
        assert!(g.top.r < 0.2);
    }

    #[test]
    fn test_parse_ansi_palette() {
        let css = r#"
            :terminal {
                --ansi-black: #1a1a2e;
                --ansi-red: #ff5555;
                --ansi-green: #50fa7b;
                --ansi-yellow: #f1fa8c;
                --ansi-blue: #6272a4;
                --ansi-magenta: #ff79c6;
                --ansi-cyan: #8be9fd;
                --ansi-white: #f8f8f2;
                --ansi-bright-black: #44475a;
                --ansi-bright-red: #ff6e6e;
                --ansi-bright-green: #69ff94;
                --ansi-bright-yellow: #ffffa5;
                --ansi-bright-blue: #d6acff;
                --ansi-bright-magenta: #ff92df;
                --ansi-bright-cyan: #a4ffff;
                --ansi-bright-white: #ffffff;
            }
        "#;

        let theme = parse_theme(css).unwrap();

        // Check normal colors
        assert!((theme.palette.red.r - 1.0).abs() < 0.01); // #ff5555
        assert!((theme.palette.green.g - 0.98).abs() < 0.02); // #50fa7b
        assert!((theme.palette.cyan.b - 0.99).abs() < 0.02); // #8be9fd

        // Check bright colors
        assert!((theme.palette.bright_white.r - 1.0).abs() < 0.01); // #ffffff
        assert!((theme.palette.bright_black.r - 0.267).abs() < 0.02); // #44475a

        // Test palette.get() method
        let red = theme.palette.get(1);
        assert!((red.r - 1.0).abs() < 0.01);

        let bright_cyan = theme.palette.get(14);
        assert!((bright_cyan.r - 0.643).abs() < 0.02); // #a4ffff
    }

    #[test]
    fn test_parse_background_image() {
        let css = r#"
            :terminal {
                background-image: url("/path/to/image.png");
                background-size: cover;
                background-position: center;
                background-repeat: no-repeat;
                --background-opacity: 0.8;
            }
        "#;

        let theme = parse_theme(css).unwrap();
        let bg = theme.background_image.expect("background_image should be set");
        assert_eq!(bg.path, Some("/path/to/image.png".to_string()));
        assert_eq!(bg.size, BackgroundSize::Cover);
        assert_eq!(bg.position, BackgroundPosition::Center);
        assert_eq!(bg.repeat, BackgroundRepeat::NoRepeat);
        assert!((bg.opacity - 0.8).abs() < 0.01);
    }

    #[test]
    fn test_parse_background_size() {
        assert_eq!(parse_background_size("cover"), BackgroundSize::Cover);
        assert_eq!(parse_background_size("contain"), BackgroundSize::Contain);
        assert_eq!(parse_background_size("auto"), BackgroundSize::Auto);
        assert_eq!(parse_background_size("100px 200px"), BackgroundSize::Fixed(100, 200));
    }

    #[test]
    fn test_parse_background_position() {
        assert_eq!(parse_background_position("center"), BackgroundPosition::Center);
        assert_eq!(parse_background_position("top left"), BackgroundPosition::TopLeft);
        assert_eq!(parse_background_position("bottom right"), BackgroundPosition::BottomRight);
    }

    #[test]
    fn test_parse_background_repeat() {
        assert_eq!(parse_background_repeat("no-repeat"), BackgroundRepeat::NoRepeat);
        assert_eq!(parse_background_repeat("repeat"), BackgroundRepeat::Repeat);
        assert_eq!(parse_background_repeat("repeat-x"), BackgroundRepeat::RepeatX);
        assert_eq!(parse_background_repeat("repeat-y"), BackgroundRepeat::RepeatY);
    }

    #[test]
    fn test_parse_extended_palette() {
        let css = r#"
            :terminal::palette {
                --color-0: #000000;
                --color-15: #ffffff;
                --color-226: #61fe71;
                --color-178: #71fe81;
                --color-255: #7d8d80;
            }
        "#;

        let theme = parse_theme(css).unwrap();

        // Base colors
        assert!((theme.palette.black.r - 0.0).abs() < 0.01);
        assert!((theme.palette.bright_white.r - 1.0).abs() < 0.01);

        // Extended colors should be set
        let color_226 = theme.palette.get_extended(226);
        assert!(color_226.is_some());
        let c = color_226.unwrap();
        assert!((c.r - 0.38).abs() < 0.02); // #61 = 97/255 = 0.38

        let color_178 = theme.palette.get_extended(178);
        assert!(color_178.is_some());

        let color_255 = theme.palette.get_extended(255);
        assert!(color_255.is_some());

        // Non-overridden extended color should return None
        let color_100 = theme.palette.get_extended(100);
        assert!(color_100.is_none());
    }
}
