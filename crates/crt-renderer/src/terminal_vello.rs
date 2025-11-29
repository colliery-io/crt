//! Terminal vello renderer for cursor and selection shapes
//!
//! Renders terminal UI elements (cursor, selection rectangles) via vello.
//! Uses cached GPU resources to avoid per-frame allocations.
//! The expensive vello::Renderer is shared across all renderers to save memory.

use std::time::{Duration, Instant};
use vello::{AaConfig, RenderParams, Renderer, Scene, kurbo, peniko};

/// Cursor shape style
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorShape {
    /// Block cursor (full cell)
    Block,
    /// Vertical bar cursor
    Bar,
    /// Underline cursor
    Underline,
}

impl Default for CursorShape {
    fn default() -> Self {
        Self::Block
    }
}

/// Cursor position and dimensions
#[derive(Debug, Clone, Copy, Default)]
pub struct CursorState {
    /// X position in pixels
    pub x: f32,
    /// Y position in pixels
    pub y: f32,
    /// Cell width in pixels
    pub cell_width: f32,
    /// Cell height in pixels
    pub cell_height: f32,
    /// Whether cursor is visible
    pub visible: bool,
}

/// Terminal vello renderer with cached GPU resources
///
/// Uses a shared vello::Renderer passed in during render calls to save memory.
/// Each instance maintains its own Scene and render target.
///
/// Pattern:
/// 1. Build shapes into Scene during prepare()
/// 2. Render Scene to cached texture via shared vello renderer
/// 3. Composite texture onto frame
pub struct TerminalVelloRenderer {
    scene: Scene,
    // Cached render target
    target_texture: Option<wgpu::Texture>,
    target_view: Option<wgpu::TextureView>,
    target_size: (u32, u32),
    // Cursor state
    cursor_shape: CursorShape,
    cursor_color: [f32; 4],
    cursor_glow: Option<([f32; 4], f32, f32)>, // (color, radius, intensity)
    cursor_state: CursorState,
    // Cursor blink state
    blink_enabled: bool,
    blink_interval: Duration,
    blink_visible: bool,
    last_blink_toggle: Instant,
    // Selection state
    selection_color: [f32; 4],
}

impl TerminalVelloRenderer {
    /// Default blink interval in milliseconds
    pub const DEFAULT_BLINK_INTERVAL_MS: u64 = 530;

    /// Create a new terminal vello renderer.
    /// Note: The expensive vello::Renderer is shared and passed during render calls.
    pub fn new(_device: &wgpu::Device) -> Self {
        Self {
            scene: Scene::new(),
            target_texture: None,
            target_view: None,
            target_size: (0, 0),
            cursor_shape: CursorShape::Block,
            cursor_color: [0.8, 0.8, 0.2, 0.9],
            cursor_glow: None,
            cursor_state: CursorState::default(),
            blink_enabled: true,
            blink_interval: Duration::from_millis(Self::DEFAULT_BLINK_INTERVAL_MS),
            blink_visible: true,
            last_blink_toggle: Instant::now(),
            selection_color: [0.3, 0.4, 0.6, 0.5],
        }
    }

    /// Enable or disable cursor blinking
    pub fn set_blink_enabled(&mut self, enabled: bool) {
        self.blink_enabled = enabled;
        if !enabled {
            self.blink_visible = true; // Always visible when not blinking
        }
    }

    /// Set the blink interval in milliseconds
    pub fn set_blink_interval_ms(&mut self, ms: u64) {
        self.blink_interval = Duration::from_millis(ms);
    }

    /// Update blink state based on elapsed time
    /// Call this every frame to update cursor visibility
    pub fn update_blink(&mut self) {
        if !self.blink_enabled {
            return;
        }

        let now = Instant::now();
        if now.duration_since(self.last_blink_toggle) >= self.blink_interval {
            self.blink_visible = !self.blink_visible;
            self.last_blink_toggle = now;
        }
    }

    /// Reset blink state (cursor becomes visible, timer resets)
    /// Call this when cursor moves or user types
    pub fn reset_blink(&mut self) {
        self.blink_visible = true;
        self.last_blink_toggle = Instant::now();
    }

    /// Set the cursor shape
    pub fn set_cursor_shape(&mut self, shape: CursorShape) {
        self.cursor_shape = shape;
    }

    /// Set the cursor color
    pub fn set_cursor_color(&mut self, color: [f32; 4]) {
        self.cursor_color = color;
    }

    /// Get the current cursor color
    pub fn cursor_color(&self) -> [f32; 4] {
        self.cursor_color
    }

    /// Set cursor glow effect (color, radius, intensity)
    pub fn set_cursor_glow(&mut self, glow: Option<([f32; 4], f32, f32)>) {
        self.cursor_glow = glow;
    }

    /// Get the current cursor shape
    pub fn cursor_shape(&self) -> CursorShape {
        self.cursor_shape
    }

    /// Update cursor position and visibility
    pub fn set_cursor(&mut self, x: f32, y: f32, cell_width: f32, cell_height: f32, visible: bool) {
        self.cursor_state = CursorState {
            x,
            y,
            cell_width,
            cell_height,
            visible,
        };
    }

    /// Set the selection highlight color
    pub fn set_selection_color(&mut self, color: [f32; 4]) {
        self.selection_color = color;
    }

    /// Ensure render target is sized correctly
    fn ensure_target(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if self.target_size != (width, height) || self.target_texture.is_none() {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Terminal Vello Target"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::STORAGE_BINDING,
                view_formats: &[],
            });
            let view = texture.create_view(&Default::default());
            self.target_texture = Some(texture);
            self.target_view = Some(view);
            self.target_size = (width, height);
        }
    }

    /// Prepare the scene with current cursor and selection state
    pub fn prepare(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        self.scene.reset();
        self.ensure_target(device, width, height);

        // Render cursor if visible (accounts for blink state)
        if self.cursor_visible() {
            self.build_cursor();
        }
    }

    /// Prepare scene for backgrounds only (no cursor)
    /// Used for rendering cell backgrounds before text
    pub fn prepare_backgrounds_only(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        self.scene.reset();
        self.ensure_target(device, width, height);
        // Don't add cursor - this pass is only for backgrounds
    }

    /// Build cursor shape into scene
    fn build_cursor(&mut self) {
        let cursor = &self.cursor_state;

        // Get base cursor rect dimensions
        let (base_x, base_y, base_w, base_h) = match self.cursor_shape {
            CursorShape::Block => (
                cursor.x,
                cursor.y,
                cursor.cell_width,
                cursor.cell_height,
            ),
            CursorShape::Bar => (cursor.x, cursor.y, 2.0, cursor.cell_height),
            CursorShape::Underline => (
                cursor.x,
                cursor.y + cursor.cell_height - 2.0,
                cursor.cell_width,
                2.0,
            ),
        };

        // Render glow effect first (behind cursor)
        if let Some((glow_color, radius, intensity)) = self.cursor_glow {
            // Render multiple layers with decreasing opacity for soft glow
            let layers = 5;
            for i in (1..=layers).rev() {
                let layer_radius = radius * (i as f32 / layers as f32);
                let layer_alpha = intensity * (1.0 - (i as f32 / (layers + 1) as f32)) * 0.4;

                let glow_brush = peniko::Brush::Solid(color_from_f32(
                    glow_color[0],
                    glow_color[1],
                    glow_color[2],
                    layer_alpha,
                ));

                let glow_rect = kurbo::Rect::new(
                    (base_x - layer_radius) as f64,
                    (base_y - layer_radius) as f64,
                    (base_x + base_w + layer_radius) as f64,
                    (base_y + base_h + layer_radius) as f64,
                );

                // Use rounded rect for softer glow appearance
                let rounded = kurbo::RoundedRect::from_rect(glow_rect, layer_radius as f64);

                self.scene.fill(
                    peniko::Fill::NonZero,
                    kurbo::Affine::IDENTITY,
                    &glow_brush,
                    None,
                    &rounded,
                );
            }
        }

        // Render solid cursor on top
        let brush = peniko::Brush::Solid(color_from_f32(
            self.cursor_color[0],
            self.cursor_color[1],
            self.cursor_color[2],
            self.cursor_color[3],
        ));

        let rect = kurbo::Rect::new(
            base_x as f64,
            base_y as f64,
            (base_x + base_w) as f64,
            (base_y + base_h) as f64,
        );

        self.scene.fill(
            peniko::Fill::NonZero,
            kurbo::Affine::IDENTITY,
            &brush,
            None,
            &rect,
        );
    }

    /// Render the scene to the internal texture using the shared renderer
    pub fn render_to_texture(
        &mut self,
        renderer: &mut Renderer,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<(), vello::Error> {
        let Some(target_view) = &self.target_view else {
            return Ok(());
        };

        let (width, height) = self.target_size;
        if width == 0 || height == 0 {
            return Ok(());
        }

        let params = RenderParams {
            base_color: peniko::Color::TRANSPARENT,
            width,
            height,
            antialiasing_method: AaConfig::Area,
        };

        renderer.render_to_texture(device, queue, &self.scene, target_view, &params)
    }

    /// Get the rendered texture view for compositing
    pub fn texture_view(&self) -> Option<&wgpu::TextureView> {
        self.target_view.as_ref()
    }

    /// Check if cursor is currently visible (accounts for blink state)
    pub fn cursor_visible(&self) -> bool {
        self.cursor_state.visible && self.blink_visible
    }

    /// Check if cursor has any state (for rendering decisions)
    pub fn has_cursor(&self) -> bool {
        self.cursor_state.visible
    }

    /// Add a selection rectangle for a single cell to the scene
    pub fn add_selection_cell(&mut self, x: f32, y: f32, cell_width: f32, cell_height: f32) {
        let rect = kurbo::Rect::new(
            x as f64,
            y as f64,
            (x + cell_width) as f64,
            (y + cell_height) as f64,
        );
        let brush = peniko::Brush::Solid(color_from_f32(
            self.selection_color[0],
            self.selection_color[1],
            self.selection_color[2],
            self.selection_color[3],
        ));

        self.scene.fill(
            peniko::Fill::NonZero,
            kurbo::Affine::IDENTITY,
            &brush,
            None,
            &rect,
        );
    }

    /// Add a selection spanning multiple cells to the scene
    pub fn add_selection_row(
        &mut self,
        start_x: f32,
        y: f32,
        num_cells: usize,
        cell_width: f32,
        cell_height: f32,
    ) {
        if num_cells == 0 {
            return;
        }

        let rect = kurbo::Rect::new(
            start_x as f64,
            y as f64,
            (start_x + cell_width * num_cells as f32) as f64,
            (y + cell_height) as f64,
        );
        let brush = peniko::Brush::Solid(color_from_f32(
            self.selection_color[0],
            self.selection_color[1],
            self.selection_color[2],
            self.selection_color[3],
        ));

        self.scene.fill(
            peniko::Fill::NonZero,
            kurbo::Affine::IDENTITY,
            &brush,
            None,
            &rect,
        );
    }

    /// Add a background rectangle for a cell
    pub fn add_background(
        &mut self,
        x: f32,
        y: f32,
        cell_width: f32,
        cell_height: f32,
        color: [f32; 4],
    ) {
        let rect = kurbo::Rect::new(
            x as f64,
            y as f64,
            (x + cell_width) as f64,
            (y + cell_height) as f64,
        );
        let brush = peniko::Brush::Solid(color_from_f32(color[0], color[1], color[2], color[3]));

        self.scene.fill(
            peniko::Fill::NonZero,
            kurbo::Affine::IDENTITY,
            &brush,
            None,
            &rect,
        );
    }

    /// Add an underline decoration for a cell
    pub fn add_underline(
        &mut self,
        x: f32,
        y: f32,
        cell_width: f32,
        cell_height: f32,
        color: [f32; 4],
    ) {
        // Position underline near the bottom of the cell, 2 pixels thick
        let underline_y = y + cell_height - 3.0;
        let rect = kurbo::Rect::new(
            x as f64,
            underline_y as f64,
            (x + cell_width) as f64,
            (underline_y + 1.5) as f64,
        );
        let brush = peniko::Brush::Solid(color_from_f32(color[0], color[1], color[2], color[3]));

        self.scene.fill(
            peniko::Fill::NonZero,
            kurbo::Affine::IDENTITY,
            &brush,
            None,
            &rect,
        );
    }

    /// Add a strikethrough decoration for a cell
    pub fn add_strikethrough(
        &mut self,
        x: f32,
        y: f32,
        cell_width: f32,
        cell_height: f32,
        color: [f32; 4],
    ) {
        // Position strikethrough at middle of cell, 1.5 pixels thick
        let strike_y = y + cell_height * 0.45;
        let rect = kurbo::Rect::new(
            x as f64,
            strike_y as f64,
            (x + cell_width) as f64,
            (strike_y + 1.5) as f64,
        );
        let brush = peniko::Brush::Solid(color_from_f32(color[0], color[1], color[2], color[3]));

        self.scene.fill(
            peniko::Fill::NonZero,
            kurbo::Affine::IDENTITY,
            &brush,
            None,
            &rect,
        );
    }

    /// Add a search bar background to the scene
    /// Returns the text position (x, y) for rendering the query text
    pub fn add_search_bar(
        &mut self,
        screen_width: f32,
        content_offset_y: f32,
        scale_factor: f32,
        background_color: [f32; 4],
        border_color: [f32; 4],
    ) -> (f32, f32, f32, f32) {
        let padding = 8.0 * scale_factor;
        let bar_height = 32.0 * scale_factor;
        let bar_width = 280.0 * scale_factor;
        let border_radius = 6.0 * scale_factor;
        let margin = 10.0 * scale_factor;

        // Position at top-right, below tab bar
        let bar_x = screen_width - bar_width - margin;
        let bar_y = content_offset_y + margin;

        // Draw background with rounded corners
        let rect = kurbo::RoundedRect::new(
            bar_x as f64,
            bar_y as f64,
            (bar_x + bar_width) as f64,
            (bar_y + bar_height) as f64,
            border_radius as f64,
        );

        // Background fill
        let bg_brush = peniko::Brush::Solid(color_from_f32(
            background_color[0],
            background_color[1],
            background_color[2],
            background_color[3],
        ));
        self.scene.fill(
            peniko::Fill::NonZero,
            kurbo::Affine::IDENTITY,
            &bg_brush,
            None,
            &rect,
        );

        // Border stroke
        let border_brush = peniko::Brush::Solid(color_from_f32(
            border_color[0],
            border_color[1],
            border_color[2],
            border_color[3],
        ));
        let stroke = kurbo::Stroke::new(1.5 * scale_factor as f64);
        self.scene
            .stroke(&stroke, kurbo::Affine::IDENTITY, &border_brush, None, &rect);

        // Return text position and dimensions for text rendering
        let text_x = bar_x + padding;
        let text_y = bar_y + padding;
        let text_width = bar_width - padding * 2.0;
        let text_height = bar_height - padding * 2.0;

        (text_x, text_y, text_width, text_height)
    }
}

/// Helper to create a peniko Color from f32 RGBA components (0.0-1.0)
fn color_from_f32(r: f32, g: f32, b: f32, a: f32) -> peniko::Color {
    peniko::Color::from_rgba8(
        (r * 255.0) as u8,
        (g * 255.0) as u8,
        (b * 255.0) as u8,
        (a * 255.0) as u8,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cursor_shapes() {
        assert_eq!(CursorShape::default(), CursorShape::Block);
    }

    #[test]
    fn test_cursor_state() {
        let state = CursorState {
            x: 10.0,
            y: 20.0,
            cell_width: 8.0,
            cell_height: 16.0,
            visible: true,
        };
        assert!(state.visible);
        assert_eq!(state.x, 10.0);
    }
}
