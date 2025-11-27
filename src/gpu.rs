//! GPU state management
//!
//! Shared and per-window GPU resources for wgpu rendering.

use crt_renderer::{GlyphCache, GridRenderer, RectRenderer, EffectPipeline, TextRenderTarget, TabBar, TerminalVelloRenderer, BackgroundImagePipeline, BackgroundImageState};

/// Shared GPU resources across all windows
pub struct SharedGpuState {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
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

        Self {
            instance,
            adapter,
            device,
            queue,
        }
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

    // Offscreen text target
    pub text_target: TextRenderTarget,

    // Effect pipeline
    pub effect_pipeline: EffectPipeline,
    pub composite_bind_group: Option<wgpu::BindGroup>,

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
}
