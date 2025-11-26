//! Rendering logic
//!
//! Multi-pass rendering pipeline for terminal content and effects.

use crate::gpu::SharedGpuState;
use crate::window::WindowState;
use std::sync::OnceLock;

/// Cached blit pipeline for compositing vello textures
static BLIT_PIPELINE: OnceLock<BlitPipeline> = OnceLock::new();

struct BlitPipeline {
    pipeline: wgpu::RenderPipeline,
    sampler: wgpu::Sampler,
    bind_group_layout: wgpu::BindGroupLayout,
}

/// Render a single frame for a window
pub fn render_frame(state: &mut WindowState, shared: &SharedGpuState) {
    state.frame_count = state.frame_count.saturating_add(1);

    // Process PTY output from active shell
    let active_tab_id = state.gpu.tab_bar.active_tab_id();
    if let Some(tab_id) = active_tab_id {
        if let Some(shell) = state.shells.get_mut(&tab_id) {
            if shell.process_pty_output() {
                state.dirty = true;
            }

            if let Some(title) = shell.check_title_change() {
                state.gpu.tab_bar.set_tab_title(tab_id, title);
            }
        }
    }

    // Force re-renders during first 60 frames
    if state.frame_count < 60 {
        state.dirty = true;
        if let Some(tab_id) = active_tab_id {
            state.content_hashes.insert(tab_id, 0);
        }
    }

    // Update text buffer and get cursor info
    let cursor_info = if state.dirty {
        state.dirty = false;
        state.update_text_buffer(shared)
    } else {
        None
    };

    // Render
    let frame = match state.gpu.surface.get_current_texture() {
        Ok(f) => f,
        Err(e) => {
            log::warn!("Failed to get surface texture: {:?}", e);
            return;
        }
    };
    let frame_view = frame.texture.create_view(&Default::default());

    let mut encoder = shared.device.create_command_encoder(&Default::default());

    // Update effect uniforms
    state.gpu.effect_pipeline.update_uniforms(
        &shared.queue,
        state.gpu.config.width as f32,
        state.gpu.config.height as f32,
    );

    // Pass 1: Render background
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Background Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &frame_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        state.gpu.effect_pipeline.background.render(&mut pass);
    }

    // Pass 2: Update cursor position if content changed
    if let Some(cursor) = cursor_info {
        state.gpu.terminal_vello.set_cursor(
            cursor.x,
            cursor.y,
            cursor.cell_width,
            cursor.cell_height,
            true, // visible
        );
        // Reset blink when cursor moves (makes cursor visible immediately)
        state.gpu.terminal_vello.reset_blink();
    }

    // Update cursor blink state
    state.gpu.terminal_vello.update_blink();

    // Pass 3: Render terminal text directly to frame (every frame)
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Terminal Text Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &frame_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        state.gpu.grid_renderer.render(&shared.queue, &mut pass);
    }

    // Pass 4: Render cursor via vello (always prepare if cursor exists, blink handled internally)
    if state.gpu.terminal_vello.has_cursor() {
        state.gpu.terminal_vello.prepare(
            &shared.device,
            state.gpu.config.width,
            state.gpu.config.height,
        );

        if let Err(e) = state.gpu.terminal_vello.render_to_texture(&shared.device, &shared.queue) {
            log::warn!("Terminal vello render error: {:?}", e);
        }

        // Composite cursor texture onto frame
        if let Some(cursor_view) = state.gpu.terminal_vello.texture_view() {
            composite_vello_texture(
                &shared.device,
                &mut encoder,
                &frame_view,
                cursor_view,
                state.gpu.config.width,
                state.gpu.config.height,
                state.gpu.config.height as f32,
            );
        }
    }

    // Pass 5: Render tab bar shapes via vello
    {
        state.gpu.tab_bar.prepare(&shared.device, &shared.queue);

        // Render vello scene to texture
        if let Err(e) = state.gpu.tab_bar.render_vello(&shared.device, &shared.queue) {
            log::warn!("Vello tab bar render error: {:?}", e);
        }

        // Composite vello texture onto frame
        if let Some(vello_view) = state.gpu.tab_bar.vello_texture_view() {
            composite_vello_texture(
                &shared.device,
                &mut encoder,
                &frame_view,
                vello_view,
                state.gpu.config.width,
                state.gpu.config.height,
                state.gpu.tab_bar.height() * state.scale_factor,
            );
        }
    }

    // Pass 6: Render tab title text with glow
    render_tab_titles(state, shared, &mut encoder, &frame_view);

    shared.queue.submit(std::iter::once(encoder.finish()));
    frame.present();
}

/// Render tab title text with glow effect
fn render_tab_titles(
    state: &mut WindowState,
    shared: &SharedGpuState,
    encoder: &mut wgpu::CommandEncoder,
    frame_view: &wgpu::TextureView,
) {
    let tab_labels = state.gpu.tab_bar.get_tab_labels();
    if tab_labels.is_empty() {
        return;
    }

    state.gpu.tab_title_renderer.clear();

    let active_color = state.gpu.tab_bar.active_tab_color();
    let inactive_color = state.gpu.tab_bar.inactive_tab_color();
    let active_shadow = state.gpu.tab_bar.active_tab_text_shadow();

    // First pass: render glow layers for active tabs
    if let Some((radius, glow_color)) = active_shadow {
        // Tighter glow offsets for a subtle halo effect
        let offsets = [
            (-0.75, -0.75), (0.75, -0.75), (-0.75, 0.75), (0.75, 0.75),
            (-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0),
            (-0.5, 0.0), (0.5, 0.0), (0.0, -0.5), (0.0, 0.5),
        ];

        let glow_alpha = (glow_color[3] * (radius / 25.0).min(1.0)).min(0.4);
        let glow_render_color = [glow_color[0], glow_color[1], glow_color[2], glow_alpha];

        for (x, y, title, is_active, _is_editing) in &tab_labels {
            if *is_active {
                for (ox, oy) in &offsets {
                    let mut glyphs = Vec::new();
                    let mut char_x = *x + ox;
                    for c in title.chars() {
                        if let Some(glyph) = state.gpu.tab_glyph_cache.position_char(c, char_x, *y + oy) {
                            glyphs.push(glyph);
                        }
                        char_x += state.gpu.tab_glyph_cache.cell_width();
                    }
                    state.gpu.tab_title_renderer.push_glyphs(&glyphs, glow_render_color);
                }
            }
        }
    }

    // Second pass: render actual text on top
    for (x, y, title, is_active, is_editing) in tab_labels {
        let mut glyphs = Vec::new();
        let mut char_x = x;
        for c in title.chars() {
            if let Some(glyph) = state.gpu.tab_glyph_cache.position_char(c, char_x, y) {
                glyphs.push(glyph);
            }
            char_x += state.gpu.tab_glyph_cache.cell_width();
        }

        let text_color = if is_editing {
            [
                (active_color[0] * 1.2).min(1.0),
                (active_color[1] * 1.2).min(1.0),
                (active_color[2] * 1.2).min(1.0),
                active_color[3],
            ]
        } else if is_active {
            active_color
        } else {
            inactive_color
        };
        state.gpu.tab_title_renderer.push_glyphs(&glyphs, text_color);
    }

    state.gpu.tab_glyph_cache.flush(&shared.queue);

    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Tab Title Render Pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: frame_view,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: wgpu::StoreOp::Store,
            },
            depth_slice: None,
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
    });

    state.gpu.tab_title_renderer.render(&shared.queue, &mut pass);
}

/// Simple blit shader for compositing vello textures
const BLIT_SHADER: &str = r#"
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    // Full-screen triangle
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    var uvs = array<vec2<f32>, 3>(
        vec2<f32>(0.0, 1.0),
        vec2<f32>(2.0, 1.0),
        vec2<f32>(0.0, -1.0),
    );

    var out: VertexOutput;
    out.position = vec4<f32>(positions[vertex_index], 0.0, 1.0);
    out.uv = uvs[vertex_index];
    return out;
}

@group(0) @binding(0) var t_source: texture_2d<f32>;
@group(0) @binding(1) var s_source: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(t_source, s_source, in.uv);
    // Pre-multiplied alpha blending will be handled by the blend state
    return color;
}
"#;

fn get_or_init_blit_pipeline(device: &wgpu::Device, format: wgpu::TextureFormat) -> &'static BlitPipeline {
    BLIT_PIPELINE.get_or_init(|| {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Blit Shader"),
            source: wgpu::ShaderSource::Wgsl(BLIT_SHADER.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Blit Bind Group Layout"),
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
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Blit Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Blit Pipeline"),
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

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Blit Sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        BlitPipeline {
            pipeline,
            sampler,
            bind_group_layout,
        }
    })
}

/// Composite vello-rendered texture onto the framebuffer
fn composite_vello_texture(
    device: &wgpu::Device,
    encoder: &mut wgpu::CommandEncoder,
    frame_view: &wgpu::TextureView,
    vello_view: &wgpu::TextureView,
    _width: u32,
    _height: u32,
    _tab_bar_height: f32,
) {
    // Get or create the blit pipeline
    // Note: We use Rgba8Unorm as source format, destination format from surface
    let blit = get_or_init_blit_pipeline(device, wgpu::TextureFormat::Bgra8UnormSrgb);

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Vello Blit Bind Group"),
        layout: &blit.bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(vello_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&blit.sampler),
            },
        ],
    });

    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Vello Composite Pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: frame_view,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: wgpu::StoreOp::Store,
            },
            depth_slice: None,
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
    });

    pass.set_pipeline(&blit.pipeline);
    pass.set_bind_group(0, &bind_group, &[]);
    pass.draw(0..3, 0..1);
}
