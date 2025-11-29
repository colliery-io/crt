//! Grid renderer for terminal text using instanced quads
//!
//! Renders glyphs as instanced quads, sampling from a glyph atlas.
//! Each glyph is one instance with position, UV coords, and color.
//! All text renders in a single draw call.

use crate::glyph_cache::PositionedGlyph;
use crate::shaders::builtin;
use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

/// Per-instance data for a glyph
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct GlyphInstance {
    /// Screen position (top-left of glyph)
    pub pos: [f32; 2],
    /// UV min (atlas coordinates)
    pub uv_min: [f32; 2],
    /// UV max (atlas coordinates)
    pub uv_max: [f32; 2],
    /// Glyph size in pixels
    pub size: [f32; 2],
    /// RGBA color
    pub color: [f32; 4],
}

impl GlyphInstance {
    pub fn from_positioned(glyph: &PositionedGlyph, color: [f32; 4]) -> Self {
        Self {
            pos: [glyph.x, glyph.y],
            uv_min: glyph.uv_min,
            uv_max: glyph.uv_max,
            size: [glyph.width, glyph.height],
            color,
        }
    }
}

/// Global uniforms for the grid shader
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct Globals {
    screen_size: [f32; 2],
    atlas_size: [f32; 2],
}

/// Grid renderer using instanced quads
pub struct GridRenderer {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    globals_buffer: wgpu::Buffer,
    instance_buffer: wgpu::Buffer,
    instance_capacity: usize,
    sampler: wgpu::Sampler,
    bind_group: Option<wgpu::BindGroup>,
    /// Pending instances to render
    instances: Vec<GlyphInstance>,
}

impl GridRenderer {
    const MAX_INSTANCES: usize = 32 * 1024;

    pub fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Grid Shader"),
            source: wgpu::ShaderSource::Wgsl(builtin::GRID.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Grid Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
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

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Grid Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Instance buffer layout - simplified without offset field
        let instance_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<GlyphInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                // pos
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // uv_min
                wgpu::VertexAttribute {
                    offset: 8,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // uv_max
                wgpu::VertexAttribute {
                    offset: 16,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // size
                wgpu::VertexAttribute {
                    offset: 24,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // color
                wgpu::VertexAttribute {
                    offset: 32,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        };

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Grid Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[instance_layout],
                compilation_options: Default::default(),
            },
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
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let globals = Globals {
            screen_size: [1.0, 1.0],
            atlas_size: [1024.0, 1024.0],
        };

        let globals_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Grid Globals Buffer"),
            contents: bytemuck::cast_slice(&[globals]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let instance_capacity = Self::MAX_INSTANCES;
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Grid Instance Buffer"),
            size: (instance_capacity * std::mem::size_of::<GlyphInstance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Grid Atlas Sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        Self {
            pipeline,
            bind_group_layout,
            globals_buffer,
            instance_buffer,
            instance_capacity,
            sampler,
            bind_group: None,
            instances: Vec::with_capacity(Self::MAX_INSTANCES),
        }
    }

    /// Update the bind group with a new glyph cache atlas
    pub fn set_glyph_cache(
        &mut self,
        device: &wgpu::Device,
        glyph_cache: &crate::glyph_cache::GlyphCache,
    ) {
        self.bind_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Grid Bind Group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.globals_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&glyph_cache.atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        }));
    }

    /// Clear pending instances
    pub fn clear(&mut self) {
        self.instances.clear();
    }

    /// Add positioned glyphs from layout
    pub fn push_glyphs(&mut self, glyphs: &[PositionedGlyph], color: [f32; 4]) {
        for glyph in glyphs {
            if self.instances.len() < self.instance_capacity {
                self.instances
                    .push(GlyphInstance::from_positioned(glyph, color));
            }
        }
    }

    /// Update screen size uniform
    pub fn update_screen_size(&self, queue: &wgpu::Queue, width: f32, height: f32) {
        let globals = Globals {
            screen_size: [width, height],
            atlas_size: [1024.0, 1024.0],
        };
        queue.write_buffer(&self.globals_buffer, 0, bytemuck::cast_slice(&[globals]));
    }

    /// Upload instances and render
    pub fn render<'a>(&'a self, queue: &wgpu::Queue, render_pass: &mut wgpu::RenderPass<'a>) {
        if self.instances.is_empty() {
            return;
        }

        let bind_group = match &self.bind_group {
            Some(bg) => bg,
            None => return,
        };

        // Upload instance data
        queue.write_buffer(
            &self.instance_buffer,
            0,
            bytemuck::cast_slice(&self.instances),
        );

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.instance_buffer.slice(..));

        // Draw 4 vertices per instance (triangle strip quad)
        render_pass.draw(0..4, 0..self.instances.len() as u32);
    }

    pub fn instance_count(&self) -> usize {
        self.instances.len()
    }
}

impl Drop for GridRenderer {
    fn drop(&mut self) {
        // Destroy buffers to release GPU memory immediately
        self.globals_buffer.destroy();
        self.instance_buffer.destroy();
    }
}
