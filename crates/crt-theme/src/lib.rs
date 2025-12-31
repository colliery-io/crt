//! CRT Theme - CSS-like theming engine
//!
//! Provides theme structures that map to shader uniforms for terminal rendering.

#![allow(clippy::collapsible_if)]
#![allow(clippy::should_implement_trait)]

pub mod parser;

use bytemuck::{Pod, Zeroable};
use std::path::Path;

/// Trait for patch structs that can be merged (CSS cascade: later values win)
pub trait Mergeable {
    /// Merge another instance into self. Fields with Some() values in other override self.
    fn merge(&mut self, other: Self);
}

/// Helper function to merge optional patch structs
pub fn merge_optional_patch<T: Mergeable + Default>(target: &mut Option<T>, source: Option<T>) {
    if let Some(source_patch) = source {
        target.get_or_insert_with(T::default).merge(source_patch);
    }
}

/// Macro to implement Mergeable for structs with all-Option fields
macro_rules! impl_mergeable {
    ($type:ty, $($field:ident),+ $(,)?) => {
        impl Mergeable for $type {
            fn merge(&mut self, other: Self) {
                $(
                    if other.$field.is_some() {
                        self.$field = other.$field;
                    }
                )+
            }
        }
    };
}

/// Trait for converting patch/theme structs to effect config key-value pairs
pub trait ToEffectConfig {
    /// Convert to a vector of (key, value) pairs for effect configuration
    fn to_config_pairs(&self) -> Vec<(&'static str, String)>;
}

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

    /// Convert to CSS rgba() string format for effect configs
    pub fn to_rgba_string(&self) -> String {
        format!(
            "rgba({}, {}, {}, {})",
            (self.r * 255.0) as u8,
            (self.g * 255.0) as u8,
            (self.b * 255.0) as u8,
            self.a
        )
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
            let to_float = |v: u8| {
                if v == 0 {
                    0.0
                } else {
                    (v as f32 * 40.0 + 55.0) / 255.0
                }
            };
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
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum BackgroundSize {
    /// Scale to cover the entire area, may crop
    #[default]
    Cover,
    /// Scale to fit within area, may show background color
    Contain,
    /// Original pixel size (does not scale with window)
    Auto,
    /// Fixed dimensions (width, height in pixels)
    Fixed(u32, u32),
    /// Percentage of canvas width (scales with window, maintains aspect ratio)
    CanvasPercent(f32),
    /// Scale relative to original image size (1.0 = original, 0.5 = half, 2.0 = double)
    /// Does not scale with window resize
    ImageScale(f32),
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

/// Cursor shape for theme overrides
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CursorShape {
    /// Solid block cursor
    #[default]
    Block,
    /// Vertical bar cursor
    Bar,
    /// Horizontal underline cursor
    Underline,
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

/// Backdrop matrix effect (falling code columns)
#[derive(Debug, Clone)]
pub struct MatrixEffect {
    pub enabled: bool,
    pub color: Color,
    pub density: f32,
    pub speed: f32,
    pub font_size: f32,
    pub charset: String,
}

impl Default for MatrixEffect {
    fn default() -> Self {
        Self {
            enabled: false,
            color: Color::rgb(0.0, 1.0, 0.27), // Classic matrix green
            density: 1.0,
            speed: 8.0,
            font_size: 14.0,
            charset: String::new(), // Empty means use default katakana + numbers
        }
    }
}

/// Shape type for geometric effect
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ShapeType {
    #[default]
    Circle,
    Rect,
    Ellipse,
    Triangle,
    Star,
    Heart,
    Polygon,
}

impl ShapeType {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "circle" => Some(Self::Circle),
            "rect" | "rectangle" => Some(Self::Rect),
            "ellipse" => Some(Self::Ellipse),
            "triangle" => Some(Self::Triangle),
            "star" => Some(Self::Star),
            "heart" => Some(Self::Heart),
            "polygon" => Some(Self::Polygon),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Circle => "circle",
            Self::Rect => "rect",
            Self::Ellipse => "ellipse",
            Self::Triangle => "triangle",
            Self::Star => "star",
            Self::Heart => "heart",
            Self::Polygon => "polygon",
        }
    }
}

/// Rotation behavior for shape effect
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ShapeRotation {
    #[default]
    None,
    Spin,
    Wobble,
}

impl ShapeRotation {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "none" => Some(Self::None),
            "spin" => Some(Self::Spin),
            "wobble" => Some(Self::Wobble),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Spin => "spin",
            Self::Wobble => "wobble",
        }
    }
}

/// Motion behavior for shape effect
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ShapeMotion {
    #[default]
    None,
    Bounce,
    Scroll,
    Float,
    Orbit,
}

impl ShapeMotion {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "none" => Some(Self::None),
            "bounce" => Some(Self::Bounce),
            "scroll" => Some(Self::Scroll),
            "float" => Some(Self::Float),
            "orbit" => Some(Self::Orbit),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Bounce => "bounce",
            Self::Scroll => "scroll",
            Self::Float => "float",
            Self::Orbit => "orbit",
        }
    }
}

/// Backdrop shape effect (single geometric shape with motion)
#[derive(Debug, Clone)]
pub struct ShapeEffect {
    pub enabled: bool,
    pub shape_type: ShapeType,
    pub size: f32,
    pub fill: Option<Color>,
    pub stroke: Option<Color>,
    pub stroke_width: f32,
    pub glow_radius: f32,
    pub glow_color: Option<Color>,
    pub rotation: ShapeRotation,
    pub rotation_speed: f32,
    pub motion: ShapeMotion,
    pub motion_speed: f32,
    pub polygon_sides: u32,
}

impl Default for ShapeEffect {
    fn default() -> Self {
        Self {
            enabled: false,
            shape_type: ShapeType::Circle,
            size: 100.0,
            fill: Some(Color::rgba(1.0, 0.4, 0.6, 0.8)),
            stroke: None,
            stroke_width: 2.0,
            glow_radius: 0.0,
            glow_color: None,
            rotation: ShapeRotation::None,
            rotation_speed: 1.0,
            motion: ShapeMotion::Bounce,
            motion_speed: 1.0,
            polygon_sides: 6,
        }
    }
}

/// Motion behavior for sprite effect
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum SpriteMotion {
    #[default]
    None,
    Bounce,
    Scroll,
    Float,
    Orbit,
}

impl SpriteMotion {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "none" => Some(Self::None),
            "bounce" => Some(Self::Bounce),
            "scroll" => Some(Self::Scroll),
            "float" => Some(Self::Float),
            "orbit" => Some(Self::Orbit),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Bounce => "bounce",
            Self::Scroll => "scroll",
            Self::Float => "float",
            Self::Orbit => "orbit",
        }
    }
}

/// Static position for sprite effect
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum SpritePosition {
    #[default]
    Center,
    TopLeft,
    Top,
    TopRight,
    Left,
    Right,
    BottomLeft,
    Bottom,
    BottomRight,
}

impl SpritePosition {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().replace(['-', '_'], " ").trim() {
            "center" => Some(Self::Center),
            "top left" | "topleft" => Some(Self::TopLeft),
            "top" => Some(Self::Top),
            "top right" | "topright" => Some(Self::TopRight),
            "left" => Some(Self::Left),
            "right" => Some(Self::Right),
            "bottom left" | "bottomleft" => Some(Self::BottomLeft),
            "bottom" => Some(Self::Bottom),
            "bottom right" | "bottomright" => Some(Self::BottomRight),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Center => "center",
            Self::TopLeft => "top-left",
            Self::Top => "top",
            Self::TopRight => "top-right",
            Self::Left => "left",
            Self::Right => "right",
            Self::BottomLeft => "bottom-left",
            Self::Bottom => "bottom",
            Self::BottomRight => "bottom-right",
        }
    }
}

/// CRT post-processing effect (scanlines, curvature, vignette)
#[derive(Debug, Clone, Copy)]
pub struct CrtEffect {
    pub enabled: bool,
    /// Scanline intensity (0.0 = none, 1.0 = very dark lines)
    pub scanline_intensity: f32,
    /// Scanline frequency (lines per pixel height)
    pub scanline_frequency: f32,
    /// Screen curvature/barrel distortion (0.0 = flat, 0.1 = slight curve)
    pub curvature: f32,
    /// Vignette intensity (0.0 = none, 1.0 = strong darkening at edges)
    pub vignette: f32,
    /// Chromatic aberration strength (0.0 = none, higher = more RGB separation)
    pub chromatic_aberration: f32,
    /// Bloom/glow intensity for bright areas
    pub bloom: f32,
    /// Flicker intensity (0.0 = none, subtle screen flicker)
    pub flicker: f32,
}

impl Default for CrtEffect {
    fn default() -> Self {
        Self {
            enabled: false,
            scanline_intensity: 0.15,
            scanline_frequency: 2.0,
            curvature: 0.02,
            vignette: 0.3,
            chromatic_aberration: 0.0,
            bloom: 0.0,
            flicker: 0.0,
        }
    }
}

/// Backdrop sprite effect (animated sprite sheet)
#[derive(Debug, Clone)]
pub struct SpriteEffect {
    pub enabled: bool,
    /// Path to sprite sheet image
    pub path: Option<String>,
    /// Base directory for resolving relative paths (typically the theme directory)
    pub base_dir: Option<std::path::PathBuf>,
    /// Width of each frame in pixels
    pub frame_width: u32,
    /// Height of each frame in pixels
    pub frame_height: u32,
    /// Number of columns in sprite sheet
    pub columns: u32,
    /// Number of rows in sprite sheet
    pub rows: u32,
    /// Total frame count (defaults to columns * rows)
    pub frame_count: Option<u32>,
    /// Animation frames per second
    pub fps: f32,
    /// Display scale
    pub scale: f32,
    /// Opacity (0.0-1.0)
    pub opacity: f32,
    /// Motion type
    pub motion: SpriteMotion,
    /// Motion speed multiplier
    pub motion_speed: f32,
    /// Static position (used when motion is None)
    pub position: SpritePosition,
}

impl Default for SpriteEffect {
    fn default() -> Self {
        Self {
            enabled: false,
            path: None,
            base_dir: None,
            frame_width: 64,
            frame_height: 64,
            columns: 1,
            rows: 1,
            frame_count: None,
            fps: 12.0,
            scale: 1.0,
            opacity: 1.0,
            motion: SpriteMotion::None,
            motion_speed: 1.0,
            position: SpritePosition::Center,
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
    /// Padding below tab bar (space before terminal content)
    pub content_padding: f32,
}

impl Default for TabBarStyle {
    fn default() -> Self {
        Self {
            background: Color::from_hex(0x1a1a2e),
            border_color: Color::from_hex(0x2a2a3e),
            height: 36.0,
            padding: 4.0,
            content_padding: 4.0,
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

/// Focus indicator styling (accessibility)
#[derive(Debug, Clone, Copy)]
pub struct FocusStyle {
    /// Focus ring color (bright, high contrast)
    pub ring_color: Color,
    /// Focus glow color (softer, more transparent)
    pub glow_color: Color,
    /// Focus ring thickness in pixels (before scale factor)
    pub ring_thickness: f32,
    /// Focus glow size in pixels (before scale factor)
    pub glow_size: f32,
}

impl Default for FocusStyle {
    fn default() -> Self {
        Self {
            ring_color: Color::rgba(0.4, 0.6, 0.9, 1.0), // Bright blue
            glow_color: Color::rgba(0.3, 0.5, 0.8, 0.4), // Soft blue glow
            ring_thickness: 2.0,
            glow_size: 4.0,
        }
    }
}

/// Hover state styling
#[derive(Debug, Clone, Copy)]
pub struct HoverStyle {
    /// Background color for hover state
    pub background: Color,
}

impl Default for HoverStyle {
    fn default() -> Self {
        Self {
            background: Color::rgba(0.25, 0.35, 0.5, 0.8),
        }
    }
}

/// Context menu styling
#[derive(Debug, Clone, Copy)]
pub struct ContextMenuStyle {
    /// Menu background color
    pub background: Color,
    /// Menu border color
    pub border_color: Color,
    /// Menu item text color
    pub text_color: Color,
    /// Keyboard shortcut text color
    pub shortcut_color: Color,
}

impl Default for ContextMenuStyle {
    fn default() -> Self {
        Self {
            background: Color::rgba(0.12, 0.12, 0.15, 0.98),
            border_color: Color::rgba(0.3, 0.3, 0.35, 0.8),
            text_color: Color::rgba(0.9, 0.9, 0.9, 1.0),
            shortcut_color: Color::rgba(0.5, 0.5, 0.55, 1.0),
        }
    }
}

/// Search bar styling
#[derive(Debug, Clone, Copy)]
pub struct SearchBarStyle {
    /// Search bar background color
    pub background: Color,
    /// Placeholder text color
    pub placeholder_color: Color,
    /// Input text color
    pub text_color: Color,
    /// No matches text color
    pub no_match_color: Color,
}

impl Default for SearchBarStyle {
    fn default() -> Self {
        Self {
            background: Color::rgba(0.15, 0.15, 0.2, 0.95),
            placeholder_color: Color::rgba(0.5, 0.5, 0.5, 0.8),
            text_color: Color::rgba(0.9, 0.9, 0.9, 1.0),
            no_match_color: Color::rgba(0.9, 0.5, 0.5, 1.0),
        }
    }
}

/// Window rename bar styling
#[derive(Debug, Clone, Copy)]
pub struct RenameBarStyle {
    /// Rename bar background color
    pub background: Color,
    /// Label text color ("Rename:")
    pub label_color: Color,
    /// Input text color
    pub text_color: Color,
}

impl Default for RenameBarStyle {
    fn default() -> Self {
        Self {
            background: Color::rgba(0.15, 0.15, 0.2, 0.98),
            label_color: Color::rgba(0.6, 0.6, 0.7, 1.0),
            text_color: Color::rgba(0.95, 0.95, 0.95, 1.0),
        }
    }
}

/// Complete UI styling (overlays, menus, focus indicators)
#[derive(Debug, Clone, Copy, Default)]
pub struct UiStyle {
    pub focus: FocusStyle,
    pub hover: HoverStyle,
    pub context_menu: ContextMenuStyle,
    pub search_bar: SearchBarStyle,
    pub rename_bar: RenameBarStyle,
}

// ============================================================================
// Event-Driven Theming Types
// ============================================================================

/// Terminal events that can trigger theme overrides
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TerminalEvent {
    /// Bell character (BEL, 0x07)
    Bell,
    /// Command completed successfully (OSC 133;D;0)
    CommandSuccess,
    /// Command failed with exit code (OSC 133;D;N where N != 0)
    CommandFail(i32),
    /// Window gained focus
    FocusGained,
    /// Window lost focus
    FocusLost,
}

/// Position for overlay sprites during events
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum SpriteOverlayPosition {
    /// Center of viewport
    #[default]
    Center,
    /// At text cursor position
    Cursor,
    /// At current backdrop sprite position (tracks if moving)
    Sprite,
    /// Random position in viewport
    Random,
}

impl SpriteOverlayPosition {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "center" => Some(Self::Center),
            "cursor" => Some(Self::Cursor),
            "sprite" => Some(Self::Sprite),
            "random" => Some(Self::Random),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Center => "center",
            Self::Cursor => "cursor",
            Self::Sprite => "sprite",
            Self::Random => "random",
        }
    }
}

/// Patches to existing backdrop sprite - keeps position and motion
/// All fields are Optional; None means "keep current value"
#[derive(Debug, Clone, Default)]
pub struct SpritePatch {
    /// Override sprite sheet path
    pub path: Option<String>,
    /// Override number of columns in sprite sheet
    pub columns: Option<u32>,
    /// Override number of rows in sprite sheet
    pub rows: Option<u32>,
    /// Override animation frames per second
    pub fps: Option<f32>,
    /// Override opacity (0.0-1.0)
    pub opacity: Option<f32>,
    /// Override display scale
    pub scale: Option<f32>,
    /// Override motion speed multiplier
    pub motion_speed: Option<f32>,
}

/// Patches to starfield effect - all fields Optional; None means "keep current value"
#[derive(Debug, Clone, Default)]
pub struct StarfieldPatch {
    /// Override star color
    pub color: Option<Color>,
    /// Override star density
    pub density: Option<u32>,
    /// Override number of parallax layers
    pub layers: Option<u32>,
    /// Override movement speed
    pub speed: Option<f32>,
    /// Override movement direction
    pub direction: Option<StarDirection>,
    /// Override glow radius
    pub glow_radius: Option<f32>,
    /// Override glow intensity
    pub glow_intensity: Option<f32>,
    /// Override twinkle enabled
    pub twinkle: Option<bool>,
    /// Override twinkle speed
    pub twinkle_speed: Option<f32>,
    /// Override minimum star size
    pub min_size: Option<f32>,
    /// Override maximum star size
    pub max_size: Option<f32>,
}

/// Patches to particle effect - all fields Optional; None means "keep current value"
#[derive(Debug, Clone, Default)]
pub struct ParticlePatch {
    /// Override particle color
    pub color: Option<Color>,
    /// Override particle count
    pub count: Option<u32>,
    /// Override particle shape
    pub shape: Option<ParticleShape>,
    /// Override particle behavior
    pub behavior: Option<ParticleBehavior>,
    /// Override particle size
    pub size: Option<f32>,
    /// Override movement speed
    pub speed: Option<f32>,
    /// Override glow radius
    pub glow_radius: Option<f32>,
    /// Override glow intensity
    pub glow_intensity: Option<f32>,
}

/// Patches to grid effect - all fields Optional; None means "keep current value"
#[derive(Debug, Clone, Default)]
pub struct GridPatch {
    /// Override grid color
    pub color: Option<Color>,
    /// Override grid spacing
    pub spacing: Option<f32>,
    /// Override line width
    pub line_width: Option<f32>,
    /// Override perspective amount
    pub perspective: Option<f32>,
    /// Override horizon position
    pub horizon: Option<f32>,
    /// Override animation speed
    pub animation_speed: Option<f32>,
    /// Override glow radius
    pub glow_radius: Option<f32>,
    /// Override glow intensity
    pub glow_intensity: Option<f32>,
    /// Override vanishing spread
    pub vanishing_spread: Option<f32>,
    /// Override curved mode
    pub curved: Option<bool>,
}

/// Patches to rain effect - all fields Optional; None means "keep current value"
#[derive(Debug, Clone, Default)]
pub struct RainPatch {
    /// Override rain color
    pub color: Option<Color>,
    /// Override raindrop density
    pub density: Option<u32>,
    /// Override fall speed
    pub speed: Option<f32>,
    /// Override fall angle
    pub angle: Option<f32>,
    /// Override raindrop length
    pub length: Option<f32>,
    /// Override raindrop thickness
    pub thickness: Option<f32>,
    /// Override glow radius
    pub glow_radius: Option<f32>,
    /// Override glow intensity
    pub glow_intensity: Option<f32>,
}

/// Patches to matrix effect - all fields Optional; None means "keep current value"
#[derive(Debug, Clone, Default)]
pub struct MatrixPatch {
    /// Override matrix color
    pub color: Option<Color>,
    /// Override column density
    pub density: Option<f32>,
    /// Override fall speed
    pub speed: Option<f32>,
    /// Override font size
    pub font_size: Option<f32>,
    /// Override character set
    pub charset: Option<String>,
}

/// Patches to shape effect - all fields Optional; None means "keep current value"
#[derive(Debug, Clone, Default)]
pub struct ShapePatch {
    /// Override shape type
    pub shape_type: Option<ShapeType>,
    /// Override size
    pub size: Option<f32>,
    /// Override fill color
    pub fill: Option<Color>,
    /// Override stroke color
    pub stroke: Option<Color>,
    /// Override stroke width
    pub stroke_width: Option<f32>,
    /// Override glow radius
    pub glow_radius: Option<f32>,
    /// Override glow color
    pub glow_color: Option<Color>,
    /// Override rotation mode
    pub rotation: Option<ShapeRotation>,
    /// Override rotation speed
    pub rotation_speed: Option<f32>,
    /// Override motion mode
    pub motion: Option<ShapeMotion>,
    /// Override motion speed
    pub motion_speed: Option<f32>,
    /// Override polygon sides
    pub polygon_sides: Option<u32>,
}

// Implement Mergeable for all patch structs
impl_mergeable!(
    SpritePatch,
    path,
    columns,
    rows,
    fps,
    opacity,
    scale,
    motion_speed
);
impl_mergeable!(
    StarfieldPatch,
    color,
    density,
    layers,
    speed,
    direction,
    glow_radius,
    glow_intensity,
    twinkle,
    twinkle_speed,
    min_size,
    max_size
);
impl_mergeable!(
    ParticlePatch,
    color,
    count,
    shape,
    behavior,
    size,
    speed,
    glow_radius,
    glow_intensity
);
impl_mergeable!(
    GridPatch,
    color,
    spacing,
    line_width,
    perspective,
    horizon,
    animation_speed,
    glow_radius,
    glow_intensity,
    vanishing_spread,
    curved
);
impl_mergeable!(
    RainPatch,
    color,
    density,
    speed,
    angle,
    length,
    thickness,
    glow_radius,
    glow_intensity
);
impl_mergeable!(MatrixPatch, color, density, speed, font_size, charset);
impl_mergeable!(
    ShapePatch,
    shape_type,
    size,
    fill,
    stroke,
    stroke_width,
    glow_radius,
    glow_color,
    rotation,
    rotation_speed,
    motion,
    motion_speed,
    polygon_sides
);

// Implement ToEffectConfig for all patch structs
impl ToEffectConfig for StarfieldPatch {
    fn to_config_pairs(&self) -> Vec<(&'static str, String)> {
        let mut pairs = Vec::new();
        if let Some(ref color) = self.color {
            pairs.push(("color", color.to_rgba_string()));
        }
        if let Some(density) = self.density {
            pairs.push(("density", density.to_string()));
        }
        if let Some(layers) = self.layers {
            pairs.push(("layers", layers.to_string()));
        }
        if let Some(speed) = self.speed {
            pairs.push(("speed", speed.to_string()));
        }
        if let Some(direction) = self.direction {
            pairs.push(("direction", direction.as_str().to_string()));
        }
        if let Some(glow_radius) = self.glow_radius {
            pairs.push(("glow-radius", glow_radius.to_string()));
        }
        if let Some(glow_intensity) = self.glow_intensity {
            pairs.push(("glow-intensity", glow_intensity.to_string()));
        }
        if let Some(twinkle) = self.twinkle {
            pairs.push((
                "twinkle",
                if twinkle { "true" } else { "false" }.to_string(),
            ));
        }
        if let Some(twinkle_speed) = self.twinkle_speed {
            pairs.push(("twinkle-speed", twinkle_speed.to_string()));
        }
        if let Some(min_size) = self.min_size {
            pairs.push(("min-size", min_size.to_string()));
        }
        if let Some(max_size) = self.max_size {
            pairs.push(("max-size", max_size.to_string()));
        }
        pairs
    }
}

impl ToEffectConfig for ParticlePatch {
    fn to_config_pairs(&self) -> Vec<(&'static str, String)> {
        let mut pairs = Vec::new();
        if let Some(ref color) = self.color {
            pairs.push(("color", color.to_rgba_string()));
        }
        if let Some(count) = self.count {
            pairs.push(("count", count.to_string()));
        }
        if let Some(shape) = self.shape {
            pairs.push(("shape", shape.as_str().to_string()));
        }
        if let Some(behavior) = self.behavior {
            pairs.push(("behavior", behavior.as_str().to_string()));
        }
        if let Some(size) = self.size {
            pairs.push(("size", size.to_string()));
        }
        if let Some(speed) = self.speed {
            pairs.push(("speed", speed.to_string()));
        }
        if let Some(glow_radius) = self.glow_radius {
            pairs.push(("glow-radius", glow_radius.to_string()));
        }
        if let Some(glow_intensity) = self.glow_intensity {
            pairs.push(("glow-intensity", glow_intensity.to_string()));
        }
        pairs
    }
}

impl ToEffectConfig for GridPatch {
    fn to_config_pairs(&self) -> Vec<(&'static str, String)> {
        let mut pairs = Vec::new();
        if let Some(ref color) = self.color {
            pairs.push(("color", color.to_rgba_string()));
        }
        if let Some(spacing) = self.spacing {
            pairs.push(("spacing", spacing.to_string()));
        }
        if let Some(line_width) = self.line_width {
            pairs.push(("line-width", line_width.to_string()));
        }
        if let Some(perspective) = self.perspective {
            pairs.push(("perspective", perspective.to_string()));
        }
        if let Some(horizon) = self.horizon {
            pairs.push(("horizon", horizon.to_string()));
        }
        if let Some(animation_speed) = self.animation_speed {
            pairs.push(("animation-speed", animation_speed.to_string()));
        }
        if let Some(glow_radius) = self.glow_radius {
            pairs.push(("glow-radius", glow_radius.to_string()));
        }
        if let Some(glow_intensity) = self.glow_intensity {
            pairs.push(("glow-intensity", glow_intensity.to_string()));
        }
        if let Some(vanishing_spread) = self.vanishing_spread {
            pairs.push(("vanishing-spread", vanishing_spread.to_string()));
        }
        if let Some(curved) = self.curved {
            pairs.push(("curved", if curved { "true" } else { "false" }.to_string()));
        }
        pairs
    }
}

impl ToEffectConfig for RainPatch {
    fn to_config_pairs(&self) -> Vec<(&'static str, String)> {
        let mut pairs = Vec::new();
        if let Some(ref color) = self.color {
            pairs.push(("color", color.to_rgba_string()));
        }
        if let Some(density) = self.density {
            pairs.push(("density", density.to_string()));
        }
        if let Some(speed) = self.speed {
            pairs.push(("speed", speed.to_string()));
        }
        if let Some(angle) = self.angle {
            pairs.push(("angle", angle.to_string()));
        }
        if let Some(length) = self.length {
            pairs.push(("length", length.to_string()));
        }
        if let Some(thickness) = self.thickness {
            pairs.push(("thickness", thickness.to_string()));
        }
        if let Some(glow_radius) = self.glow_radius {
            pairs.push(("glow-radius", glow_radius.to_string()));
        }
        if let Some(glow_intensity) = self.glow_intensity {
            pairs.push(("glow-intensity", glow_intensity.to_string()));
        }
        pairs
    }
}

impl ToEffectConfig for MatrixPatch {
    fn to_config_pairs(&self) -> Vec<(&'static str, String)> {
        let mut pairs = Vec::new();
        if let Some(ref color) = self.color {
            pairs.push(("color", color.to_rgba_string()));
        }
        if let Some(density) = self.density {
            pairs.push(("density", density.to_string()));
        }
        if let Some(speed) = self.speed {
            pairs.push(("speed", speed.to_string()));
        }
        if let Some(font_size) = self.font_size {
            pairs.push(("font-size", font_size.to_string()));
        }
        if let Some(ref charset) = self.charset {
            pairs.push(("charset", charset.clone()));
        }
        pairs
    }
}

impl ToEffectConfig for ShapePatch {
    fn to_config_pairs(&self) -> Vec<(&'static str, String)> {
        let mut pairs = Vec::new();
        if let Some(shape_type) = self.shape_type {
            pairs.push(("type", shape_type.as_str().to_string()));
        }
        if let Some(size) = self.size {
            pairs.push(("size", size.to_string()));
        }
        if let Some(ref fill) = self.fill {
            pairs.push(("fill", fill.to_rgba_string()));
        }
        if let Some(ref stroke) = self.stroke {
            pairs.push(("stroke", stroke.to_rgba_string()));
        }
        if let Some(stroke_width) = self.stroke_width {
            pairs.push(("stroke-width", stroke_width.to_string()));
        }
        if let Some(glow_radius) = self.glow_radius {
            pairs.push(("glow-radius", glow_radius.to_string()));
        }
        if let Some(ref glow_color) = self.glow_color {
            pairs.push(("glow-color", glow_color.to_rgba_string()));
        }
        if let Some(rotation) = self.rotation {
            pairs.push(("rotation", rotation.as_str().to_string()));
        }
        if let Some(rotation_speed) = self.rotation_speed {
            pairs.push(("rotation-speed", rotation_speed.to_string()));
        }
        if let Some(motion) = self.motion {
            pairs.push(("motion", motion.as_str().to_string()));
        }
        if let Some(motion_speed) = self.motion_speed {
            pairs.push(("motion-speed", motion_speed.to_string()));
        }
        if let Some(polygon_sides) = self.polygon_sides {
            pairs.push(("polygon-sides", polygon_sides.to_string()));
        }
        pairs
    }
}

// Implement ToEffectConfig for base effect structs (for restore operations)
impl ToEffectConfig for StarfieldEffect {
    fn to_config_pairs(&self) -> Vec<(&'static str, String)> {
        vec![
            ("color", self.color.to_rgba_string()),
            ("density", self.density.to_string()),
            ("layers", self.layers.to_string()),
            ("speed", self.speed.to_string()),
            ("direction", self.direction.as_str().to_string()),
            ("glow-radius", self.glow_radius.to_string()),
            ("glow-intensity", self.glow_intensity.to_string()),
            (
                "twinkle",
                if self.twinkle { "true" } else { "false" }.to_string(),
            ),
            ("twinkle-speed", self.twinkle_speed.to_string()),
            ("min-size", self.min_size.to_string()),
            ("max-size", self.max_size.to_string()),
        ]
    }
}

impl ToEffectConfig for ParticleEffect {
    fn to_config_pairs(&self) -> Vec<(&'static str, String)> {
        vec![
            ("color", self.color.to_rgba_string()),
            ("count", self.count.to_string()),
            ("shape", self.shape.as_str().to_string()),
            ("behavior", self.behavior.as_str().to_string()),
            ("size", self.size.to_string()),
            ("speed", self.speed.to_string()),
            ("glow-radius", self.glow_radius.to_string()),
            ("glow-intensity", self.glow_intensity.to_string()),
        ]
    }
}

impl ToEffectConfig for GridEffect {
    fn to_config_pairs(&self) -> Vec<(&'static str, String)> {
        vec![
            ("color", self.color.to_rgba_string()),
            ("spacing", self.spacing.to_string()),
            ("line-width", self.line_width.to_string()),
            ("perspective", self.perspective.to_string()),
            ("horizon", self.horizon.to_string()),
            ("animation-speed", self.animation_speed.to_string()),
            ("glow-radius", self.glow_radius.to_string()),
            ("glow-intensity", self.glow_intensity.to_string()),
            ("vanishing-spread", self.vanishing_spread.to_string()),
            (
                "curved",
                if self.curved { "true" } else { "false" }.to_string(),
            ),
        ]
    }
}

impl ToEffectConfig for RainEffect {
    fn to_config_pairs(&self) -> Vec<(&'static str, String)> {
        vec![
            ("color", self.color.to_rgba_string()),
            ("density", self.density.to_string()),
            ("speed", self.speed.to_string()),
            ("angle", self.angle.to_string()),
            ("length", self.length.to_string()),
            ("thickness", self.thickness.to_string()),
            ("glow-radius", self.glow_radius.to_string()),
            ("glow-intensity", self.glow_intensity.to_string()),
        ]
    }
}

impl ToEffectConfig for MatrixEffect {
    fn to_config_pairs(&self) -> Vec<(&'static str, String)> {
        vec![
            ("color", self.color.to_rgba_string()),
            ("density", self.density.to_string()),
            ("speed", self.speed.to_string()),
            ("font-size", self.font_size.to_string()),
            ("charset", self.charset.clone()),
        ]
    }
}

impl ToEffectConfig for ShapeEffect {
    fn to_config_pairs(&self) -> Vec<(&'static str, String)> {
        let mut pairs = vec![
            ("type", self.shape_type.as_str().to_string()),
            ("size", self.size.to_string()),
        ];
        if let Some(ref fill) = self.fill {
            pairs.push(("fill", fill.to_rgba_string()));
        } else {
            pairs.push(("fill", "none".to_string()));
        }
        if let Some(ref stroke) = self.stroke {
            pairs.push(("stroke", stroke.to_rgba_string()));
        } else {
            pairs.push(("stroke", "none".to_string()));
        }
        pairs.push(("stroke-width", self.stroke_width.to_string()));
        pairs.push(("glow-radius", self.glow_radius.to_string()));
        if let Some(ref glow_color) = self.glow_color {
            pairs.push(("glow-color", glow_color.to_rgba_string()));
        }
        pairs.push(("rotation", self.rotation.as_str().to_string()));
        pairs.push(("rotation-speed", self.rotation_speed.to_string()));
        pairs.push(("motion", self.motion.as_str().to_string()));
        pairs.push(("motion-speed", self.motion_speed.to_string()));
        pairs.push(("polygon-sides", self.polygon_sides.to_string()));
        pairs
    }
}

/// One-shot overlay sprite at specified position
#[derive(Debug, Clone)]
pub struct SpriteOverlay {
    /// Path to sprite sheet (required)
    pub path: String,
    /// Position for the overlay
    pub position: SpriteOverlayPosition,
    /// Number of columns in sprite sheet
    pub columns: u32,
    /// Number of rows in sprite sheet
    pub rows: u32,
    /// Animation frames per second
    pub fps: f32,
    /// Display scale
    pub scale: f32,
    /// Opacity (0.0-1.0)
    pub opacity: f32,
}

impl Default for SpriteOverlay {
    fn default() -> Self {
        Self {
            path: String::new(),
            position: SpriteOverlayPosition::Center,
            columns: 1,
            rows: 1,
            fps: 12.0,
            scale: 1.0,
            opacity: 1.0,
        }
    }
}

/// Event override - temporary theme modifications triggered by terminal events
/// Duration of 0 means "persist until cleared by another event" (used for ::on-blur)
#[derive(Debug, Clone, Default)]
pub struct EventOverride {
    /// Duration in milliseconds. 0 = persist until cleared by another event
    pub duration_ms: u32,

    /// Patches to existing backdrop sprite (keeps position/motion)
    pub sprite_patch: Option<SpritePatch>,

    /// Separate overlay sprite (one-shot effect at specified position)
    pub sprite_overlay: Option<SpriteOverlay>,

    // Theme property overrides (all optional - None means "keep base theme value")
    /// Override foreground color
    pub foreground: Option<Color>,
    /// Override background gradient
    pub background: Option<LinearGradient>,
    /// Override cursor color
    pub cursor_color: Option<Color>,
    /// Override cursor shape
    pub cursor_shape: Option<CursorShape>,
    /// Override text shadow/glow
    pub text_shadow: Option<TextShadow>,
    /// Patches to starfield effect
    pub starfield_patch: Option<StarfieldPatch>,
    /// Patches to particle effect
    pub particle_patch: Option<ParticlePatch>,
    /// Patches to grid effect
    pub grid_patch: Option<GridPatch>,
    /// Patches to rain effect
    pub rain_patch: Option<RainPatch>,
    /// Patches to matrix effect
    pub matrix_patch: Option<MatrixPatch>,
    /// Patches to shape effect
    pub shape_patch: Option<ShapePatch>,
}

impl EventOverride {
    /// Merge another EventOverride into this one (CSS cascade: later values win)
    pub fn merge(&mut self, other: EventOverride) {
        // Duration: non-zero wins
        if other.duration_ms > 0 {
            self.duration_ms = other.duration_ms;
        }

        // Merge patch structs using trait
        merge_optional_patch(&mut self.sprite_patch, other.sprite_patch);
        merge_optional_patch(&mut self.starfield_patch, other.starfield_patch);
        merge_optional_patch(&mut self.particle_patch, other.particle_patch);
        merge_optional_patch(&mut self.grid_patch, other.grid_patch);
        merge_optional_patch(&mut self.rain_patch, other.rain_patch);
        merge_optional_patch(&mut self.matrix_patch, other.matrix_patch);
        merge_optional_patch(&mut self.shape_patch, other.shape_patch);

        // Overlay replaces entirely if specified
        if other.sprite_overlay.is_some() {
            self.sprite_overlay = other.sprite_overlay;
        }

        // Theme properties: later wins
        if other.foreground.is_some() {
            self.foreground = other.foreground;
        }
        if other.background.is_some() {
            self.background = other.background;
        }
        if other.cursor_color.is_some() {
            self.cursor_color = other.cursor_color;
        }
        if other.cursor_shape.is_some() {
            self.cursor_shape = other.cursor_shape;
        }
        if other.text_shadow.is_some() {
            self.text_shadow = other.text_shadow;
        }
    }
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
    pub cursor_glow: Option<TextShadow>,

    // Effects
    pub text_shadow: Option<TextShadow>,
    pub grid: Option<GridEffect>,
    pub starfield: Option<StarfieldEffect>,
    pub rain: Option<RainEffect>,
    pub particles: Option<ParticleEffect>,
    pub matrix: Option<MatrixEffect>,
    pub shape: Option<ShapeEffect>,
    pub sprite: Option<SpriteEffect>,
    pub crt: Option<CrtEffect>,

    // Tab styling
    pub tabs: TabTheme,

    // UI styling (focus, hover, menus)
    pub ui: UiStyle,

    // Event-driven theme overrides
    pub on_bell: Option<EventOverride>,
    pub on_command_fail: Option<EventOverride>,
    pub on_command_success: Option<EventOverride>,
    pub on_focus: Option<EventOverride>,
    pub on_blur: Option<EventOverride>,
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
            cursor_glow: None,
            text_shadow: Some(TextShadow::default()),
            grid: Some(GridEffect::default()),
            starfield: None,
            rain: None,
            particles: None,
            matrix: None,
            shape: None,
            sprite: None,
            crt: None,
            tabs: TabTheme::default(),
            ui: UiStyle::default(),
            on_bell: None,
            on_command_fail: None,
            on_command_success: None,
            on_focus: None,
            on_blur: None,
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
            cursor_glow: None,
            text_shadow: None,
            grid: None,
            starfield: None,
            rain: None,
            particles: None,
            matrix: None,
            shape: None,
            sprite: None,
            crt: None,
            tabs: TabTheme::default(),
            ui: UiStyle::default(),
            on_bell: None,
            on_command_fail: None,
            on_command_success: None,
            on_focus: None,
            on_blur: None,
        }
    }

    /// Load theme from CSS string
    pub fn from_css(css: &str) -> Result<Self, parser::ThemeParseError> {
        parser::parse_theme(css)
    }

    /// Load theme from CSS string with base directory for resolving relative paths
    pub fn from_css_with_base(
        css: &str,
        base_dir: impl AsRef<Path>,
    ) -> Result<Self, parser::ThemeParseError> {
        let mut theme = parser::parse_theme(css)?;
        let base_path = base_dir.as_ref().to_path_buf();
        // Set base_dir on background_image if present
        if let Some(ref mut bg) = theme.background_image {
            bg.base_dir = Some(base_path.clone());
        }
        // Set base_dir on sprite if present
        if let Some(ref mut sprite) = theme.sprite {
            sprite.base_dir = Some(base_path);
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
    pub screen_size: [f32; 2],     // offset 0
    pub time: f32,                 // offset 8
    pub grid_intensity: f32,       // offset 12
    pub gradient_top: [f32; 4],    // offset 16 (16-byte aligned)
    pub gradient_bottom: [f32; 4], // offset 32
    pub grid_color: [f32; 4],      // offset 48
    pub grid_spacing: f32,         // offset 64
    pub grid_line_width: f32,      // offset 68
    pub grid_perspective: f32,     // offset 72
    pub grid_horizon: f32,         // offset 76
    pub glow_color: [f32; 4],      // offset 80 (16-byte aligned, 80 % 16 == 0)
    pub glow_radius: f32,          // offset 96
    pub glow_intensity: f32,       // offset 100
    pub _pad1: [f32; 2],           // offset 104 - padding to align text_color to 16 bytes
    pub text_color: [f32; 4],      // offset 112 (16-byte aligned)
    pub _pad2: [f32; 4],           // offset 128 - final padding
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

    // ============================================
    // EventOverride Merge Tests
    // ============================================

    #[test]
    fn test_event_override_merge_duration() {
        let mut base = EventOverride {
            duration_ms: 500,
            ..Default::default()
        };
        let other = EventOverride {
            duration_ms: 1000,
            ..Default::default()
        };
        base.merge(other);
        assert_eq!(base.duration_ms, 1000);
    }

    #[test]
    fn test_event_override_merge_zero_duration_preserves_base() {
        let mut base = EventOverride {
            duration_ms: 500,
            ..Default::default()
        };
        let other = EventOverride {
            duration_ms: 0,
            ..Default::default()
        };
        base.merge(other);
        assert_eq!(base.duration_ms, 500);
    }

    #[test]
    fn test_event_override_merge_starfield_patch() {
        let mut base = EventOverride {
            starfield_patch: Some(StarfieldPatch {
                speed: Some(0.5),
                color: Some(Color::rgb(1.0, 0.0, 0.0)),
                ..Default::default()
            }),
            ..Default::default()
        };
        let other = EventOverride {
            starfield_patch: Some(StarfieldPatch {
                speed: Some(1.0),
                glow_radius: Some(5.0),
                ..Default::default()
            }),
            ..Default::default()
        };
        base.merge(other);

        let patch = base.starfield_patch.unwrap();
        // Speed should be overwritten
        assert_eq!(patch.speed, Some(1.0));
        // Color should be preserved from base
        assert!(patch.color.is_some());
        // Glow radius should be added from other
        assert_eq!(patch.glow_radius, Some(5.0));
    }

    #[test]
    fn test_event_override_merge_particle_patch() {
        let mut base = EventOverride {
            particle_patch: Some(ParticlePatch {
                count: Some(50),
                speed: Some(0.5),
                ..Default::default()
            }),
            ..Default::default()
        };
        let other = EventOverride {
            particle_patch: Some(ParticlePatch {
                count: Some(100),
                shape: Some(ParticleShape::Sparkle),
                ..Default::default()
            }),
            ..Default::default()
        };
        base.merge(other);

        let patch = base.particle_patch.unwrap();
        assert_eq!(patch.count, Some(100)); // Overwritten
        assert_eq!(patch.speed, Some(0.5)); // Preserved
        assert_eq!(patch.shape, Some(ParticleShape::Sparkle)); // Added
    }

    #[test]
    fn test_event_override_merge_grid_patch() {
        let mut base = EventOverride::default();
        let other = EventOverride {
            grid_patch: Some(GridPatch {
                color: Some(Color::rgb(1.0, 0.0, 0.0)),
                animation_speed: Some(2.0),
                ..Default::default()
            }),
            ..Default::default()
        };
        base.merge(other);

        assert!(base.grid_patch.is_some());
        let patch = base.grid_patch.unwrap();
        assert!(patch.color.is_some());
        assert_eq!(patch.animation_speed, Some(2.0));
    }

    #[test]
    fn test_event_override_merge_multiple_patches() {
        let mut base = EventOverride {
            duration_ms: 500,
            cursor_color: Some(Color::rgb(1.0, 1.0, 1.0)),
            starfield_patch: Some(StarfieldPatch {
                speed: Some(0.3),
                ..Default::default()
            }),
            ..Default::default()
        };
        let other = EventOverride {
            duration_ms: 1000,
            particle_patch: Some(ParticlePatch {
                count: Some(50),
                ..Default::default()
            }),
            grid_patch: Some(GridPatch {
                animation_speed: Some(2.0),
                ..Default::default()
            }),
            ..Default::default()
        };
        base.merge(other);

        assert_eq!(base.duration_ms, 1000);
        assert!(base.cursor_color.is_some()); // Preserved
        assert!(base.starfield_patch.is_some()); // Preserved
        assert!(base.particle_patch.is_some()); // Added
        assert!(base.grid_patch.is_some()); // Added
    }

    #[test]
    fn test_sprite_patch_default() {
        let patch = SpritePatch::default();
        assert!(patch.path.is_none());
        assert!(patch.fps.is_none());
        assert!(patch.opacity.is_none());
        assert!(patch.scale.is_none());
        assert!(patch.motion_speed.is_none());
    }

    #[test]
    fn test_starfield_patch_default() {
        let patch = StarfieldPatch::default();
        assert!(patch.color.is_none());
        assert!(patch.speed.is_none());
        assert!(patch.density.is_none());
        assert!(patch.glow_radius.is_none());
    }

    #[test]
    fn test_particle_patch_default() {
        let patch = ParticlePatch::default();
        assert!(patch.color.is_none());
        assert!(patch.count.is_none());
        assert!(patch.shape.is_none());
        assert!(patch.behavior.is_none());
    }

    #[test]
    fn test_grid_patch_default() {
        let patch = GridPatch::default();
        assert!(patch.color.is_none());
        assert!(patch.spacing.is_none());
        assert!(patch.animation_speed.is_none());
        assert!(patch.curved.is_none());
    }

    #[test]
    fn test_rain_patch_default() {
        let patch = RainPatch::default();
        assert!(patch.color.is_none());
        assert!(patch.density.is_none());
        assert!(patch.speed.is_none());
        assert!(patch.angle.is_none());
    }

    #[test]
    fn test_matrix_patch_default() {
        let patch = MatrixPatch::default();
        assert!(patch.color.is_none());
        assert!(patch.density.is_none());
        assert!(patch.speed.is_none());
        assert!(patch.charset.is_none());
    }

    #[test]
    fn test_shape_patch_default() {
        let patch = ShapePatch::default();
        assert!(patch.shape_type.is_none());
        assert!(patch.size.is_none());
        assert!(patch.fill.is_none());
        assert!(patch.stroke.is_none());
        assert!(patch.rotation.is_none());
        assert!(patch.motion.is_none());
    }
}
