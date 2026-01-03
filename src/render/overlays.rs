//! Overlay rendering
//!
//! Renders transient UI overlays: indicators, toasts, and flash effects.

use crate::gpu::SharedGpuState;
use crate::window::{ToastType, WindowState};

/// Render bell flash overlay (theme-driven color and intensity)
pub fn render_bell_flash(
    state: &mut WindowState,
    shared: &SharedGpuState,
    encoder: &mut wgpu::CommandEncoder,
    frame_view: &wgpu::TextureView,
    color: crt_theme::Color,
    intensity: f32,
) {
    // Use rect_renderer to draw a full-screen semi-transparent colored rectangle
    state.gpu.rect_renderer.clear();
    state.gpu.rect_renderer.update_screen_size(
        &shared.queue,
        state.gpu.config.width as f32,
        state.gpu.config.height as f32,
    );

    // Flash color from theme with fading alpha based on intensity
    let flash_color = [color.r / 255.0, color.g / 255.0, color.b / 255.0, intensity];

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

    state
        .gpu
        .rect_renderer
        .render(&shared.queue, &mut pass, &state.gpu.rect_instance_buffer);
}

/// Render zoom indicator overlay (centered pill showing zoom percentage)
pub fn render_zoom_indicator(
    state: &mut WindowState,
    shared: &SharedGpuState,
    encoder: &mut wgpu::CommandEncoder,
    frame_view: &wgpu::TextureView,
) {
    let opacity = state.ui.zoom_indicator.opacity();
    if opacity <= 0.0 {
        return;
    }

    let scale = state.ui.zoom_indicator.scale;
    let percentage = (scale * 100.0).round() as i32;
    let text = format!("{}%", percentage);

    // Calculate dimensions
    let screen_width = state.gpu.config.width as f32;
    let screen_height = state.gpu.config.height as f32;

    // Use tab glyph cache metrics for consistent sizing
    let char_width = state.gpu.tab_glyph_cache.cell_width();
    let line_height = state.gpu.tab_glyph_cache.line_height();

    let padding_x = char_width * 1.5;
    let padding_y = line_height * 0.4;
    let text_width = char_width * text.len() as f32;
    let pill_width = text_width + padding_x * 2.0;
    let pill_height = line_height + padding_y * 2.0;

    // Center the pill on screen
    let pill_x = (screen_width - pill_width) / 2.0;
    let pill_y = (screen_height - pill_height) / 2.0;

    // Background color: dark semi-transparent
    let bg_color = [0.1, 0.1, 0.1, 0.85 * opacity];
    // Text color: white
    let text_color = [1.0, 1.0, 1.0, opacity];

    // Render background pill
    state.gpu.rect_renderer.clear();
    state
        .gpu
        .rect_renderer
        .update_screen_size(&shared.queue, screen_width, screen_height);
    state
        .gpu
        .rect_renderer
        .push_rect(pill_x, pill_y, pill_width, pill_height, bg_color);

    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Zoom Indicator Background Pass"),
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

    // Render text using tab title renderer
    state.gpu.tab_title_renderer.clear();
    state
        .gpu
        .tab_title_renderer
        .update_screen_size(&shared.queue, screen_width, screen_height);

    let text_x = pill_x + padding_x;
    let text_y = pill_y + padding_y;

    let mut glyphs = Vec::new();
    let mut char_x = text_x;
    for ch in text.chars() {
        if let Some(glyph) = state.gpu.tab_glyph_cache.position_char(ch, char_x, text_y) {
            glyphs.push(glyph);
        }
        char_x += char_width;
    }
    state
        .gpu
        .tab_title_renderer
        .push_glyphs(&glyphs, text_color);
    state.gpu.tab_glyph_cache.flush(&shared.queue);

    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Zoom Indicator Text Pass"),
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
}

/// Render copy indicator ("Copied!" feedback pill)
pub fn render_copy_indicator(
    state: &mut WindowState,
    shared: &SharedGpuState,
    encoder: &mut wgpu::CommandEncoder,
    frame_view: &wgpu::TextureView,
) {
    let opacity = state.ui.copy_indicator.opacity();
    if opacity <= 0.0 {
        return;
    }

    let text = "Copied!";

    // Calculate dimensions
    let screen_width = state.gpu.config.width as f32;
    let screen_height = state.gpu.config.height as f32;

    // Use tab glyph cache metrics for consistent sizing
    let char_width = state.gpu.tab_glyph_cache.cell_width();
    let line_height = state.gpu.tab_glyph_cache.line_height();

    let padding_x = char_width * 1.5;
    let padding_y = line_height * 0.4;
    let text_width = char_width * text.len() as f32;
    let pill_width = text_width + padding_x * 2.0;
    let pill_height = line_height + padding_y * 2.0;

    // Center the pill on screen
    let pill_x = (screen_width - pill_width) / 2.0;
    let pill_y = (screen_height - pill_height) / 2.0;

    // Background color: green-tinted semi-transparent (success feedback)
    let bg_color = [0.1, 0.3, 0.1, 0.85 * opacity];
    // Text color: white
    let text_color = [1.0, 1.0, 1.0, opacity];

    // Render background pill
    state.gpu.rect_renderer.clear();
    state
        .gpu
        .rect_renderer
        .update_screen_size(&shared.queue, screen_width, screen_height);
    state
        .gpu
        .rect_renderer
        .push_rect(pill_x, pill_y, pill_width, pill_height, bg_color);

    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Copy Indicator Background Pass"),
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

    // Render text using tab title renderer
    state.gpu.tab_title_renderer.clear();
    state
        .gpu
        .tab_title_renderer
        .update_screen_size(&shared.queue, screen_width, screen_height);

    let text_x = pill_x + padding_x;
    let text_y = pill_y + padding_y;

    let mut glyphs = Vec::new();
    let mut char_x = text_x;
    for ch in text.chars() {
        if let Some(glyph) = state.gpu.tab_glyph_cache.position_char(ch, char_x, text_y) {
            glyphs.push(glyph);
        }
        char_x += char_width;
    }
    state
        .gpu
        .tab_title_renderer
        .push_glyphs(&glyphs, text_color);
    state.gpu.tab_glyph_cache.flush(&shared.queue);

    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Copy Indicator Text Pass"),
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
}

/// Render toast notification (bottom-centered)
pub fn render_toast(
    state: &mut WindowState,
    shared: &SharedGpuState,
    encoder: &mut wgpu::CommandEncoder,
    frame_view: &wgpu::TextureView,
) {
    let opacity = state.ui.toast.opacity();
    if opacity <= 0.0 {
        return;
    }

    let message = &state.ui.toast.message;
    if message.is_empty() {
        return;
    }

    // Calculate dimensions
    let screen_width = state.gpu.config.width as f32;
    let screen_height = state.gpu.config.height as f32;

    // Use tab glyph cache metrics for consistent sizing
    let char_width = state.gpu.tab_glyph_cache.cell_width();
    let line_height = state.gpu.tab_glyph_cache.line_height();

    let padding_x = char_width * 1.5;
    let padding_y = line_height * 0.5;
    let text_width = char_width * message.len() as f32;
    let pill_width = text_width + padding_x * 2.0;
    let pill_height = line_height + padding_y * 2.0;

    // Position at bottom center with some margin
    let margin_bottom = line_height * 2.0;
    let pill_x = (screen_width - pill_width) / 2.0;
    let pill_y = screen_height - pill_height - margin_bottom;

    // Background and text colors based on toast type
    let (bg_color, text_color) = match state.ui.toast.toast_type {
        ToastType::Error => (
            [0.6, 0.1, 0.1, 0.95 * opacity], // Dark red background
            [1.0, 1.0, 1.0, opacity],        // White text
        ),
        ToastType::Warning => (
            [0.6, 0.4, 0.1, 0.95 * opacity], // Dark orange background
            [1.0, 1.0, 1.0, opacity],        // White text
        ),
        ToastType::Info => (
            [0.1, 0.2, 0.5, 0.95 * opacity], // Dark blue background
            [1.0, 1.0, 1.0, opacity],        // White text
        ),
    };

    // Render background pill
    state.gpu.rect_renderer.clear();
    state
        .gpu
        .rect_renderer
        .update_screen_size(&shared.queue, screen_width, screen_height);
    state
        .gpu
        .rect_renderer
        .push_rect(pill_x, pill_y, pill_width, pill_height, bg_color);

    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Toast Background Pass"),
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

    // Render text using tab title renderer
    state.gpu.tab_title_renderer.clear();
    state
        .gpu
        .tab_title_renderer
        .update_screen_size(&shared.queue, screen_width, screen_height);

    let text_x = pill_x + padding_x;
    let text_y = pill_y + padding_y;

    let mut glyphs = Vec::new();
    let mut char_x = text_x;
    for ch in message.chars() {
        if let Some(glyph) = state.gpu.tab_glyph_cache.position_char(ch, char_x, text_y) {
            glyphs.push(glyph);
        }
        char_x += char_width;
    }
    state
        .gpu
        .tab_title_renderer
        .push_glyphs(&glyphs, text_color);
    state.gpu.tab_glyph_cache.flush(&shared.queue);

    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Toast Text Pass"),
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
}
