//! Prototype A: Static Shader + Uniform Mapping
//!
//! This prototype demonstrates mapping CSS-like style declarations
//! to wgpu shader uniforms for CRT effects.
//!
//! Key concepts:
//! - CrtStyle struct mirrors CSS custom properties
//! - Style values map directly to shader uniforms
//! - Runtime updates via buffer writes

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
// CSS-like Style Definition
// ---------------------------------------------------------------------------

/// CRT display style - maps to shader uniforms
///
/// This struct represents a CSS-like styling interface:
/// ```css
/// .crt-display {
///     --scanline-intensity: 0.3;
///     --scanline-count: 240;
///     --phosphor-intensity: 0.2;
///     --glow-intensity: 0.1;
///     --glow-radius: 2.0;
///     --curvature: 0.02;
///     --brightness: 1.1;
///     --contrast: 1.1;
///     --saturation: 1.0;
/// }
/// ```
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct CrtStyle {
    // Display dimensions
    pub resolution: [f32; 2],
    pub screen_size: [f32; 2],

    // Scanline effect (--scanline-*)
    pub scanline_intensity: f32,
    pub scanline_count: f32,

    // Phosphor mask (--phosphor-*)
    pub phosphor_intensity: f32,

    // Glow/bloom (--glow-*)
    pub glow_intensity: f32,
    pub glow_radius: f32,

    // Screen shape (--curvature)
    pub curvature: f32,

    // Color grading (--brightness, --contrast, --saturation)
    pub brightness: f32,
    pub contrast: f32,
    pub saturation: f32,

    // Padding for 16-byte alignment
    pub _padding: f32,
}

impl Default for CrtStyle {
    fn default() -> Self {
        Self {
            resolution: [320.0, 240.0],
            screen_size: [800.0, 600.0],
            scanline_intensity: 0.3,
            scanline_count: 240.0,
            phosphor_intensity: 0.2,
            glow_intensity: 0.1,
            glow_radius: 2.0,
            curvature: 0.02,
            brightness: 1.1,
            contrast: 1.1,
            saturation: 1.0,
            _padding: 0.0,
        }
    }
}

impl CrtStyle {
    /// Create style from CSS-like property map
    pub fn from_properties(props: &[(&str, f32)]) -> Self {
        let mut style = Self::default();

        for (name, value) in props {
            match *name {
                "--scanline-intensity" => style.scanline_intensity = *value,
                "--scanline-count" => style.scanline_count = *value,
                "--phosphor-intensity" => style.phosphor_intensity = *value,
                "--glow-intensity" => style.glow_intensity = *value,
                "--glow-radius" => style.glow_radius = *value,
                "--curvature" => style.curvature = *value,
                "--brightness" => style.brightness = *value,
                "--contrast" => style.contrast = *value,
                "--saturation" => style.saturation = *value,
                _ => log::warn!("Unknown CSS property: {}", name),
            }
        }

        style
    }

    /// Update screen dimensions (called on resize)
    pub fn set_screen_size(&mut self, width: f32, height: f32) {
        self.screen_size = [width, height];
    }
}

// ---------------------------------------------------------------------------
// Presets (like CSS classes)
// ---------------------------------------------------------------------------

/// Predefined style presets - like CSS class definitions
pub mod presets {
    use super::CrtStyle;

    /// Classic CRT look - moderate effects
    pub fn classic() -> CrtStyle {
        CrtStyle {
            scanline_intensity: 0.25,
            scanline_count: 240.0,
            phosphor_intensity: 0.15,
            glow_intensity: 0.1,
            glow_radius: 1.5,
            curvature: 0.03,
            brightness: 1.1,
            contrast: 1.1,
            saturation: 1.0,
            ..Default::default()
        }
    }

    /// Arcade monitor - more intense phosphors
    pub fn arcade() -> CrtStyle {
        CrtStyle {
            scanline_intensity: 0.4,
            scanline_count: 224.0,
            phosphor_intensity: 0.35,
            glow_intensity: 0.15,
            glow_radius: 2.0,
            curvature: 0.04,
            brightness: 1.2,
            contrast: 1.2,
            saturation: 1.1,
            ..Default::default()
        }
    }

    /// PVM - professional video monitor, subtle effects
    pub fn pvm() -> CrtStyle {
        CrtStyle {
            scanline_intensity: 0.15,
            scanline_count: 480.0,
            phosphor_intensity: 0.1,
            glow_intensity: 0.05,
            glow_radius: 1.0,
            curvature: 0.01,
            brightness: 1.0,
            contrast: 1.05,
            saturation: 1.0,
            ..Default::default()
        }
    }

    /// No effects - bypass
    pub fn none() -> CrtStyle {
        CrtStyle {
            scanline_intensity: 0.0,
            phosphor_intensity: 0.0,
            glow_intensity: 0.0,
            curvature: 0.0,
            brightness: 1.0,
            contrast: 1.0,
            saturation: 1.0,
            ..Default::default()
        }
    }
}

// ---------------------------------------------------------------------------
// Application State
// ---------------------------------------------------------------------------

struct GpuState {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    style_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    style: CrtStyle,
}

struct App {
    window: Option<Arc<Window>>,
    gpu: Option<GpuState>,
    current_preset: usize,
}

impl App {
    fn new() -> Self {
        Self {
            window: None,
            gpu: None,
            current_preset: 0,
        }
    }

    fn cycle_preset(&mut self) {
        let presets = ["classic", "arcade", "pvm", "none"];
        self.current_preset = (self.current_preset + 1) % presets.len();

        let new_style = match presets[self.current_preset] {
            "classic" => presets::classic(),
            "arcade" => presets::arcade(),
            "pvm" => presets::pvm(),
            "none" => presets::none(),
            _ => presets::classic(),
        };

        if let Some(gpu) = &mut self.gpu {
            gpu.style = new_style;
            gpu.style.set_screen_size(
                gpu.config.width as f32,
                gpu.config.height as f32,
            );
            gpu.queue.write_buffer(
                &gpu.style_buffer,
                0,
                bytemuck::bytes_of(&gpu.style),
            );
            log::info!("Switched to preset: {}", presets[self.current_preset]);
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
                        .with_title("CRT Prototype A: Static Shader + Uniforms")
                        .with_inner_size(winit::dpi::LogicalSize::new(800, 600)),
                )
                .expect("Failed to create window"),
        );

        // Initialize wgpu
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = instance.create_surface(window.clone()).unwrap();

        let (adapter, device, queue) = pollster::block_on(async {
            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::default(),
                    compatible_surface: Some(&surface),
                    force_fallback_adapter: false,
                })
                .await
                .expect("Failed to find adapter");

            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor::default())
                .await
                .expect("Failed to create device");

            (adapter, device, queue)
        });

        let size = window.inner_size();
        let caps = surface.get_capabilities(&adapter);
        let format = caps.formats[0];

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        // Create style with screen size
        let mut style = presets::classic();
        style.set_screen_size(size.width as f32, size.height as f32);

        // Create uniform buffer for style
        let style_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("CRT Style Buffer"),
            contents: bytemuck::bytes_of(&style),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Create a dummy texture for the input (checkerboard pattern)
        let texture_size = 320u32;
        let mut texture_data = vec![0u8; (texture_size * texture_size * 4) as usize];
        for y in 0..texture_size {
            for x in 0..texture_size {
                let idx = ((y * texture_size + x) * 4) as usize;
                let checker = ((x / 16) + (y / 16)) % 2 == 0;
                let color = if checker { 200u8 } else { 50u8 };
                texture_data[idx] = color;     // R
                texture_data[idx + 1] = color; // G
                texture_data[idx + 2] = color; // B
                texture_data[idx + 3] = 255;   // A
            }
        }

        let input_texture = device.create_texture_with_data(
            &queue,
            &wgpu::TextureDescriptor {
                label: Some("Input Texture"),
                size: wgpu::Extent3d {
                    width: texture_size,
                    height: texture_size,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &texture_data,
        );

        let texture_view = input_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // Load shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("CRT Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/crt.wgsl").into()),
        });

        // Create bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("CRT Bind Group Layout"),
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

        // Create bind group
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("CRT Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: style_buffer.as_entire_binding(),
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

        // Create pipeline
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("CRT Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("CRT Pipeline"),
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
                    format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
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
            style_buffer,
            bind_group,
            style,
        });

        log::info!("Initialized! Press SPACE to cycle presets, ESC to quit");
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state.is_pressed() {
                    use winit::keyboard::{Key, NamedKey};
                    match event.logical_key {
                        Key::Named(NamedKey::Escape) => event_loop.exit(),
                        Key::Named(NamedKey::Space) => self.cycle_preset(),
                        _ => {}
                    }
                }
            }
            WindowEvent::Resized(new_size) => {
                if let Some(gpu) = &mut self.gpu {
                    gpu.config.width = new_size.width.max(1);
                    gpu.config.height = new_size.height.max(1);
                    gpu.surface.configure(&gpu.device, &gpu.config);

                    // Update style with new screen size
                    gpu.style.set_screen_size(
                        gpu.config.width as f32,
                        gpu.config.height as f32,
                    );
                    gpu.queue.write_buffer(
                        &gpu.style_buffer,
                        0,
                        bytemuck::bytes_of(&gpu.style),
                    );
                }
            }
            WindowEvent::RedrawRequested => {
                if let Some(gpu) = &self.gpu {
                    let frame = gpu.surface.get_current_texture().unwrap();
                    let view = frame.texture.create_view(&Default::default());

                    let mut encoder = gpu.device.create_command_encoder(&Default::default());
                    {
                        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: Some("CRT Render Pass"),
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

    // Demonstrate CSS-like property API
    let _style_from_props = CrtStyle::from_properties(&[
        ("--scanline-intensity", 0.35),
        ("--phosphor-intensity", 0.25),
        ("--glow-intensity", 0.15),
        ("--curvature", 0.025),
    ]);

    log::info!("CRT Prototype A: Static Shader + Uniform Mapping");
    log::info!("This demonstrates CSS-like style declarations mapping to shader uniforms");

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App::new();
    event_loop.run_app(&mut app).unwrap();
}
