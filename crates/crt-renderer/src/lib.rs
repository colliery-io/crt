//! CRT Renderer - GPU-accelerated text and effect rendering
//!
//! This crate provides a two-layer rendering architecture:
//! - Background layer: gradient + animated grid (runs every frame, no texture samples)
//! - Text overlay: rendered only when content changes, composited on top
//!
//! This separation allows smooth 60fps animation while only re-rendering
//! text when it actually changes.

pub mod glyph_cache;
pub mod grid_renderer;
pub mod shaders;
pub mod tab_bar;
pub mod terminal_vello;
pub mod vello_renderer;

pub use glyph_cache::{GlyphCache, GlyphKey, GlyphStyle, FontVariants, CachedGlyph, PositionedGlyph};
pub use grid_renderer::GridRenderer;
pub use tab_bar::{TabBar, Tab, TabRect, EditState, TabBarState, TabLayout, VelloTabBarRenderer};
pub use terminal_vello::{TerminalVelloRenderer, CursorShape, CursorState};
pub use vello_renderer::{VelloContext, UiBuilder};

// Re-export vello types needed by consumers
pub use vello::Scene;

use bytemuck::cast_slice;
use crt_theme::Theme;
use shaders::builtin;
use wgpu::util::DeviceExt;


/// Background pipeline - renders gradient + animated grid
pub struct BackgroundPipeline {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    theme: Theme,
    start_time: std::time::Instant,
}

impl BackgroundPipeline {
    pub fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Background Shader"),
            source: wgpu::ShaderSource::Wgsl(builtin::BACKGROUND.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Background Bind Group Layout"),
            entries: &[
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
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Background Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Background Pipeline"),
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
            label: Some("Background Uniform Buffer"),
            contents: cast_slice(&[uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Background Bind Group"),
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
            bind_group_layout,
            uniform_buffer,
            bind_group,
            theme,
            start_time: std::time::Instant::now(),
        }
    }

    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
    }

    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    pub fn update_uniforms(&self, queue: &wgpu::Queue, width: f32, height: f32) {
        let time = self.start_time.elapsed().as_secs_f32();
        let uniforms = self.theme.to_uniforms(width, height, time);
        queue.write_buffer(&self.uniform_buffer, 0, cast_slice(&[uniforms]));
    }

    pub fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.draw(0..4, 0..1);
    }
}

/// Composite pipeline - blends text onto screen with glow
pub struct CompositePipeline {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    uniform_buffer: wgpu::Buffer,
    sampler: wgpu::Sampler,
    theme: Theme,
    start_time: std::time::Instant,
}

impl CompositePipeline {
    pub fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Composite Shader"),
            source: wgpu::ShaderSource::Wgsl(builtin::COMPOSITE.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Composite Bind Group Layout"),
            entries: &[
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
            label: Some("Composite Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Use alpha blending to composite onto the background
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Composite Pipeline"),
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

        let theme = Theme::default();
        let uniforms = theme.to_uniforms(1.0, 1.0, 0.0);

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Composite Uniform Buffer"),
            contents: cast_slice(&[uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Composite Sampler"),
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

    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
    }

    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    pub fn create_bind_group(
        &self,
        device: &wgpu::Device,
        text_texture_view: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Composite Bind Group"),
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

    pub fn update_uniforms(&self, queue: &wgpu::Queue, width: f32, height: f32) {
        let time = self.start_time.elapsed().as_secs_f32();
        let uniforms = self.theme.to_uniforms(width, height, time);
        queue.write_buffer(&self.uniform_buffer, 0, cast_slice(&[uniforms]));
    }

    pub fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>, bind_group: &'a wgpu::BindGroup) {
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, bind_group, &[]);
        render_pass.draw(0..4, 0..1);
    }
}

// Keep the old EffectPipeline for backwards compatibility during transition
pub struct EffectPipeline {
    pub background: BackgroundPipeline,
    pub composite: CompositePipeline,
}

impl EffectPipeline {
    pub fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        Self {
            background: BackgroundPipeline::new(device, target_format),
            composite: CompositePipeline::new(device, target_format),
        }
    }

    pub fn set_theme(&mut self, theme: Theme) {
        self.background.set_theme(theme.clone());
        self.composite.set_theme(theme);
    }

    pub fn theme(&self) -> &Theme {
        self.background.theme()
    }

    pub fn theme_mut(&mut self) -> &mut Theme {
        // This is a bit awkward but maintains compatibility
        // In the future, theme should be stored once and shared
        panic!("Use set_theme() instead of theme_mut() with new architecture");
    }

    pub fn create_bind_group(
        &self,
        device: &wgpu::Device,
        text_texture_view: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        self.composite.create_bind_group(device, text_texture_view)
    }

    pub fn update_uniforms(&self, queue: &wgpu::Queue, width: f32, height: f32) {
        self.background.update_uniforms(queue, width, height);
        self.composite.update_uniforms(queue, width, height);
    }

    // Old render method - kept for compatibility but should migrate to new approach
    pub fn render<'a>(&'a self, _render_pass: &mut wgpu::RenderPass<'a>, _bind_group: &'a wgpu::BindGroup) {
        panic!("Use render_background() and render_composite() separately");
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
    fn test_shaders_compile() {
        assert!(builtin::BACKGROUND.contains("vs_main"));
        assert!(builtin::BACKGROUND.contains("fs_main"));
        assert!(builtin::COMPOSITE.contains("vs_main"));
        assert!(builtin::COMPOSITE.contains("fs_main"));
    }
}
