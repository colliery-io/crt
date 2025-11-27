//! Effects renderer - manages and renders backdrop effects to texture
//!
//! The EffectsRenderer maintains a collection of backdrop effects,
//! updates their animation state each frame, and renders them to a
//! texture via the shared Vello renderer.

use std::sync::{Arc, Mutex};

use vello::kurbo::Rect;
use vello::{peniko, AaConfig, RenderParams, Renderer, Scene};

use super::{BackdropEffect, EffectConfig};

/// Manages backdrop effects and renders them to a texture
///
/// # Usage
///
/// ```ignore
/// let effects_renderer = EffectsRenderer::new(vello_renderer);
///
/// // Configure from theme
/// effects_renderer.configure(&theme);
///
/// // Each frame:
/// effects_renderer.update(dt);
/// let texture = effects_renderer.render(device, queue, (width, height));
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
}

impl EffectsRenderer {
    /// Create a new effects renderer with shared Vello renderer
    pub fn new(vello_renderer: Arc<Mutex<Option<Renderer>>>) -> Self {
        Self {
            effects: Vec::new(),
            vello_renderer,
            scene: Scene::new(),
            target_texture: None,
            target_view: None,
            target_size: (0, 0),
            time: 0.0,
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

            self.target_texture = Some(texture);
            self.target_view = Some(view);
            self.target_size = (width, height);
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

        self.target_view.as_ref()
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
