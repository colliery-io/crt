//! Shared render pipelines and samplers
//!
//! All stateless GPU resources that can be shared across windows.
//! Created once on first window and reused for all subsequent windows.

/// Shared render pipelines and samplers across all windows
///
/// These resources are stateless (no per-window data) and can be safely
/// shared. Each window creates its own buffers and bind groups that
/// reference these shared pipelines.
pub struct SharedPipelines {
    /// Shared linear filtering sampler (used by most pipelines)
    pub linear_sampler: wgpu::Sampler,
    /// Shared nearest filtering sampler (for pixel-perfect rendering)
    pub nearest_sampler: wgpu::Sampler,
    // TODO: Add shared pipelines in future phases:
    // pub grid_pipeline: wgpu::RenderPipeline,
    // pub grid_bind_group_layout: wgpu::BindGroupLayout,
    // pub rect_pipeline: wgpu::RenderPipeline,
    // pub rect_bind_group_layout: wgpu::BindGroupLayout,
    // pub background_pipeline: wgpu::RenderPipeline,
    // pub composite_pipeline: wgpu::RenderPipeline,
    // pub crt_pipeline: wgpu::RenderPipeline,
    // etc.
}

impl SharedPipelines {
    /// Create shared pipelines for the given texture format
    pub fn new(device: &wgpu::Device, _format: wgpu::TextureFormat) -> Self {
        log::debug!("Creating shared GPU samplers");

        // Linear filtering sampler - used for most texture sampling
        let linear_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Shared Linear Sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // Nearest filtering sampler - for pixel-perfect rendering
        let nearest_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Shared Nearest Sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        Self {
            linear_sampler,
            nearest_sampler,
        }
    }
}
