//! Prototype B: Shader Generation
//!
//! This prototype demonstrates generating WGSL shader code dynamically
//! based on CSS-like style configuration. Effects are only included
//! in the shader if they're enabled.
//!
//! Key concepts:
//! - Effect modules as composable WGSL snippets
//! - CSS config determines which effects to include
//! - Shader regeneration when config changes
//! - Smaller/faster shaders when effects are disabled

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
// CSS-like Effect Configuration
// ---------------------------------------------------------------------------

/// Individual effect that can be enabled/disabled
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    Scanlines,
    PhosphorMask,
    Glow,
    Curvature,
    ColorGrading,
}

/// CSS-like style configuration that drives shader generation
#[derive(Debug, Clone)]
pub struct CrtConfig {
    // Which effects are enabled (like CSS classes)
    pub effects: Vec<Effect>,

    // Effect parameters (like CSS custom properties)
    pub scanline_intensity: f32,
    pub scanline_count: f32,
    pub phosphor_intensity: f32,
    pub glow_intensity: f32,
    pub glow_radius: f32,
    pub curvature: f32,
    pub brightness: f32,
    pub contrast: f32,
    pub saturation: f32,
}

impl Default for CrtConfig {
    fn default() -> Self {
        Self {
            effects: vec![
                Effect::Scanlines,
                Effect::PhosphorMask,
                Effect::Glow,
                Effect::Curvature,
                Effect::ColorGrading,
            ],
            scanline_intensity: 0.3,
            scanline_count: 240.0,
            phosphor_intensity: 0.2,
            glow_intensity: 0.1,
            glow_radius: 2.0,
            curvature: 0.02,
            brightness: 1.1,
            contrast: 1.1,
            saturation: 1.0,
        }
    }
}

impl CrtConfig {
    pub fn has_effect(&self, effect: Effect) -> bool {
        self.effects.contains(&effect)
    }

    /// Minimal config - no effects
    pub fn minimal() -> Self {
        Self {
            effects: vec![],
            ..Default::default()
        }
    }

    /// Only scanlines
    pub fn scanlines_only() -> Self {
        Self {
            effects: vec![Effect::Scanlines],
            ..Default::default()
        }
    }

    /// Classic CRT - all effects
    pub fn classic() -> Self {
        Self::default()
    }
}

// ---------------------------------------------------------------------------
// Shader Generator
// ---------------------------------------------------------------------------

/// Generates WGSL shader code based on configuration
pub struct ShaderGenerator;

impl ShaderGenerator {
    /// Generate complete WGSL shader from config
    pub fn generate(config: &CrtConfig) -> String {
        let mut shader = String::new();

        // Header comment showing which effects are enabled
        shader.push_str("// Generated CRT Shader\n");
        shader.push_str("// Enabled effects: ");
        if config.effects.is_empty() {
            shader.push_str("none (passthrough)");
        } else {
            let names: Vec<_> = config.effects.iter().map(|e| format!("{:?}", e)).collect();
            shader.push_str(&names.join(", "));
        }
        shader.push_str("\n\n");

        // Generate uniform struct based on enabled effects
        shader.push_str(&Self::generate_uniforms(config));

        // Vertex shader (always the same)
        shader.push_str(&Self::generate_vertex_shader());

        // Generate effect functions only for enabled effects
        if config.has_effect(Effect::Curvature) {
            shader.push_str(&Self::effect_curvature());
        }
        if config.has_effect(Effect::Scanlines) {
            shader.push_str(&Self::effect_scanlines());
        }
        if config.has_effect(Effect::PhosphorMask) {
            shader.push_str(&Self::effect_phosphor());
        }
        if config.has_effect(Effect::Glow) {
            shader.push_str(&Self::effect_glow());
        }
        if config.has_effect(Effect::ColorGrading) {
            shader.push_str(&Self::effect_color_grading());
        }

        // Generate fragment shader that chains enabled effects
        shader.push_str(&Self::generate_fragment_shader(config));

        shader
    }

    fn generate_uniforms(config: &CrtConfig) -> String {
        let mut s = String::from("struct Params {\n");
        s.push_str("    screen_size: vec2<f32>,\n");

        if config.has_effect(Effect::Scanlines) {
            s.push_str("    scanline_intensity: f32,\n");
            s.push_str("    scanline_count: f32,\n");
        }
        if config.has_effect(Effect::PhosphorMask) {
            s.push_str("    phosphor_intensity: f32,\n");
        }
        if config.has_effect(Effect::Glow) {
            s.push_str("    glow_intensity: f32,\n");
            s.push_str("    glow_radius: f32,\n");
        }
        if config.has_effect(Effect::Curvature) {
            s.push_str("    curvature: f32,\n");
        }
        if config.has_effect(Effect::ColorGrading) {
            s.push_str("    brightness: f32,\n");
            s.push_str("    contrast: f32,\n");
            s.push_str("    saturation: f32,\n");
        }

        // Padding to ensure 16-byte alignment
        s.push_str("    _pad: f32,\n");
        s.push_str("}\n\n");

        s.push_str("@group(0) @binding(0) var<uniform> params: Params;\n");
        s.push_str("@group(0) @binding(1) var input_texture: texture_2d<f32>;\n");
        s.push_str("@group(0) @binding(2) var input_sampler: sampler;\n\n");

        s
    }

    fn generate_vertex_shader() -> String {
        r#"
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    let x = f32(i32(vertex_index & 1u)) * 4.0 - 1.0;
    let y = f32(i32(vertex_index >> 1u)) * 4.0 - 1.0;
    out.position = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);
    return out;
}

"#
        .to_string()
    }

    fn effect_curvature() -> String {
        r#"
fn apply_curvature(uv: vec2<f32>) -> vec2<f32> {
    let centered = uv - 0.5;
    let dist = dot(centered, centered);
    let curved = centered * (1.0 + dist * params.curvature);
    return curved + 0.5;
}

"#
        .to_string()
    }

    fn effect_scanlines() -> String {
        r#"
fn apply_scanlines(color: vec3<f32>, uv: vec2<f32>) -> vec3<f32> {
    let scanline = sin(uv.y * params.scanline_count * 3.14159) * 0.5 + 0.5;
    let factor = mix(1.0, scanline, params.scanline_intensity);
    return color * factor;
}

"#
        .to_string()
    }

    fn effect_phosphor() -> String {
        r#"
fn apply_phosphor(color: vec3<f32>, screen_pos: vec2<f32>) -> vec3<f32> {
    let pixel_x = i32(screen_pos.x) % 3;
    var mask = vec3<f32>(1.0);
    let dim = 1.0 - params.phosphor_intensity * 0.5;
    if pixel_x == 0 {
        mask = vec3<f32>(1.0, dim, dim);
    } else if pixel_x == 1 {
        mask = vec3<f32>(dim, 1.0, dim);
    } else {
        mask = vec3<f32>(dim, dim, 1.0);
    }
    return color * mask;
}

"#
        .to_string()
    }

    fn effect_glow() -> String {
        r#"
fn sample_with_glow(uv: vec2<f32>) -> vec3<f32> {
    let texel_size = 1.0 / params.screen_size;
    var color = textureSample(input_texture, input_sampler, uv).rgb;

    if params.glow_intensity > 0.0 {
        var glow = vec3<f32>(0.0);
        for (var i = -2.0; i <= 2.0; i += 1.0) {
            for (var j = -2.0; j <= 2.0; j += 1.0) {
                let offset = vec2<f32>(i, j) * texel_size * params.glow_radius;
                glow += textureSample(input_texture, input_sampler, uv + offset).rgb;
            }
        }
        glow /= 25.0;
        color = mix(color, max(color, glow), params.glow_intensity);
    }
    return color;
}

"#
        .to_string()
    }

    fn effect_color_grading() -> String {
        r#"
fn apply_color_grading(color: vec3<f32>) -> vec3<f32> {
    var c = color * params.brightness;
    c = (c - 0.5) * params.contrast + 0.5;
    let gray = dot(c, vec3<f32>(0.299, 0.587, 0.114));
    c = mix(vec3<f32>(gray), c, params.saturation);
    return clamp(c, vec3<f32>(0.0), vec3<f32>(1.0));
}

"#
        .to_string()
    }

    fn generate_fragment_shader(config: &CrtConfig) -> String {
        let mut s = String::from("@fragment\nfn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {\n");

        // UV coordinate handling
        if config.has_effect(Effect::Curvature) {
            s.push_str("    var uv = apply_curvature(in.uv);\n");
            s.push_str("    if uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0 {\n");
            s.push_str("        return vec4<f32>(0.0, 0.0, 0.0, 1.0);\n");
            s.push_str("    }\n");
        } else {
            s.push_str("    let uv = in.uv;\n");
        }

        // Sample texture (with or without glow)
        if config.has_effect(Effect::Glow) {
            s.push_str("    var color = sample_with_glow(uv);\n");
        } else {
            s.push_str("    var color = textureSample(input_texture, input_sampler, uv).rgb;\n");
        }

        // Apply effects in order
        if config.has_effect(Effect::Scanlines) {
            s.push_str("    color = apply_scanlines(color, uv);\n");
        }
        if config.has_effect(Effect::PhosphorMask) {
            s.push_str("    color = apply_phosphor(color, in.position.xy);\n");
        }
        if config.has_effect(Effect::ColorGrading) {
            s.push_str("    color = apply_color_grading(color);\n");
        }

        s.push_str("    return vec4<f32>(color, 1.0);\n");
        s.push_str("}\n");

        s
    }
}

// ---------------------------------------------------------------------------
// Dynamic Uniform Buffer
// ---------------------------------------------------------------------------

/// Uniform data that matches the generated shader's Params struct
/// This is built dynamically based on config
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct MaxParams {
    screen_size: [f32; 2],
    scanline_intensity: f32,
    scanline_count: f32,
    phosphor_intensity: f32,
    glow_intensity: f32,
    glow_radius: f32,
    curvature: f32,
    brightness: f32,
    contrast: f32,
    saturation: f32,
    _pad: f32,
}

impl From<&CrtConfig> for MaxParams {
    fn from(config: &CrtConfig) -> Self {
        Self {
            screen_size: [800.0, 600.0],
            scanline_intensity: config.scanline_intensity,
            scanline_count: config.scanline_count,
            phosphor_intensity: config.phosphor_intensity,
            glow_intensity: config.glow_intensity,
            glow_radius: config.glow_radius,
            curvature: config.curvature,
            brightness: config.brightness,
            contrast: config.contrast,
            saturation: config.saturation,
            _pad: 0.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Application
// ---------------------------------------------------------------------------

struct GpuState {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface_config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    params_buffer: wgpu::Buffer,
    texture_view: wgpu::TextureView,
    sampler: wgpu::Sampler,
}

struct App {
    window: Option<Arc<Window>>,
    gpu: Option<GpuState>,
    config: CrtConfig,
    config_index: usize,
}

impl App {
    fn new() -> Self {
        Self {
            window: None,
            gpu: None,
            config: CrtConfig::classic(),
            config_index: 0,
        }
    }

    fn cycle_config(&mut self) {
        let configs = [
            ("all effects", CrtConfig::classic()),
            ("scanlines only", CrtConfig::scanlines_only()),
            ("minimal (passthrough)", CrtConfig::minimal()),
        ];

        self.config_index = (self.config_index + 1) % configs.len();
        let (name, new_config) = &configs[self.config_index];

        log::info!("Switching to: {} - regenerating shader...", name);

        // Generate new shader
        let shader_source = ShaderGenerator::generate(new_config);
        log::info!("Generated shader ({} bytes):\n{}", shader_source.len(), shader_source);

        self.config = new_config.clone();

        // Recreate pipeline with new shader
        if let Some(gpu) = &mut self.gpu {
            let shader = gpu.device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Generated CRT Shader"),
                source: wgpu::ShaderSource::Wgsl(shader_source.into()),
            });

            let pipeline_layout = gpu.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("CRT Pipeline Layout"),
                bind_group_layouts: &[&gpu.bind_group_layout],
                push_constant_ranges: &[],
            });

            gpu.pipeline = gpu.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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
                        format: gpu.surface_config.format,
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

            // Update params buffer
            let params = MaxParams::from(&self.config);
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
                        .with_title("CRT Prototype B: Shader Generation")
                        .with_inner_size(winit::dpi::LogicalSize::new(800, 600)),
                )
                .expect("Failed to create window"),
        );

        // Generate initial shader
        let shader_source = ShaderGenerator::generate(&self.config);
        log::info!("Initial generated shader:\n{}", shader_source);

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
        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: caps.formats[0],
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);

        // Create params buffer
        let mut params = MaxParams::from(&self.config);
        params.screen_size = [size.width as f32, size.height as f32];
        let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Params Buffer"),
            contents: bytemuck::bytes_of(&params),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Create test texture
        let texture_size = 320u32;
        let mut texture_data = vec![0u8; (texture_size * texture_size * 4) as usize];
        for y in 0..texture_size {
            for x in 0..texture_size {
                let idx = ((y * texture_size + x) * 4) as usize;
                let checker = ((x / 16) + (y / 16)) % 2 == 0;
                let color = if checker { 200u8 } else { 50u8 };
                texture_data[idx] = color;
                texture_data[idx + 1] = color;
                texture_data[idx + 2] = color;
                texture_data[idx + 3] = 255;
            }
        }

        let texture = device.create_texture_with_data(
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
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &texture_data,
        );
        let texture_view = texture.create_view(&Default::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // Create bind group layout (same for all generated shaders)
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

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("CRT Bind Group"),
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
            label: Some("Generated CRT Shader"),
            source: wgpu::ShaderSource::Wgsl(shader_source.into()),
        });

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
                    format: surface_config.format,
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
            surface_config,
            pipeline,
            bind_group_layout,
            bind_group,
            params_buffer,
            texture_view,
            sampler,
        });

        log::info!("Initialized! Press SPACE to cycle configs (regenerates shader), ESC to quit");
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state.is_pressed() {
                    use winit::keyboard::{Key, NamedKey};
                    match event.logical_key {
                        Key::Named(NamedKey::Escape) => event_loop.exit(),
                        Key::Named(NamedKey::Space) => self.cycle_config(),
                        _ => {}
                    }
                }
            }
            WindowEvent::Resized(new_size) => {
                if let Some(gpu) = &mut self.gpu {
                    gpu.surface_config.width = new_size.width.max(1);
                    gpu.surface_config.height = new_size.height.max(1);
                    gpu.surface.configure(&gpu.device, &gpu.surface_config);

                    let mut params = MaxParams::from(&self.config);
                    params.screen_size = [new_size.width as f32, new_size.height as f32];
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

    log::info!("CRT Prototype B: Shader Generation");
    log::info!("Demonstrates generating WGSL shaders from CSS-like config");
    log::info!("Effects are only included in shader when enabled");

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App::new();
    event_loop.run_app(&mut app).unwrap();
}
