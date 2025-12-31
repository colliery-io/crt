//! Rendering logic
//!
//! Multi-pass rendering pipeline for terminal content and effects.

use std::time::Instant;

use crate::gpu::SharedGpuState;
use crate::profiling::{self, FrameTiming, GridSnapshot};
use crate::window::{ContextMenuItem, DecorationKind, OverrideEventType, WindowState};
use crt_core::{SelectionRange, ShellEvent};
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
            render_selection_rects(state, &selection, display_offset);
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
        render_search_bar(state, shared, &mut encoder, render_target);
    }

    // Pass 8.5: Render window rename bar overlay (if rename is active)
    if state.ui.window_rename.active {
        render_window_rename(state, shared, &mut encoder, render_target);
    }

    // Pass 9: Render bell flash overlay (if active via CSS theme)
    if let Some((color, intensity)) = state.ui.overrides.get_effective_flash()
        && intensity > 0.0
    {
        render_bell_flash(state, shared, &mut encoder, render_target, color, intensity);
    }

    // Pass 10: Render context menu (if visible)
    if state.ui.context_menu.visible {
        render_context_menu(state, shared, &mut encoder, render_target);
    }

    // Pass 11: Render zoom indicator (if recently changed)
    if state.ui.zoom_indicator.is_visible() {
        render_zoom_indicator(state, shared, &mut encoder, render_target);
    }

    // Pass 12: Render copy indicator (brief "Copied!" feedback)
    if state.ui.copy_indicator.is_visible() {
        render_copy_indicator(state, shared, &mut encoder, render_target);
    }

    // Pass 13: Render toast notifications (errors, warnings)
    if state.ui.toast.is_visible() {
        render_toast(state, shared, &mut encoder, render_target);
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

/// Render selection rectangles via RectRenderer (direct, no intermediate texture)
/// Selection coordinates are in grid space (negative = scrollback history, 0+ = visible screen)
/// display_offset converts grid coordinates to viewport coordinates for rendering
fn render_selection_rects(
    state: &mut WindowState,
    selection: &SelectionRange,
    display_offset: i32,
) {
    let cell_width = state.gpu.glyph_cache.cell_width();
    let line_height = state.gpu.glyph_cache.line_height();
    let (offset_x, offset_y) = state.gpu.tab_bar.content_offset();
    let padding = 10.0 * state.scale_factor;
    let screen_lines = state.rows as i32;

    // Selection highlight color (semi-transparent blue)
    let selection_color = [0.3, 0.4, 0.6, 0.5];

    // Selection coordinates are in grid space, convert to viewport space for rendering
    // viewport_line = grid_line + display_offset
    let start_grid_line = selection.start.line.0;
    let end_grid_line = selection.end.line.0;
    let start_col = selection.start.column.0;
    let end_col = selection.end.column.0;

    // Convert grid lines to viewport lines
    let start_viewport_line = start_grid_line + display_offset;
    let end_viewport_line = end_grid_line + display_offset;

    // Clamp to visible viewport (0 to screen_lines-1)
    let visible_start = start_viewport_line.max(0);
    let visible_end = end_viewport_line.min(screen_lines - 1);

    // If selection is entirely outside visible area, skip rendering
    if visible_start > screen_lines - 1 || visible_end < 0 {
        return;
    }

    if selection.is_block {
        // Block selection: rectangle from start to end
        let min_col = start_col.min(end_col);
        let max_col = start_col.max(end_col);

        for viewport_line in visible_start..=visible_end {
            let y = offset_y + padding + (viewport_line as f32 * line_height);
            let x = offset_x + padding + (min_col as f32 * cell_width);
            let num_cells = max_col - min_col + 1;
            let width = num_cells as f32 * cell_width;
            state
                .gpu
                .overlay_rect_renderer
                .push_rect(x, y, width, line_height, selection_color);
        }
    } else {
        // Normal selection: spans from start point to end point
        for viewport_line in visible_start..=visible_end {
            let y = offset_y + padding + (viewport_line as f32 * line_height);

            // Convert back to grid line to compare with selection boundaries
            let grid_line = viewport_line - display_offset;

            let (line_start_col, line_end_col) = if start_grid_line == end_grid_line {
                // Single line selection - normalize columns for right-to-left selection
                (start_col.min(end_col), start_col.max(end_col))
            } else if grid_line == start_grid_line {
                // First line: from start column to end of line
                (start_col, 999)
            } else if grid_line == end_grid_line {
                // Last line: from start of line to end column
                (0, end_col)
            } else {
                // Middle line: full line
                (0, 999)
            };

            let x = offset_x + padding + (line_start_col as f32 * cell_width);
            let num_cells = (line_end_col - line_start_col + 1).min(500);
            let width = num_cells as f32 * cell_width;
            state
                .gpu
                .overlay_rect_renderer
                .push_rect(x, y, width, line_height, selection_color);
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

/// Render search bar overlay
fn render_search_bar(
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

/// Render bell flash overlay (theme-driven color and intensity)
fn render_bell_flash(
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

/// Render context menu overlay
fn render_context_menu(
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
    let item_height = 24.0 * scale; // Slightly smaller to fit more items
    let menu_width = 220.0 * scale; // Wider to accommodate theme names

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

    // For vertical positioning: if menu is taller than screen, start at top
    // Otherwise, try to fit it by moving up if needed
    if menu_height > screen_height - 8.0 {
        // Menu is taller than screen - start at top, it will be clipped at bottom
        menu_y = 4.0;
    } else if menu_y + menu_height > screen_height - 4.0 {
        // Menu would go off bottom - move it up
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
    // Top border
    state
        .gpu
        .rect_renderer
        .push_rect(menu_x, menu_y, menu_width, border_thickness, border_color);
    // Bottom border
    state.gpu.rect_renderer.push_rect(
        menu_x,
        menu_y + menu_height - border_thickness,
        menu_width,
        border_thickness,
        border_color,
    );
    // Left border
    state
        .gpu
        .rect_renderer
        .push_rect(menu_x, menu_y, border_thickness, menu_height, border_color);
    // Right border
    state.gpu.rect_renderer.push_rect(
        menu_x + menu_width - border_thickness,
        menu_y,
        border_thickness,
        menu_height,
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

    // Hover highlight (mouse hover) - only for selectable items
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

        // Draw focus ring as 4 rectangles (top, bottom, left, right)
        // Top edge
        state.gpu.rect_renderer.push_rect(
            menu_x + inset,
            focus_y,
            menu_width - (inset * 2.0),
            focus_border,
            focus_color,
        );
        // Bottom edge
        state.gpu.rect_renderer.push_rect(
            menu_x + inset,
            focus_y + item_height - focus_border,
            menu_width - (inset * 2.0),
            focus_border,
            focus_color,
        );
        // Left edge
        state.gpu.rect_renderer.push_rect(
            menu_x + inset,
            focus_y,
            focus_border,
            item_height,
            focus_color,
        );
        // Right edge
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

    for (idx, item) in items.iter().enumerate() {
        if item.is_separator() {
            continue; // Don't render text for separators
        }

        let item_y = item_y_positions[idx] + text_offset_y;

        // Check if this is a theme item with checkmark
        let (label, show_checkmark) = match item {
            ContextMenuItem::Theme(name) => {
                let is_current = state.ui.context_menu.is_current_theme(name);
                (item.label(), is_current)
            }
            _ => (item.label(), false),
        };

        // Render checkmark for current theme
        let label_start_x = if show_checkmark {
            let checkmark = "\u{2713}"; // Unicode checkmark
            let mut char_x = menu_x + padding_x;
            for c in checkmark.chars() {
                if let Some(glyph) = state.gpu.tab_glyph_cache.position_char(c, char_x, item_y) {
                    state
                        .gpu
                        .tab_title_renderer
                        .push_glyphs(&[glyph], text_color);
                }
                char_x += state.gpu.tab_glyph_cache.cell_width();
            }
            menu_x + padding_x + (2.0 * state.gpu.tab_glyph_cache.cell_width())
        } else if matches!(item, ContextMenuItem::Theme(_)) {
            // Indent theme items that don't have checkmark to align with those that do
            menu_x + padding_x + (2.0 * state.gpu.tab_glyph_cache.cell_width())
        } else {
            menu_x + padding_x
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

        // Render shortcut (right-aligned) - only for non-theme items
        let shortcut = item.shortcut();
        if !shortcut.is_empty() {
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
            state
                .gpu
                .tab_title_renderer
                .push_glyphs(&shortcut_glyphs, shortcut_color);
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

/// Render zoom indicator overlay (centered pill showing zoom percentage)
fn render_zoom_indicator(
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

fn render_copy_indicator(
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

/// Render window rename input bar overlay
fn render_window_rename(
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

fn render_toast(
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
        crate::window::ToastType::Error => (
            [0.6, 0.1, 0.1, 0.95 * opacity], // Dark red background
            [1.0, 1.0, 1.0, opacity],        // White text
        ),
        crate::window::ToastType::Warning => (
            [0.6, 0.4, 0.1, 0.95 * opacity], // Dark orange background
            [1.0, 1.0, 1.0, opacity],        // White text
        ),
        crate::window::ToastType::Info => (
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
