//! Vello-based tab bar rendering
//!
//! Renders tab bar shapes (backgrounds, borders, rounded rects) using vello.
//! Text glow effects are still handled by the existing text renderer.
//! The expensive vello::Renderer is shared across all renderers to save memory.

use crt_theme::{Color, TabTheme};
use vello::{AaConfig, RenderParams, Renderer, Scene, kurbo, peniko};

use super::layout::TabLayout;
use super::state::TabBarState;

/// Vello-based tab bar renderer
///
/// Uses a shared vello::Renderer passed in during render calls to save memory.
/// Each instance maintains its own Scene and render target.
///
/// Pattern:
/// 1. Build shapes into Scene during prepare()
/// 2. Render Scene to texture via shared vello renderer
/// 3. Text/glow effects rendered separately by existing pipeline
pub struct VelloTabBarRenderer {
    scene: Scene,
    // Render target for vello output
    target_texture: Option<wgpu::Texture>,
    target_view: Option<wgpu::TextureView>,
    target_size: (u32, u32),
}

impl VelloTabBarRenderer {
    /// Create a new tab bar vello renderer.
    /// Note: The expensive vello::Renderer is shared and passed during render calls.
    pub fn new(_device: &wgpu::Device, _format: wgpu::TextureFormat) -> Self {
        Self {
            scene: Scene::new(),
            target_texture: None,
            target_view: None,
            target_size: (0, 0),
        }
    }

    /// Ensure render target is sized correctly
    fn ensure_target(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if self.target_size != (width, height) || self.target_texture.is_none() {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Vello Tab Bar Target"),
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

    /// Build the tab bar scene from current state
    ///
    /// This is the main pattern: convert layout + theme into vello shapes
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        state: &TabBarState,
        layout: &TabLayout,
        theme: &TabTheme,
    ) {
        // Reset scene for new frame
        self.scene.reset();

        // Size target to actual tab bar dimensions, not full screen
        let (screen_width, _screen_height) = layout.screen_size();
        let bar_height = (layout.height() * layout.scale_factor()) as u32;
        self.ensure_target(device, screen_width as u32, bar_height.max(1));

        // Build shapes
        self.build_scene(state, layout, theme);
    }

    /// Build vello shapes for the tab bar
    fn build_scene(&mut self, state: &TabBarState, layout: &TabLayout, theme: &TabTheme) {
        let s = layout.scale_factor() as f64;
        let bar_height = layout.height() as f64 * s;
        let (screen_width, _) = layout.screen_size();
        let screen_width = screen_width as f64;

        let tab_rects = layout.tab_rects();
        let active_tab = state.active_tab_index();

        // Tab bar background
        let bar_bg = color_to_brush(&theme.bar.background);
        self.scene.fill(
            peniko::Fill::NonZero,
            kurbo::Affine::IDENTITY,
            &bar_bg,
            None,
            &kurbo::Rect::new(0.0, 0.0, screen_width, bar_height),
        );

        // Bottom border
        let border_color = color_to_brush(&theme.bar.border_color);
        self.scene.fill(
            peniko::Fill::NonZero,
            kurbo::Affine::IDENTITY,
            &border_color,
            None,
            &kurbo::Rect::new(0.0, bar_height - s, screen_width, bar_height),
        );

        // Draw individual tabs
        let border_radius = 4.0 * s; // Slight rounding for modern look

        for (i, rect) in tab_rects.iter().enumerate() {
            let is_active = i == active_tab;
            let bg_color = if is_active {
                color_to_brush(&theme.active.background)
            } else {
                color_to_brush(&theme.tab.background)
            };

            let x = rect.x as f64;
            let y = rect.y as f64;
            let w = rect.width as f64;
            let h = rect.height as f64;

            // Tab background with rounded top corners
            let tab_rect = kurbo::RoundedRect::new(
                x,
                y,
                x + w,
                y + h,
                kurbo::RoundedRectRadii::new(border_radius, border_radius, 0.0, 0.0),
            );
            self.scene.fill(
                peniko::Fill::NonZero,
                kurbo::Affine::IDENTITY,
                &bg_color,
                None,
                &tab_rect,
            );

            // Tab border (top and sides only for cleaner look)
            let stroke = kurbo::Stroke::new(s);

            // Build a path for top + sides border
            let mut path = kurbo::BezPath::new();
            path.move_to((x, y + h)); // bottom left
            path.line_to((x, y + border_radius)); // left side up
            path.quad_to((x, y), (x + border_radius, y)); // top-left corner
            path.line_to((x + w - border_radius, y)); // top
            path.quad_to((x + w, y), (x + w, y + border_radius)); // top-right corner
            path.line_to((x + w, y + h)); // right side down

            self.scene
                .stroke(&stroke, kurbo::Affine::IDENTITY, &border_color, None, &path);

            // Active tab accent line at bottom
            if is_active {
                let accent = color_to_brush(&theme.active.accent);
                let accent_height = 2.0 * s;
                self.scene.fill(
                    peniko::Fill::NonZero,
                    kurbo::Affine::IDENTITY,
                    &accent,
                    None,
                    &kurbo::Rect::new(x, y + h - accent_height, x + w, y + h),
                );
            }
        }
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

    /// Get target size
    pub fn target_size(&self) -> (u32, u32) {
        self.target_size
    }
}

impl Drop for VelloTabBarRenderer {
    fn drop(&mut self) {
        // Destroy render target texture to release GPU memory
        if let Some(ref texture) = self.target_texture {
            texture.destroy();
        }
    }
}

/// Convert theme color to vello brush
fn color_to_brush(color: &Color) -> peniko::Brush {
    peniko::Brush::Solid(peniko::Color::from_rgba8(
        (color.r * 255.0) as u8,
        (color.g * 255.0) as u8,
        (color.b * 255.0) as u8,
        (color.a * 255.0) as u8,
    ))
}
