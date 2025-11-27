//! GPU-accelerated sprite renderer using raw wgpu
//!
//! Renders animated sprites directly via wgpu, bypassing vello's atlas system
//! to avoid memory growth issues.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use wgpu::util::DeviceExt;

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
    pub fn resolve(&self, screen_w: f32, screen_h: f32, sprite_w: f32, sprite_h: f32, margin: f32) -> (f32, f32) {
        match self {
            SpritePosition::Fixed(x, y) => (*x, *y),
            SpritePosition::Center => (screen_w / 2.0, screen_h / 2.0),
            SpritePosition::BottomRight => (screen_w - sprite_w / 2.0 - margin, screen_h - sprite_h / 2.0 - margin),
            SpritePosition::BottomLeft => (sprite_w / 2.0 + margin, screen_h - sprite_h / 2.0 - margin),
            SpritePosition::TopRight => (screen_w - sprite_w / 2.0 - margin, sprite_h / 2.0 + margin),
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
            path, width, height, columns, rows, frame_count
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

        Self { texture, view, sampler }
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
    pub fn load_sheet(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        sheet: SpriteSheet,
    ) {
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
        let Some(bind_group) = &self.bind_group else { return };

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
}

impl Default for SpriteConfig {
    fn default() -> Self {
        Self {
            path: PathBuf::new(),
            frame_width: 32,
            frame_height: 32,
            columns: 1,
            rows: 1,
            frame_count: None,
            fps: 12.0,
            scale: 1.0,
            opacity: 1.0,
            position: SpritePosition::BottomRight,
            motion: SpriteMotion::Static,
            motion_speed: 1.0,
        }
    }
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
    pub fn render<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>, queue: &wgpu::Queue, screen_width: f32, screen_height: f32) {
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
}
