//! Text Glow Prototype
//!
//! Demonstrates CSS text-shadow style glow effects:
//! text-shadow: 0px 0px 10px teal, 0px 0px 20px teal;
//!
//! This prototype uses a bitmap "text" texture and applies
//! multi-layer gaussian blur for the glow effect.

use bytemuck::{Pod, Zeroable};
use std::sync::Arc;
use wgpu::util::DeviceExt;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

// ---------------------------------------------------------------------------
// CSS-like Glow Configuration
// ---------------------------------------------------------------------------

/// RGB color from CSS rgb() notation
#[derive(Debug, Clone, Copy)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self {
            r: r as f32 / 255.0,
            g: g as f32 / 255.0,
            b: b as f32 / 255.0,
        }
    }

    pub fn to_array(&self) -> [f32; 3] {
        [self.r, self.g, self.b]
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
}

/// Text shadow definition (like CSS text-shadow)
#[derive(Debug, Clone, Copy)]
pub struct TextShadow {
    pub offset_x: f32,
    pub offset_y: f32,
    pub blur_radius: f32,
    pub color: Color,
    pub intensity: f32,
}

impl TextShadow {
    /// Create shadow like: text-shadow: 0px 0px 10px teal
    pub fn glow(blur_radius: f32, color: Color) -> Self {
        Self {
            offset_x: 0.0,
            offset_y: 0.0,
            blur_radius,
            color,
            intensity: 1.0,
        }
    }
}

/// Complete text style configuration
#[derive(Debug, Clone)]
pub struct TextStyle {
    pub color: Color,
    pub shadows: Vec<TextShadow>,
}

impl Default for TextStyle {
    fn default() -> Self {
        // Recreate: text-shadow: 0px 0px 10px teal, 0px 0px 20px teal;
        Self {
            color: colors::TEAL,
            shadows: vec![
                TextShadow::glow(10.0, colors::TEAL),
                TextShadow::glow(20.0, colors::TEAL),
            ],
        }
    }
}

// ---------------------------------------------------------------------------
// GPU Uniform Data
// ---------------------------------------------------------------------------

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct GlowParams {
    screen_size: [f32; 2],
    glow1_radius: f32,
    glow1_intensity: f32,

    glow1_color: [f32; 4],  // rgb + unused alpha

    glow2_radius: f32,
    glow2_intensity: f32,
    _pad1: f32,
    _pad2: f32,

    glow2_color: [f32; 4],  // rgb + unused alpha

    text_color: [f32; 4],   // rgb + unused alpha
}

impl GlowParams {
    fn from_style(style: &TextStyle, screen_size: [f32; 2]) -> Self {
        let shadow1 = style.shadows.get(0).copied().unwrap_or(TextShadow::glow(0.0, colors::TEAL));
        let shadow2 = style.shadows.get(1).copied().unwrap_or(TextShadow::glow(0.0, colors::TEAL));

        let c1 = shadow1.color.to_array();
        let c2 = shadow2.color.to_array();
        let tc = style.color.to_array();

        Self {
            screen_size,
            glow1_radius: shadow1.blur_radius,
            glow1_intensity: shadow1.intensity,
            glow1_color: [c1[0], c1[1], c1[2], 1.0],
            glow2_radius: shadow2.blur_radius,
            glow2_intensity: shadow2.intensity,
            _pad1: 0.0,
            _pad2: 0.0,
            glow2_color: [c2[0], c2[1], c2[2], 1.0],
            text_color: [tc[0], tc[1], tc[2], 1.0],
        }
    }
}

// ---------------------------------------------------------------------------
// Simple Bitmap Text Generator
// ---------------------------------------------------------------------------

fn create_text_texture(width: u32, height: u32) -> Vec<u8> {
    let mut data = vec![0u8; (width * height * 4) as usize];

    // Simple bitmap font for "GLOW" - each char is 8x12
    let chars: &[&[u8]] = &[
        // G
        &[
            0b01111100,
            0b11000110,
            0b11000000,
            0b11000000,
            0b11001110,
            0b11000110,
            0b11000110,
            0b01111100,
        ],
        // L
        &[
            0b11000000,
            0b11000000,
            0b11000000,
            0b11000000,
            0b11000000,
            0b11000000,
            0b11000000,
            0b11111110,
        ],
        // O
        &[
            0b01111100,
            0b11000110,
            0b11000110,
            0b11000110,
            0b11000110,
            0b11000110,
            0b11000110,
            0b01111100,
        ],
        // W
        &[
            0b11000110,
            0b11000110,
            0b11000110,
            0b11010110,
            0b11111110,
            0b11101110,
            0b11000110,
            0b11000110,
        ],
    ];

    let start_x = (width / 2 - (4 * 10)) as i32;
    let start_y = (height / 2 - 6) as i32;

    for (char_idx, char_data) in chars.iter().enumerate() {
        let char_x = start_x + (char_idx as i32 * 10);

        for (row, &row_bits) in char_data.iter().enumerate() {
            for col in 0..8 {
                if (row_bits >> (7 - col)) & 1 == 1 {
                    let px = char_x + col;
                    let py = start_y + row as i32;

                    if px >= 0 && px < width as i32 && py >= 0 && py < height as i32 {
                        let idx = ((py as u32 * width + px as u32) * 4) as usize;
                        data[idx] = 255;     // R
                        data[idx + 1] = 255; // G
                        data[idx + 2] = 255; // B
                        data[idx + 3] = 255; // A
                    }
                }
            }
        }
    }

    // Add some additional text lines to make it look more like a terminal
    let lines = [
        (start_y + 20, "$ echo 'Hello, World!'"),
        (start_y + 32, "Hello, World!"),
        (start_y + 44, "$ ls -la"),
    ];

    // Simple horizontal bars to represent text lines
    for (y_offset, _text) in lines {
        let line_y = y_offset;
        if line_y >= 0 && line_y < height as i32 {
            // Draw a bar to represent text
            for x in start_x..(start_x + 150) {
                if x >= 0 && x < width as i32 {
                    // Vary brightness slightly for visual interest
                    let brightness = if (x % 3) == 0 { 200u8 } else { 255u8 };
                    let idx = ((line_y as u32 * width + x as u32) * 4) as usize;
                    data[idx] = brightness;
                    data[idx + 1] = brightness;
                    data[idx + 2] = brightness;
                    data[idx + 3] = 255;
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
    style: TextStyle,
    style_index: usize,
}

impl App {
    fn new() -> Self {
        Self {
            window: None,
            gpu: None,
            style: TextStyle::default(),
            style_index: 0,
        }
    }

    fn cycle_style(&mut self) {
        let styles = [
            (
                "Teal glow (default)",
                TextStyle {
                    color: colors::TEAL,
                    shadows: vec![
                        TextShadow::glow(10.0, colors::TEAL),
                        TextShadow::glow(25.0, colors::TEAL),
                    ],
                },
            ),
            (
                "Gold glow",
                TextStyle {
                    color: colors::GOLD,
                    shadows: vec![
                        TextShadow::glow(8.0, colors::GOLD),
                        TextShadow::glow(20.0, colors::GOLD),
                    ],
                },
            ),
            (
                "Pink glow",
                TextStyle {
                    color: colors::PINK,
                    shadows: vec![
                        TextShadow::glow(12.0, colors::PINK),
                        TextShadow::glow(30.0, colors::PINK),
                    ],
                },
            ),
            (
                "Multi-color (teal + pink)",
                TextStyle {
                    color: colors::TEAL,
                    shadows: vec![
                        TextShadow::glow(8.0, colors::TEAL),
                        TextShadow::glow(25.0, colors::PINK),
                    ],
                },
            ),
            (
                "No glow",
                TextStyle {
                    color: colors::TEAL,
                    shadows: vec![],
                },
            ),
        ];

        self.style_index = (self.style_index + 1) % styles.len();
        let (name, new_style) = &styles[self.style_index];

        log::info!("Switching to: {}", name);
        self.style = new_style.clone();

        if let Some(gpu) = &self.gpu {
            let params = GlowParams::from_style(
                &self.style,
                [gpu.config.width as f32, gpu.config.height as f32],
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
                        .with_title("Text Glow Prototype - CSS text-shadow")
                        .with_inner_size(winit::dpi::LogicalSize::new(800, 600)),
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
                .request_device(&wgpu::DeviceDescriptor::default())
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
        let params = GlowParams::from_style(&self.style, [size.width as f32, size.height as f32]);
        let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Glow Params"),
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
            label: Some("Glow Bind Group Layout"),
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
            label: Some("Glow Bind Group"),
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
            label: Some("Glow Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/text_glow.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Glow Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Glow Pipeline"),
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

        log::info!("Initialized! Press SPACE to cycle glow styles, ESC to quit");
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state.is_pressed() {
                    use winit::keyboard::{Key, NamedKey};
                    match event.logical_key {
                        Key::Named(NamedKey::Escape) => event_loop.exit(),
                        Key::Named(NamedKey::Space) => self.cycle_style(),
                        _ => {}
                    }
                }
            }
            WindowEvent::Resized(new_size) => {
                if let Some(gpu) = &mut self.gpu {
                    gpu.config.width = new_size.width.max(1);
                    gpu.config.height = new_size.height.max(1);
                    gpu.surface.configure(&gpu.device, &gpu.config);

                    let params = GlowParams::from_style(
                        &self.style,
                        [new_size.width as f32, new_size.height as f32],
                    );
                    gpu.queue.write_buffer(&gpu.params_buffer, 0, bytemuck::bytes_of(&params));
                }
            }
            WindowEvent::RedrawRequested => {
                if let Some(gpu) = &self.gpu {
                    let frame = gpu.surface.get_current_texture().unwrap();
                    let view = frame.texture.create_view(&Default::default());

                    let mut encoder = gpu.device.create_command_encoder(&Default::default());
                    {
                        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: Some("Glow Render Pass"),
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                view: &view,
                                resolve_target: None,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Clear(wgpu::Color {
                                        r: 0.125,
                                        g: 0.035,
                                        b: 0.2,
                                        a: 1.0,
                                    }),
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

    log::info!("Text Glow Prototype");
    log::info!("Demonstrates CSS text-shadow style glow effects");
    log::info!("Based on: text-shadow: 0px 0px 10px teal, 0px 0px 20px teal;");

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App::new();
    event_loop.run_app(&mut app).unwrap();
}
