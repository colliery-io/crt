//! Tab bar GPU rendering
//!
//! Handles all wgpu resources and rendering operations.
//! Tab bar is always rendered at the top of the window.

use crate::shaders::builtin;
use crt_theme::{TabTheme, Color};
use wgpu::util::DeviceExt;
use bytemuck::{Pod, Zeroable};

use super::state::TabBarState;
use super::layout::TabLayout;

/// Vertex for tab bar quads
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct TabVertex {
    position: [f32; 2],
    color: [f32; 4],
}

/// Uniforms for tab bar rendering
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct TabUniforms {
    screen_size: [f32; 2],
    _pad: [f32; 2],
}

/// Maximum vertices for tab bar (enough for many tabs)
const MAX_VERTICES: usize = 1024;

/// Tab bar GPU renderer
pub struct TabBarRenderer {
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    vertex_count: usize,
}

impl TabBarRenderer {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Tab Bar Shader"),
            source: wgpu::ShaderSource::Wgsl(builtin::TAB_BAR.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Tab Bar Bind Group Layout"),
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
            label: Some("Tab Bar Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Tab Bar Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<TabVertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: 8,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                    ],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
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

        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Tab Bar Vertex Buffer"),
            size: (MAX_VERTICES * std::mem::size_of::<TabVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Tab Bar Uniform Buffer"),
            contents: bytemuck::cast_slice(&[TabUniforms {
                screen_size: [800.0, 600.0],
                _pad: [0.0; 2],
            }]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Tab Bar Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
            ],
        });

        Self {
            pipeline,
            vertex_buffer,
            uniform_buffer,
            bind_group,
            vertex_count: 0,
        }
    }

    /// Build vertices for the tab bar and upload to GPU
    pub fn prepare(
        &mut self,
        queue: &wgpu::Queue,
        state: &TabBarState,
        layout: &TabLayout,
        theme: &TabTheme,
    ) {
        let (screen_width, screen_height) = layout.screen_size();

        // Update uniforms
        let uniforms = TabUniforms {
            screen_size: [screen_width, screen_height],
            _pad: [0.0; 2],
        };
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));

        // Build and upload vertices
        let vertices = self.build_vertices(state, layout, theme);
        self.vertex_count = vertices.len();
        if !vertices.is_empty() {
            queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&vertices));
        }
    }

    /// Build vertices for the tab bar (always at top)
    fn build_vertices(
        &self,
        state: &TabBarState,
        layout: &TabLayout,
        theme: &TabTheme,
    ) -> Vec<TabVertex> {
        let mut vertices = Vec::new();

        let bar_bg = color_to_array(&theme.bar.background);
        let tab_bg = color_to_array(&theme.tab.background);
        let active_bg = color_to_array(&theme.active.background);
        let border_color = color_to_array(&theme.bar.border_color);

        let s = layout.scale_factor();
        let bar_height = layout.height() * s;
        let border_width = s;
        let (screen_width, _screen_height) = layout.screen_size();

        let tab_rects = layout.tab_rects();
        let active_tab = state.active_tab_index();

        // Tab bar background (full width, at top)
        add_quad(&mut vertices, 0.0, 0.0, screen_width, bar_height, bar_bg);

        // Bottom border
        add_quad(&mut vertices, 0.0, bar_height - s, screen_width, s, border_color);

        // Draw individual tabs
        for (i, rect) in tab_rects.iter().enumerate() {
            let is_active = i == active_tab;
            let bg_color = if is_active { active_bg } else { tab_bg };

            // Tab background
            add_quad(&mut vertices, rect.x, rect.y, rect.width, rect.height, bg_color);

            // Tab borders
            add_quad(&mut vertices, rect.x, rect.y, rect.width, border_width, border_color);
            add_quad(&mut vertices, rect.x, rect.y + rect.height - border_width, rect.width, border_width, border_color);
            add_quad(&mut vertices, rect.x, rect.y, border_width, rect.height, border_color);
            add_quad(&mut vertices, rect.x + rect.width - border_width, rect.y, border_width, rect.height, border_color);

            // Active tab accent (bottom highlight)
            if is_active {
                let accent = color_to_array(&theme.active.accent);
                add_quad(&mut vertices, rect.x, rect.y + rect.height - 2.0 * s, rect.width, 2.0 * s, accent);
            }
        }

        vertices
    }

    /// Render the tab bar
    pub fn render<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
        if self.vertex_count == 0 {
            return;
        }

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        pass.draw(0..self.vertex_count as u32, 0..1);
    }
}

fn color_to_array(color: &Color) -> [f32; 4] {
    [color.r, color.g, color.b, color.a]
}

fn add_quad(vertices: &mut Vec<TabVertex>, x: f32, y: f32, width: f32, height: f32, color: [f32; 4]) {
    // Two triangles for a quad
    // Triangle 1: top-left, top-right, bottom-left
    vertices.push(TabVertex { position: [x, y], color });
    vertices.push(TabVertex { position: [x + width, y], color });
    vertices.push(TabVertex { position: [x, y + height], color });

    // Triangle 2: top-right, bottom-right, bottom-left
    vertices.push(TabVertex { position: [x + width, y], color });
    vertices.push(TabVertex { position: [x + width, y + height], color });
    vertices.push(TabVertex { position: [x, y + height], color });
}
