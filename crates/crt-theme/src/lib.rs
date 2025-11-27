//! CRT Theme - CSS-like theming engine
//!
//! Provides theme structures that map to shader uniforms for terminal rendering.

pub mod parser;

use bytemuck::{Pod, Zeroable};
use std::path::Path;

/// RGBA color with f32 components (0.0 - 1.0)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// Create from hex color (e.g., 0xff5555)
    pub const fn from_hex(hex: u32) -> Self {
        Self {
            r: ((hex >> 16) & 0xff) as f32 / 255.0,
            g: ((hex >> 8) & 0xff) as f32 / 255.0,
            b: (hex & 0xff) as f32 / 255.0,
            a: 1.0,
        }
    }

    /// Create from hex with alpha (e.g., 0xff555580)
    pub const fn from_hex_alpha(hex: u32) -> Self {
        Self {
            r: ((hex >> 24) & 0xff) as f32 / 255.0,
            g: ((hex >> 16) & 0xff) as f32 / 255.0,
            b: ((hex >> 8) & 0xff) as f32 / 255.0,
            a: (hex & 0xff) as f32 / 255.0,
        }
    }

    pub const fn to_array(self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
    }
}

impl Default for Color {
    fn default() -> Self {
        Self::rgb(1.0, 1.0, 1.0)
    }
}

/// Typography settings
#[derive(Debug, Clone)]
pub struct Typography {
    pub font_family: Vec<String>,
    pub font_size: f32,
    pub line_height: f32,
    pub font_bold: Option<String>,
    pub font_italic: Option<String>,
    pub font_bold_italic: Option<String>,
    pub ligatures: bool,
}

impl Default for Typography {
    fn default() -> Self {
        Self {
            font_family: vec!["monospace".to_string()],
            font_size: 14.0,
            line_height: 1.3,
            font_bold: None,
            font_italic: None,
            font_bold_italic: None,
            ligatures: true,
        }
    }
}

/// 256-color ANSI palette
/// Base 16 colors are stored as named fields, extended colors (16-255) in a HashMap
#[derive(Debug, Clone)]
pub struct AnsiPalette {
    // Base 16 colors (0-15)
    pub black: Color,
    pub red: Color,
    pub green: Color,
    pub yellow: Color,
    pub blue: Color,
    pub magenta: Color,
    pub cyan: Color,
    pub white: Color,
    pub bright_black: Color,
    pub bright_red: Color,
    pub bright_green: Color,
    pub bright_yellow: Color,
    pub bright_blue: Color,
    pub bright_magenta: Color,
    pub bright_cyan: Color,
    pub bright_white: Color,
    // Extended colors (16-255) - only stores overrides, None means use calculated default
    extended: std::collections::HashMap<u8, Color>,
}

impl AnsiPalette {
    /// Get base color by ANSI index (0-15)
    pub fn get(&self, index: u8) -> Color {
        match index {
            0 => self.black,
            1 => self.red,
            2 => self.green,
            3 => self.yellow,
            4 => self.blue,
            5 => self.magenta,
            6 => self.cyan,
            7 => self.white,
            8 => self.bright_black,
            9 => self.bright_red,
            10 => self.bright_green,
            11 => self.bright_yellow,
            12 => self.bright_blue,
            13 => self.bright_magenta,
            14 => self.bright_cyan,
            15 => self.bright_white,
            _ => self.white,
        }
    }

    /// Get extended color by index (16-255), returns None if not overridden
    pub fn get_extended(&self, index: u8) -> Option<Color> {
        self.extended.get(&index).copied()
    }

    /// Set an extended color (16-255)
    pub fn set_extended(&mut self, index: u8, color: Color) {
        if index >= 16 {
            self.extended.insert(index, color);
        }
    }

    /// Check if an extended color is overridden
    pub fn has_extended(&self, index: u8) -> bool {
        self.extended.contains_key(&index)
    }

    /// Calculate the default color for extended palette indices (16-255)
    /// Returns the standard 256-color palette value
    pub fn calculate_extended(index: u8) -> Color {
        if index < 16 {
            // Should use get() for base colors
            Color::rgb(1.0, 1.0, 1.0)
        } else if index < 232 {
            // 216 color cube (16-231)
            // 6x6x6 color cube: r, g, b each from 0-5
            let idx = index - 16;
            let r = (idx / 36) % 6;
            let g = (idx / 6) % 6;
            let b = idx % 6;
            let to_float = |v: u8| if v == 0 { 0.0 } else { (v as f32 * 40.0 + 55.0) / 255.0 };
            Color::rgb(to_float(r), to_float(g), to_float(b))
        } else {
            // Grayscale (232-255)
            // 24 shades of gray from dark to light
            let gray = ((index - 232) as f32 * 10.0 + 8.0) / 255.0;
            Color::rgb(gray, gray, gray)
        }
    }
}

impl Default for AnsiPalette {
    fn default() -> Self {
        // Synthwave-inspired defaults
        Self {
            black: Color::from_hex(0x0d0d0d),
            red: Color::from_hex(0xff5555),
            green: Color::from_hex(0x50fa7b),
            yellow: Color::from_hex(0xf1fa8c),
            blue: Color::from_hex(0xbd93f9),
            magenta: Color::from_hex(0xff79c6),
            cyan: Color::from_hex(0x8be9fd),
            white: Color::from_hex(0xf8f8f2),
            bright_black: Color::from_hex(0x4d4d4d),
            bright_red: Color::from_hex(0xff6e6e),
            bright_green: Color::from_hex(0x69ff94),
            bright_yellow: Color::from_hex(0xffffa5),
            bright_blue: Color::from_hex(0xd6acff),
            bright_magenta: Color::from_hex(0xff92df),
            bright_cyan: Color::from_hex(0xa4ffff),
            bright_white: Color::from_hex(0xffffff),
            extended: std::collections::HashMap::new(),
        }
    }
}

/// Text shadow / glow effect
#[derive(Debug, Clone, Copy)]
pub struct TextShadow {
    pub color: Color,
    pub radius: f32,
    pub intensity: f32,
}

impl Default for TextShadow {
    fn default() -> Self {
        Self {
            color: Color::rgba(0.0, 1.0, 1.0, 0.6), // cyan glow
            radius: 8.0,
            intensity: 0.6,
        }
    }
}

/// Linear gradient (two-stop for now)
#[derive(Debug, Clone, Copy)]
pub struct LinearGradient {
    pub top: Color,
    pub bottom: Color,
}

impl Default for LinearGradient {
    fn default() -> Self {
        Self {
            top: Color::from_hex(0x1a0a2e),
            bottom: Color::from_hex(0x16213e),
        }
    }
}

/// Background image sizing mode (CSS background-size)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BackgroundSize {
    /// Scale to cover the entire area, may crop
    #[default]
    Cover,
    /// Scale to fit within area, may show background color
    Contain,
    /// Original size
    Auto,
    /// Fixed dimensions (width, height in pixels)
    Fixed(u32, u32),
}

/// Background image positioning (CSS background-position)
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum BackgroundPosition {
    /// Center in both axes
    #[default]
    Center,
    /// Top-left corner
    TopLeft,
    /// Top center
    Top,
    /// Top-right corner
    TopRight,
    /// Left center
    Left,
    /// Right center
    Right,
    /// Bottom-left corner
    BottomLeft,
    /// Bottom center
    Bottom,
    /// Bottom-right corner
    BottomRight,
    /// Custom position as percentages (0.0 = left/top, 1.0 = right/bottom)
    Percent(f32, f32),
}

/// Background image repeat mode (CSS background-repeat)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BackgroundRepeat {
    /// Don't repeat
    #[default]
    NoRepeat,
    /// Repeat in both directions
    Repeat,
    /// Repeat horizontally only
    RepeatX,
    /// Repeat vertically only
    RepeatY,
}

/// Background image configuration
#[derive(Debug, Clone, Default)]
pub struct BackgroundImage {
    /// Path to image file (relative to theme file or absolute)
    pub path: Option<String>,
    /// Base directory for resolving relative paths (typically the theme directory)
    pub base_dir: Option<std::path::PathBuf>,
    /// How to size the image
    pub size: BackgroundSize,
    /// Where to position the image
    pub position: BackgroundPosition,
    /// How to repeat the image
    pub repeat: BackgroundRepeat,
    /// Opacity (0.0 = transparent, 1.0 = opaque)
    pub opacity: f32,
}

impl BackgroundImage {
    pub fn new(path: impl Into<String>) -> Self {
        Self {
            path: Some(path.into()),
            base_dir: None,
            size: BackgroundSize::Cover,
            position: BackgroundPosition::Center,
            repeat: BackgroundRepeat::NoRepeat,
            opacity: 1.0,
        }
    }

    pub fn has_image(&self) -> bool {
        self.path.is_some()
    }

    /// Get the resolved path, handling relative paths against base_dir
    pub fn resolved_path(&self) -> Option<std::path::PathBuf> {
        let path_str = self.path.as_ref()?;
        let path = std::path::Path::new(path_str);

        // If absolute, return as-is
        if path.is_absolute() {
            return Some(path.to_path_buf());
        }

        // If relative, resolve against base_dir
        if let Some(base) = &self.base_dir {
            let resolved = base.join(path);
            return Some(resolved);
        }

        // No base_dir, return as-is (will likely fail to load)
        Some(path.to_path_buf())
    }
}

/// Backdrop grid effect (synthwave floor)
#[derive(Debug, Clone, Copy)]
pub struct GridEffect {
    pub enabled: bool,
    pub color: Color,
    pub spacing: f32,
    pub line_width: f32,
    pub perspective: f32,
    pub horizon: f32,
    pub animation_speed: f32,
    pub glow_radius: f32,
    pub glow_intensity: f32,
    pub vanishing_spread: f32,
    pub curved: bool,
}

impl Default for GridEffect {
    fn default() -> Self {
        Self {
            enabled: true,
            color: Color::rgba(1.0, 0.0, 1.0, 0.3), // magenta
            spacing: 8.0,
            line_width: 1.5,
            perspective: 2.0,
            horizon: 0.35,
            animation_speed: 0.5,
            glow_radius: 0.0,
            glow_intensity: 0.0,
            vanishing_spread: 0.3,
            curved: true,
        }
    }
}

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
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "static" | "none" => Some(Self::Static),
            "up" => Some(Self::Up),
            "down" => Some(Self::Down),
            "left" => Some(Self::Left),
            "right" => Some(Self::Right),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Static => "static",
            Self::Up => "up",
            Self::Down => "down",
            Self::Left => "left",
            Self::Right => "right",
        }
    }
}

/// Backdrop starfield effect (parallax star layers)
#[derive(Debug, Clone, Copy)]
pub struct StarfieldEffect {
    pub enabled: bool,
    pub color: Color,
    pub density: u32,
    pub layers: u32,
    pub speed: f32,
    pub direction: StarDirection,
    pub glow_radius: f32,
    pub glow_intensity: f32,
    pub twinkle: bool,
    pub twinkle_speed: f32,
    pub min_size: f32,
    pub max_size: f32,
}

impl Default for StarfieldEffect {
    fn default() -> Self {
        Self {
            enabled: false,
            color: Color::rgba(1.0, 1.0, 1.0, 1.0), // white
            density: 100,
            layers: 3,
            speed: 0.3,
            direction: StarDirection::Static,
            glow_radius: 0.0,
            glow_intensity: 0.0,
            twinkle: true,
            twinkle_speed: 2.0,
            min_size: 1.0,
            max_size: 3.0,
        }
    }
}

/// Backdrop rain effect (falling raindrops)
#[derive(Debug, Clone, Copy)]
pub struct RainEffect {
    pub enabled: bool,
    pub color: Color,
    pub density: u32,
    pub speed: f32,
    pub angle: f32,
    pub length: f32,
    pub thickness: f32,
    pub glow_radius: f32,
    pub glow_intensity: f32,
}

impl Default for RainEffect {
    fn default() -> Self {
        Self {
            enabled: false,
            color: Color::rgba(0.59, 0.71, 0.86, 0.7), // Light blue-gray
            density: 150,
            speed: 1.0,
            angle: 0.0,
            length: 20.0,
            thickness: 1.5,
            glow_radius: 0.0,
            glow_intensity: 0.0,
        }
    }
}

/// Particle shape type
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ParticleShape {
    #[default]
    Dot,
    Circle,
    Star,
    Heart,
    Sparkle,
}

impl ParticleShape {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "dot" => Some(Self::Dot),
            "circle" => Some(Self::Circle),
            "star" => Some(Self::Star),
            "heart" => Some(Self::Heart),
            "sparkle" => Some(Self::Sparkle),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Dot => "dot",
            Self::Circle => "circle",
            Self::Star => "star",
            Self::Heart => "heart",
            Self::Sparkle => "sparkle",
        }
    }
}

/// Particle behavior/movement pattern
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ParticleBehavior {
    #[default]
    Float,
    Drift,
    Rise,
    Fall,
}

impl ParticleBehavior {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "float" => Some(Self::Float),
            "drift" => Some(Self::Drift),
            "rise" => Some(Self::Rise),
            "fall" => Some(Self::Fall),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Float => "float",
            Self::Drift => "drift",
            Self::Rise => "rise",
            Self::Fall => "fall",
        }
    }
}

/// Backdrop particle effect (floating shapes)
#[derive(Debug, Clone, Copy)]
pub struct ParticleEffect {
    pub enabled: bool,
    pub color: Color,
    pub count: u32,
    pub shape: ParticleShape,
    pub behavior: ParticleBehavior,
    pub size: f32,
    pub speed: f32,
    pub glow_radius: f32,
    pub glow_intensity: f32,
}

impl Default for ParticleEffect {
    fn default() -> Self {
        Self {
            enabled: false,
            color: Color::rgba(1.0, 0.78, 0.86, 0.78), // Soft pink
            count: 50,
            shape: ParticleShape::Dot,
            behavior: ParticleBehavior::Float,
            size: 4.0,
            speed: 0.5,
            glow_radius: 0.0,
            glow_intensity: 0.0,
        }
    }
}

/// Selection appearance
#[derive(Debug, Clone, Copy)]
pub struct SelectionStyle {
    pub background: Color,
    pub foreground: Color,
}

impl Default for SelectionStyle {
    fn default() -> Self {
        Self {
            background: Color::from_hex(0x44475a),
            foreground: Color::from_hex(0xf8f8f2),
        }
    }
}

/// Highlight appearance (search matches)
#[derive(Debug, Clone, Copy)]
pub struct HighlightStyle {
    /// Background for non-active search matches
    pub background: Color,
    /// Text color for highlighted text
    pub foreground: Color,
    /// Background for the current/active search match (brighter)
    pub current_background: Color,
}

impl Default for HighlightStyle {
    fn default() -> Self {
        Self {
            background: Color::from_hex_alpha(0x99803366), // Semi-transparent yellow
            foreground: Color::from_hex(0x1a1a1a),
            current_background: Color::from_hex_alpha(0xe6b800b3), // Brighter yellow for current
        }
    }
}

/// Tab bar styling
#[derive(Debug, Clone, Copy)]
pub struct TabBarStyle {
    /// Tab bar background color
    pub background: Color,
    /// Border color between tabs and terminal content
    pub border_color: Color,
    /// Tab bar height in pixels
    pub height: f32,
    /// Padding around tabs
    pub padding: f32,
}

impl Default for TabBarStyle {
    fn default() -> Self {
        Self {
            background: Color::from_hex(0x1a1a2e),
            border_color: Color::from_hex(0x2a2a3e),
            height: 36.0,
            padding: 4.0,
        }
    }
}

/// Individual tab styling
#[derive(Debug, Clone, Copy)]
pub struct TabStyle {
    /// Tab background color
    pub background: Color,
    /// Tab text color
    pub foreground: Color,
    /// Tab border radius
    pub border_radius: f32,
    /// Tab padding (horizontal)
    pub padding_x: f32,
    /// Tab padding (vertical)
    pub padding_y: f32,
    /// Minimum tab width
    pub min_width: f32,
    /// Maximum tab width
    pub max_width: f32,
    /// Text glow/shadow effect
    pub text_shadow: Option<TextShadow>,
}

impl Default for TabStyle {
    fn default() -> Self {
        Self {
            background: Color::from_hex(0x2a2a3e),
            foreground: Color::from_hex(0x888899),
            border_radius: 4.0,
            padding_x: 12.0,
            padding_y: 6.0,
            min_width: 80.0,
            max_width: 200.0,
            text_shadow: None,
        }
    }
}

/// Active tab styling (inherits from TabStyle where not specified)
#[derive(Debug, Clone, Copy)]
pub struct TabActiveStyle {
    /// Active tab background color
    pub background: Color,
    /// Active tab text color
    pub foreground: Color,
    /// Accent color (underline or highlight)
    pub accent: Color,
    /// Text glow/shadow effect for active tab
    pub text_shadow: Option<TextShadow>,
}

impl Default for TabActiveStyle {
    fn default() -> Self {
        Self {
            background: Color::from_hex(0x3a3a4e),
            foreground: Color::from_hex(0xf8f8f2),
            accent: Color::from_hex(0x00ffff),
            text_shadow: None,
        }
    }
}

/// Tab close button styling
#[derive(Debug, Clone, Copy)]
pub struct TabCloseStyle {
    /// Close button background (normal)
    pub background: Color,
    /// Close button icon color (normal)
    pub foreground: Color,
    /// Close button background (hover)
    pub hover_background: Color,
    /// Close button icon color (hover)
    pub hover_foreground: Color,
    /// Close button size
    pub size: f32,
}

impl Default for TabCloseStyle {
    fn default() -> Self {
        Self {
            background: Color::rgba(0.0, 0.0, 0.0, 0.0),
            foreground: Color::from_hex(0x666677),
            hover_background: Color::from_hex(0xff5555),
            hover_foreground: Color::from_hex(0xffffff),
            size: 16.0,
        }
    }
}

/// Complete tab theme
#[derive(Debug, Clone, Copy, Default)]
pub struct TabTheme {
    pub bar: TabBarStyle,
    pub tab: TabStyle,
    pub active: TabActiveStyle,
    pub close: TabCloseStyle,
}

/// Complete terminal theme
#[derive(Debug, Clone)]
pub struct Theme {
    // Typography
    pub typography: Typography,

    // Colors
    pub foreground: Color,
    pub background: LinearGradient,
    pub background_image: Option<BackgroundImage>,
    pub palette: AnsiPalette,

    // States
    pub selection: SelectionStyle,
    pub highlight: HighlightStyle,
    pub cursor_color: Color,

    // Effects
    pub text_shadow: Option<TextShadow>,
    pub grid: Option<GridEffect>,
    pub starfield: Option<StarfieldEffect>,
    pub rain: Option<RainEffect>,
    pub particles: Option<ParticleEffect>,

    // Tab styling
    pub tabs: TabTheme,
}

impl Default for Theme {
    fn default() -> Self {
        Self::synthwave()
    }
}

impl Theme {
    /// Synthwave theme - the default extra AF experience
    pub fn synthwave() -> Self {
        Self {
            typography: Typography {
                font_family: vec![
                    "JetBrains Mono".to_string(),
                    "Fira Code".to_string(),
                    "monospace".to_string(),
                ],
                font_size: 14.0,
                line_height: 1.3,
                font_bold: Some("JetBrains Mono Bold".to_string()),
                font_italic: Some("JetBrains Mono Italic".to_string()),
                font_bold_italic: Some("JetBrains Mono Bold Italic".to_string()),
                ligatures: true,
            },
            foreground: Color::from_hex(0xc8c8c8),
            background: LinearGradient::default(),
            background_image: None,
            palette: AnsiPalette::default(),
            selection: SelectionStyle::default(),
            highlight: HighlightStyle::default(),
            cursor_color: Color::from_hex(0x00ffff),
            text_shadow: Some(TextShadow::default()),
            grid: Some(GridEffect::default()),
            starfield: None,
            rain: None,
            particles: None,
            tabs: TabTheme::default(),
        }
    }

    /// Minimal theme - no effects, just colors
    pub fn minimal() -> Self {
        Self {
            typography: Typography::default(),
            foreground: Color::from_hex(0xc8c8c8),
            background: LinearGradient {
                top: Color::from_hex(0x1a1a1a),
                bottom: Color::from_hex(0x1a1a1a),
            },
            background_image: None,
            palette: AnsiPalette::default(),
            selection: SelectionStyle::default(),
            highlight: HighlightStyle::default(),
            cursor_color: Color::from_hex(0xffffff),
            text_shadow: None,
            grid: None,
            starfield: None,
            rain: None,
            particles: None,
            tabs: TabTheme::default(),
        }
    }

    /// Load theme from CSS string
    pub fn from_css(css: &str) -> Result<Self, parser::ThemeParseError> {
        parser::parse_theme(css)
    }

    /// Load theme from CSS string with base directory for resolving relative paths
    pub fn from_css_with_base(css: &str, base_dir: impl AsRef<Path>) -> Result<Self, parser::ThemeParseError> {
        let mut theme = parser::parse_theme(css)?;
        // Set base_dir on background_image if present
        if let Some(ref mut bg) = theme.background_image {
            bg.base_dir = Some(base_dir.as_ref().to_path_buf());
        }
        Ok(theme)
    }

    /// Load theme from CSS file
    pub fn from_css_file(path: impl AsRef<Path>) -> Result<Self, Box<dyn std::error::Error>> {
        let path_ref = path.as_ref();
        let css = std::fs::read_to_string(path_ref)?;
        let base_dir = path_ref.parent().unwrap_or(Path::new("."));
        Ok(Self::from_css_with_base(&css, base_dir)?)
    }

    /// Convert theme to GPU-ready uniforms
    pub fn to_uniforms(&self, screen_width: f32, screen_height: f32, time: f32) -> ThemeUniforms {
        let grid = self.grid.unwrap_or(GridEffect {
            enabled: false,
            ..Default::default()
        });
        let glow = self.text_shadow.unwrap_or(TextShadow {
            radius: 0.0,
            intensity: 0.0,
            color: Color::rgba(0.0, 0.0, 0.0, 0.0),
        });

        ThemeUniforms {
            screen_size: [screen_width, screen_height],
            time,
            grid_intensity: if grid.enabled { 1.0 } else { 0.0 },
            gradient_top: self.background.top.to_array(),
            gradient_bottom: self.background.bottom.to_array(),
            grid_color: grid.color.to_array(),
            grid_spacing: grid.spacing,
            grid_line_width: grid.line_width,
            grid_perspective: grid.perspective,
            grid_horizon: grid.horizon,
            glow_color: glow.color.to_array(),
            glow_radius: glow.radius,
            glow_intensity: glow.intensity,
            _pad1: [0.0; 2],
            text_color: self.foreground.to_array(),
            _pad2: [0.0; 4],
        }
    }
}

/// GPU-ready uniform buffer matching WGSL Params struct
/// WGSL requires vec4 to be 16-byte aligned, so we add explicit padding
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct ThemeUniforms {
    pub screen_size: [f32; 2],      // offset 0
    pub time: f32,                   // offset 8
    pub grid_intensity: f32,         // offset 12
    pub gradient_top: [f32; 4],      // offset 16 (16-byte aligned)
    pub gradient_bottom: [f32; 4],   // offset 32
    pub grid_color: [f32; 4],        // offset 48
    pub grid_spacing: f32,           // offset 64
    pub grid_line_width: f32,        // offset 68
    pub grid_perspective: f32,       // offset 72
    pub grid_horizon: f32,           // offset 76
    pub glow_color: [f32; 4],        // offset 80 (16-byte aligned, 80 % 16 == 0)
    pub glow_radius: f32,            // offset 96
    pub glow_intensity: f32,         // offset 100
    pub _pad1: [f32; 2],             // offset 104 - padding to align text_color to 16 bytes
    pub text_color: [f32; 4],        // offset 112 (16-byte aligned)
    pub _pad2: [f32; 4],             // offset 128 - final padding
    // Total: 144 bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_from_hex() {
        let c = Color::from_hex(0xff5555);
        assert!((c.r - 1.0).abs() < 0.01);
        assert!((c.g - 0.333).abs() < 0.01);
        assert!((c.b - 0.333).abs() < 0.01);
    }

    #[test]
    fn test_theme_uniforms_size() {
        // Ensure uniform buffer matches WGSL layout (144 bytes)
        assert_eq!(std::mem::size_of::<ThemeUniforms>(), 144);
    }

    #[test]
    fn test_ansi_palette_get() {
        let palette = AnsiPalette::default();
        let red = palette.get(1);
        assert!((red.r - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_ansi_palette_extended() {
        let mut palette = AnsiPalette::default();

        // No extended colors initially
        assert!(palette.get_extended(226).is_none());
        assert!(!palette.has_extended(226));

        // Set an extended color
        let green = Color::from_hex(0x61fe71);
        palette.set_extended(226, green);

        // Now it should be present
        assert!(palette.has_extended(226));
        let retrieved = palette.get_extended(226);
        assert!(retrieved.is_some());
        let c = retrieved.unwrap();
        assert!((c.r - green.r).abs() < 0.01);
        assert!((c.g - green.g).abs() < 0.01);
        assert!((c.b - green.b).abs() < 0.01);

        // Base colors (0-15) should not be set via set_extended
        palette.set_extended(5, green);
        assert!(!palette.has_extended(5));
    }

    #[test]
    fn test_calculate_extended() {
        // Test color cube calculation (16-231)
        // Color 16 is rgb(0,0,0) in the cube
        let c16 = AnsiPalette::calculate_extended(16);
        assert!((c16.r - 0.0).abs() < 0.01);
        assert!((c16.g - 0.0).abs() < 0.01);
        assert!((c16.b - 0.0).abs() < 0.01);

        // Color 226 is yellow in standard 256-color palette
        // Index 226-16 = 210, r=210/36=5, g=(210/6)%6=5, b=210%6=0
        let c226 = AnsiPalette::calculate_extended(226);
        assert!(c226.r > 0.9); // Should be bright red component
        assert!(c226.g > 0.9); // Should be bright green component
        assert!((c226.b - 0.0).abs() < 0.01); // No blue

        // Test grayscale (232-255)
        let c232 = AnsiPalette::calculate_extended(232);
        assert!((c232.r - c232.g).abs() < 0.01); // Should be gray
        assert!((c232.g - c232.b).abs() < 0.01);
        assert!(c232.r < 0.1); // Dark gray

        let c255 = AnsiPalette::calculate_extended(255);
        assert!((c255.r - c255.g).abs() < 0.01); // Should be gray
        assert!(c255.r > 0.8); // Light gray
    }
}
