//! GPU state management
//!
//! Shared and per-window GPU resources for wgpu rendering.

use crt_renderer::{GlyphCache, GridRenderer, RectRenderer, EffectPipeline, TabBar, TerminalVelloRenderer, BackgroundImagePipeline, BackgroundImageState};

/// Shared GPU resources across all windows
pub struct SharedGpuState {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    /// Shared Vello renderer - lazy loaded only when CSS effects need it
    /// (rounded corners, gradients, shadows, etc.)
    pub vello_renderer: Option<vello::Renderer>,
}

impl SharedGpuState {
    /// Initialize shared GPU resources
    pub fn new() -> Self {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());

        // Request adapter without a surface first (we'll create surfaces per-window)
        let adapter = pollster::block_on(async {
            instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::default(),
                    compatible_surface: None,
                    force_fallback_adapter: false,
                })
                .await
                .expect("Failed to find suitable GPU adapter")
        });

        let (device, queue) = pollster::block_on(async {
            adapter
                .request_device(&wgpu::DeviceDescriptor::default())
                .await
                .expect("Failed to create device")
        });

        // Vello renderer is lazy-loaded only when CSS effects need it
        // (rounded corners, gradients, shadows, complex paths)
        let vello_renderer = None;

        Self {
            instance,
            adapter,
            device,
            queue,
            vello_renderer,
        }
    }

    /// Get or create the Vello renderer (lazy initialization)
    ///
    /// Call this when you need advanced CSS effects like rounded corners,
    /// gradients, or complex paths. The renderer is cached after first creation.
    #[allow(dead_code)]
    pub fn get_or_create_vello_renderer(&mut self) -> &mut vello::Renderer {
        if self.vello_renderer.is_none() {
            log::info!("Lazy-loading Vello renderer for advanced CSS effects");
            self.vello_renderer = Some(
                vello::Renderer::new(
                    &self.device,
                    vello::RendererOptions {
                        pipeline_cache: None,
                        ..Default::default()
                    },
                ).expect("Failed to create Vello renderer")
            );
        }
        self.vello_renderer.as_mut().unwrap()
    }
}

/// Per-window GPU state (surface tied to specific window)
pub struct WindowGpuState {
    pub surface: wgpu::Surface<'static>,
    pub config: wgpu::SurfaceConfiguration,

    // Text rendering with swash glyph cache (scales with zoom)
    pub glyph_cache: GlyphCache,
    pub grid_renderer: GridRenderer,

    // Fixed-size glyph cache for tab titles (doesn't scale with zoom)
    pub tab_glyph_cache: GlyphCache,
    // Separate renderer for tab titles to avoid buffer conflicts
    // (terminal and tab titles render in different passes but the GPU
    // commands are batched, so they need separate instance buffers)
    pub tab_title_renderer: GridRenderer,

    // Effect pipeline
    pub effect_pipeline: EffectPipeline,

    // Tab bar
    pub tab_bar: TabBar,

    // Terminal vello renderer for cursor and selection
    pub terminal_vello: TerminalVelloRenderer,

    // Rect renderer for cell backgrounds
    pub rect_renderer: RectRenderer,

    // Background image rendering (optional)
    pub background_image_pipeline: BackgroundImagePipeline,
    pub background_image_state: Option<BackgroundImageState>,
    pub background_image_bind_group: Option<wgpu::BindGroup>,

    // Intermediate text texture for glow effect
    // Text is rendered here first, then composited with Gaussian blur
    pub text_texture: wgpu::Texture,
    pub text_texture_view: wgpu::TextureView,
    pub composite_bind_group: wgpu::BindGroup,
}
