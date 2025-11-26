//! Rect renderer for solid color rectangles using instanced quads
//!
//! Renders colored rectangles for cell backgrounds.
//! All rectangles render in a single draw call.

use crate::shaders::builtin;
use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

/// Per-instance data for a rectangle
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct RectInstance {
    /// Screen position (top-left of rect)
    pub pos: [f32; 2],
    /// Rect size in pixels
    pub size: [f32; 2],
    /// RGBA color
    pub color: [f32; 4],
}

/// Global uniforms for the rect shader
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct Globals {
    screen_size: [f32; 2],
    _pad: [f32; 2],
}

/// Rect renderer using instanced quads
pub struct RectRenderer {
    pipeline: wgpu::RenderPipeline,
    globals_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    instance_buffer: wgpu::Buffer,
    instance_capacity: usize,
    /// Pending instances to render
    instances: Vec<RectInstance>,
}

impl RectRenderer {
    const MAX_INSTANCES: usize = 16 * 1024;

    pub fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Rect Shader"),
            source: wgpu::ShaderSource::Wgsl(builtin::RECT.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Rect Bind Group Layout"),
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
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Rect Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Instance buffer layout
        let instance_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<RectInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                // pos
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // size
                wgpu::VertexAttribute {
                    offset: 8,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // color
                wgpu::VertexAttribute {
                    offset: 16,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        };

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Rect Pipeline"),
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
            _pad: [0.0, 0.0],
        };

        let globals_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Rect Globals Buffer"),
            contents: bytemuck::cast_slice(&[globals]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Rect Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: globals_buffer.as_entire_binding(),
                },
            ],
        });

        let instance_capacity = Self::MAX_INSTANCES;
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Rect Instance Buffer"),
            size: (instance_capacity * std::mem::size_of::<RectInstance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            globals_buffer,
            bind_group,
            instance_buffer,
            instance_capacity,
            instances: Vec::with_capacity(Self::MAX_INSTANCES),
        }
    }

    /// Clear pending instances
    pub fn clear(&mut self) {
        self.instances.clear();
    }

    /// Add a rectangle
    pub fn push_rect(&mut self, x: f32, y: f32, width: f32, height: f32, color: [f32; 4]) {
        if self.instances.len() < self.instance_capacity {
            self.instances.push(RectInstance {
                pos: [x, y],
                size: [width, height],
                color,
            });
        }
    }

    /// Update screen size uniform
    pub fn update_screen_size(&self, queue: &wgpu::Queue, width: f32, height: f32) {
        let globals = Globals {
            screen_size: [width, height],
            _pad: [0.0, 0.0],
        };
        queue.write_buffer(&self.globals_buffer, 0, bytemuck::cast_slice(&[globals]));
    }

    /// Upload instances and render
    pub fn render<'a>(
        &'a self,
        queue: &wgpu::Queue,
        render_pass: &mut wgpu::RenderPass<'a>,
    ) {
        if self.instances.is_empty() {
            return;
        }

        // Upload instance data
        queue.write_buffer(
            &self.instance_buffer,
            0,
            bytemuck::cast_slice(&self.instances),
        );

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.instance_buffer.slice(..));

        // Draw 4 vertices per instance (triangle strip quad)
        render_pass.draw(0..4, 0..self.instances.len() as u32);
    }

    pub fn instance_count(&self) -> usize {
        self.instances.len()
    }
}
