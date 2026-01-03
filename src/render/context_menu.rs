//! Context menu rendering
//!
//! Renders the right-click context menu with nested theme submenu.

use crate::gpu::SharedGpuState;
use crate::window::{ContextMenuItem, WindowState};

/// Render context menu overlay
pub fn render(
    state: &mut WindowState,
    shared: &SharedGpuState,
    encoder: &mut wgpu::CommandEncoder,
    frame_view: &wgpu::TextureView,
) {
    let scale = state.scale_factor;
    let items = state.ui.context_menu.items();
    let item_count = items.len();
    let separator_height = 12.0 * scale;

    // Menu dimensions
    let padding_x = 12.0 * scale;
    let padding_y = 6.0 * scale;
    let item_height = 24.0 * scale;
    let menu_width = 160.0 * scale; // Main menu is narrower now

    // Calculate total height accounting for separators
    let mut menu_height = padding_y * 2.0;
    for item in &items {
        if item.is_separator() {
            menu_height += separator_height;
        } else {
            menu_height += item_height;
        }
    }

    // Get menu position and adjust if near screen edges
    let screen_width = state.gpu.config.width as f32;
    let screen_height = state.gpu.config.height as f32;

    let mut menu_x = state.ui.context_menu.x;
    let mut menu_y = state.ui.context_menu.y;

    // Keep menu within screen bounds
    if menu_x + menu_width > screen_width {
        menu_x = screen_width - menu_width - 4.0;
    }
    if menu_x < 4.0 {
        menu_x = 4.0;
    }

    if menu_height > screen_height - 8.0 {
        menu_y = 4.0;
    } else if menu_y + menu_height > screen_height - 4.0 {
        menu_y = screen_height - menu_height - 4.0;
    }
    if menu_y < 4.0 {
        menu_y = 4.0;
    }

    // Update context menu dimensions for hit testing
    state.ui.context_menu.x = menu_x;
    state.ui.context_menu.y = menu_y;
    state.ui.context_menu.width = menu_width;
    state.ui.context_menu.height = menu_height;
    state.ui.context_menu.item_height = item_height;

    // Colors from theme
    let ui_style = &state.gpu.effect_pipeline.theme().ui;
    let bg_color = ui_style.context_menu.background.to_array();
    let border_color = ui_style.context_menu.border_color.to_array();
    let hover_color = ui_style.hover.background.to_array();
    let focus_color = ui_style.focus.ring_color.to_array();
    let text_color = ui_style.context_menu.text_color.to_array();
    let shortcut_color = ui_style.context_menu.shortcut_color.to_array();

    // Render background using rect_renderer
    state.gpu.rect_renderer.clear();
    state
        .gpu
        .rect_renderer
        .update_screen_size(&shared.queue, screen_width, screen_height);

    // Menu background
    state
        .gpu
        .rect_renderer
        .push_rect(menu_x, menu_y, menu_width, menu_height, bg_color);

    // Border (simple rectangles around the edges)
    let border_thickness = 1.0 * scale;
    push_menu_border(
        &mut state.gpu.rect_renderer,
        menu_x,
        menu_y,
        menu_width,
        menu_height,
        border_thickness,
        border_color,
    );

    // Pre-compute item positions (accounting for variable heights)
    let mut item_y_positions = Vec::with_capacity(item_count);
    let mut current_y = menu_y + padding_y;
    for item in &items {
        item_y_positions.push(current_y);
        if item.is_separator() {
            current_y += separator_height;
        } else {
            current_y += item_height;
        }
    }

    // Check if we should show submenu (hover on Themes item)
    let themes_idx = state.ui.context_menu.themes_item_index();
    let show_submenu = themes_idx.map_or(false, |idx| {
        state.ui.context_menu.hovered_item == Some(idx)
            || state.ui.context_menu.focused_item == Some(idx)
            || state.ui.context_menu.submenu_visible
    });

    // Calculate submenu position and dimensions
    let theme_items = state.ui.context_menu.theme_items();
    let submenu_item_count = theme_items.len();
    let submenu_width = 200.0 * scale; // Wider for theme names
    let submenu_height = padding_y * 2.0 + (submenu_item_count as f32 * item_height);

    // Position submenu to the right of main menu, aligned with Themes item
    let mut submenu_x = menu_x + menu_width - border_thickness;
    let mut submenu_y = if let Some(idx) = themes_idx {
        item_y_positions.get(idx).copied().unwrap_or(menu_y)
    } else {
        menu_y
    };

    // Adjust if submenu would go off screen
    if submenu_x + submenu_width > screen_width - 4.0 {
        // Show submenu on the left side instead
        submenu_x = menu_x - submenu_width + border_thickness;
    }
    if submenu_x < 4.0 {
        submenu_x = 4.0;
    }
    if submenu_y + submenu_height > screen_height - 4.0 {
        submenu_y = screen_height - submenu_height - 4.0;
    }
    if submenu_y < 4.0 {
        submenu_y = 4.0;
    }

    // Update submenu state
    if show_submenu && !theme_items.is_empty() {
        state.ui.context_menu.submenu_visible = true;
        state.ui.context_menu.submenu_x = submenu_x;
        state.ui.context_menu.submenu_y = submenu_y;
        state.ui.context_menu.submenu_width = submenu_width;
        state.ui.context_menu.submenu_height = submenu_height;

        // Render submenu background
        state
            .gpu
            .rect_renderer
            .push_rect(submenu_x, submenu_y, submenu_width, submenu_height, bg_color);

        // Submenu border
        push_menu_border(
            &mut state.gpu.rect_renderer,
            submenu_x,
            submenu_y,
            submenu_width,
            submenu_height,
            border_thickness,
            border_color,
        );

        // Submenu hover highlight
        if let Some(hover_idx) = state.ui.context_menu.submenu_hovered_item {
            if hover_idx < submenu_item_count {
                let hover_y = submenu_y + padding_y + (hover_idx as f32 * item_height);
                state.gpu.rect_renderer.push_rect(
                    submenu_x + border_thickness,
                    hover_y,
                    submenu_width - (border_thickness * 2.0),
                    item_height,
                    hover_color,
                );
            }
        }
    }

    // Hover highlight for main menu
    if let Some(hover_idx) = state.ui.context_menu.hovered_item
        && hover_idx < item_count
        && items[hover_idx].is_selectable()
    {
        let hover_y = item_y_positions[hover_idx];
        state.gpu.rect_renderer.push_rect(
            menu_x + border_thickness,
            hover_y,
            menu_width - (border_thickness * 2.0),
            item_height,
            hover_color,
        );
    }

    // Focus indicator (keyboard focus) - rendered as a border/ring
    if let Some(focus_idx) = state.ui.context_menu.focused_item
        && focus_idx < item_count
        && items[focus_idx].is_selectable()
    {
        let focus_y = item_y_positions[focus_idx];
        let focus_border = 2.0 * scale;
        let inset = border_thickness + 2.0 * scale;

        // Draw focus ring as 4 rectangles
        state.gpu.rect_renderer.push_rect(
            menu_x + inset,
            focus_y,
            menu_width - (inset * 2.0),
            focus_border,
            focus_color,
        );
        state.gpu.rect_renderer.push_rect(
            menu_x + inset,
            focus_y + item_height - focus_border,
            menu_width - (inset * 2.0),
            focus_border,
            focus_color,
        );
        state.gpu.rect_renderer.push_rect(
            menu_x + inset,
            focus_y,
            focus_border,
            item_height,
            focus_color,
        );
        state.gpu.rect_renderer.push_rect(
            menu_x + menu_width - inset - focus_border,
            focus_y,
            focus_border,
            item_height,
            focus_color,
        );
    }

    // Render separators as horizontal lines
    for (idx, item) in items.iter().enumerate() {
        if item.is_separator() {
            let sep_y = item_y_positions[idx] + separator_height / 2.0;
            let sep_inset = padding_x / 2.0;
            state.gpu.rect_renderer.push_rect(
                menu_x + sep_inset,
                sep_y,
                menu_width - (sep_inset * 2.0),
                1.0 * scale,
                border_color,
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

        state
            .gpu
            .rect_renderer
            .render(&shared.queue, &mut pass, &state.gpu.rect_instance_buffer);
    }

    // Render menu text
    state.gpu.tab_title_renderer.clear();

    let font_height = 12.0 * scale;
    let text_offset_y = (item_height - font_height) / 2.0;

    // Render main menu items
    for (idx, item) in items.iter().enumerate() {
        if item.is_separator() {
            continue;
        }

        let item_y = item_y_positions[idx] + text_offset_y;
        let label = item.label();
        let label_start_x = menu_x + padding_x;

        // Render label
        let mut glyphs = Vec::new();
        let mut char_x = label_start_x;
        for c in label.chars() {
            if let Some(glyph) = state.gpu.tab_glyph_cache.position_char(c, char_x, item_y) {
                glyphs.push(glyph);
            }
            char_x += state.gpu.tab_glyph_cache.cell_width();
        }
        state
            .gpu
            .tab_title_renderer
            .push_glyphs(&glyphs, text_color);

        // Render shortcut/arrow (right-aligned)
        let shortcut = item.shortcut();
        if !shortcut.is_empty() {
            let shortcut_width =
                shortcut.chars().count() as f32 * state.gpu.tab_glyph_cache.cell_width();
            let shortcut_x = menu_x + menu_width - padding_x - shortcut_width;

            let mut shortcut_glyphs = Vec::new();
            let mut char_x = shortcut_x;
            for c in shortcut.chars() {
                if let Some(glyph) = state.gpu.tab_glyph_cache.position_char(c, char_x, item_y) {
                    shortcut_glyphs.push(glyph);
                }
                char_x += state.gpu.tab_glyph_cache.cell_width();
            }
            state
                .gpu
                .tab_title_renderer
                .push_glyphs(&shortcut_glyphs, shortcut_color);
        }
    }

    // Render submenu items
    if show_submenu && !theme_items.is_empty() {
        for (idx, item) in theme_items.iter().enumerate() {
            let item_y = submenu_y + padding_y + (idx as f32 * item_height) + text_offset_y;

            // Check if this theme is current (for checkmark)
            let (label, show_checkmark) = match item {
                ContextMenuItem::Theme(name) => {
                    let is_current = state.ui.context_menu.is_current_theme(name);
                    (item.label(), is_current)
                }
                _ => (item.label(), false),
            };

            // Render checkmark for current theme
            let label_start_x = if show_checkmark {
                let checkmark = "\u{2713}";
                let mut char_x = submenu_x + padding_x;
                for c in checkmark.chars() {
                    if let Some(glyph) = state.gpu.tab_glyph_cache.position_char(c, char_x, item_y)
                    {
                        state
                            .gpu
                            .tab_title_renderer
                            .push_glyphs(&[glyph], text_color);
                    }
                    char_x += state.gpu.tab_glyph_cache.cell_width();
                }
                submenu_x + padding_x + (2.0 * state.gpu.tab_glyph_cache.cell_width())
            } else {
                // Indent to align with checkmarked items
                submenu_x + padding_x + (2.0 * state.gpu.tab_glyph_cache.cell_width())
            };

            // Render label
            let mut glyphs = Vec::new();
            let mut char_x = label_start_x;
            for c in label.chars() {
                if let Some(glyph) = state.gpu.tab_glyph_cache.position_char(c, char_x, item_y) {
                    glyphs.push(glyph);
                }
                char_x += state.gpu.tab_glyph_cache.cell_width();
            }
            state
                .gpu
                .tab_title_renderer
                .push_glyphs(&glyphs, text_color);
        }
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

        state.gpu.tab_title_renderer.render(
            &shared.queue,
            &mut pass,
            &state.gpu.overlay_text_instance_buffer,
        );
    }
}

/// Helper to push menu border rectangles
fn push_menu_border(
    renderer: &mut crt_renderer::RectRenderer,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    thickness: f32,
    color: [f32; 4],
) {
    // Top
    renderer.push_rect(x, y, width, thickness, color);
    // Bottom
    renderer.push_rect(x, y + height - thickness, width, thickness, color);
    // Left
    renderer.push_rect(x, y, thickness, height, color);
    // Right
    renderer.push_rect(x + width - thickness, y, thickness, height, color);
}
