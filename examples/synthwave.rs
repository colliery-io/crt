//! Synthwave Theme Prototype
//!
//! Combines all CSS-like effects from the Hyper theme:
//! - Linear gradient background
//! - Perspective grid (synthwave floor)
//! - Text glow effects
//!
//! Based on the Hyper terminal theme with:
//! - Deep purple to light purple gradient
//! - Pink grid lines with perspective
//! - Teal text with glow

use bytemuck::{Pod, Zeroable};
use std::sync::Arc;
use std::time::Instant;
use wgpu::util::DeviceExt;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

// ---------------------------------------------------------------------------
// CSS-like Theme Configuration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self {
            r: r as f32 / 255.0,
            g: g as f32 / 255.0,
            b: b as f32 / 255.0,
            a: 1.0,
        }
    }

    pub const fn rgba(r: u8, g: u8, b: u8, a: f32) -> Self {
        Self {
            r: r as f32 / 255.0,
            g: g as f32 / 255.0,
            b: b as f32 / 255.0,
            a,
        }
    }

    pub fn to_array(&self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
    }
}

// Theme colors from the Hyper config
pub mod colors {
    use super::Color;
    pub const TEAL: Color = Color::rgb(97, 226, 254);
    pub const GOLD: Color = Color::rgb(254, 203, 0);
    pub const PINK: Color = Color::rgb(250, 25, 153);
    pub const PURPLE: Color = Color::rgb(88, 0, 226);
    pub const DEEP_PURPLE: Color = Color::rgb(32, 9, 51);
    pub const LIGHT_PURPLE: Color = Color::rgb(67, 9, 73);
}

/// Complete theme configuration
#[derive(Debug, Clone)]
pub struct Theme {
    // Background gradient
    pub gradient_top: Color,
    pub gradient_bottom: Color,

    // Grid
    pub grid_color: Color,
    pub grid_spacing: f32,
    pub grid_line_width: f32,
    pub grid_perspective: f32,
    pub grid_horizon: f32,
    pub grid_intensity: f32,

    // Text
    pub text_color: Color,
    pub glow_color: Color,
    pub glow_radius: f32,
    pub glow_intensity: f32,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            // background-image: linear-gradient(to bottom, DEEP_PURPLE 70%, LIGHT_PURPLE)
            gradient_top: colors::DEEP_PURPLE,
            gradient_bottom: colors::LIGHT_PURPLE,

            // Grid from the :after pseudo-element
            grid_color: Color::rgba(252, 25, 154, 0.15), // Pink with low alpha
            grid_spacing: 8.0,
            grid_line_width: 0.02,
            grid_perspective: 1.5,
            grid_horizon: 0.6, // Grid starts 60% down the screen
            grid_intensity: 1.0,

            // text-shadow: 0px 0px 10px teal, 0px 0px 20px teal
            text_color: colors::TEAL,
            glow_color: colors::TEAL,
            glow_radius: 15.0,
            glow_intensity: 0.8,
        }
    }
}

impl Theme {
    pub fn gold_variant() -> Self {
        Self {
            text_color: colors::GOLD,
            glow_color: colors::GOLD,
            grid_color: Color::rgba(254, 203, 0, 0.1),
            ..Default::default()
        }
    }

    pub fn intense_grid() -> Self {
        Self {
            grid_intensity: 1.5,
            grid_color: Color::rgba(252, 25, 154, 0.25),
            grid_spacing: 6.0,
            ..Default::default()
        }
    }

    pub fn no_grid() -> Self {
        Self {
            grid_intensity: 0.0,
            ..Default::default()
        }
    }
}

// ---------------------------------------------------------------------------
// GPU Uniform Data
// ---------------------------------------------------------------------------

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct Params {
    screen_size: [f32; 2],
    time: f32,
    grid_intensity: f32,

    gradient_top: [f32; 4],
    gradient_bottom: [f32; 4],

    grid_color: [f32; 4],
    grid_spacing: f32,
    grid_line_width: f32,
    grid_perspective: f32,
    grid_horizon: f32,

    glow_color: [f32; 4],
    glow_radius: f32,
    glow_intensity: f32,
    _pad1: f32,
    _pad2: f32,

    text_color: [f32; 4],

    _pad3: [f32; 4],  // Pad to 144 bytes (16-byte aligned)
}

impl Params {
    fn from_theme(theme: &Theme, screen_size: [f32; 2], time: f32) -> Self {
        Self {
            screen_size,
            time,
            grid_intensity: theme.grid_intensity,

            gradient_top: theme.gradient_top.to_array(),
            gradient_bottom: theme.gradient_bottom.to_array(),

            grid_color: theme.grid_color.to_array(),
            grid_spacing: theme.grid_spacing,
            grid_line_width: theme.grid_line_width,
            grid_perspective: theme.grid_perspective,
            grid_horizon: theme.grid_horizon,

            glow_color: theme.glow_color.to_array(),
            glow_radius: theme.glow_radius,
            glow_intensity: theme.glow_intensity,
            _pad1: 0.0,
            _pad2: 0.0,

            text_color: theme.text_color.to_array(),

            _pad3: [0.0, 0.0, 0.0, 0.0],
        }
    }
}

// ---------------------------------------------------------------------------
// Simple Bitmap Text Generator
// ---------------------------------------------------------------------------

fn create_text_texture(width: u32, height: u32) -> Vec<u8> {
    let mut data = vec![0u8; (width * height * 4) as usize];

    // Simple bitmap representations
    let terminal_lines = [
        (0.15, 0.25, "$ neofetch"),
        (0.15, 0.30, "  ___  ___"),
        (0.15, 0.35, " / __|| _ \\___"),
        (0.15, 0.40, "| (__ |   / -_)"),
        (0.15, 0.45, " \\___||_|_\\___|"),
        (0.15, 0.52, "$ echo 'Hello, Synthwave!'"),
        (0.15, 0.57, "Hello, Synthwave!"),
    ];

    for (x_frac, y_frac, text) in terminal_lines {
        let start_x = (width as f32 * x_frac) as i32;
        let start_y = (height as f32 * y_frac) as i32;

        // Draw text as horizontal bars (simplified)
        for (i, c) in text.chars().enumerate() {
            if c == ' ' {
                continue;
            }

            let char_x = start_x + (i as i32 * 8);
            let brightness = if c.is_alphanumeric() { 255u8 } else { 200u8 };

            // Draw a small block for each character
            for dy in 0..10 {
                for dx in 0..6 {
                    let px = char_x + dx;
                    let py = start_y + dy;

                    if px >= 0 && px < width as i32 && py >= 0 && py < height as i32 {
                        let idx = ((py as u32 * width + px as u32) * 4) as usize;
                        // Simple character patterns
                        let draw = match c {
                            '_' => dy >= 8,
                            '|' => dx == 2 || dx == 3,
                            '/' => (dx as i32 - dy as i32).abs() <= 1,
                            '\\' => (dx as i32 - (9 - dy as i32)).abs() <= 1,
                            '-' => dy >= 4 && dy <= 5,
                            '\'' => dy <= 2 && dx >= 2 && dx <= 3,
                            '!' => (dx == 2 || dx == 3) && (dy <= 6 || dy >= 8),
                            ',' => dy >= 8 && dx >= 2 && dx <= 3,
                            '$' => true, // Fill for special chars
                            _ => dy >= 1 && dy <= 8 && dx >= 1 && dx <= 4,
                        };

                        if draw {
                            data[idx] = brightness;
                            data[idx + 1] = brightness;
                            data[idx + 2] = brightness;
                            data[idx + 3] = 255;
                        }
                    }
                }
            }
        }
    }

    data
}

// ---------------------------------------------------------------------------
// Application
// ---------------------------------------------------------------------------

struct GpuState {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    params_buffer: wgpu::Buffer,
}

struct App {
    window: Option<Arc<Window>>,
    gpu: Option<GpuState>,
    theme: Theme,
    theme_index: usize,
    start_time: Instant,
}

impl App {
    fn new() -> Self {
        Self {
            window: None,
            gpu: None,
            theme: Theme::default(),
            theme_index: 0,
            start_time: Instant::now(),
        }
    }

    fn cycle_theme(&mut self) {
        let themes = [
            ("Synthwave (default)", Theme::default()),
            ("Gold variant", Theme::gold_variant()),
            ("Intense grid", Theme::intense_grid()),
            ("No grid", Theme::no_grid()),
        ];

        self.theme_index = (self.theme_index + 1) % themes.len();
        let (name, new_theme) = &themes[self.theme_index];

        log::info!("Switching to: {}", name);
        self.theme = new_theme.clone();
    }

    fn update_params(&self) {
        if let Some(gpu) = &self.gpu {
            let time = self.start_time.elapsed().as_secs_f32();
            let params = Params::from_theme(
                &self.theme,
                [gpu.config.width as f32, gpu.config.height as f32],
                time,
            );
            gpu.queue.write_buffer(&gpu.params_buffer, 0, bytemuck::bytes_of(&params));
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title("Synthwave Theme - Gradient + Grid + Glow")
                        .with_inner_size(winit::dpi::LogicalSize::new(900, 700)),
                )
                .expect("Failed to create window"),
        );

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let surface = instance.create_surface(window.clone()).unwrap();

        let (adapter, device, queue) = pollster::block_on(async {
            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    compatible_surface: Some(&surface),
                    ..Default::default()
                })
                .await
                .unwrap();
            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor::default(), None)
                .await
                .unwrap();
            (adapter, device, queue)
        });

        let size = window.inner_size();
        let caps = surface.get_capabilities(&adapter);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: caps.formats[0],
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        // Create params buffer
        let params = Params::from_theme(&self.theme, [size.width as f32, size.height as f32], 0.0);
        let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Params"),
            contents: bytemuck::bytes_of(&params),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Create text texture
        let texture_data = create_text_texture(size.width, size.height);
        let texture = device.create_texture_with_data(
            &queue,
            &wgpu::TextureDescriptor {
                label: Some("Text Texture"),
                size: wgpu::Extent3d {
                    width: size.width,
                    height: size.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &texture_data,
        );
        let texture_view = texture.create_view(&Default::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // Create bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        // Create shader and pipeline
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Synthwave Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/synthwave.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        self.window = Some(window);
        self.gpu = Some(GpuState {
            surface,
            device,
            queue,
            config,
            pipeline,
            bind_group,
            params_buffer,
        });

        log::info!("Initialized! Press SPACE to cycle themes, ESC to quit");
        log::info!("Grid animates automatically");
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state.is_pressed() {
                    use winit::keyboard::{Key, NamedKey};
                    match event.logical_key {
                        Key::Named(NamedKey::Escape) => event_loop.exit(),
                        Key::Named(NamedKey::Space) => self.cycle_theme(),
                        _ => {}
                    }
                }
            }
            WindowEvent::Resized(new_size) => {
                if let Some(gpu) = &mut self.gpu {
                    gpu.config.width = new_size.width.max(1);
                    gpu.config.height = new_size.height.max(1);
                    gpu.surface.configure(&gpu.device, &gpu.config);
                }
            }
            WindowEvent::RedrawRequested => {
                // Update time for animation
                self.update_params();

                if let Some(gpu) = &self.gpu {
                    let frame = gpu.surface.get_current_texture().unwrap();
                    let view = frame.texture.create_view(&Default::default());

                    let mut encoder = gpu.device.create_command_encoder(&Default::default());
                    {
                        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: Some("Render Pass"),
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                view: &view,
                                resolve_target: None,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                                    store: wgpu::StoreOp::Store,
                                },
                            })],
                            depth_stencil_attachment: None,
                            timestamp_writes: None,
                            occlusion_query_set: None,
                        });

                        pass.set_pipeline(&gpu.pipeline);
                        pass.set_bind_group(0, &gpu.bind_group, &[]);
                        pass.draw(0..3, 0..1);
                    }

                    gpu.queue.submit(std::iter::once(encoder.finish()));
                    frame.present();
                }

                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            _ => {}
        }
    }
}

fn main() {
    env_logger::init();

    log::info!("Synthwave Theme Prototype");
    log::info!("Demonstrates CSS-like theming:");
    log::info!("  - linear-gradient background");
    log::info!("  - perspective grid (synthwave floor)");
    log::info!("  - text-shadow glow effects");

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App::new();
    event_loop.run_app(&mut app).unwrap();
}
