//! Dialog rendering
//!
//! Renders input dialog overlays: search bar and window rename.

use crate::gpu::SharedGpuState;
use crate::window::WindowState;

/// Render search bar overlay
pub fn render_search_bar(
    state: &mut WindowState,
    shared: &mut SharedGpuState,
    encoder: &mut wgpu::CommandEncoder,
    frame_view: &wgpu::TextureView,
) {
    let (_, content_offset_y) = state.gpu.tab_bar.content_offset();
    let s = state.scale_factor;
    // content_offset_y is in logical pixels, scale to physical
    let content_offset_y = content_offset_y * s;

    // Theme colors for search bar
    let ui_style = &state.gpu.effect_pipeline.theme().ui;
    let bg_color = ui_style.search_bar.background.to_array();
    let focus_glow_color = ui_style.focus.glow_color.to_array();
    let focus_border_color = ui_style.focus.ring_color.to_array();

    // Calculate search bar dimensions (same as vello version)
    let bar_width = 300.0 * s;
    let bar_height = 32.0 * s;
    let margin = 20.0 * s;
    let padding = 8.0 * s;
    let border_width = ui_style.focus.ring_thickness * s;
    let glow_size = ui_style.focus.glow_size * s;

    let bar_x = state.gpu.config.width as f32 - bar_width - margin;
    let bar_y = content_offset_y + margin;

    // Render search bar using RectRenderer (direct, no intermediate texture)
    state.gpu.rect_renderer.clear();
    state.gpu.rect_renderer.update_screen_size(
        &shared.queue,
        state.gpu.config.width as f32,
        state.gpu.config.height as f32,
    );

    // Outer glow (focus indicator) - slightly larger than the bar
    state.gpu.rect_renderer.push_rect(
        bar_x - glow_size,
        bar_y - glow_size,
        bar_width + glow_size * 2.0,
        bar_height + glow_size * 2.0,
        focus_glow_color,
    );

    // Focus border rect (bright blue)
    state
        .gpu
        .rect_renderer
        .push_rect(bar_x, bar_y, bar_width, bar_height, focus_border_color);

    // Inner background rect
    state.gpu.rect_renderer.push_rect(
        bar_x + border_width,
        bar_y + border_width,
        bar_width - border_width * 2.0,
        bar_height - border_width * 2.0,
        bg_color,
    );

    // Render search bar background directly to frame
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Search Bar Background Pass"),
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

        state
            .gpu
            .rect_renderer
            .render(&shared.queue, &mut pass, &state.gpu.rect_instance_buffer);
    }

    // Calculate text position
    let text_x = bar_x + border_width + padding;
    let text_y = bar_y + border_width + padding;
    let text_height = bar_height - border_width * 2.0 - padding * 2.0;

    // Render search text using tab glyph cache
    state.gpu.tab_title_renderer.clear();

    // Build display text: query with cursor + match count
    let query = &state.ui.search.query;
    let match_count = state.ui.search.matches.len();
    let current_match = state.ui.search.current_match + 1; // 1-indexed for display

    let display_text = if query.is_empty() {
        "Find...".to_string()
    } else if match_count > 0 {
        format!("{}| ({}/{})", query, current_match, match_count)
    } else {
        format!("{}| (no matches)", query)
    };

    // Render text - get fresh reference to ui_style for text colors
    let ui_style = &state.gpu.effect_pipeline.theme().ui;
    let text_color = if query.is_empty() {
        ui_style.search_bar.placeholder_color.to_array()
    } else if match_count > 0 {
        ui_style.search_bar.text_color.to_array()
    } else {
        ui_style.search_bar.no_match_color.to_array()
    };

    let mut glyphs = Vec::new();
    let mut char_x = text_x;
    let font_height = 14.0 * state.scale_factor;
    let text_baseline_y = text_y + (text_height - font_height) / 2.0;

    for c in display_text.chars() {
        if let Some(glyph) = state
            .gpu
            .tab_glyph_cache
            .position_char(c, char_x, text_baseline_y)
        {
            glyphs.push(glyph);
        }
        char_x += state.gpu.tab_glyph_cache.cell_width();
    }

    state
        .gpu
        .tab_title_renderer
        .push_glyphs(&glyphs, text_color);
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

    state.gpu.tab_title_renderer.render(
        &shared.queue,
        &mut pass,
        &state.gpu.overlay_text_instance_buffer,
    );
}

/// Render window rename input bar overlay
pub fn render_window_rename(
    state: &mut WindowState,
    shared: &mut SharedGpuState,
    encoder: &mut wgpu::CommandEncoder,
    frame_view: &wgpu::TextureView,
) {
    let (_, content_offset_y) = state.gpu.tab_bar.content_offset();
    let s = state.scale_factor;
    // content_offset_y is in logical pixels, scale to physical
    let content_offset_y = content_offset_y * s;

    // Theme colors for rename bar
    let ui_style = &state.gpu.effect_pipeline.theme().ui;
    let bg_color = ui_style.rename_bar.background.to_array();
    let focus_glow_color = ui_style.focus.glow_color.to_array();
    let focus_border_color = ui_style.focus.ring_color.to_array();

    // Calculate rename bar dimensions (wider than search bar, centered)
    let bar_width = 400.0 * s;
    let bar_height = 36.0 * s;
    let margin = 40.0 * s;
    let padding = 10.0 * s;
    let border_width = ui_style.focus.ring_thickness * s;
    let glow_size = ui_style.focus.glow_size * s;

    // Center horizontally
    let bar_x = (state.gpu.config.width as f32 - bar_width) / 2.0;
    let bar_y = content_offset_y + margin;

    // Render rename bar using RectRenderer
    state.gpu.rect_renderer.clear();
    state.gpu.rect_renderer.update_screen_size(
        &shared.queue,
        state.gpu.config.width as f32,
        state.gpu.config.height as f32,
    );

    // Outer glow (focus indicator)
    state.gpu.rect_renderer.push_rect(
        bar_x - glow_size,
        bar_y - glow_size,
        bar_width + glow_size * 2.0,
        bar_height + glow_size * 2.0,
        focus_glow_color,
    );

    // Focus border rect (bright blue)
    state
        .gpu
        .rect_renderer
        .push_rect(bar_x, bar_y, bar_width, bar_height, focus_border_color);

    // Inner background rect
    state.gpu.rect_renderer.push_rect(
        bar_x + border_width,
        bar_y + border_width,
        bar_width - border_width * 2.0,
        bar_height - border_width * 2.0,
        bg_color,
    );

    // Render rename bar background directly to frame
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Window Rename Bar Background Pass"),
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

        state
            .gpu
            .rect_renderer
            .render(&shared.queue, &mut pass, &state.gpu.rect_instance_buffer);
    }

    // Calculate text position
    let text_x = bar_x + border_width + padding;
    let text_y = bar_y + border_width + padding;
    let text_height = bar_height - border_width * 2.0 - padding * 2.0;

    // Render rename text using tab glyph cache
    state.gpu.tab_title_renderer.clear();

    // Build display text: "Rename: " + input + cursor
    let input = &state.ui.window_rename.input;
    let display_text = format!("Rename: {}|", input);

    // Render text - get fresh reference to ui_style for text colors
    let ui_style = &state.gpu.effect_pipeline.theme().ui;
    let label_color = ui_style.rename_bar.label_color.to_array();
    let input_color = ui_style.rename_bar.text_color.to_array();

    let mut glyphs = Vec::new();
    let mut char_x = text_x;
    let font_height = 14.0 * state.scale_factor;
    let text_baseline_y = text_y + (text_height - font_height) / 2.0;
    let label_len = "Rename: ".len();

    for (idx, c) in display_text.chars().enumerate() {
        if let Some(glyph) = state
            .gpu
            .tab_glyph_cache
            .position_char(c, char_x, text_baseline_y)
        {
            glyphs.push(glyph);
        }

        // Render label part first, then input part
        if idx == label_len - 1 {
            // Push label glyphs
            state
                .gpu
                .tab_title_renderer
                .push_glyphs(&glyphs, label_color);
            glyphs.clear();
        }

        char_x += state.gpu.tab_glyph_cache.cell_width();
    }

    // Push remaining input glyphs
    if !glyphs.is_empty() {
        state
            .gpu
            .tab_title_renderer
            .push_glyphs(&glyphs, input_color);
    }

    state.gpu.tab_glyph_cache.flush(&shared.queue);

    // Render text pass
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Window Rename Bar Text Render Pass"),
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

    state.gpu.tab_title_renderer.render(
        &shared.queue,
        &mut pass,
        &state.gpu.overlay_text_instance_buffer,
    );
}
