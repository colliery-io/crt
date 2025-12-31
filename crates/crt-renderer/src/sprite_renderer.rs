//! GPU-accelerated sprite renderer using raw wgpu
//!
//! Renders animated sprites directly via wgpu, bypassing vello's atlas system
//! to avoid memory growth issues.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use wgpu::util::DeviceExt;

use crt_theme::{SpriteOverlay, SpriteOverlayPosition, SpritePatch};
use image::GenericImageView;

/// Sprite position on screen
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SpritePosition {
    /// Fixed position at coordinates
    Fixed(f32, f32),
    /// Centered on screen
    Center,
    /// Bottom-right corner
    BottomRight,
    /// Bottom-left corner
    BottomLeft,
    /// Top-right corner
    TopRight,
    /// Top-left corner
    TopLeft,
}

impl Default for SpritePosition {
    fn default() -> Self {
        SpritePosition::BottomRight
    }
}

impl SpritePosition {
    /// Parse position from string
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "center" => SpritePosition::Center,
            "bottom-right" | "bottomright" => SpritePosition::BottomRight,
            "bottom-left" | "bottomleft" => SpritePosition::BottomLeft,
            "top-right" | "topright" => SpritePosition::TopRight,
            "top-left" | "topleft" => SpritePosition::TopLeft,
            _ => SpritePosition::BottomRight,
        }
    }

    /// Calculate actual position given screen and sprite dimensions
    pub fn resolve(
        &self,
        screen_w: f32,
        screen_h: f32,
        sprite_w: f32,
        sprite_h: f32,
        margin: f32,
    ) -> (f32, f32) {
        match self {
            SpritePosition::Fixed(x, y) => (*x, *y),
            SpritePosition::Center => (screen_w / 2.0, screen_h / 2.0),
            SpritePosition::BottomRight => (
                screen_w - sprite_w / 2.0 - margin,
                screen_h - sprite_h / 2.0 - margin,
            ),
            SpritePosition::BottomLeft => {
                (sprite_w / 2.0 + margin, screen_h - sprite_h / 2.0 - margin)
            }
            SpritePosition::TopRight => {
                (screen_w - sprite_w / 2.0 - margin, sprite_h / 2.0 + margin)
            }
            SpritePosition::TopLeft => (sprite_w / 2.0 + margin, sprite_h / 2.0 + margin),
        }
    }
}

/// Motion behavior for sprite
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SpriteMotion {
    /// No motion - stay at position
    Static,
    /// Bounce around screen
    Bounce,
    /// Horizontal patrol
    Patrol,
    /// Random wandering
    Wander,
}

impl Default for SpriteMotion {
    fn default() -> Self {
        SpriteMotion::Static
    }
}

impl SpriteMotion {
    /// Parse motion from string
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "bounce" => SpriteMotion::Bounce,
            "patrol" => SpriteMotion::Patrol,
            "wander" => SpriteMotion::Wander,
            _ => SpriteMotion::Static,
        }
    }
}

/// Loaded sprite sheet data
pub struct SpriteSheet {
    /// Raw RGBA pixel data
    data: Vec<u8>,
    /// Sheet dimensions
    pub width: u32,
    pub height: u32,
    /// Frame dimensions
    pub frame_width: u32,
    pub frame_height: u32,
    /// Grid layout
    pub columns: u32,
    pub rows: u32,
    /// Total frame count
    pub frame_count: u32,
}

impl SpriteSheet {
    /// Load sprite sheet from file
    pub fn load(
        path: &Path,
        frame_width: u32,
        frame_height: u32,
        columns: u32,
        rows: u32,
        frame_count: Option<u32>,
    ) -> Result<Self, String> {
        let img = image::open(path)
            .map_err(|e| format!("Failed to load sprite sheet {:?}: {}", path, e))?;

        let rgba = img.to_rgba8();
        let (width, height) = rgba.dimensions();

        // Validate dimensions
        let expected_width = frame_width * columns;
        let expected_height = frame_height * rows;

        if width < expected_width || height < expected_height {
            return Err(format!(
                "Sprite sheet {}x{} too small for {}x{} grid of {}x{} frames",
                width, height, columns, rows, frame_width, frame_height
            ));
        }

        let max_frames = columns * rows;
        let frame_count = frame_count.unwrap_or(max_frames).min(max_frames);

        log::info!(
            "Loaded sprite sheet {:?}: {}x{}, {}x{} frames, {} total",
            path,
            width,
            height,
            columns,
            rows,
            frame_count
        );

        Ok(Self {
            data: rgba.into_raw(),
            width,
            height,
            frame_width,
            frame_height,
            columns,
            rows,
            frame_count,
        })
    }

    /// Get UV coordinates for a frame (offset_x, offset_y, size_x, size_y)
    pub fn frame_uv(&self, frame: u32) -> [f32; 4] {
        let col = frame % self.columns;
        let row = frame / self.columns;

        let offset_x = (col * self.frame_width) as f32 / self.width as f32;
        let offset_y = (row * self.frame_height) as f32 / self.height as f32;
        let size_x = self.frame_width as f32 / self.width as f32;
        let size_y = self.frame_height as f32 / self.height as f32;

        [offset_x, offset_y, size_x, size_y]
    }
}

/// GPU texture for sprite sheet
pub struct SpriteTexture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
}

impl SpriteTexture {
    /// Create GPU texture from sprite sheet
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, sheet: &SpriteSheet) -> Self {
        let texture = device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some("Sprite Sheet Texture"),
                size: wgpu::Extent3d {
                    width: sheet.width,
                    height: sheet.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &sheet.data,
        );

        let view = texture.create_view(&Default::default());

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Sprite Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        Self {
            texture,
            view,
            sampler,
        }
    }
}

/// Uniforms for sprite rendering
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct SpriteUniforms {
    /// Position (xy) and half-size (zw) in NDC
    transform: [f32; 4],
    /// Frame UV offset (xy) and size (zw)
    frame_uv: [f32; 4],
    /// Opacity (x), padding (yzw)
    params: [f32; 4],
}

/// Sprite renderer using raw wgpu
pub struct SpriteRenderer {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    uniform_buffer: wgpu::Buffer,
    bind_group: Option<wgpu::BindGroup>,
    texture: Option<SpriteTexture>,
    sheet: Option<Arc<SpriteSheet>>,
}

impl SpriteRenderer {
    /// Create a new sprite renderer
    pub fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Sprite Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/sprite.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Sprite Bind Group Layout"),
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
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Sprite Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Sprite Pipeline"),
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
                    format: target_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            multiview: None,
            cache: None,
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Sprite Uniform Buffer"),
            size: std::mem::size_of::<SpriteUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            bind_group_layout,
            uniform_buffer,
            bind_group: None,
            texture: None,
            sheet: None,
        }
    }

    /// Load a sprite sheet
    pub fn load_sheet(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, sheet: SpriteSheet) {
        let texture = SpriteTexture::new(device, queue, &sheet);

        // Create bind group with texture
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Sprite Bind Group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&texture.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.uniform_buffer.as_entire_binding(),
                },
            ],
        });

        self.texture = Some(texture);
        self.bind_group = Some(bind_group);
        self.sheet = Some(Arc::new(sheet));

        log::info!("Sprite sheet loaded and GPU resources created");
    }

    /// Check if a sprite sheet is loaded
    pub fn is_loaded(&self) -> bool {
        self.sheet.is_some()
    }

    /// Get the sprite sheet (for frame info)
    pub fn sheet(&self) -> Option<&SpriteSheet> {
        self.sheet.as_ref().map(|s| s.as_ref())
    }

    /// Render the sprite
    ///
    /// # Arguments
    /// * `pass` - Render pass to draw into
    /// * `queue` - Queue for updating uniforms
    /// * `frame` - Current animation frame
    /// * `screen_width` - Screen width in pixels
    /// * `screen_height` - Screen height in pixels
    /// * `x` - Sprite center X position in pixels
    /// * `y` - Sprite center Y position in pixels
    /// * `scale` - Display scale factor
    /// * `opacity` - Opacity (0.0-1.0)
    pub fn render<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        queue: &wgpu::Queue,
        frame: u32,
        screen_width: f32,
        screen_height: f32,
        x: f32,
        y: f32,
        scale: f32,
        opacity: f32,
    ) {
        let Some(sheet) = &self.sheet else { return };
        let Some(bind_group) = &self.bind_group else {
            return;
        };

        // Calculate sprite size in pixels
        let sprite_w = sheet.frame_width as f32 * scale;
        let sprite_h = sheet.frame_height as f32 * scale;

        // Convert position to NDC (-1 to 1)
        // Note: Y is flipped (0 at top in screen coords, -1 at top in NDC)
        let ndc_x = (x / screen_width) * 2.0 - 1.0;
        let ndc_y = 1.0 - (y / screen_height) * 2.0;

        // Half-size in NDC
        let half_w = sprite_w / screen_width;
        let half_h = sprite_h / screen_height;

        // Get frame UV
        let frame_uv = sheet.frame_uv(frame % sheet.frame_count);

        let uniforms = SpriteUniforms {
            transform: [ndc_x, ndc_y, half_w, half_h],
            frame_uv,
            params: [opacity, 0.0, 0.0, 0.0],
        };

        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, bind_group, &[]);
        pass.draw(0..4, 0..1);
    }
}

/// Configuration for sprite animation
#[derive(Debug, Clone)]
pub struct SpriteConfig {
    /// Path to sprite sheet
    pub path: PathBuf,
    /// Frame dimensions
    pub frame_width: u32,
    pub frame_height: u32,
    /// Grid layout
    pub columns: u32,
    pub rows: u32,
    /// Total frame count (if less than grid)
    pub frame_count: Option<u32>,
    /// Animation frames per second
    pub fps: f32,
    /// Display scale
    pub scale: f32,
    /// Opacity
    pub opacity: f32,
    /// Position anchor
    pub position: SpritePosition,
    /// Motion behavior
    pub motion: SpriteMotion,
    /// Motion speed multiplier
    pub motion_speed: f32,
    /// Base directory for resolving relative paths (from theme)
    pub base_dir: PathBuf,
}

/// Stored original sprite values for restoration after patch expires
#[derive(Debug, Clone)]
pub struct OriginalSpriteValues {
    pub fps: f32,
    pub opacity: f32,
    pub scale: f32,
    pub motion_speed: f32,
    pub path: String,
}

/// Complete sprite animation state (similar to BackgroundImageState)
pub struct SpriteAnimationState {
    /// Renderer for GPU operations
    renderer: SpriteRenderer,
    /// Configuration
    config: SpriteConfig,
    /// Current animation frame
    current_frame: u32,
    /// Total frames in animation
    total_frames: u32,
    /// Time accumulator for frame timing
    frame_time_accum: f32,
    /// Seconds per frame
    seconds_per_frame: f32,
    /// Current position (for motion)
    pos_x: f32,
    pos_y: f32,
    /// Velocity (for bounce/wander)
    vel_x: f32,
    vel_y: f32,
    /// Whether position has been initialized
    position_initialized: bool,
    /// Sprite dimensions (scaled)
    sprite_width: f32,
    sprite_height: f32,
    /// Original values for restoration after patch expires
    original_values: OriginalSpriteValues,
    /// Whether a patch is currently applied
    patch_applied: bool,
    /// Base directory for resolving relative sprite paths (from theme)
    base_dir: std::path::PathBuf,
    /// Original sprite sheet for restoration after patch
    original_sheet: Option<Arc<SpriteSheet>>,
}

impl SpriteAnimationState {
    /// Create new sprite animation state from config
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        config: SpriteConfig,
        target_format: wgpu::TextureFormat,
    ) -> Result<Self, String> {
        let mut renderer = SpriteRenderer::new(device, target_format);

        // Load sprite sheet
        let sheet = SpriteSheet::load(
            &config.path,
            config.frame_width,
            config.frame_height,
            config.columns,
            config.rows,
            config.frame_count,
        )?;

        let total_frames = sheet.frame_count;
        let sprite_width = sheet.frame_width as f32 * config.scale;
        let sprite_height = sheet.frame_height as f32 * config.scale;

        renderer.load_sheet(device, queue, sheet);

        let seconds_per_frame = if config.fps > 0.0 {
            1.0 / config.fps
        } else {
            1.0 / 12.0
        };

        // Initialize velocity for motion
        let (vel_x, vel_y) = match config.motion {
            SpriteMotion::Static => (0.0, 0.0),
            SpriteMotion::Bounce => (100.0 * config.motion_speed, 75.0 * config.motion_speed),
            SpriteMotion::Patrol => (80.0 * config.motion_speed, 0.0),
            SpriteMotion::Wander => (50.0 * config.motion_speed, 30.0 * config.motion_speed),
        };

        // Store original values for restoration after patches
        let original_values = OriginalSpriteValues {
            fps: config.fps,
            opacity: config.opacity,
            scale: config.scale,
            motion_speed: config.motion_speed,
            path: config.path.to_string_lossy().to_string(),
        };

        // Use base_dir from config (set from theme directory)
        let base_dir = config.base_dir.clone();

        Ok(Self {
            renderer,
            config,
            current_frame: 0,
            total_frames,
            frame_time_accum: 0.0,
            seconds_per_frame,
            pos_x: 0.0,
            pos_y: 0.0,
            vel_x,
            vel_y,
            position_initialized: false,
            sprite_width,
            sprite_height,
            original_values,
            patch_applied: false,
            base_dir,
            original_sheet: None,
        })
    }

    /// Update animation state
    pub fn update(&mut self, dt: f32, screen_width: f32, screen_height: f32) {
        // Initialize position on first update (need screen dimensions)
        if !self.position_initialized {
            let (x, y) = self.config.position.resolve(
                screen_width,
                screen_height,
                self.sprite_width,
                self.sprite_height,
                20.0, // margin
            );
            self.pos_x = x;
            self.pos_y = y;
            self.position_initialized = true;
        }

        // Update animation frame
        self.frame_time_accum += dt;
        while self.frame_time_accum >= self.seconds_per_frame {
            self.frame_time_accum -= self.seconds_per_frame;
            self.current_frame = (self.current_frame + 1) % self.total_frames;
        }

        // Update position based on motion
        match self.config.motion {
            SpriteMotion::Static => {
                // Recalculate position for static (handles resize)
                let (x, y) = self.config.position.resolve(
                    screen_width,
                    screen_height,
                    self.sprite_width,
                    self.sprite_height,
                    20.0,
                );
                self.pos_x = x;
                self.pos_y = y;
            }
            SpriteMotion::Bounce => {
                self.pos_x += self.vel_x * dt;
                self.pos_y += self.vel_y * dt;

                let half_w = self.sprite_width / 2.0;
                let half_h = self.sprite_height / 2.0;

                // Bounce off walls
                if self.pos_x - half_w < 0.0 {
                    self.pos_x = half_w;
                    self.vel_x = self.vel_x.abs();
                } else if self.pos_x + half_w > screen_width {
                    self.pos_x = screen_width - half_w;
                    self.vel_x = -self.vel_x.abs();
                }

                if self.pos_y - half_h < 0.0 {
                    self.pos_y = half_h;
                    self.vel_y = self.vel_y.abs();
                } else if self.pos_y + half_h > screen_height {
                    self.pos_y = screen_height - half_h;
                    self.vel_y = -self.vel_y.abs();
                }
            }
            SpriteMotion::Patrol => {
                self.pos_x += self.vel_x * dt;

                let half_w = self.sprite_width / 2.0;

                if self.pos_x - half_w < 0.0 {
                    self.pos_x = half_w;
                    self.vel_x = self.vel_x.abs();
                } else if self.pos_x + half_w > screen_width {
                    self.pos_x = screen_width - half_w;
                    self.vel_x = -self.vel_x.abs();
                }
            }
            SpriteMotion::Wander => {
                // Simple random-ish wandering using time-based sine waves
                let time = self.frame_time_accum + self.current_frame as f32 * 0.1;
                self.vel_x = (time * 0.7).sin() * 100.0 * self.config.motion_speed;
                self.vel_y = (time * 1.1).cos() * 60.0 * self.config.motion_speed;

                self.pos_x += self.vel_x * dt;
                self.pos_y += self.vel_y * dt;

                // Keep in bounds
                let half_w = self.sprite_width / 2.0;
                let half_h = self.sprite_height / 2.0;

                self.pos_x = self.pos_x.clamp(half_w, screen_width - half_w);
                self.pos_y = self.pos_y.clamp(half_h, screen_height - half_h);
            }
        }
    }

    /// Render the sprite
    pub fn render<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        queue: &wgpu::Queue,
        screen_width: f32,
        screen_height: f32,
    ) {
        self.renderer.render(
            pass,
            queue,
            self.current_frame,
            screen_width,
            screen_height,
            self.pos_x,
            self.pos_y,
            self.config.scale,
            self.config.opacity,
        );
    }

    /// Check if loaded
    pub fn is_loaded(&self) -> bool {
        self.renderer.is_loaded()
    }

    /// Check if a patch is currently applied
    pub fn has_patch(&self) -> bool {
        self.patch_applied
    }

    /// Apply a sprite patch, overriding specific properties
    ///
    /// The patch preserves the current position and motion pattern while
    /// allowing properties like fps, opacity, scale, and motion_speed to
    /// be temporarily overridden.
    ///
    /// Note: Changing the sprite path requires device/queue and is not
    /// yet supported - patches can only modify animation properties.
    pub fn apply_patch(&mut self, patch: &SpritePatch) {
        // Apply FPS override
        if let Some(fps) = patch.fps {
            self.config.fps = fps;
            self.seconds_per_frame = if fps > 0.0 { 1.0 / fps } else { 1.0 / 12.0 };
            log::debug!("Sprite patch: fps = {}", fps);
        }

        // Apply opacity override
        if let Some(opacity) = patch.opacity {
            self.config.opacity = opacity;
            log::debug!("Sprite patch: opacity = {}", opacity);
        }

        // Apply scale override
        if let Some(scale) = patch.scale {
            // Update config and recalculate sprite dimensions
            let old_scale = self.config.scale;
            self.config.scale = scale;

            // Recalculate sprite dimensions using frame dimensions from renderer
            if let Some(sheet) = self.renderer.sheet() {
                self.sprite_width = sheet.frame_width as f32 * scale;
                self.sprite_height = sheet.frame_height as f32 * scale;
            } else {
                // Fallback: scale proportionally
                self.sprite_width *= scale / old_scale;
                self.sprite_height *= scale / old_scale;
            }
            log::debug!("Sprite patch: scale = {}", scale);
        }

        // Apply motion speed override
        if let Some(motion_speed) = patch.motion_speed {
            let old_speed = self.config.motion_speed;
            self.config.motion_speed = motion_speed;

            // Scale velocity proportionally
            if old_speed > 0.0 {
                let ratio = motion_speed / old_speed;
                self.vel_x *= ratio;
                self.vel_y *= ratio;
            }
            log::debug!("Sprite patch: motion_speed = {}", motion_speed);
        }

        self.patch_applied = true;
        log::debug!("Sprite patch applied");
    }

    /// Apply a sprite patch with device/queue access for texture reloading
    pub fn apply_patch_with_device(
        &mut self,
        patch: &crt_theme::SpritePatch,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) {
        // Handle path change - reload texture
        if let Some(ref new_path) = patch.path {
            // Store original sheet before replacing
            if self.original_sheet.is_none() {
                self.original_sheet = self.renderer.sheet.clone();
            }

            // Resolve the path relative to base_dir (theme directory)
            let full_path = self.base_dir.join(new_path);

            log::info!("Loading patch sprite from: {:?}", full_path);

            // Load new sprite sheet with same frame dimensions
            match SpriteSheet::load(
                &full_path,
                self.config.frame_width,
                self.config.frame_height,
                self.config.columns,
                self.config.rows,
                self.config.frame_count,
            ) {
                Ok(sheet) => {
                    self.renderer.load_sheet(device, queue, sheet);
                    log::info!("Sprite patch: loaded new sprite sheet");
                }
                Err(e) => {
                    log::error!("Failed to load patch sprite: {}", e);
                }
            }
        }

        // Apply other patch values (fps, opacity, scale, motion_speed)
        self.apply_patch(patch);
    }

    /// Restore original sprite values after patch expires
    pub fn restore(&mut self) {
        if !self.patch_applied {
            return;
        }

        // Restore FPS
        self.config.fps = self.original_values.fps;
        self.seconds_per_frame = if self.original_values.fps > 0.0 {
            1.0 / self.original_values.fps
        } else {
            1.0 / 12.0
        };

        // Restore opacity
        self.config.opacity = self.original_values.opacity;

        // Restore scale and recalculate dimensions
        let old_scale = self.config.scale;
        self.config.scale = self.original_values.scale;

        if let Some(sheet) = self.renderer.sheet() {
            self.sprite_width = sheet.frame_width as f32 * self.original_values.scale;
            self.sprite_height = sheet.frame_height as f32 * self.original_values.scale;
        } else {
            let ratio = self.original_values.scale / old_scale;
            self.sprite_width *= ratio;
            self.sprite_height *= ratio;
        }

        // Restore motion speed and velocity
        let old_speed = self.config.motion_speed;
        self.config.motion_speed = self.original_values.motion_speed;

        if old_speed > 0.0 {
            let ratio = self.original_values.motion_speed / old_speed;
            self.vel_x *= ratio;
            self.vel_y *= ratio;
        }

        self.patch_applied = false;
        log::debug!("Sprite restored to original values");
    }

    /// Restore original sprite values with device access for texture reloading
    pub fn restore_with_device(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        if !self.patch_applied {
            return;
        }

        // Restore original sprite sheet if it was changed
        if let Some(ref original_sheet) = self.original_sheet.take() {
            // Clone the sheet to reload
            let sheet = SpriteSheet {
                data: original_sheet.data.clone(),
                width: original_sheet.width,
                height: original_sheet.height,
                frame_width: original_sheet.frame_width,
                frame_height: original_sheet.frame_height,
                columns: original_sheet.columns,
                rows: original_sheet.rows,
                frame_count: original_sheet.frame_count,
            };
            self.renderer.load_sheet(device, queue, sheet);
            log::info!("Restored original sprite sheet");
        }

        // Restore other values
        self.restore();
    }

    /// Check if a sprite path change is pending (needs device access)
    pub fn needs_device_for_patch(patch: &crt_theme::SpritePatch) -> bool {
        patch.path.is_some()
    }

    /// Get the original (un-patched) sprite values
    pub fn original_values(&self) -> &OriginalSpriteValues {
        &self.original_values
    }

    /// Get current position (for overlay tracking)
    pub fn current_position(&self) -> (f32, f32) {
        (self.pos_x, self.pos_y)
    }
}

/// One-shot overlay sprite state for event effects
///
/// Unlike SpriteAnimationState which loops continuously, this plays
/// the animation once and marks itself as completed.
pub struct SpriteOverlayState {
    /// Renderer for GPU operations
    renderer: SpriteRenderer,
    /// Configuration from theme
    config: SpriteOverlay,
    /// Current animation frame
    current_frame: u32,
    /// Total frames in animation
    total_frames: u32,
    /// Time accumulator for frame timing
    frame_time_accum: f32,
    /// Seconds per frame
    seconds_per_frame: f32,
    /// Computed position (x, y)
    position: (f32, f32),
    /// Initial random position (stored for Random position type)
    random_position: Option<(f32, f32)>,
    /// Whether the animation has completed (played all frames once)
    completed: bool,
    /// Sprite dimensions (scaled) - retained for potential future use
    #[allow(dead_code)]
    sprite_width: f32,
    #[allow(dead_code)]
    sprite_height: f32,
}

impl SpriteOverlayState {
    /// Create a new sprite overlay from config
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        config: SpriteOverlay,
        base_path: &Path,
        target_format: wgpu::TextureFormat,
        screen_width: f32,
        screen_height: f32,
    ) -> Result<Self, String> {
        let mut renderer = SpriteRenderer::new(device, target_format);

        // Resolve path relative to base (theme directory)
        let sprite_path = base_path.join(&config.path);

        // Calculate frame dimensions from sheet and grid
        // We need to load the image to get its dimensions first
        let img = image::open(&sprite_path)
            .map_err(|e| format!("Failed to load overlay sprite {:?}: {}", sprite_path, e))?;
        let (sheet_width, sheet_height) = img.dimensions();

        let frame_width = sheet_width / config.columns;
        let frame_height = sheet_height / config.rows;

        // Load sprite sheet
        let sheet = SpriteSheet::load(
            &sprite_path,
            frame_width,
            frame_height,
            config.columns,
            config.rows,
            None, // Use all frames
        )?;

        let total_frames = sheet.frame_count;
        let sprite_width = frame_width as f32 * config.scale;
        let sprite_height = frame_height as f32 * config.scale;

        renderer.load_sheet(device, queue, sheet);

        let seconds_per_frame = if config.fps > 0.0 {
            1.0 / config.fps
        } else {
            1.0 / 12.0
        };

        // Calculate initial position based on position type
        let (position, random_position) = match config.position {
            SpriteOverlayPosition::Center => ((screen_width / 2.0, screen_height / 2.0), None),
            SpriteOverlayPosition::Random => {
                // Generate random position within screen bounds
                use std::time::{SystemTime, UNIX_EPOCH};
                let seed = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos() as u64;

                let margin = sprite_width.max(sprite_height) / 2.0 + 20.0;
                let x_range = screen_width - 2.0 * margin;
                let y_range = screen_height - 2.0 * margin;

                // Simple pseudo-random using seed
                let x = margin + (seed % 1000) as f32 / 1000.0 * x_range;
                let y = margin + ((seed / 1000) % 1000) as f32 / 1000.0 * y_range;

                let pos = (x, y);
                (pos, Some(pos))
            }
            // Cursor and Sprite positions are updated dynamically
            SpriteOverlayPosition::Cursor | SpriteOverlayPosition::Sprite => {
                ((screen_width / 2.0, screen_height / 2.0), None) // Default until first update
            }
        };

        log::info!(
            "Created sprite overlay: {:?}, {}x{} frames, position: {:?}",
            sprite_path,
            config.columns,
            config.rows,
            config.position
        );

        Ok(Self {
            renderer,
            config,
            current_frame: 0,
            total_frames,
            frame_time_accum: 0.0,
            seconds_per_frame,
            position,
            random_position,
            completed: false,
            sprite_width,
            sprite_height,
        })
    }

    /// Update overlay animation state
    ///
    /// # Arguments
    /// * `dt` - Delta time in seconds
    /// * `backdrop_pos` - Current backdrop sprite position (for Sprite position type)
    /// * `cursor_pos` - Current text cursor position in pixels (for Cursor position type)
    /// * `screen_width` - Screen width for bounds checking
    /// * `screen_height` - Screen height for bounds checking
    pub fn update(
        &mut self,
        dt: f32,
        backdrop_pos: Option<(f32, f32)>,
        cursor_pos: (f32, f32),
        screen_width: f32,
        screen_height: f32,
    ) {
        if self.completed {
            return;
        }

        // Update position based on position type
        match self.config.position {
            SpriteOverlayPosition::Center => {
                self.position = (screen_width / 2.0, screen_height / 2.0);
            }
            SpriteOverlayPosition::Cursor => {
                self.position = cursor_pos;
            }
            SpriteOverlayPosition::Sprite => {
                if let Some(pos) = backdrop_pos {
                    self.position = pos;
                }
            }
            SpriteOverlayPosition::Random => {
                // Keep the initially computed random position
                if let Some(pos) = self.random_position {
                    self.position = pos;
                }
            }
        }

        // Advance animation (one-shot - stop at last frame)
        self.frame_time_accum += dt;
        while self.frame_time_accum >= self.seconds_per_frame && !self.completed {
            self.frame_time_accum -= self.seconds_per_frame;
            self.current_frame += 1;

            if self.current_frame >= self.total_frames {
                self.completed = true;
                self.current_frame = self.total_frames - 1; // Stay on last frame
                log::debug!("Sprite overlay animation completed");
            }
        }
    }

    /// Render the overlay sprite
    pub fn render<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        queue: &wgpu::Queue,
        screen_width: f32,
        screen_height: f32,
    ) {
        if self.completed {
            return;
        }

        self.renderer.render(
            pass,
            queue,
            self.current_frame,
            screen_width,
            screen_height,
            self.position.0,
            self.position.1,
            self.config.scale,
            self.config.opacity,
        );
    }

    /// Check if the overlay animation has completed
    pub fn is_completed(&self) -> bool {
        self.completed
    }

    /// Check if the overlay is loaded and ready
    pub fn is_loaded(&self) -> bool {
        self.renderer.is_loaded()
    }

    /// Get current animation progress (0.0 to 1.0)
    pub fn progress(&self) -> f32 {
        if self.total_frames == 0 {
            1.0
        } else {
            self.current_frame as f32 / self.total_frames as f32
        }
    }

    /// Get the position type
    pub fn position_type(&self) -> SpriteOverlayPosition {
        self.config.position
    }
}
