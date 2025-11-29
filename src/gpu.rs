//! GPU state management
//!
//! Shared and per-window GPU resources for wgpu rendering.

use std::sync::{Arc, Mutex};

use crt_renderer::{
    BackgroundImagePipeline, BackgroundImageState, CrtPipeline, EffectPipeline, EffectsRenderer,
    GlyphCache, GridRenderer, RectRenderer, SpriteAnimationState, TabBar, TerminalVelloRenderer,
};

/// Shared GPU resources across all windows
pub struct SharedGpuState {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    /// Shared Vello renderer - lazy loaded only when CSS effects need it
    /// (rounded corners, gradients, shadows, backdrop effects, etc.)
    /// Wrapped in Arc<Mutex> for sharing with EffectsRenderer
    pub vello_renderer: Arc<Mutex<Option<vello::Renderer>>>,
}

impl SharedGpuState {
    /// Initialize shared GPU resources
    pub fn new() -> Self {
        log::debug!("Initializing shared GPU state");
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

        log::debug!(
            "GPU adapter: {:?} ({:?})",
            adapter.get_info().name,
            adapter.get_info().backend
        );

        let (device, queue) = pollster::block_on(async {
            adapter
                .request_device(&wgpu::DeviceDescriptor::default())
                .await
                .expect("Failed to create device")
        });

        log::debug!("GPU device created successfully");

        // Vello renderer is lazy-loaded only when CSS effects need it
        // (rounded corners, gradients, shadows, backdrop effects, complex paths)
        let vello_renderer = Arc::new(Mutex::new(None));

        Self {
            instance,
            adapter,
            device,
            queue,
            vello_renderer,
        }
    }

    /// Ensure the Vello renderer is initialized (lazy initialization)
    ///
    /// Call this when you need advanced CSS effects like rounded corners,
    /// gradients, backdrop effects, or complex paths. The renderer is
    /// cached after first creation.
    pub fn ensure_vello_renderer(&self) {
        let mut guard = self.vello_renderer.lock().unwrap();
        if guard.is_none() {
            log::info!("Lazy-loading Vello renderer for advanced CSS/backdrop effects");
            *guard = Some(
                vello::Renderer::new(
                    &self.device,
                    vello::RendererOptions {
                        pipeline_cache: None,
                        ..Default::default()
                    },
                )
                .expect("Failed to create Vello renderer"),
            );
        }
    }

    /// Get a clone of the shared Vello renderer Arc for passing to EffectsRenderer
    pub fn vello_renderer_arc(&self) -> Arc<Mutex<Option<vello::Renderer>>> {
        self.vello_renderer.clone()
    }

    /// Reset the Vello renderer to clean up accumulated texture atlas resources.
    ///
    /// Vello's internal atlas/texture caches grow over time and don't have a
    /// built-in cleanup mechanism. Recreating the renderer periodically prevents
    /// unbounded GPU memory growth.
    pub fn reset_vello_renderer(&self) {
        let mut guard = self.vello_renderer.lock().unwrap();
        if guard.is_some() {
            log::info!("Resetting Vello renderer to free accumulated GPU resources");
            // Drop the old renderer
            *guard = None;
            // Create a new one
            *guard = Some(
                vello::Renderer::new(
                    &self.device,
                    vello::RendererOptions {
                        pipeline_cache: None,
                        ..Default::default()
                    },
                )
                .expect("Failed to recreate Vello renderer"),
            );
        }
    }
}

/// Per-window GPU state (surface tied to specific window)
pub struct WindowGpuState {
    pub surface: wgpu::Surface<'static>,
    pub config: wgpu::SurfaceConfiguration,

    // Text rendering with swash glyph cache (scales with zoom)
    pub glyph_cache: GlyphCache,
    // Grid renderer for cursor line (rendered with glow effect)
    pub grid_renderer: GridRenderer,
    // Grid renderer for output text (rendered flat, no glow)
    pub output_grid_renderer: GridRenderer,

    // Fixed-size glyph cache for tab titles (doesn't scale with zoom)
    pub tab_glyph_cache: GlyphCache,
    // Separate renderer for tab titles to avoid buffer conflicts
    // (terminal and tab titles render in different passes but the GPU
    // commands are batched, so they need separate instance buffers)
    pub tab_title_renderer: GridRenderer,

    // Effect pipeline
    pub effect_pipeline: EffectPipeline,

    // Backdrop effects renderer (grid, starfield, particles, etc.)
    pub effects_renderer: EffectsRenderer,

    // Tab bar
    pub tab_bar: TabBar,

    // Terminal vello renderer for cursor and selection
    pub terminal_vello: TerminalVelloRenderer,

    // Rect renderer for cell backgrounds and tab bar shapes
    pub rect_renderer: RectRenderer,

    // Separate rect renderer for overlays (cursor, selection, underlines)
    // to avoid buffer conflicts with tab bar rendering
    pub overlay_rect_renderer: RectRenderer,

    // Background image rendering (optional)
    pub background_image_pipeline: BackgroundImagePipeline,
    pub background_image_state: Option<BackgroundImageState>,
    pub background_image_bind_group: Option<wgpu::BindGroup>,

    // Sprite animation rendering (optional, bypasses vello for memory efficiency)
    pub sprite_state: Option<SpriteAnimationState>,

    // Intermediate text texture for glow effect
    // Text is rendered here first, then composited with Gaussian blur
    pub text_texture: wgpu::Texture,
    pub text_texture_view: wgpu::TextureView,
    pub composite_bind_group: wgpu::BindGroup,

    // CRT post-processing (optional - scanlines, curvature, vignette)
    pub crt_pipeline: CrtPipeline,
    // Intermediate texture for CRT post-processing
    // When CRT is enabled, everything renders here first, then CRT effect outputs to surface
    pub crt_texture: Option<wgpu::Texture>,
    pub crt_texture_view: Option<wgpu::TextureView>,
    pub crt_bind_group: Option<wgpu::BindGroup>,
}
