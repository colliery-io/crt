//! Effects renderer - manages and renders backdrop effects to texture
//!
//! The EffectsRenderer maintains a collection of backdrop effects,
//! updates their animation state each frame, and renders them to a
//! texture via the shared Vello renderer.

use std::sync::{Arc, Mutex};

use vello::kurbo::Rect;
use vello::{AaConfig, RenderParams, Renderer, Scene, peniko};

use super::{BackdropEffect, EffectConfig};

/// Manages backdrop effects and renders them to a texture
///
/// # Usage
///
/// ```ignore
/// let effects_renderer = EffectsRenderer::new(device, vello_renderer, format);
///
/// // Configure from theme
/// effects_renderer.configure(&theme);
///
/// // Each frame:
/// effects_renderer.update(dt);
/// effects_renderer.render(device, queue, (width, height));
/// effects_renderer.composite(render_pass);
/// ```
pub struct EffectsRenderer {
    /// Collection of active effects
    effects: Vec<Box<dyn BackdropEffect>>,

    /// Shared Vello renderer (lazy-loaded, saves ~187MB when not used)
    vello_renderer: Arc<Mutex<Option<Renderer>>>,

    /// Vello scene built each frame
    scene: Scene,

    /// Render target texture
    target_texture: Option<wgpu::Texture>,

    /// Render target view
    target_view: Option<wgpu::TextureView>,

    /// Current target size
    target_size: (u32, u32),

    /// Total elapsed time
    time: f32,

    /// Blit pipeline for compositing effects to frame
    blit_pipeline: wgpu::RenderPipeline,

    /// Bind group layout for blit
    blit_bind_group_layout: wgpu::BindGroupLayout,

    /// Sampler for blit
    blit_sampler: wgpu::Sampler,

    /// Current bind group (recreated when texture changes)
    blit_bind_group: Option<wgpu::BindGroup>,
}

impl EffectsRenderer {
    /// Create a new effects renderer with shared Vello renderer
    pub fn new(
        device: &wgpu::Device,
        vello_renderer: Arc<Mutex<Option<Renderer>>>,
        format: wgpu::TextureFormat,
    ) -> Self {
        // Create blit shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Effects Blit Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/effects_blit.wgsl").into()),
        });

        // Bind group layout
        let blit_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Effects Blit Bind Group Layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        // Pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Effects Blit Pipeline Layout"),
            bind_group_layouts: &[&blit_bind_group_layout],
            push_constant_ranges: &[],
        });

        // Blit pipeline with alpha blending
        let blit_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Effects Blit Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            multiview: None,
            cache: None,
        });

        // Sampler
        let blit_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Effects Blit Sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        Self {
            effects: Vec::new(),
            vello_renderer,
            scene: Scene::new(),
            target_texture: None,
            target_view: None,
            target_size: (0, 0),
            time: 0.0,
            blit_pipeline,
            blit_bind_group_layout,
            blit_sampler,
            blit_bind_group: None,
        }
    }

    /// Add an effect to the renderer
    pub fn add_effect(&mut self, effect: Box<dyn BackdropEffect>) {
        self.effects.push(effect);
    }

    /// Remove all effects
    pub fn clear_effects(&mut self) {
        self.effects.clear();
    }

    /// Get a mutable reference to effects for configuration
    pub fn effects_mut(&mut self) -> &mut Vec<Box<dyn BackdropEffect>> {
        &mut self.effects
    }

    /// Configure all effects from theme config
    ///
    /// Each effect receives properties prefixed with its type.
    /// E.g., GridEffect receives properties like "grid-enabled", "grid-color".
    pub fn configure(&mut self, config: &EffectConfig) {
        for effect in &mut self.effects {
            // Extract properties for this effect type
            let prefix = format!("{}-", effect.effect_type());
            let mut effect_config = EffectConfig::new();

            for (key, value) in &config.properties {
                if let Some(suffix) = key.strip_prefix(&prefix) {
                    effect_config.insert(suffix.to_string(), value.clone());
                }
            }

            effect.configure(&effect_config);
            log::info!(
                "Configured effect '{}': enabled={}",
                effect.effect_type(),
                effect.is_enabled()
            );
        }
    }

    /// Update all effects' animation state
    ///
    /// # Arguments
    /// * `dt` - Delta time since last frame in seconds
    pub fn update(&mut self, dt: f32) {
        self.time += dt;

        for effect in &mut self.effects {
            if effect.is_enabled() {
                effect.update(dt, self.time);
            }
        }
    }

    /// Check if any effects are enabled
    pub fn has_enabled_effects(&self) -> bool {
        self.effects.iter().any(|e| e.is_enabled())
    }

    /// Apply a temporary patch configuration to a specific effect type
    ///
    /// This allows overriding specific effect properties without reconfiguring
    /// the entire effects system.
    pub fn apply_effect_patch(&mut self, effect_type: &str, config: &EffectConfig) {
        for effect in &mut self.effects {
            if effect.effect_type() == effect_type {
                effect.configure(config);
                log::debug!(
                    "Applied patch to effect '{}' with {} properties",
                    effect_type,
                    config.properties.len()
                );
                break;
            }
        }
    }

    /// Ensure render target is sized correctly
    fn ensure_target(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if self.target_size != (width, height) || self.target_texture.is_none() {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Effects Render Target"),
                size: wgpu::Extent3d {
                    width: width.max(1),
                    height: height.max(1),
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

            // Create bind group for blitting this texture
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Effects Blit Bind Group"),
                layout: &self.blit_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.blit_sampler),
                    },
                ],
            });

            self.target_texture = Some(texture);
            self.target_view = Some(view);
            self.blit_bind_group = Some(bind_group);
            self.target_size = (width, height);
        }
    }

    /// Composite the effects texture onto the frame
    ///
    /// Call this after render() to draw the effects onto the frame.
    /// The render pass should already be started with the frame as target.
    pub fn composite<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
        if let Some(bind_group) = &self.blit_bind_group {
            pass.set_pipeline(&self.blit_pipeline);
            pass.set_bind_group(0, bind_group, &[]);
            pass.draw(0..4, 0..1);
        }
    }

    /// Render all enabled effects to texture
    ///
    /// Returns the texture view for compositing, or None if no effects are enabled
    /// or the size is invalid.
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        size: (u32, u32),
    ) -> Option<&wgpu::TextureView> {
        let (width, height) = size;

        // Skip if no effects enabled or invalid size
        if !self.has_enabled_effects() || width == 0 || height == 0 {
            return None;
        }

        // Log once when we first render
        static LOGGED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        if !LOGGED.swap(true, std::sync::atomic::Ordering::Relaxed) {
            log::info!("Effects render starting: {}x{}", width, height);
        }

        // Ensure target is sized
        self.ensure_target(device, width, height);

        // Reset scene for new frame
        self.scene.reset();

        // Build scene from all enabled effects
        let bounds = Rect::new(0.0, 0.0, width as f64, height as f64);

        for effect in &self.effects {
            if effect.is_enabled() {
                effect.render(&mut self.scene, bounds);
            }
        }

        // Render scene to texture via shared Vello renderer
        let target_view = self.target_view.as_ref()?;

        let mut renderer_guard = self.vello_renderer.lock().ok()?;
        let renderer = renderer_guard.as_mut()?;

        let params = RenderParams {
            base_color: peniko::Color::TRANSPARENT,
            width,
            height,
            antialiasing_method: AaConfig::Area,
        };

        if let Err(e) = renderer.render_to_texture(device, queue, &self.scene, target_view, &params)
        {
            log::error!("Failed to render effects: {:?}", e);
            return None;
        }

        // Log success once
        static LOGGED2: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        if !LOGGED2.swap(true, std::sync::atomic::Ordering::Relaxed) {
            log::info!("Effects rendered to texture successfully");
        }

        self.target_view.as_ref()
    }

    /// Render all enabled effects directly to a target texture view
    ///
    /// This renders the effects scene directly to the provided target view,
    /// which can be the frame's surface texture for direct rendering.
    ///
    /// Returns true if rendering occurred, false if skipped (no effects enabled
    /// or invalid size).
    pub fn render_to_view(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target_view: &wgpu::TextureView,
        width: u32,
        height: u32,
    ) -> bool {
        // Skip if no effects enabled or invalid size
        if !self.has_enabled_effects() || width == 0 || height == 0 {
            return false;
        }

        // Prepare GPU resources for effects that need them (e.g., texture registration)
        let mut did_gpu_setup = false;
        {
            let mut renderer_guard = match self.vello_renderer.lock() {
                Ok(guard) => guard,
                Err(_) => return false,
            };
            let renderer = match renderer_guard.as_mut() {
                Some(r) => r,
                None => return false,
            };
            for effect in &mut self.effects {
                if effect.needs_gpu_resources() {
                    effect.prepare_gpu_resources(device, queue, renderer);
                    did_gpu_setup = true;
                }
            }
        }

        // Skip rendering on frames where we registered textures to avoid encoder conflicts
        if did_gpu_setup {
            log::debug!("Skipping render frame after GPU resource setup");
            return false;
        }

        // Reset scene for new frame
        self.scene.reset();

        // Build scene from all enabled effects
        let bounds = Rect::new(0.0, 0.0, width as f64, height as f64);

        for effect in &self.effects {
            if effect.is_enabled() {
                effect.render(&mut self.scene, bounds);
            }
        }

        // Render scene directly to target via shared Vello renderer
        let mut renderer_guard = match self.vello_renderer.lock() {
            Ok(guard) => guard,
            Err(_) => return false,
        };
        let renderer = match renderer_guard.as_mut() {
            Some(r) => r,
            None => return false,
        };

        let params = RenderParams {
            base_color: peniko::Color::TRANSPARENT,
            width,
            height,
            antialiasing_method: AaConfig::Area,
        };

        match renderer.render_to_texture(device, queue, &self.scene, target_view, &params) {
            Ok(_) => true,
            Err(e) => {
                log::error!("Failed to render effects to view: {:?}", e);
                false
            }
        }
    }

    /// Get the current render target texture view
    pub fn texture_view(&self) -> Option<&wgpu::TextureView> {
        self.target_view.as_ref()
    }

    /// Get current target size
    pub fn target_size(&self) -> (u32, u32) {
        self.target_size
    }

    /// Get total elapsed time
    pub fn elapsed_time(&self) -> f32 {
        self.time
    }

    /// Reset elapsed time
    pub fn reset_time(&mut self) {
        self.time = 0.0;
    }
}

impl Drop for EffectsRenderer {
    fn drop(&mut self) {
        // Destroy render target texture to release GPU memory
        if let Some(ref texture) = self.target_texture {
            texture.destroy();
        }
    }
}
