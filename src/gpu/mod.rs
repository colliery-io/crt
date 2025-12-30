//! GPU state management
//!
//! Shared and per-window GPU resources for wgpu rendering.
//!
//! ## Module Structure
//! - `buffer_pool` - Instance/uniform buffer pooling with RAII semantics
//! - `texture_pool` - Render target texture pooling by size bucket

mod buffer_pool;
mod texture_pool;

// Buffer pool exports - API ready, see buffer_pool.rs for full pool integration
#[allow(unused_imports)]
pub use buffer_pool::{BufferClass, BufferPool, PoolStats, PooledBuffer};

// Texture pool - fully integrated
pub use texture_pool::{PooledTexture, TexturePool};
#[allow(unused_imports)]
pub use texture_pool::{TextureBucket, TexturePoolStats};

use std::sync::{Arc, Mutex};

use crt_renderer::{
    BackgroundImagePipeline, BackgroundImageState, CrtPipeline, EffectPipeline, EffectsRenderer,
    GlyphCache, GridRenderer, RectRenderer, SpriteAnimationState, TabBar, TerminalVelloRenderer,
};

/// Shared GPU resources across all windows
pub struct SharedGpuState {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: Arc<wgpu::Device>,
    pub queue: wgpu::Queue,
    /// Shared Vello renderer - lazy loaded only when CSS effects need it
    /// (rounded corners, gradients, shadows, backdrop effects, etc.)
    /// Wrapped in Arc<Mutex> for sharing with EffectsRenderer
    pub vello_renderer: Arc<Mutex<Option<vello::Renderer>>>,
    /// Buffer pool for reusing instance/uniform buffers
    /// API ready - see buffer_pool.rs for full pool integration
    #[allow(dead_code)]
    pub buffer_pool: BufferPool,
    /// Texture pool for reusing render target textures (fully integrated)
    pub texture_pool: TexturePool,
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

        // Wrap device in Arc for sharing with buffer pool
        let device = Arc::new(device);

        // Vello renderer is lazy-loaded only when CSS effects need it
        // (rounded corners, gradients, shadows, backdrop effects, complex paths)
        let vello_renderer = Arc::new(Mutex::new(None));

        // Create buffer pool for reusing instance/uniform buffers
        // Max 4 buffers per class keeps memory reasonable while allowing reuse
        let buffer_pool = BufferPool::new(device.clone(), 4);

        // Create texture pool for reusing render target textures
        // Max 2 textures per bucket since textures are large (~8MB+ each)
        let texture_pool = TexturePool::new(device.clone(), 2);

        Self {
            instance,
            adapter,
            device,
            queue,
            vello_renderer,
            buffer_pool,
            texture_pool,
        }
    }

    /// Ensure the Vello renderer is initialized (lazy initialization)
    ///
    /// Call this when you need advanced CSS effects like rounded corners,
    /// gradients, backdrop effects, or complex paths. The renderer is
    /// cached after first creation.
    pub fn ensure_vello_renderer(&self) {
        let mut guard = match self.vello_renderer.lock() {
            Ok(g) => g,
            Err(e) => {
                log::error!(
                    "Vello renderer lock poisoned, skipping initialization: {}",
                    e
                );
                return;
            }
        };
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
        let mut guard = match self.vello_renderer.lock() {
            Ok(g) => g,
            Err(e) => {
                log::warn!("Vello renderer lock poisoned, skipping reset: {}", e);
                return;
            }
        };
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

    // Instance buffers for grid renderers (external for pooling)
    pub grid_instance_buffer: wgpu::Buffer,
    pub output_grid_instance_buffer: wgpu::Buffer,
    pub tab_title_instance_buffer: wgpu::Buffer,
    pub overlay_text_instance_buffer: wgpu::Buffer,

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

    // Instance buffers for rect renderers (external for pooling)
    pub rect_instance_buffer: wgpu::Buffer,
    pub overlay_rect_instance_buffer: wgpu::Buffer,

    // Background image rendering (optional)
    pub background_image_pipeline: BackgroundImagePipeline,
    pub background_image_state: Option<BackgroundImageState>,
    pub background_image_bind_group: Option<wgpu::BindGroup>,

    // Sprite animation rendering (optional, bypasses vello for memory efficiency)
    pub sprite_state: Option<SpriteAnimationState>,

    // Intermediate text texture for glow effect (pooled for memory reuse)
    // Text is rendered here first, then composited with Gaussian blur
    pub text_texture: PooledTexture,
    pub composite_bind_group: wgpu::BindGroup,

    // CRT post-processing (optional - scanlines, curvature, vignette)
    pub crt_pipeline: CrtPipeline,
    // Intermediate texture for CRT post-processing (pooled for memory reuse)
    // When CRT is enabled, everything renders here first, then CRT effect outputs to surface
    pub crt_texture: Option<PooledTexture>,
    pub crt_bind_group: Option<wgpu::BindGroup>,
}

impl WindowGpuState {
    /// Explicitly release GPU resources before dropping.
    ///
    /// Call this before removing WindowState to ensure proper cleanup.
    /// This unconfigures the surface which releases swap chain buffers (IOSurface on macOS).
    /// Note: PooledTextures (text_texture, crt_texture) are automatically returned
    /// to the pool when dropped, so we don't explicitly destroy them here.
    pub fn cleanup(&mut self, device: &wgpu::Device) {
        log::debug!("WindowGpuState cleanup - releasing GPU resources");

        // Note: text_texture and crt_texture are PooledTextures that will be
        // returned to the pool automatically when dropped. We don't destroy them
        // manually - the pool handles cleanup and potential reuse.

        // Unconfigure surface to release swap chain buffers (IOSurface on macOS)
        // This signals to the Metal driver that we're done with this surface
        self.surface.configure(
            device,
            &wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: self.config.format,
                width: 1,
                height: 1,
                present_mode: wgpu::PresentMode::Fifo,
                alpha_mode: self.config.alpha_mode,
                view_formats: vec![],
                desired_maximum_frame_latency: 1,
            },
        );

        // Poll to process the unconfigure
        let _ = device.poll(wgpu::PollType::Wait);
    }
}

impl Drop for WindowGpuState {
    fn drop(&mut self) {
        log::debug!("Dropping WindowGpuState");
        // Note: Most cleanup happens in cleanup() which should be called first.
        // Surface is cleaned up automatically on drop in wgpu 26+
        // Other resources (buffers, bind groups, pipelines) in sub-components
        // are cleaned up by their own Drop implementations
    }
}
