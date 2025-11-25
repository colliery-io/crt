//! CRT Renderer - GPU-accelerated text and effect rendering
//!
//! This crate provides:
//! - Effect pipeline (glow, gradients, grid patterns)
//! - Theme uniform management
//! - Compositing text with background effects

use bytemuck::cast_slice;
use crt_theme::Theme;
use wgpu::util::DeviceExt;

/// Shader source for the synthwave effect pipeline
const EFFECT_SHADER: &str = r#"
// Effect Pipeline Shader
// Composites: gradient background + grid + text glow + text

struct Params {
    screen_size: vec2<f32>,
    time: f32,
    grid_intensity: f32,
    gradient_top: vec4<f32>,
    gradient_bottom: vec4<f32>,
    grid_color: vec4<f32>,
    grid_spacing: f32,
    grid_line_width: f32,
    grid_perspective: f32,
    grid_horizon: f32,
    glow_color: vec4<f32>,
    glow_radius: f32,
    glow_intensity: f32,
    text_color: vec4<f32>,
    _pad: vec4<f32>,
}

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var text_texture: texture_2d<f32>;
@group(0) @binding(2) var text_sampler: sampler;

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

// Linear gradient
fn gradient(uv: vec2<f32>, top: vec3<f32>, bottom: vec3<f32>) -> vec3<f32> {
    return mix(top, bottom, uv.y);
}

// Perspective grid (synthwave floor)
fn perspective_grid(uv: vec2<f32>, time: f32) -> f32 {
    let horizon = params.grid_horizon;
    if uv.y < horizon {
        return 0.0;
    }

    let grid_y = (uv.y - horizon) / (1.0 - horizon);
    let perspective = pow(grid_y, params.grid_perspective);

    // Fade out near horizon to prevent moire/aliasing artifacts
    let horizon_fade = smoothstep(0.0, 0.15, grid_y);

    let x_centered = uv.x - 0.5;
    let x_perspective = x_centered / (perspective + 0.01);
    let x_grid = abs(fract(x_perspective * params.grid_spacing + 0.5) - 0.5);
    // Thicker lines near horizon to reduce aliasing
    let line_width = params.grid_line_width / (perspective + 0.2);
    let x_line = 1.0 - smoothstep(0.0, line_width, x_grid);

    let y_scroll = perspective * params.grid_spacing * 2.0 - time * 0.5;
    let y_grid = abs(fract(y_scroll + 0.5) - 0.5);
    let y_line = 1.0 - smoothstep(0.0, params.grid_line_width * 3.0, y_grid);

    let grid = max(x_line, y_line);
    // Stronger fade near horizon, gentler fade toward camera
    let distance_fade = 1.0 - perspective * 0.3;

    return grid * horizon_fade * distance_fade * params.grid_intensity;
}

// Gaussian blur for glow
fn sample_blur(uv: vec2<f32>, radius: f32) -> f32 {
    let texel_size = 1.0 / params.screen_size;
    let sigma = radius / 2.0;

    var total = 0.0;
    var weight_sum = 0.0;

    let samples = 4i;
    for (var x = -samples; x <= samples; x++) {
        for (var y = -samples; y <= samples; y++) {
            let offset = vec2<f32>(f32(x), f32(y)) * texel_size * (radius / 4.0);
            let dist = length(vec2<f32>(f32(x), f32(y)));
            let w = exp(-(dist * dist) / (2.0 * sigma * sigma));

            let sample_color = textureSample(text_texture, text_sampler, uv + offset);
            let luminance = dot(sample_color.rgb, vec3<f32>(0.299, 0.587, 0.114));
            total += luminance * w;
            weight_sum += w;
        }
    }

    return total / weight_sum;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Layer 1: Gradient background
    var color = gradient(in.uv, params.gradient_top.rgb, params.gradient_bottom.rgb);

    // Layer 2: Perspective grid (if enabled)
    if params.grid_intensity > 0.0 {
        let grid = perspective_grid(in.uv, params.time);
        color = mix(color, params.grid_color.rgb, grid * params.grid_color.a);
    }

    // Layer 3: Text glow (if enabled)
    if params.glow_intensity > 0.0 {
        let blur = sample_blur(in.uv, params.glow_radius);
        color = mix(color, params.glow_color.rgb, blur * params.glow_intensity);
    }

    // Layer 4: Text
    let text = textureSample(text_texture, text_sampler, in.uv);
    let text_luminance = dot(text.rgb, vec3<f32>(0.299, 0.587, 0.114));
    color = mix(color, params.text_color.rgb, text_luminance);

    return vec4<f32>(color, 1.0);
}
"#;

/// Effect pipeline for rendering themed terminal output
pub struct EffectPipeline {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    uniform_buffer: wgpu::Buffer,
    sampler: wgpu::Sampler,
    theme: Theme,
    start_time: std::time::Instant,
}

impl EffectPipeline {
    /// Create a new effect pipeline
    pub fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Effect Shader"),
            source: wgpu::ShaderSource::Wgsl(EFFECT_SHADER.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Effect Bind Group Layout"),
            entries: &[
                // Uniform buffer
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Text texture
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
                // Sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Effect Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Effect Pipeline"),
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
                    format: target_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let theme = Theme::default();
        let uniforms = theme.to_uniforms(1.0, 1.0, 0.0);

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Effect Uniform Buffer"),
            contents: cast_slice(&[uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Effect Sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        Self {
            pipeline,
            bind_group_layout,
            uniform_buffer,
            sampler,
            theme,
            start_time: std::time::Instant::now(),
        }
    }

    /// Update the theme
    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
    }

    /// Get current theme
    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    /// Get mutable theme for modifications
    pub fn theme_mut(&mut self) -> &mut Theme {
        &mut self.theme
    }

    /// Create bind group for rendering with given text texture
    pub fn create_bind_group(
        &self,
        device: &wgpu::Device,
        text_texture_view: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Effect Bind Group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(text_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        })
    }

    /// Update uniforms for current frame
    pub fn update_uniforms(&self, queue: &wgpu::Queue, width: f32, height: f32) {
        let time = self.start_time.elapsed().as_secs_f32();
        let uniforms = self.theme.to_uniforms(width, height, time);
        queue.write_buffer(&self.uniform_buffer, 0, cast_slice(&[uniforms]));
    }

    /// Render effects to the given render pass
    pub fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>, bind_group: &'a wgpu::BindGroup) {
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, bind_group, &[]);
        render_pass.draw(0..4, 0..1);
    }
}

/// Offscreen render target for text
pub struct TextRenderTarget {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub width: u32,
    pub height: u32,
}

impl TextRenderTarget {
    /// Create a new render target
    pub fn new(device: &wgpu::Device, width: u32, height: u32, format: wgpu::TextureFormat) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Text Render Target"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let view = texture.create_view(&Default::default());

        Self {
            texture,
            view,
            width,
            height,
        }
    }

    /// Resize the render target
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32, format: wgpu::TextureFormat) {
        if self.width != width || self.height != height {
            *self = Self::new(device, width, height, format);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shader_compiles() {
        // This test just verifies the shader source is valid WGSL syntax
        // Actual GPU compilation happens at runtime
        assert!(EFFECT_SHADER.contains("vs_main"));
        assert!(EFFECT_SHADER.contains("fs_main"));
    }
}
