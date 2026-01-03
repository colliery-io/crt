//! Rendering logic
//!
//! Multi-pass rendering pipeline for terminal content and effects.

mod context_menu;
mod dialogs;
mod overlays;
mod selection;

use std::time::Instant;

use crate::gpu::SharedGpuState;
use crate::profiling::{self, FrameTiming, GridSnapshot};
use crate::window::{DecorationKind, OverrideEventType, WindowState};
use crt_core::ShellEvent;
use crt_renderer::EffectConfig;
use crt_theme::ToEffectConfig;

/// Convert a ToEffectConfig implementor to an EffectConfig
fn to_effect_config<T: ToEffectConfig>(source: &T) -> EffectConfig {
    let mut config = EffectConfig::new();
    for (key, value) in source.to_config_pairs() {
        config.insert(key, value);
    }
    config
}

/// How often to reset vello renderer to clean up atlas resources (in frames)
/// At 60fps, 300 frames = every 5 seconds
const VELLO_RESET_INTERVAL: u32 = 300;

/// Render a single frame for a window
pub fn render_frame(state: &mut WindowState, shared: &mut SharedGpuState) {
    // Skip rendering for fully occluded windows (covered by other windows)
    if state.render.occluded {
        return;
    }

    let frame_start = Instant::now();
    let mut timing = FrameTiming::default();
    state.render.frame_count = state.render.frame_count.saturating_add(1);

    // Log every 300 frames (~5 seconds at 60fps) or on frame 1
    if state.render.frame_count == 1 || state.render.frame_count.is_multiple_of(300) {
        log::debug!(
            "Frame {} starting (dirty={}, effects_enabled={})",
            state.render.frame_count,
            state.render.dirty,
            state.gpu.effects_renderer.has_enabled_effects()
        );
    }

    // Periodically reset vello renderer to clean up accumulated texture atlas resources
    // This prevents unbounded GPU memory growth from vello's internal caches
    // NOTE: The primary memory fix is frame throttling in main.rs (see about_to_wait).
    // This reset is a secondary defense against vello atlas accumulation.
    if state
        .render
        .frame_count
        .is_multiple_of(VELLO_RESET_INTERVAL)
        && state.gpu.effects_renderer.has_enabled_effects()
    {
        shared.reset_vello_renderer();
    }

    // Update backdrop effects animation (assume ~60fps for dt)
    let update_start = Instant::now();
    let dt = 1.0 / 60.0;
    state.gpu.effects_renderer.update(dt);
    timing.effects_us = update_start.elapsed().as_micros() as u64;

    // Keep redrawing if effects are animating
    if state.gpu.effects_renderer.has_enabled_effects() {
        state.render.dirty = true;
    }

    // Keep redrawing if sprite animation is active (uses raw wgpu, no memory growth)
    if state.gpu.sprite_state.is_some() {
        state.render.dirty = true;
    }

    // Process PTY output from active shell
    let active_tab_id = state.gpu.tab_bar.active_tab_id();
    if let Some(tab_id) = active_tab_id
        && let Some(shell) = state.shells.get_mut(&tab_id)
    {
        if shell.process_pty_output() {
            state.render.dirty = true;
            // Invalidate content hash to ensure re-render captures all changes
            state.content_hashes.insert(tab_id, 0);
        }

        // Check for shell events (bell, command success/fail) and title changes
        let (shell_events, title) = shell.take_shell_events();
        if let Some(title) = title {
            state.gpu.tab_bar.set_tab_title(tab_id, title);
        }

        // Process shell events for theme overrides
        let theme = state.gpu.effect_pipeline.theme();
        for event in &shell_events {
            log::debug!("Processing shell event: {:?}", event);

            // Trigger bell flash for bell events
            if matches!(event, ShellEvent::Bell) {
                log::info!("Bell triggered - activating flash");
                state.ui.bell.trigger();
            }

            // Command success clears any active fail override (put out the fire!)
            if matches!(event, ShellEvent::CommandSuccess) {
                log::info!("Command success - clearing fail override");
                state
                    .ui
                    .overrides
                    .clear_event(OverrideEventType::CommandFail);
            }

            if matches!(event, ShellEvent::CommandFail(_)) {
                log::info!("Command failed - checking for theme override");
            }

            // Get the corresponding theme override for this event
            let override_opt = match event {
                ShellEvent::Bell => theme.on_bell.clone(),
                ShellEvent::CommandSuccess => theme.on_command_success.clone(),
                ShellEvent::CommandFail(_) => theme.on_command_fail.clone(),
            };

            // Add to active overrides if theme defines this event
            if let Some(properties) = override_opt {
                log::info!(
                    "Applying theme override for {:?} (duration: {}ms, cursor_color: {:?})",
                    event,
                    properties.duration_ms,
                    properties.cursor_color
                );
                let event_type = OverrideEventType::from(*event);
                state.ui.overrides.add(event_type, properties);
            } else {
                log::debug!("No theme override defined for {:?}", event);
            }
        }
    }

    // Keep redrawing while bell flash is active
    if state.ui.bell.is_active() {
        state.render.dirty = true;
    }

    // Update overrides and keep redrawing while any are active
    if state.ui.overrides.update() {
        state.render.dirty = true;
    }

    // Apply starfield patch from active overrides (if any)
    if let Some(patch) = state.ui.overrides.get_starfield_patch() {
        if !state.ui.overrides.is_patched("starfield") {
            let config = to_effect_config(patch);
            state
                .gpu
                .effects_renderer
                .apply_effect_patch("starfield", &config);
            state.ui.overrides.set_patched("starfield");
            state.render.dirty = true;
        }
    } else if state.ui.overrides.is_patched("starfield") {
        // Restore starfield from base theme
        let theme = state.gpu.effect_pipeline.theme();
        if let Some(ref starfield) = theme.starfield {
            let config = to_effect_config(starfield);
            state
                .gpu
                .effects_renderer
                .apply_effect_patch("starfield", &config);
        }
        state.ui.overrides.clear_patched("starfield");
        log::debug!("Restored starfield to base theme");
    }

    // Apply particle patch from active overrides (if any)
    if let Some(patch) = state.ui.overrides.get_particle_patch() {
        if !state.ui.overrides.is_patched("particles") {
            let config = to_effect_config(patch);
            state
                .gpu
                .effects_renderer
                .apply_effect_patch("particles", &config);
            state.ui.overrides.set_patched("particles");
            state.render.dirty = true;
        }
    } else if state.ui.overrides.is_patched("particles") {
        // Restore particles from base theme
        let theme = state.gpu.effect_pipeline.theme();
        if let Some(ref particles) = theme.particles {
            let config = to_effect_config(particles);
            state
                .gpu
                .effects_renderer
                .apply_effect_patch("particles", &config);
        }
        state.ui.overrides.clear_patched("particles");
        log::debug!("Restored particles to base theme");
    }

    // Apply grid patch from active overrides (if any)
    if let Some(patch) = state.ui.overrides.get_grid_patch() {
        if !state.ui.overrides.is_patched("grid") {
            let config = to_effect_config(patch);
            state
                .gpu
                .effects_renderer
                .apply_effect_patch("grid", &config);
            state.ui.overrides.set_patched("grid");
            state.render.dirty = true;
        }
    } else if state.ui.overrides.is_patched("grid") {
        // Restore grid from base theme
        let theme = state.gpu.effect_pipeline.theme();
        if let Some(ref grid) = theme.grid {
            let config = to_effect_config(grid);
            state
                .gpu
                .effects_renderer
                .apply_effect_patch("grid", &config);
        }
        state.ui.overrides.clear_patched("grid");
        log::debug!("Restored grid to base theme");
    }

    // Apply rain patch from active overrides (if any)
    if let Some(patch) = state.ui.overrides.get_rain_patch() {
        if !state.ui.overrides.is_patched("rain") {
            let config = to_effect_config(patch);
            state
                .gpu
                .effects_renderer
                .apply_effect_patch("rain", &config);
            state.ui.overrides.set_patched("rain");
            state.render.dirty = true;
        }
    } else if state.ui.overrides.is_patched("rain") {
        // Restore rain from base theme
        let theme = state.gpu.effect_pipeline.theme();
        if let Some(ref rain) = theme.rain {
            let config = to_effect_config(rain);
            state
                .gpu
                .effects_renderer
                .apply_effect_patch("rain", &config);
        }
        state.ui.overrides.clear_patched("rain");
        log::debug!("Restored rain to base theme");
    }

    // Apply matrix patch from active overrides (if any)
    if let Some(patch) = state.ui.overrides.get_matrix_patch() {
        if !state.ui.overrides.is_patched("matrix") {
            let config = to_effect_config(patch);
            state
                .gpu
                .effects_renderer
                .apply_effect_patch("matrix", &config);
            state.ui.overrides.set_patched("matrix");
            state.render.dirty = true;
        }
    } else if state.ui.overrides.is_patched("matrix") {
        // Restore matrix from base theme
        let theme = state.gpu.effect_pipeline.theme();
        if let Some(ref matrix) = theme.matrix {
            let config = to_effect_config(matrix);
            state
                .gpu
                .effects_renderer
                .apply_effect_patch("matrix", &config);
        }
        state.ui.overrides.clear_patched("matrix");
        log::debug!("Restored matrix to base theme");
    }

    // Apply shape patch from active overrides (if any)
    if let Some(patch) = state.ui.overrides.get_shape_patch() {
        if !state.ui.overrides.is_patched("shape") {
            let config = to_effect_config(patch);
            state
                .gpu
                .effects_renderer
                .apply_effect_patch("shape", &config);
            state.ui.overrides.set_patched("shape");
            state.render.dirty = true;
        }
    } else if state.ui.overrides.is_patched("shape") {
        // Restore shape from base theme
        let theme = state.gpu.effect_pipeline.theme();
        if let Some(ref shape) = theme.shape {
            let config = to_effect_config(shape);
            state
                .gpu
                .effects_renderer
                .apply_effect_patch("shape", &config);
        }
        state.ui.overrides.clear_patched("shape");
        log::debug!("Restored shape to base theme");
    }

    // Keep redrawing while overlay indicators are visible (for fade animation)
    if state.ui.zoom_indicator.is_visible()
        || state.ui.copy_indicator.is_visible()
        || state.ui.toast.is_visible()
    {
        state.render.dirty = true;
    }

    // Force re-renders during first 60 frames
    if state.render.frame_count < 60 {
        state.render.dirty = true;
        if let Some(tab_id) = active_tab_id {
            state.content_hashes.insert(tab_id, 0);
        }
    }

    // Skip GPU rendering when window is occluded (minimized, hidden, or fully covered)
    // PTY processing above still runs to keep shells responsive
    if state.render.occluded {
        return;
    }

    // Update text buffer and get cursor/decoration info
    let text_update_start = Instant::now();
    let update_result = if state.render.dirty {
        state.render.dirty = false;
        state.update_text_buffer(shared)
    } else {
        None
    };
    timing.update_us = text_update_start.elapsed().as_micros() as u64;

    // Render backdrop effects to their intermediate texture (if any effects are enabled)
    // This must happen before we create our command encoder since Vello submits its own commands
    // NOTE: Memory stability depends on frame throttling in main.rs (see about_to_wait)
    let effects_render_start = Instant::now();
    let effects_rendered = if state.gpu.effects_renderer.has_enabled_effects() {
        // Ensure Vello renderer is initialized
        shared.ensure_vello_renderer();

        // Render effects to intermediate texture
        state
            .gpu
            .effects_renderer
            .render(
                &shared.device,
                &shared.queue,
                (state.gpu.config.width, state.gpu.config.height),
            )
            .is_some()
    } else {
        false
    };
    timing.effects_us += effects_render_start.elapsed().as_micros() as u64;

    // Render
    let frame = match state.gpu.surface.get_current_texture() {
        Ok(f) => f,
        Err(e) => {
            log::warn!("Failed to get surface texture: {:?}", e);
            return;
        }
    };
    let frame_view = frame.texture.create_view(&Default::default());

    // Determine render target: CRT intermediate texture if enabled, otherwise surface directly
    let crt_enabled = state.gpu.crt_pipeline.is_enabled();
    // Take ownership of CRT view to avoid borrow conflicts
    let crt_view_clone = state.gpu.crt_texture.as_ref().map(|pooled| {
        // Create a new view from the texture each frame (cheap operation)
        pooled.texture().create_view(&Default::default())
    });
    let render_target: &wgpu::TextureView = if crt_enabled {
        crt_view_clone.as_ref().unwrap_or(&frame_view)
    } else {
        &frame_view
    };

    let render_start = Instant::now();
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
                view: render_target,
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

    // Pass 1.25: Composite backdrop effects (grid, etc.) if rendered
    if effects_rendered {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Backdrop Effects Composite Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: render_target,
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

        state.gpu.effects_renderer.composite(&mut pass);
    }

    // Pass 1.3: Render sprite animation (if configured, uses raw wgpu to bypass vello memory issues)
    if let Some(ref mut sprite_state) = state.gpu.sprite_state {
        let width = state.gpu.config.width as f32;
        let height = state.gpu.config.height as f32;

        // Log once to confirm sprite is rendering
        static LOGGED_SPRITE: std::sync::atomic::AtomicBool =
            std::sync::atomic::AtomicBool::new(false);
        if !LOGGED_SPRITE.swap(true, std::sync::atomic::Ordering::Relaxed) {
            log::info!("Rendering sprite: {}x{}", width, height);
        }

        // Apply sprite patch from active overrides (if any)
        if let Some(patch) = state.ui.overrides.get_sprite_patch() {
            if !state.ui.overrides.is_patched("sprite") {
                // Use apply_patch_with_device if path change is needed
                if crt_renderer::SpriteAnimationState::needs_device_for_patch(patch) {
                    sprite_state.apply_patch_with_device(patch, &shared.device, &shared.queue);
                } else {
                    sprite_state.apply_patch(patch);
                }
                state.ui.overrides.set_patched("sprite");
            }
        } else if state.ui.overrides.is_patched("sprite") {
            // No active override, restore original values (with device for texture restore)
            sprite_state.restore_with_device(&shared.device, &shared.queue);
            state.ui.overrides.clear_patched("sprite");
        }

        // Update animation state
        sprite_state.update(dt, width, height);

        // Render sprite
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Sprite Animation Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: render_target,
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

        sprite_state.render(&mut pass, &shared.queue, width, height);

        // Keep redrawing while sprite is animated
        state.render.dirty = true;
    }

    // Pass 1.5: Render background image (if configured)
    if let (Some(bg_state), Some(bind_group)) = (
        &mut state.gpu.background_image_state,
        &state.gpu.background_image_bind_group,
    ) {
        // Update animation if this is an animated GIF
        if bg_state.update(&shared.queue) {
            // Animation frame changed, need to redraw
            state.render.dirty = true;
        }

        // Keep redrawing for animations
        if bg_state.image.is_animated() {
            state.render.dirty = true;
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
                view: render_target,
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
            .background_image_pipeline
            .render(&mut pass, bind_group);
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
    if let Some(mut result) = update_result {
        // Use take() to avoid cloning the decorations vector
        state.render.cached.decorations = std::mem::take(&mut result.decorations);
        state.render.cached.cursor = Some(result.cursor);
    }

    // Pass 3: Render cell backgrounds via RectRenderer (before text)
    // Always render from cached decorations so they persist across frames
    {
        let bg_count = state
            .render
            .cached
            .decorations
            .iter()
            .filter(|d| d.kind == DecorationKind::Background)
            .count();
        if bg_count > 0 {
            state.gpu.rect_renderer.clear();
            state.gpu.rect_renderer.update_screen_size(
                &shared.queue,
                state.gpu.config.width as f32,
                state.gpu.config.height as f32,
            );

            // Add background rectangles from cached decorations
            for decoration in &state.render.cached.decorations {
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
                    view: render_target,
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

            state.gpu.rect_renderer.render(
                &shared.queue,
                &mut pass,
                &state.gpu.rect_instance_buffer,
            );
        }
    }

    // Pass 3.5: Render output text directly to frame (flat, no glow)
    // This is all terminal text EXCEPT the cursor line
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Output Text Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: render_target,
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

        state.gpu.output_grid_renderer.render(
            &shared.queue,
            &mut pass,
            &state.gpu.output_grid_instance_buffer,
        );
    }

    // Pass 4: Render cursor line text to intermediate texture (for glow effect)
    {
        // Clear the text texture first
        let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Clear Text Texture Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: state.gpu.text_texture.view(),
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        drop(pass);

        // Render terminal text to the texture
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Terminal Text Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: state.gpu.text_texture.view(),
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
            .grid_renderer
            .render(&shared.queue, &mut pass, &state.gpu.grid_instance_buffer);
    }

    // Pass 4.5: Composite text texture onto frame with Gaussian blur glow
    {
        state.gpu.effect_pipeline.composite.update_uniforms(
            &shared.queue,
            state.gpu.config.width as f32,
            state.gpu.config.height as f32,
        );

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Text Composite Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: render_target,
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
            .effect_pipeline
            .composite
            .render(&mut pass, &state.gpu.composite_bind_group);
    }

    // Pass 5: Render cursor, selection, underlines, strikethroughs via RectRenderer
    // (Direct rendering without intermediate texture saves ~8MB)
    {
        // Get selection and display_offset from active terminal (if any)
        let (selection, display_offset) = active_tab_id
            .and_then(|id| state.shells.get(&id))
            .map(|shell| {
                let content = shell.terminal().renderable_content();
                (content.selection, shell.terminal().display_offset() as i32)
            })
            .unwrap_or((None, 0));

        // Collect all overlay rectangles (uses separate renderer to avoid buffer conflicts with tab bar)
        state.gpu.overlay_rect_renderer.clear();
        state.gpu.overlay_rect_renderer.update_screen_size(
            &shared.queue,
            state.gpu.config.width as f32,
            state.gpu.config.height as f32,
        );

        // Add selection rectangles
        if let Some(selection) = selection {
            selection::render_selection_rects(state, &selection, display_offset);
        }

        // Add underlines and strikethroughs from cached decorations
        for decoration in &state.render.cached.decorations {
            match decoration.kind {
                DecorationKind::Background => {} // Already rendered in Pass 3
                DecorationKind::Underline => {
                    // Underline: thin rect near bottom of cell
                    let underline_y = decoration.y + decoration.cell_height - 3.0;
                    state.gpu.overlay_rect_renderer.push_rect(
                        decoration.x,
                        underline_y,
                        decoration.cell_width,
                        1.5,
                        decoration.color,
                    );
                }
                DecorationKind::Strikethrough => {
                    // Strikethrough: positioned at center of x-height using font metrics
                    let strike_y = decoration.y + state.gpu.glyph_cache.strikethrough_offset();
                    state.gpu.overlay_rect_renderer.push_rect(
                        decoration.x,
                        strike_y,
                        decoration.cell_width,
                        1.5,
                        decoration.color,
                    );
                }
            }
        }

        // Add cursor (if visible after blink check AND terminal hasn't hidden it)
        if state.gpu.terminal_vello.cursor_visible()
            && let Some(cursor) = &state.render.cached.cursor
        {
            // Only render if terminal hasn't hidden the cursor (e.g., TUI apps hide it)
            if cursor.visible {
                // Check for active theme override cursor color first
                let cursor_color =
                    if let Some(override_color) = state.ui.overrides.get_cursor_color() {
                        log::debug!(
                            "Using override cursor color: rgba({}, {}, {}, {})",
                            override_color.r,
                            override_color.g,
                            override_color.b,
                            override_color.a
                        );
                        [
                            override_color.r,
                            override_color.g,
                            override_color.b,
                            override_color.a,
                        ]
                    } else {
                        state.gpu.terminal_vello.cursor_color()
                    };

                // Use configured cursor shape as default, but allow terminal/apps to override
                // via escape sequences (DECSCUSR). If the terminal explicitly requests a
                // non-Block shape, use that; otherwise use the user's configured preference.
                // Theme overrides take highest priority.
                let configured_shape = state.gpu.terminal_vello.cursor_shape();
                let terminal_shape = cursor.shape;
                let override_shape = state.ui.overrides.get_cursor_shape();

                // Determine effective cursor shape:
                // - Theme override takes priority (for event effects)
                // - If terminal explicitly set non-Block shape, use it
                // - If terminal shape is Hidden, honor that
                // - Otherwise use user's configured shape
                let cursor_rect = if terminal_shape == crt_core::CursorShape::Hidden {
                    None
                } else if let Some(override_shape) = override_shape {
                    // Theme override takes priority
                    match override_shape {
                        crt_theme::CursorShape::Block => {
                            Some((cursor.x, cursor.y, cursor.cell_width, cursor.cell_height))
                        }
                        crt_theme::CursorShape::Bar => {
                            Some((cursor.x, cursor.y, 2.0, cursor.cell_height))
                        }
                        crt_theme::CursorShape::Underline => Some((
                            cursor.x,
                            cursor.y + cursor.cell_height - 2.0,
                            cursor.cell_width,
                            2.0,
                        )),
                    }
                } else if terminal_shape != crt_core::CursorShape::Block {
                    // Terminal explicitly requested a non-default shape
                    match terminal_shape {
                        crt_core::CursorShape::Beam => {
                            Some((cursor.x, cursor.y, 2.0, cursor.cell_height))
                        }
                        crt_core::CursorShape::Underline => Some((
                            cursor.x,
                            cursor.y + cursor.cell_height - 2.0,
                            cursor.cell_width,
                            2.0,
                        )),
                        crt_core::CursorShape::HollowBlock => {
                            // Hollow block renders as outline only (handled specially below)
                            Some((cursor.x, cursor.y, cursor.cell_width, cursor.cell_height))
                        }
                        _ => Some((cursor.x, cursor.y, cursor.cell_width, cursor.cell_height)),
                    }
                } else {
                    // Use user's configured shape
                    match configured_shape {
                        crt_renderer::CursorShape::Block => {
                            Some((cursor.x, cursor.y, cursor.cell_width, cursor.cell_height))
                        }
                        crt_renderer::CursorShape::Bar => {
                            Some((cursor.x, cursor.y, 2.0, cursor.cell_height))
                        }
                        crt_renderer::CursorShape::Underline => Some((
                            cursor.x,
                            cursor.y + cursor.cell_height - 2.0,
                            cursor.cell_width,
                            2.0,
                        )),
                    }
                };

                if let Some((rect_x, rect_y, rect_w, rect_h)) = cursor_rect {
                    log::debug!(
                        "Cursor rect: x={}, y={}, w={}, h={}, color={:?}",
                        rect_x,
                        rect_y,
                        rect_w,
                        rect_h,
                        cursor_color
                    );
                    // Render cursor glow effect (layered rectangles with decreasing opacity)
                    if let Some((glow_color, radius, intensity)) =
                        state.gpu.terminal_vello.cursor_glow()
                    {
                        let layers = 5;
                        for i in (1..=layers).rev() {
                            let layer_factor = i as f32 / layers as f32;
                            let expand = radius * layer_factor;
                            let alpha = glow_color[3] * intensity * (1.0 - layer_factor) * 0.3;

                            state.gpu.overlay_rect_renderer.push_rect(
                                rect_x - expand,
                                rect_y - expand,
                                rect_w + expand * 2.0,
                                rect_h + expand * 2.0,
                                [glow_color[0], glow_color[1], glow_color[2], alpha],
                            );
                        }
                    }

                    // Render cursor
                    let is_hollow = terminal_shape == crt_core::CursorShape::HollowBlock;
                    if is_hollow {
                        // Hollow block: render as 4 edge rectangles (outline)
                        let border = 2.0;
                        // Top edge
                        state.gpu.overlay_rect_renderer.push_rect(
                            rect_x,
                            rect_y,
                            rect_w,
                            border,
                            cursor_color,
                        );
                        // Bottom edge
                        state.gpu.overlay_rect_renderer.push_rect(
                            rect_x,
                            rect_y + rect_h - border,
                            rect_w,
                            border,
                            cursor_color,
                        );
                        // Left edge (excluding corners already covered)
                        state.gpu.overlay_rect_renderer.push_rect(
                            rect_x,
                            rect_y + border,
                            border,
                            rect_h - border * 2.0,
                            cursor_color,
                        );
                        // Right edge (excluding corners already covered)
                        state.gpu.overlay_rect_renderer.push_rect(
                            rect_x + rect_w - border,
                            rect_y + border,
                            border,
                            rect_h - border * 2.0,
                            cursor_color,
                        );
                    } else {
                        // Solid cursor (block, bar, underline)
                        state.gpu.overlay_rect_renderer.push_rect(
                            rect_x,
                            rect_y,
                            rect_w,
                            rect_h,
                            cursor_color,
                        );
                    }
                }
            }
        }

        // Render all overlay rects directly to frame
        if state.gpu.overlay_rect_renderer.instance_count() > 0 {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Overlay Rect Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: render_target,
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

            state.gpu.overlay_rect_renderer.render(
                &shared.queue,
                &mut pass,
                &state.gpu.overlay_rect_instance_buffer,
            );
        }
    }

    // Pass 6: Render tab bar shapes via RectRenderer (no Vello needed)
    {
        // Recalculate layout if dirty
        state.gpu.tab_bar.prepare(&shared.device, &shared.queue);

        // Render tab bar shapes directly via RectRenderer
        state.gpu.rect_renderer.clear();
        state.gpu.rect_renderer.update_screen_size(
            &shared.queue,
            state.gpu.config.width as f32,
            state.gpu.config.height as f32,
        );
        state
            .gpu
            .tab_bar
            .render_shapes_to_rects(&mut state.gpu.rect_renderer);

        // Add focus ring around active tab (accessibility indicator)
        if let Some((tab_x, tab_y, tab_w, tab_h)) = state.gpu.tab_bar.active_tab_rect() {
            let s = state.scale_factor;
            let ui_style = &state.gpu.effect_pipeline.theme().ui;
            let focus_thickness = ui_style.focus.ring_thickness * s;
            let focus_color = ui_style.focus.ring_color.to_array();

            // Draw focus ring as 4 rectangles (top, bottom, left, right)
            // Top edge
            state
                .gpu
                .rect_renderer
                .push_rect(tab_x, tab_y, tab_w, focus_thickness, focus_color);
            // Bottom edge
            state.gpu.rect_renderer.push_rect(
                tab_x,
                tab_y + tab_h - focus_thickness,
                tab_w,
                focus_thickness,
                focus_color,
            );
            // Left edge
            state
                .gpu
                .rect_renderer
                .push_rect(tab_x, tab_y, focus_thickness, tab_h, focus_color);
            // Right edge
            state.gpu.rect_renderer.push_rect(
                tab_x + tab_w - focus_thickness,
                tab_y,
                focus_thickness,
                tab_h,
                focus_color,
            );
        }

        if state.gpu.rect_renderer.instance_count() > 0 {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Tab Bar Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: render_target,
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

            state.gpu.rect_renderer.render(
                &shared.queue,
                &mut pass,
                &state.gpu.rect_instance_buffer,
            );
        }
    }

    // Pass 7: Render tab title text with glow
    render_tab_titles(state, shared, &mut encoder, render_target);

    // Pass 8: Render search bar overlay (if search is active)
    if state.ui.search.active {
        dialogs::render_search_bar(state, shared, &mut encoder, render_target);
    }

    // Pass 8.5: Render window rename bar overlay (if rename is active)
    if state.ui.window_rename.active {
        dialogs::render_window_rename(state, shared, &mut encoder, render_target);
    }

    // Pass 9: Render bell flash overlay (if active via CSS theme)
    if let Some((color, intensity)) = state.ui.overrides.get_effective_flash()
        && intensity > 0.0
    {
        overlays::render_bell_flash(state, shared, &mut encoder, render_target, color, intensity);
    }

    // Pass 10: Render context menu (if visible)
    if state.ui.context_menu.visible {
        context_menu::render(state, shared, &mut encoder, render_target);
    }

    // Pass 11: Render zoom indicator (if recently changed)
    if state.ui.zoom_indicator.is_visible() {
        overlays::render_zoom_indicator(state, shared, &mut encoder, render_target);
    }

    // Pass 12: Render copy indicator (brief "Copied!" feedback)
    if state.ui.copy_indicator.is_visible() {
        overlays::render_copy_indicator(state, shared, &mut encoder, render_target);
    }

    // Pass 13: Render toast notifications (errors, warnings)
    if state.ui.toast.is_visible() {
        overlays::render_toast(state, shared, &mut encoder, render_target);
    }

    // Final Pass: Apply CRT post-processing (if enabled)
    if crt_enabled {
        if let Some(bind_group) = &state.gpu.crt_bind_group {
            // Update CRT uniforms
            state.gpu.crt_pipeline.update_uniforms(
                &shared.queue,
                state.gpu.config.width as f32,
                state.gpu.config.height as f32,
            );

            // Render from CRT intermediate texture to actual frame
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("CRT Post-Process Pass"),
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

            state.gpu.crt_pipeline.render(&mut pass, bind_group);
        }

        // Keep redrawing for CRT flicker effect
        if state.gpu.crt_pipeline.is_enabled() {
            state.render.dirty = true;
        }
    }

    timing.render_us = render_start.elapsed().as_micros() as u64;
    shared.queue.submit(std::iter::once(encoder.finish()));

    let present_start = Instant::now();
    frame.present();
    timing.present_us = present_start.elapsed().as_micros() as u64;

    // Record frame timing for profiling
    timing.total_us = frame_start.elapsed().as_micros() as u64;
    profiling::record_frame(timing);

    // Record grid snapshot for debugging (rate limited internally)
    if profiling::is_enabled()
        && let Some(tab_id) = state.gpu.tab_bar.active_tab_id()
        && let Some(shell) = state.shells.get(&tab_id)
    {
        let terminal = shell.terminal();
        let cursor = terminal.cursor();
        let size = terminal.size();

        // Get visible lines content (only lines with index >= 0, i.e. visible area)
        let all_lines = terminal.all_lines_text();
        let visible_content: Vec<String> = all_lines
            .into_iter()
            .filter(|(idx, _)| *idx >= 0)
            .map(|(_, text)| text)
            .collect();

        let cursor_shape_str = match cursor.shape {
            crt_core::CursorShape::Block => "Block",
            crt_core::CursorShape::Beam => "Beam",
            crt_core::CursorShape::Underline => "Underline",
            crt_core::CursorShape::HollowBlock => "HollowBlock",
            crt_core::CursorShape::Hidden => "Hidden",
        };
        let mode_visible = terminal.cursor_mode_visible();
        let shape_str = format!(
            "{} (mode:{})",
            cursor_shape_str,
            if mode_visible { "show" } else { "hide" }
        );
        let snapshot = GridSnapshot {
            columns: size.columns,
            lines: size.lines,
            cursor_col: cursor.point.column.0,
            cursor_line: cursor.point.line.0,
            cursor_visible: mode_visible,
            cursor_shape: shape_str,
            display_offset: terminal.display_offset(),
            history_size: terminal.history_size(),
            visible_content,
        };
        profiling::record_grid_snapshot(snapshot);
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

    // Pre-allocate glyph buffer to avoid per-loop allocations
    // Typical tab title is ~30 chars max, so 64 is plenty
    let mut glyph_buffer: Vec<crt_renderer::PositionedGlyph> = Vec::with_capacity(64);

    // First pass: render glow layers for active tabs
    if let Some((radius, glow_color)) = active_shadow {
        // Tighter glow offsets for a subtle halo effect
        let offsets = [
            (-0.75, -0.75),
            (0.75, -0.75),
            (-0.75, 0.75),
            (0.75, 0.75),
            (-1.0, 0.0),
            (1.0, 0.0),
            (0.0, -1.0),
            (0.0, 1.0),
            (-0.5, 0.0),
            (0.5, 0.0),
            (0.0, -0.5),
            (0.0, 0.5),
        ];

        let glow_alpha = (glow_color[3] * (radius / 25.0).min(1.0)).min(0.4);
        let glow_render_color = [glow_color[0], glow_color[1], glow_color[2], glow_alpha];

        for (x, y, title, is_active, _is_editing) in &tab_labels {
            if *is_active {
                for (ox, oy) in &offsets {
                    glyph_buffer.clear();
                    let mut char_x = *x + ox;
                    for c in title.chars() {
                        if let Some(glyph) =
                            state.gpu.tab_glyph_cache.position_char(c, char_x, *y + oy)
                        {
                            glyph_buffer.push(glyph);
                        }
                        char_x += state.gpu.tab_glyph_cache.cell_width();
                    }
                    state
                        .gpu
                        .tab_title_renderer
                        .push_glyphs(&glyph_buffer, glow_render_color);
                }
            }
        }
    }

    // Second pass: render actual text on top
    for (x, y, title, is_active, is_editing) in tab_labels {
        glyph_buffer.clear();
        let mut char_x = x;
        for c in title.chars() {
            if let Some(glyph) = state.gpu.tab_glyph_cache.position_char(c, char_x, y) {
                glyph_buffer.push(glyph);
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
        state
            .gpu
            .tab_title_renderer
            .push_glyphs(&glyph_buffer, text_color);
    }

    // Render close button 'x' characters
    let close_positions = state.gpu.tab_bar.get_close_button_labels();
    for (x, y) in close_positions {
        if let Some(glyph) = state.gpu.tab_glyph_cache.position_char('x', x, y) {
            state
                .gpu
                .tab_title_renderer
                .push_glyphs(&[glyph], inactive_color);
        }
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

    state.gpu.tab_title_renderer.render(
        &shared.queue,
        &mut pass,
        &state.gpu.tab_title_instance_buffer,
    );
}
