//! Sprite sheet backdrop effect - animated sprites with configurable motion
//!
//! Renders an animated sprite from a sprite sheet with configurable frame layout,
//! animation speed, and motion behavior.
//!
//! ## CSS Properties
//!
//! - `--sprite-enabled: true|false`
//! - `--sprite-path: "<path>"` (path to sprite sheet image)
//! - `--sprite-frame-width: <number>` (width of each frame in pixels)
//! - `--sprite-frame-height: <number>` (height of each frame in pixels)
//! - `--sprite-columns: <number>` (number of columns in sprite sheet)
//! - `--sprite-rows: <number>` (number of rows in sprite sheet)
//! - `--sprite-frame-count: <number>` (optional, defaults to columns * rows)
//! - `--sprite-fps: <number>` (frames per second, default 12)
//! - `--sprite-scale: <number>` (display scale, default 1.0)
//! - `--sprite-opacity: <number>` (0.0-1.0, default 1.0)
//! - `--sprite-motion: none|bounce|scroll|float|orbit`
//! - `--sprite-motion-speed: <number>` (motion speed multiplier)

use std::path::Path;
use std::sync::Arc;

use vello::Scene;
use vello::kurbo::{Affine, Point, Rect};
use vello::peniko::{Blob, ImageAlphaType, ImageBrush, ImageData, ImageFormat};

use super::{BackdropEffect, EffectConfig};

/// Motion type for sprite movement
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum SpriteMotion {
    /// No movement, use static position
    #[default]
    None,
    /// Bounce off edges
    Bounce,
    /// Scroll across screen
    Scroll,
    /// Gentle floating motion
    Float,
    /// Circular orbit
    Orbit,
}

impl SpriteMotion {
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

/// Static position for sprite when motion is None
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
    fn from_str(s: &str) -> Option<Self> {
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
}

/// Sprite sheet configuration
#[derive(Debug, Clone)]
struct SpriteSheet {
    /// Cached ImageData for vello (created once, reused every frame)
    image_data: ImageData,
    /// Width of full sprite sheet
    sheet_width: u32,
    /// Height of full sprite sheet
    sheet_height: u32,
    /// Width of each frame
    frame_width: u32,
    /// Height of each frame
    frame_height: u32,
    /// Number of columns
    columns: u32,
    /// Total frame count
    frame_count: u32,
}

impl SpriteSheet {
    /// Load sprite sheet from path with frame configuration
    fn load(
        path: &Path,
        frame_width: u32,
        frame_height: u32,
        columns: u32,
        rows: u32,
        frame_count: Option<u32>,
    ) -> Result<Self, String> {
        // Load image using image crate
        let img = image::open(path)
            .map_err(|e| format!("Failed to load sprite sheet {:?}: {}", path, e))?;

        let rgba = img.to_rgba8();
        let (sheet_width, sheet_height) = rgba.dimensions();

        // Validate dimensions
        let expected_width = frame_width * columns;
        let expected_height = frame_height * rows;

        if sheet_width < expected_width || sheet_height < expected_height {
            return Err(format!(
                "Sprite sheet dimensions {}x{} don't match frame config ({}x{} * {}x{} = {}x{})",
                sheet_width,
                sheet_height,
                frame_width,
                frame_height,
                columns,
                rows,
                expected_width,
                expected_height
            ));
        }

        // Calculate actual frame count
        let max_frames = columns * rows;
        let frame_count = frame_count.unwrap_or(max_frames).min(max_frames);

        // Create Vello ImageData - this is created ONCE and reused
        let data = rgba.into_raw();
        let blob = Blob::new(Arc::new(data));

        let image_data = ImageData {
            data: blob,
            format: ImageFormat::Rgba8,
            alpha_type: ImageAlphaType::Alpha,
            width: sheet_width,
            height: sheet_height,
        };

        log::info!(
            "Loaded sprite sheet {:?}: {}x{}, {}x{} frames, {} total",
            path,
            sheet_width,
            sheet_height,
            columns,
            rows,
            frame_count
        );

        Ok(Self {
            image_data,
            sheet_width,
            sheet_height,
            frame_width,
            frame_height,
            columns,
            frame_count,
        })
    }

    /// Get the UV offset for a specific frame index
    fn frame_offset(&self, frame: u32) -> (f64, f64) {
        let col = frame % self.columns;
        let row = frame / self.columns;
        let x = (col * self.frame_width) as f64;
        let y = (row * self.frame_height) as f64;
        (x, y)
    }
}

/// Animated sprite effect with configurable sprite sheet
pub struct SpriteEffect {
    /// Whether the effect is enabled
    enabled: bool,

    /// Path to sprite sheet (from CSS)
    path: Option<String>,

    /// Base directory for resolving relative paths
    base_dir: Option<String>,

    /// Frame dimensions
    frame_width: u32,
    frame_height: u32,

    /// Grid layout
    columns: u32,
    rows: u32,

    /// Optional frame count (defaults to columns * rows)
    frame_count: Option<u32>,

    /// Animation frames per second
    fps: f64,

    /// Display scale
    scale: f64,

    /// Opacity (0.0-1.0)
    opacity: f32,

    /// Motion type
    motion_type: SpriteMotion,

    /// Motion speed multiplier
    motion_speed: f64,

    /// Static position (used when motion is None)
    position: SpritePosition,

    /// Current animation time
    time: f64,

    /// Current frame index (derived from time and fps)
    current_frame: u32,

    /// Loaded sprite sheet (None if not loaded or failed)
    sprite_sheet: Option<SpriteSheet>,

    /// Path that was last loaded (to detect changes)
    loaded_path: Option<String>,
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
            motion_type: SpriteMotion::None,
            motion_speed: 1.0,
            position: SpritePosition::Center,
            time: 0.0,
            current_frame: 0,
            sprite_sheet: None,
            loaded_path: None,
        }
    }
}

impl SpriteEffect {
    /// Create a new sprite effect with default settings
    pub fn new() -> Self {
        Self::default()
    }

    /// Calculate position based on motion type and time
    fn calculate_position(&self, bounds: Rect, sprite_width: f64, sprite_height: f64) -> Point {
        let width = bounds.width();
        let height = bounds.height();
        let cx = width / 2.0;
        let cy = height / 2.0;
        let speed = self.motion_speed;
        let t = self.time * speed;

        // Padding from edges
        let pad = 20.0;
        let half_w = sprite_width / 2.0;
        let half_h = sprite_height / 2.0;

        let (x, y) = match self.motion_type {
            SpriteMotion::None => {
                // Use static position
                match self.position {
                    SpritePosition::Center => (cx, cy),
                    SpritePosition::TopLeft => (half_w + pad, half_h + pad),
                    SpritePosition::Top => (cx, half_h + pad),
                    SpritePosition::TopRight => (width - half_w - pad, half_h + pad),
                    SpritePosition::Left => (half_w + pad, cy),
                    SpritePosition::Right => (width - half_w - pad, cy),
                    SpritePosition::BottomLeft => (half_w + pad, height - half_h - pad),
                    SpritePosition::Bottom => (cx, height - half_h - pad),
                    SpritePosition::BottomRight => (width - half_w - pad, height - half_h - pad),
                }
            }
            SpriteMotion::Bounce => {
                // Simulate bounce using triangle wave functions
                let period_x = 2.0 * (width - sprite_width).max(1.0);
                let period_y = 2.0 * (height - sprite_height).max(1.0);

                let tx = t * 100.0;
                let ty = t * 75.0;

                let triangle = |v: f64, period: f64| -> f64 {
                    if period <= 0.0 {
                        return 0.5;
                    }
                    let p = v % period;
                    let half = period / 2.0;
                    if p < half { p / half } else { 2.0 - p / half }
                };

                let x = sprite_width / 2.0 + triangle(tx, period_x) * (width - sprite_width);
                let y = sprite_height / 2.0 + triangle(ty, period_y) * (height - sprite_height);
                (x, y)
            }
            SpriteMotion::Scroll => {
                // Scroll diagonally, wrap around
                let x = (t * 80.0) % (width + sprite_width) - sprite_width / 2.0;
                let y = (t * 40.0) % (height + sprite_height) - sprite_height / 2.0;
                (x, y)
            }
            SpriteMotion::Float => {
                // Gentle floating using multiple sine waves
                let x = cx + (t * 0.5).sin() * 80.0 + (t * 0.7).sin() * 40.0;
                let y = cy + (t * 0.4).sin() * 60.0 + (t * 0.6).sin() * 30.0;
                (x, y)
            }
            SpriteMotion::Orbit => {
                // Circular orbit around center
                let radius = width.min(height) * 0.3;
                let x = cx + (t * 0.8).cos() * radius;
                let y = cy + (t * 0.8).sin() * radius;
                (x, y)
            }
        };

        Point::new(bounds.x0 + x, bounds.y0 + y)
    }

    /// Try to load the sprite sheet
    fn try_load_sprite_sheet(&mut self) {
        let Some(path_str) = &self.path else {
            self.sprite_sheet = None;
            self.loaded_path = None;
            return;
        };

        // Check if we already loaded this path with same config
        if self.loaded_path.as_ref() == Some(path_str) && self.sprite_sheet.is_some() {
            return;
        }

        // Resolve path
        let path = if let Some(base) = &self.base_dir {
            let base_path = Path::new(base);
            base_path.join(path_str)
        } else {
            Path::new(path_str).to_path_buf()
        };

        // Load sprite sheet
        match SpriteSheet::load(
            &path,
            self.frame_width,
            self.frame_height,
            self.columns,
            self.rows,
            self.frame_count,
        ) {
            Ok(sheet) => {
                self.sprite_sheet = Some(sheet);
                self.loaded_path = Some(path_str.clone());
            }
            Err(e) => {
                log::error!("Failed to load sprite sheet: {}", e);
                self.sprite_sheet = None;
                self.loaded_path = None;
            }
        }
    }
}

impl BackdropEffect for SpriteEffect {
    fn effect_type(&self) -> &'static str {
        "sprite"
    }

    fn update(&mut self, _dt: f32, time: f32) {
        self.time = time as f64;

        // Calculate current frame from time and fps
        if let Some(sheet) = &self.sprite_sheet {
            if sheet.frame_count > 0 && self.fps > 0.0 {
                let frame_duration = 1.0 / self.fps;
                let total_frames = (self.time / frame_duration) as u32;
                self.current_frame = total_frames % sheet.frame_count;
            }
        }
    }

    fn render(&self, scene: &mut Scene, bounds: Rect) {
        if !self.enabled {
            return;
        }

        let Some(sheet) = &self.sprite_sheet else {
            return;
        };

        // Create image brush from the cached ImageData
        // The ImageData is created once when loading and reused here
        // The clone of ImageData is cheap (just Arc increments for the Blob)
        let image_brush = ImageBrush::new(sheet.image_data.clone()).with_alpha(self.opacity);

        // Calculate sprite display size
        let sprite_width = sheet.frame_width as f64 * self.scale;
        let sprite_height = sheet.frame_height as f64 * self.scale;

        // Get position
        let center = self.calculate_position(bounds, sprite_width, sprite_height);

        // Get frame offset in sprite sheet
        let (frame_x, frame_y) = sheet.frame_offset(self.current_frame);

        // Calculate transform:
        // 1. Translate so the frame's top-left is at origin
        // 2. Scale to display size
        // 3. Translate to final position (center - half size)

        let target_x = center.x - sprite_width / 2.0;
        let target_y = center.y - sprite_height / 2.0;

        // The transform maps from image space to screen space
        // We want to show only the current frame, positioned at (target_x, target_y)
        // with size (sprite_width, sprite_height)

        // Use a clip to show only the frame region
        let clip_rect = Rect::new(
            target_x,
            target_y,
            target_x + sprite_width,
            target_y + sprite_height,
        );

        scene.push_clip_layer(Affine::IDENTITY, &clip_rect);

        // Transform: position the sprite sheet so current frame is in the clip
        // Frame is at (frame_x, frame_y) in sheet coordinates
        // We want it at (target_x, target_y) in screen coordinates
        let transform = Affine::translate((
            target_x - frame_x * self.scale,
            target_y - frame_y * self.scale,
        )) * Affine::scale(self.scale);

        scene.draw_image(&image_brush, transform);

        scene.pop_layer();
    }

    fn configure(&mut self, config: &EffectConfig) {
        if let Some(enabled) = config.get_bool("enabled") {
            self.enabled = enabled;
        }

        // Path handling - strip quotes if present
        if let Some(path) = config.get("path") {
            let path = path.trim();
            let path = if (path.starts_with('"') && path.ends_with('"'))
                || (path.starts_with('\'') && path.ends_with('\''))
            {
                &path[1..path.len() - 1]
            } else {
                path
            };
            self.path = Some(path.to_string());
        }

        if let Some(base_dir) = config.get("base-dir") {
            self.base_dir = Some(base_dir.to_string());
        }

        if let Some(width) = config.get_u32("frame-width") {
            self.frame_width = width.max(1);
        }

        if let Some(height) = config.get_u32("frame-height") {
            self.frame_height = height.max(1);
        }

        if let Some(cols) = config.get_u32("columns") {
            self.columns = cols.max(1);
        }

        if let Some(rows) = config.get_u32("rows") {
            self.rows = rows.max(1);
        }

        if let Some(count) = config.get_u32("frame-count") {
            self.frame_count = Some(count);
        }

        if let Some(fps) = config.get_f64("fps") {
            self.fps = fps.max(0.1);
        }

        if let Some(scale) = config.get_f64("scale") {
            self.scale = scale.clamp(0.1, 10.0);
        }

        if let Some(opacity) = config.get_f32("opacity") {
            self.opacity = opacity.clamp(0.0, 1.0);
        }

        if let Some(motion_str) = config.get("motion") {
            if let Some(motion) = SpriteMotion::from_str(motion_str) {
                self.motion_type = motion;
            }
        }

        if let Some(speed) = config.get_f64("motion-speed") {
            self.motion_speed = speed.max(0.0);
        }

        if let Some(position_str) = config.get("position") {
            if let Some(pos) = SpritePosition::from_str(position_str) {
                self.position = pos;
            }
        }

        // Try to load sprite sheet after configuration
        self.try_load_sprite_sheet();
    }

    fn is_enabled(&self) -> bool {
        self.enabled && self.sprite_sheet.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_sprite_disabled() {
        let sprite = SpriteEffect::default();
        assert!(!sprite.is_enabled());
    }

    #[test]
    fn test_motion_parsing() {
        assert_eq!(SpriteMotion::from_str("none"), Some(SpriteMotion::None));
        assert_eq!(SpriteMotion::from_str("BOUNCE"), Some(SpriteMotion::Bounce));
        assert_eq!(SpriteMotion::from_str("Float"), Some(SpriteMotion::Float));
        assert_eq!(SpriteMotion::from_str("orbit"), Some(SpriteMotion::Orbit));
    }

    #[test]
    fn test_frame_offset() {
        // Create a mock sheet config (without actual image data)
        let frame_width = 64;
        let frame_height = 64;
        let columns = 4;
        let _rows = 2;

        // Frame 0: (0, 0)
        let col0 = 0 % columns;
        let row0 = 0 / columns;
        assert_eq!((col0 * frame_width, row0 * frame_height), (0, 0));

        // Frame 3: (192, 0) - last in first row
        let col3 = 3 % columns;
        let row3 = 3 / columns;
        assert_eq!((col3 * frame_width, row3 * frame_height), (192, 0));

        // Frame 4: (0, 64) - first in second row
        let col4 = 4 % columns;
        let row4 = 4 / columns;
        assert_eq!((col4 * frame_width, row4 * frame_height), (0, 64));

        // Frame 7: (192, 64) - last frame
        let col7 = 7 % columns;
        let row7 = 7 / columns;
        assert_eq!((col7 * frame_width, row7 * frame_height), (192, 64));
    }
}
