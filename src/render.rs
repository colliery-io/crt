//! Rendering logic
//!
//! Multi-pass rendering pipeline for terminal content and effects.

use crate::gpu::SharedGpuState;
use crate::window::{ContextMenuItem, WindowState, DecorationKind};
use crt_core::SelectionRange;
use std::sync::OnceLock;

/// Cached blit pipeline for compositing vello textures
static BLIT_PIPELINE: OnceLock<BlitPipeline> = OnceLock::new();

struct BlitPipeline {
    pipeline: wgpu::RenderPipeline,
    sampler: wgpu::Sampler,
    bind_group_layout: wgpu::BindGroupLayout,
}

/// Render a single frame for a window
pub fn render_frame(state: &mut WindowState, shared: &mut SharedGpuState) {
    state.frame_count = state.frame_count.saturating_add(1);

    // Process PTY output from active shell
    let active_tab_id = state.gpu.tab_bar.active_tab_id();
    if let Some(tab_id) = active_tab_id {
        if let Some(shell) = state.shells.get_mut(&tab_id) {
            if shell.process_pty_output() {
                state.dirty = true;
            }

            // Check for terminal events (title changes and bell)
            let (title, bell) = shell.check_events();
            if let Some(title) = title {
                state.gpu.tab_bar.set_tab_title(tab_id, title);
            }
            if bell {
                state.bell.trigger();
                log::debug!("Bell triggered");
            }
        }
    }

    // Keep redrawing while bell flash is active
    if state.bell.is_active() {
        state.dirty = true;
    }

    // Force re-renders during first 60 frames
    if state.frame_count < 60 {
        state.dirty = true;
        if let Some(tab_id) = active_tab_id {
            state.content_hashes.insert(tab_id, 0);
        }
    }

    // Update text buffer and get cursor/decoration info
    let update_result = if state.dirty {
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

    // Pass 1: Render background gradient
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

    // Pass 1.5: Render background image (if configured)
    if let (Some(bg_state), Some(bind_group)) = (
        &mut state.gpu.background_image_state,
        &state.gpu.background_image_bind_group,
    ) {
        // Update animation if this is an animated GIF
        if bg_state.update(&shared.queue) {
            // Animation frame changed, need to redraw
            state.dirty = true;
        }

        // Keep redrawing for animations
        if bg_state.image.is_animated() {
            state.dirty = true;
        }

        // Update uniforms with UV transform and opacity
        let uv_transform = bg_state.calculate_uv_transform(
            state.gpu.config.width as f32,
            state.gpu.config.height as f32,
        );
        state.gpu.background_image_pipeline.update_uniforms(
            &shared.queue,
            uv_transform,
            bg_state.opacity(),
        );

        // Render background image
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Background Image Render Pass"),
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

        state.gpu.background_image_pipeline.render(&mut pass, bind_group);
    }

    // Pass 2: Update cursor position if content changed
    if let Some(ref result) = update_result {
        state.gpu.terminal_vello.set_cursor(
            result.cursor.x,
            result.cursor.y,
            result.cursor.cell_width,
            result.cursor.cell_height,
            true, // visible
        );
        // Reset blink when cursor moves (makes cursor visible immediately)
        state.gpu.terminal_vello.reset_blink();
    }

    // Update cursor blink state
    state.gpu.terminal_vello.update_blink();

    // Update cached decorations when content changes
    if let Some(ref result) = update_result {
        state.cached_render.decorations = result.decorations.clone();
        state.cached_render.cursor = Some(result.cursor);
    }

    // Pass 3: Render cell backgrounds via RectRenderer (before text)
    // Always render from cached decorations so they persist across frames
    {
        let bg_count = state.cached_render.decorations.iter().filter(|d| d.kind == DecorationKind::Background).count();
        if bg_count > 0 {
            state.gpu.rect_renderer.clear();
            state.gpu.rect_renderer.update_screen_size(
                &shared.queue,
                state.gpu.config.width as f32,
                state.gpu.config.height as f32,
            );

            // Add background rectangles from cached decorations
            for decoration in &state.cached_render.decorations {
                if decoration.kind == DecorationKind::Background {
                    state.gpu.rect_renderer.push_rect(
                        decoration.x,
                        decoration.y,
                        decoration.cell_width,
                        decoration.cell_height,
                        decoration.color,
                    );
                }
            }

            // Render backgrounds directly to frame
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Background Rect Render Pass"),
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

            state.gpu.rect_renderer.render(&shared.queue, &mut pass);
        }
    }

    // Pass 4: Render terminal text directly to frame
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

    // Pass 5: Render cursor, selection, underlines, strikethroughs via vello
    {
        // Get selection from active terminal (if any)
        let selection = active_tab_id
            .and_then(|id| state.shells.get(&id))
            .and_then(|shell| shell.terminal().renderable_content().selection);

        // Prepare vello scene (resets scene, builds cursor)
        state.gpu.terminal_vello.prepare(
            &shared.device,
            state.gpu.config.width,
            state.gpu.config.height,
        );

        // Add selection rectangles after prepare (scene was reset)
        if let Some(selection) = selection {
            render_selection(state, &selection);
        }

        // Add underlines and strikethroughs from cached decorations
        for decoration in &state.cached_render.decorations {
            match decoration.kind {
                DecorationKind::Background => {} // Already rendered in Pass 3
                DecorationKind::Underline => {
                    state.gpu.terminal_vello.add_underline(
                        decoration.x,
                        decoration.y,
                        decoration.cell_width,
                        decoration.cell_height,
                        decoration.color,
                    );
                }
                DecorationKind::Strikethrough => {
                    state.gpu.terminal_vello.add_strikethrough(
                        decoration.x,
                        decoration.y,
                        decoration.cell_width,
                        decoration.cell_height,
                        decoration.color,
                    );
                }
            }
        }

        if let Err(e) = state.gpu.terminal_vello.render_to_texture(&mut shared.vello_renderer, &shared.device, &shared.queue) {
            log::warn!("Terminal vello render error: {:?}", e);
        }

        // Composite cursor/selection/decorations texture onto frame
        if let Some(vello_view) = state.gpu.terminal_vello.texture_view() {
            composite_vello_texture(
                &shared.device,
                &mut encoder,
                &frame_view,
                vello_view,
                state.gpu.config.width,
                state.gpu.config.height,
                state.gpu.config.height as f32,
            );
        }
    }

    // Pass 6: Render tab bar shapes via vello
    {
        state.gpu.tab_bar.prepare(&shared.device, &shared.queue);

        // Render vello scene to texture using shared renderer
        if let Err(e) = state.gpu.tab_bar.render_vello(&mut shared.vello_renderer, &shared.device, &shared.queue) {
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

    // Pass 7: Render tab title text with glow
    render_tab_titles(state, shared, &mut encoder, &frame_view);

    // Pass 8: Render search bar overlay (if search is active)
    if state.search.active {
        render_search_bar(state, shared, &mut encoder, &frame_view);
    }

    // Pass 9: Render bell flash overlay (if bell was triggered)
    let flash_intensity = state.bell.flash_intensity();
    if flash_intensity > 0.0 {
        render_bell_flash(state, shared, &mut encoder, &frame_view, flash_intensity);
    }

    // Pass 10: Render context menu (if visible)
    if state.context_menu.visible {
        render_context_menu(state, shared, &mut encoder, &frame_view);
    }

    shared.queue.submit(std::iter::once(encoder.finish()));
    frame.present();
}

/// Render selection rectangles via vello
fn render_selection(state: &mut WindowState, selection: &SelectionRange) {
    let cell_width = state.gpu.glyph_cache.cell_width();
    let line_height = state.gpu.glyph_cache.line_height();
    let (offset_x, offset_y) = state.gpu.tab_bar.content_offset();
    let padding = 10.0 * state.scale_factor;

    let start_line = selection.start.line.0;
    let end_line = selection.end.line.0;
    let start_col = selection.start.column.0;
    let end_col = selection.end.column.0;

    if selection.is_block {
        // Block selection: rectangle from start to end
        let min_col = start_col.min(end_col);
        let max_col = start_col.max(end_col);

        for line in start_line..=end_line {
            let y = offset_y + padding + (line as f32 * line_height);
            let x = offset_x + padding + (min_col as f32 * cell_width);
            let num_cells = max_col - min_col + 1;
            state.gpu.terminal_vello.add_selection_row(x, y, num_cells, cell_width, line_height);
        }
    } else {
        // Normal selection: spans from start point to end point
        for line in start_line..=end_line {
            let y = offset_y + padding + (line as f32 * line_height);

            let (line_start_col, line_end_col) = if start_line == end_line {
                // Single line selection
                (start_col, end_col)
            } else if line == start_line {
                // First line: from start column to end of line
                // We use a large number for "end of line" - will be clipped by terminal width
                (start_col, 999)
            } else if line == end_line {
                // Last line: from start of line to end column
                (0, end_col)
            } else {
                // Middle line: full line
                (0, 999)
            };

            let x = offset_x + padding + (line_start_col as f32 * cell_width);
            let num_cells = (line_end_col - line_start_col + 1).min(500); // Cap to reasonable number
            state.gpu.terminal_vello.add_selection_row(x, y, num_cells, cell_width, line_height);
        }
    }
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

/// Render search bar overlay
fn render_search_bar(
    state: &mut WindowState,
    shared: &mut SharedGpuState,
    encoder: &mut wgpu::CommandEncoder,
    frame_view: &wgpu::TextureView,
) {
    let (_, content_offset_y) = state.gpu.tab_bar.content_offset();

    // Theme colors for search bar
    let bg_color = [0.15, 0.15, 0.2, 0.95]; // Dark semi-transparent background
    let border_color = [0.3, 0.5, 0.7, 0.8]; // Accent border

    // Prepare vello scene with search bar background
    state.gpu.terminal_vello.prepare(
        &shared.device,
        state.gpu.config.width,
        state.gpu.config.height,
    );

    let (text_x, text_y, _text_width, text_height) = state.gpu.terminal_vello.add_search_bar(
        state.gpu.config.width as f32,
        content_offset_y,
        state.scale_factor,
        bg_color,
        border_color,
    );

    // Render vello scene to texture using shared renderer
    if let Err(e) = state.gpu.terminal_vello.render_to_texture(&mut shared.vello_renderer, &shared.device, &shared.queue) {
        log::warn!("Search bar vello render error: {:?}", e);
    }

    // Composite search bar background onto frame
    if let Some(vello_view) = state.gpu.terminal_vello.texture_view() {
        composite_vello_texture(
            &shared.device,
            encoder,
            frame_view,
            vello_view,
            state.gpu.config.width,
            state.gpu.config.height,
            state.gpu.config.height as f32,
        );
    }

    // Render search text using tab glyph cache
    state.gpu.tab_title_renderer.clear();

    // Build display text: query with cursor + match count
    let query = &state.search.query;
    let match_count = state.search.matches.len();
    let current_match = state.search.current_match + 1; // 1-indexed for display

    let display_text = if query.is_empty() {
        "Find...".to_string()
    } else if match_count > 0 {
        format!("{}| ({}/{})", query, current_match, match_count)
    } else {
        format!("{}| (no matches)", query)
    };

    // Render text
    let text_color = if query.is_empty() {
        [0.5, 0.5, 0.5, 0.8] // Placeholder color
    } else if match_count > 0 {
        [0.9, 0.9, 0.9, 1.0] // Normal text
    } else {
        [0.9, 0.5, 0.5, 1.0] // Red for no matches
    };

    let mut glyphs = Vec::new();
    let mut char_x = text_x;
    let font_height = 14.0 * state.scale_factor;
    let text_baseline_y = text_y + (text_height - font_height) / 2.0;

    for c in display_text.chars() {
        if let Some(glyph) = state.gpu.tab_glyph_cache.position_char(c, char_x, text_baseline_y) {
            glyphs.push(glyph);
        }
        char_x += state.gpu.tab_glyph_cache.cell_width();
    }

    state.gpu.tab_title_renderer.push_glyphs(&glyphs, text_color);
    state.gpu.tab_glyph_cache.flush(&shared.queue);

    // Render text pass
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Search Bar Text Render Pass"),
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
///
/// When `target_height` equals `screen_height`, does a full-screen blit.
/// When `target_height` is smaller, restricts rendering to the top portion.
fn composite_vello_texture(
    device: &wgpu::Device,
    encoder: &mut wgpu::CommandEncoder,
    frame_view: &wgpu::TextureView,
    vello_view: &wgpu::TextureView,
    screen_width: u32,
    screen_height: u32,
    target_height: f32,
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

    // Use viewport to restrict rendering when target is smaller than screen
    let is_partial = (target_height as u32) < screen_height;
    if is_partial {
        pass.set_viewport(
            0.0,
            0.0,
            screen_width as f32,
            target_height,
            0.0,
            1.0,
        );
    }

    pass.set_pipeline(&blit.pipeline);
    pass.set_bind_group(0, &bind_group, &[]);
    pass.draw(0..3, 0..1);
}

/// Render bell flash overlay (semi-transparent white flash)
fn render_bell_flash(
    state: &mut WindowState,
    shared: &SharedGpuState,
    encoder: &mut wgpu::CommandEncoder,
    frame_view: &wgpu::TextureView,
    intensity: f32,
) {
    // Use rect_renderer to draw a full-screen semi-transparent white rectangle
    state.gpu.rect_renderer.clear();
    state.gpu.rect_renderer.update_screen_size(
        &shared.queue,
        state.gpu.config.width as f32,
        state.gpu.config.height as f32,
    );

    // Flash color: white with fading alpha based on intensity
    // Intensity already includes the configured max value
    let flash_color = [1.0, 1.0, 1.0, intensity];

    // Cover the entire screen
    state.gpu.rect_renderer.push_rect(
        0.0,
        0.0,
        state.gpu.config.width as f32,
        state.gpu.config.height as f32,
        flash_color,
    );

    // Render flash overlay
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Bell Flash Render Pass"),
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

    state.gpu.rect_renderer.render(&shared.queue, &mut pass);
}

/// Render context menu overlay
fn render_context_menu(
    state: &mut WindowState,
    shared: &SharedGpuState,
    encoder: &mut wgpu::CommandEncoder,
    frame_view: &wgpu::TextureView,
) {
    let scale = state.scale_factor;
    let items = ContextMenuItem::all();
    let item_count = items.len();

    // Menu dimensions
    let padding_x = 12.0 * scale;
    let padding_y = 6.0 * scale;
    let item_height = 28.0 * scale;
    let menu_width = 180.0 * scale;
    let menu_height = (item_count as f32 * item_height) + (padding_y * 2.0);

    // Get menu position and adjust if near screen edges
    let screen_width = state.gpu.config.width as f32;
    let screen_height = state.gpu.config.height as f32;

    let mut menu_x = state.context_menu.x;
    let mut menu_y = state.context_menu.y;

    // Keep menu within screen bounds
    if menu_x + menu_width > screen_width {
        menu_x = screen_width - menu_width - 4.0;
    }
    if menu_y + menu_height > screen_height {
        menu_y = screen_height - menu_height - 4.0;
    }
    if menu_x < 4.0 {
        menu_x = 4.0;
    }
    if menu_y < 4.0 {
        menu_y = 4.0;
    }

    // Update context menu dimensions for hit testing
    state.context_menu.x = menu_x;
    state.context_menu.y = menu_y;
    state.context_menu.width = menu_width;
    state.context_menu.height = menu_height;
    state.context_menu.item_height = item_height;

    // Colors
    let bg_color = [0.12, 0.12, 0.15, 0.98];
    let border_color = [0.3, 0.3, 0.35, 0.8];
    let hover_color = [0.25, 0.35, 0.5, 0.8];
    let text_color = [0.9, 0.9, 0.9, 1.0];
    let shortcut_color = [0.5, 0.5, 0.55, 1.0];

    // Render background using rect_renderer
    state.gpu.rect_renderer.clear();
    state.gpu.rect_renderer.update_screen_size(
        &shared.queue,
        screen_width,
        screen_height,
    );

    // Menu background
    state.gpu.rect_renderer.push_rect(menu_x, menu_y, menu_width, menu_height, bg_color);

    // Border (simple rectangles around the edges)
    let border_thickness = 1.0 * scale;
    // Top border
    state.gpu.rect_renderer.push_rect(menu_x, menu_y, menu_width, border_thickness, border_color);
    // Bottom border
    state.gpu.rect_renderer.push_rect(menu_x, menu_y + menu_height - border_thickness, menu_width, border_thickness, border_color);
    // Left border
    state.gpu.rect_renderer.push_rect(menu_x, menu_y, border_thickness, menu_height, border_color);
    // Right border
    state.gpu.rect_renderer.push_rect(menu_x + menu_width - border_thickness, menu_y, border_thickness, menu_height, border_color);

    // Hover highlight
    if let Some(hover_idx) = state.context_menu.hovered_item {
        if hover_idx < item_count {
            let hover_y = menu_y + padding_y + (hover_idx as f32 * item_height);
            state.gpu.rect_renderer.push_rect(
                menu_x + border_thickness,
                hover_y,
                menu_width - (border_thickness * 2.0),
                item_height,
                hover_color,
            );
        }
    }

    // Render background pass
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Context Menu Background Pass"),
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

        state.gpu.rect_renderer.render(&shared.queue, &mut pass);
    }

    // Render menu text
    state.gpu.tab_title_renderer.clear();

    let font_height = 12.0 * scale;
    let text_offset_y = (item_height - font_height) / 2.0;

    for (idx, item) in items.iter().enumerate() {
        let item_y = menu_y + padding_y + (idx as f32 * item_height) + text_offset_y;

        // Render label
        let mut glyphs = Vec::new();
        let mut char_x = menu_x + padding_x;
        for c in item.label().chars() {
            if let Some(glyph) = state.gpu.tab_glyph_cache.position_char(c, char_x, item_y) {
                glyphs.push(glyph);
            }
            char_x += state.gpu.tab_glyph_cache.cell_width();
        }
        state.gpu.tab_title_renderer.push_glyphs(&glyphs, text_color);

        // Render shortcut (right-aligned)
        let shortcut = item.shortcut();
        let shortcut_width = shortcut.len() as f32 * state.gpu.tab_glyph_cache.cell_width();
        let shortcut_x = menu_x + menu_width - padding_x - shortcut_width;

        let mut shortcut_glyphs = Vec::new();
        let mut char_x = shortcut_x;
        for c in shortcut.chars() {
            if let Some(glyph) = state.gpu.tab_glyph_cache.position_char(c, char_x, item_y) {
                shortcut_glyphs.push(glyph);
            }
            char_x += state.gpu.tab_glyph_cache.cell_width();
        }
        state.gpu.tab_title_renderer.push_glyphs(&shortcut_glyphs, shortcut_color);
    }

    state.gpu.tab_glyph_cache.flush(&shared.queue);

    // Render text pass
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Context Menu Text Pass"),
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
}
