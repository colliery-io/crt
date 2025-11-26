//! Vello 2D renderer integration
//!
//! Provides a Scene-based 2D rendering context that integrates with our
//! existing wgpu pipeline. Vello handles shapes, gradients, and text
//! while custom shaders handle perspective grids and blur effects.

use vello::{peniko, kurbo, Scene, Renderer, RendererOptions, RenderParams, AaConfig};
use wgpu::{Device, Queue, TextureFormat, TextureView};

/// Vello render context for 2D UI elements
pub struct VelloContext {
    renderer: Renderer,
    scene: Scene,
}

impl VelloContext {
    /// Create a new Vello context with the given wgpu device
    pub fn new(device: &Device, _format: TextureFormat) -> Self {
        let renderer = Renderer::new(
            device,
            RendererOptions {
                pipeline_cache: None,
                ..Default::default()
            },
        ).expect("Failed to create Vello renderer");

        Self {
            renderer,
            scene: Scene::new(),
        }
    }

    /// Clear the scene for a new frame
    pub fn begin_frame(&mut self) {
        self.scene.reset();
    }

    /// Get mutable access to the scene for drawing
    pub fn scene(&mut self) -> &mut Scene {
        &mut self.scene
    }

    /// Render the scene to the given texture view
    pub fn render(
        &mut self,
        device: &Device,
        queue: &Queue,
        target: &TextureView,
        width: u32,
        height: u32,
    ) -> Result<(), vello::Error> {
        let params = RenderParams {
            base_color: peniko::Color::TRANSPARENT,
            width,
            height,
            antialiasing_method: AaConfig::Area,
        };

        self.renderer.render_to_texture(
            device,
            queue,
            &self.scene,
            target,
            &params,
        )
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

/// Builder for drawing 2D UI elements using vello primitives
pub struct UiBuilder<'a> {
    scene: &'a mut Scene,
}

impl<'a> UiBuilder<'a> {
    pub fn new(scene: &'a mut Scene) -> Self {
        Self { scene }
    }

    /// Draw a filled rectangle
    pub fn fill_rect(&mut self, x: f64, y: f64, width: f64, height: f64, color: [f32; 4]) {
        let rect = kurbo::Rect::new(x, y, x + width, y + height);
        let brush = peniko::Brush::Solid(color_from_f32(color[0], color[1], color[2], color[3]));
        self.scene.fill(
            peniko::Fill::NonZero,
            kurbo::Affine::IDENTITY,
            &brush,
            None,
            &rect,
        );
    }

    /// Draw a rounded rectangle
    pub fn fill_rounded_rect(
        &mut self,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        radius: f64,
        color: [f32; 4],
    ) {
        let rect = kurbo::RoundedRect::new(x, y, x + width, y + height, radius);
        let brush = peniko::Brush::Solid(color_from_f32(color[0], color[1], color[2], color[3]));
        self.scene.fill(
            peniko::Fill::NonZero,
            kurbo::Affine::IDENTITY,
            &brush,
            None,
            &rect,
        );
    }

    /// Draw a stroked rectangle (border only)
    pub fn stroke_rect(
        &mut self,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        stroke_width: f64,
        color: [f32; 4],
    ) {
        let rect = kurbo::Rect::new(x, y, x + width, y + height);
        let brush = peniko::Brush::Solid(color_from_f32(color[0], color[1], color[2], color[3]));
        let stroke = kurbo::Stroke::new(stroke_width);
        self.scene.stroke(
            &stroke,
            kurbo::Affine::IDENTITY,
            &brush,
            None,
            &rect,
        );
    }

    /// Draw a linear gradient filled rectangle
    pub fn fill_gradient_rect(
        &mut self,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        top_color: [f32; 4],
        bottom_color: [f32; 4],
    ) {
        let rect = kurbo::Rect::new(x, y, x + width, y + height);

        let gradient = peniko::Gradient::new_linear(
            kurbo::Point::new(x + width / 2.0, y),
            kurbo::Point::new(x + width / 2.0, y + height),
        )
        .with_stops([
            peniko::ColorStop {
                offset: 0.0,
                color: color_from_f32(top_color[0], top_color[1], top_color[2], top_color[3]).into(),
            },
            peniko::ColorStop {
                offset: 1.0,
                color: color_from_f32(bottom_color[0], bottom_color[1], bottom_color[2], bottom_color[3]).into(),
            },
        ]);

        self.scene.fill(
            peniko::Fill::NonZero,
            kurbo::Affine::IDENTITY,
            &peniko::Brush::Gradient(gradient),
            None,
            &rect,
        );
    }

    /// Draw a circle
    pub fn fill_circle(&mut self, cx: f64, cy: f64, radius: f64, color: [f32; 4]) {
        let circle = kurbo::Circle::new(kurbo::Point::new(cx, cy), radius);
        let brush = peniko::Brush::Solid(color_from_f32(color[0], color[1], color[2], color[3]));
        self.scene.fill(
            peniko::Fill::NonZero,
            kurbo::Affine::IDENTITY,
            &brush,
            None,
            &circle,
        );
    }

    /// Draw a line
    pub fn stroke_line(
        &mut self,
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        stroke_width: f64,
        color: [f32; 4],
    ) {
        let line = kurbo::Line::new(
            kurbo::Point::new(x1, y1),
            kurbo::Point::new(x2, y2),
        );
        let brush = peniko::Brush::Solid(color_from_f32(color[0], color[1], color[2], color[3]));
        let stroke = kurbo::Stroke::new(stroke_width);
        self.scene.stroke(
            &stroke,
            kurbo::Affine::IDENTITY,
            &brush,
            None,
            &line,
        );
    }
}
