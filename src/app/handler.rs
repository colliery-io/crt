//! ApplicationHandler implementation for winit event loop.
//!
//! Handles window events, keyboard/mouse input, and frame timing.

use std::time::Instant;

use crate::input::{
    drag::{self, TabDragState},
    handle_cursor_moved, handle_keyboard_input, handle_mouse_input, handle_mouse_wheel,
    handle_resize, handle_tab_click, KeyboardAction,
};
use super::initialization::{DetachPayload, MergePayload};
use crate::render::render_frame;
use crate::window;
use crt_core::SpawnOptions;
use winit::{
    application::ApplicationHandler,
    event::{ElementState, WindowEvent},
    event_loop::ActiveEventLoop,
    window::WindowId,
};

use super::initialization::handle_scale_factor_change;
use super::App;
use crate::config::Config;

#[cfg(target_os = "macos")]
use crate::menu::{build_menu_bar, menu_id_to_action, set_windows_menu};

#[cfg(target_os = "macos")]
use muda::MenuEvent;

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.windows.is_empty() {
            self.create_window(event_loop);

            #[cfg(target_os = "macos")]
            if self.menu.is_none() {
                let theme_names = self.theme_registry.list_themes();
                let current_theme = self.theme_registry.default_theme_name();
                let (menu, ids, window_submenu) = build_menu_bar(&theme_names, current_theme);
                menu.init_for_nsapp();
                // Register the Window menu with macOS so it automatically lists windows
                set_windows_menu(&window_submenu);
                self.menu = Some(menu);
                self.menu_ids = Some(ids);
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        // Pre-collect window screen rects for drag target resolution (avoids borrow conflicts)
        // Only compute when a drag is active to avoid unnecessary work per event.
        let drag_window_rects: Option<Vec<drag::WindowScreenRect>> =
            if self.drag_state.as_ref().is_some_and(|d| d.drag_active) {
                Some(
                    self.windows
                        .iter()
                        .filter_map(|(wid, ws)| {
                            let origin = ws.window.inner_position().ok()?;
                            Some(drag::WindowScreenRect {
                                window_id: *wid,
                                origin,
                                size: ws.window.inner_size(),
                                tab_bar_height: ws.gpu.tab_bar.height() * ws.scale_factor,
                                tab_rects: ws.gpu.tab_bar.tab_rects().to_vec(),
                            })
                        })
                        .collect(),
                )
            } else {
                None
            };

        let Some(state) = self.windows.get_mut(&id) else {
            return;
        };

        match event {
            WindowEvent::CloseRequested => {
                self.windows.remove(&id);
                if self.focused_window == Some(id) {
                    self.focused_window = self.windows.keys().next().copied();
                }
            }

            WindowEvent::Focused(focused) => {
                state.render.focused = focused;

                // Trigger theme override for focus events
                if focused {
                    self.focused_window = Some(id);
                    // Apply on_focus override if theme defines it
                    if let Some(ref override_props) = self.theme.on_focus {
                        state.ui.overrides.add(
                            window::OverrideEventType::FocusGained,
                            override_props.clone(),
                        );
                        log::debug!("Focus gained - applied theme override");
                    }
                    // Redraw immediately when gaining focus to resume effects
                    state.window.request_redraw();
                } else {
                    // Apply on_blur override if theme defines it
                    if let Some(ref override_props) = self.theme.on_blur {
                        state
                            .ui
                            .overrides
                            .add(window::OverrideEventType::FocusLost, override_props.clone());
                        log::debug!("Focus lost - applied theme override");
                    }
                }
            }

            WindowEvent::Occluded(occluded) => {
                if let Some(state) = self.windows.get_mut(&id) {
                    state.render.occluded = occluded;
                    log::debug!("Window {:?} occluded: {}", id, occluded);
                }
            }

            WindowEvent::ModifiersChanged(m) => {
                self.modifiers = m;
            }

            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                // Cancel active drag on Escape
                if event.logical_key == winit::keyboard::Key::Named(winit::keyboard::NamedKey::Escape) {
                    if self.drag_state.take().is_some() {
                        log::debug!("Tab drag cancelled via Escape");
                        self.drag_overlay = None; // Close overlay
                        if let Some(state) = self.windows.get_mut(&id) {
                            state.window.set_cursor(winit::window::CursorIcon::Default);
                            state.window.request_redraw();
                        }
                        return;
                    }
                }

                let Some(state) = self.windows.get_mut(&id) else {
                    return;
                };

                // Delegate to keyboard handler
                let action = handle_keyboard_input(
                    state,
                    &event.logical_key,
                    event.text.as_ref().map(|s| s.as_str()),
                    &self.modifiers,
                );

                // Handle actions that require App-level access
                match action {
                    KeyboardAction::Quit => {
                        event_loop.exit();
                    }
                    KeyboardAction::CloseWindow => {
                        self.windows.remove(&id);
                        if self.focused_window == Some(id) {
                            self.focused_window = self.windows.keys().next().copied();
                        }
                    }
                    KeyboardAction::NewWindow => {
                        self.pending_new_window = true;
                    }
                    KeyboardAction::NewTab => {
                        // Extract config values before borrowing state mutably
                        let shell_program = self.config.shell.program.clone();
                        let semantic_prompts = self.config.shell.semantic_prompts;
                        let shell_assets_dir = Config::shell_assets_dir();
                        let new_tab_id = self.next_tab_id();

                        if let Some(state) = self.windows.get_mut(&id) {
                            let cwd = state.active_shell_cwd();
                            let tab_num = state.gpu.tab_bar.tab_count() + 1;
                            let tab_id = new_tab_id;
                            state.gpu.tab_bar.add_tab(tab_id, format!("Terminal {}", tab_num));
                            state
                                .gpu
                                .tab_bar
                                .select_tab_index(state.gpu.tab_bar.tab_count() - 1);
                            let spawn_options = SpawnOptions {
                                shell: shell_program,
                                cwd,
                                semantic_prompts,
                                shell_assets_dir,
                            };
                            state.create_shell_for_tab(tab_id, spawn_options);
                            state.render.dirty = true;
                            state.window.request_redraw();
                        }
                    }
                    KeyboardAction::Handled
                    | KeyboardAction::NotHandled
                    | KeyboardAction::Scroll(_)
                    | KeyboardAction::CloseTab(_)
                    | KeyboardAction::Copy
                    | KeyboardAction::Paste
                    | KeyboardAction::ToggleSearch
                    | KeyboardAction::SearchNavigate { .. }
                    | KeyboardAction::PrevTab
                    | KeyboardAction::NextTab
                    | KeyboardAction::SelectTab(_) => {
                        // Already handled by keyboard module or no action needed
                    }
                }
            }

            WindowEvent::Resized(size) => {
                let shared = self.shared_gpu.as_ref().unwrap();
                handle_resize(state, shared, size.width, size.height);
            }

            WindowEvent::ScaleFactorChanged {
                scale_factor,
                inner_size_writer: _,
            } => {
                let new_scale = scale_factor as f32;
                let old_scale = state.scale_factor;

                if (new_scale - old_scale).abs() > 0.001 {
                    log::debug!(
                        "Scale factor changed: {} -> {} (window {:?})",
                        old_scale,
                        new_scale,
                        id
                    );

                    let shared = self.shared_gpu.as_ref().unwrap();
                    handle_scale_factor_change(state, shared, &self.config, new_scale);
                }
            }

            WindowEvent::CursorMoved { position, .. } => {
                // Update drag state if a drag is in progress
                if let Some(ref mut drag_state) = self.drag_state {
                    drag_state.current_pos = winit::dpi::PhysicalPosition::new(
                        position.x,
                        position.y,
                    );
                    if !drag_state.drag_active && drag_state.exceeds_threshold() {
                        drag_state.drag_active = true;
                        log::debug!("Tab drag activated for tab {}", drag_state.tab_id);
                        // Set grabbing cursor
                        state.window.set_cursor(winit::window::CursorIcon::Grabbing);
                    }
                    // Resolve drop target across all windows
                    if drag_state.drag_active {
                        if let Some(ref window_rects) = drag_window_rects {
                            let dragged_idx = state
                                .gpu
                                .tab_bar
                                .tab_index(drag_state.tab_id)
                                .unwrap_or(0);
                            let source_id = drag_state.source_window_id;

                            // Convert cursor to screen coordinates
                            let cursor_screen =
                                if let Ok(win_pos) = state.window.inner_position() {
                                    winit::dpi::PhysicalPosition::new(
                                        position.x + win_pos.x as f64,
                                        position.y + win_pos.y as f64,
                                    )
                                } else {
                                    drag_state.current_pos
                                };

                            let source_tab_count = state.gpu.tab_bar.tab_count();
                            drag_state.drop_target = drag::resolve_drop_target(
                                cursor_screen,
                                source_id,
                                dragged_idx,
                                source_tab_count,
                                window_rects,
                            );
                        }
                        // Move the overlay window to follow cursor
                        if let Some(ref overlay) = self.drag_overlay {
                            if let Ok(win_pos) = state.window.inner_position() {
                                let screen_x = position.x as i32 + win_pos.x - 75;
                                let screen_y = position.y as i32 + win_pos.y + 15;
                                overlay.set_outer_position(
                                    winit::dpi::PhysicalPosition::new(screen_x, screen_y),
                                );
                            }
                        }

                        state.window.request_redraw();
                    }
                }
                handle_cursor_moved(state, position.x as f32, position.y as f32);
            }

            WindowEvent::MouseInput {
                state: button_state,
                button,
                ..
            } => {
                use winit::event::MouseButton;

                // Handle drag initiation/completion at App level
                let mut handled_by_drag = false;
                if button == MouseButton::Left && button_state == ElementState::Pressed {
                    let (x, y) = state.interaction.cursor_position;
                    if let Some(tab_id) = drag::should_start_drag(
                        &state.gpu.tab_bar,
                        state.ui.context_menu.visible,
                        x,
                        y,
                    ) {
                        // Start potential drag — don't select tab yet
                        self.drag_state = Some(TabDragState::new(
                            tab_id,
                            id,
                            winit::dpi::PhysicalPosition::new(x as f64, y as f64),
                        ));
                        handled_by_drag = true;
                    }
                } else if button == MouseButton::Left && button_state == ElementState::Released {
                    if let Some(drag) = self.drag_state.take() {
                        // Clean up overlay and cursor
                        self.drag_overlay = None;
                        state.window.set_cursor(winit::window::CursorIcon::Default);

                        if drag.drag_active {
                            match drag.drop_target {
                                drag::DragDropTarget::Reorder { insert_index } => {
                                    if let Some(from_idx) =
                                        state.gpu.tab_bar.tab_index(drag.tab_id)
                                    {
                                        if from_idx != insert_index {
                                            state.gpu.tab_bar.move_tab(from_idx, insert_index);
                                            state.render.dirty = true;
                                            state.window.request_redraw();
                                            log::debug!(
                                                "Tab {} reordered: {} -> {}",
                                                drag.tab_id,
                                                from_idx,
                                                insert_index
                                            );
                                        }
                                    }
                                }
                                drag::DragDropTarget::Detach => {
                                    // Extract tab+shell from source window
                                    if let Some(tab) =
                                        state.gpu.tab_bar.remove_tab(drag.tab_id)
                                    {
                                        if let Some(shell) =
                                            state.shells.remove(&drag.tab_id)
                                        {
                                            let content_hash = state
                                                .content_hashes
                                                .remove(&drag.tab_id)
                                                .unwrap_or(0);
                                            state.render.dirty = true;
                                            state.window.request_redraw();

                                            // Convert cursor to screen position for window placement
                                            let screen_pos = state
                                                .window
                                                .inner_position()
                                                .ok()
                                                .map(|wp| {
                                                    winit::dpi::PhysicalPosition::new(
                                                        wp.x + drag.current_pos.x as i32,
                                                        wp.y + drag.current_pos.y as i32,
                                                    )
                                                });

                                            self.pending_detach = Some(DetachPayload {
                                                tab,
                                                shell,
                                                content_hash,
                                                screen_position: screen_pos,
                                            });
                                            // Close source window if it's now empty
                                            if state.gpu.tab_bar.tab_count() == 0 {
                                                self.pending_close_empty =
                                                    Some(drag.source_window_id);
                                            }
                                            log::debug!(
                                                "Tab {} detached from window {:?}",
                                                drag.tab_id,
                                                drag.source_window_id
                                            );
                                        }
                                    }
                                }
                                drag::DragDropTarget::Merge {
                                    target_window_id,
                                    insert_index,
                                } => {
                                    // Extract tab+shell from source window
                                    if let Some(tab) =
                                        state.gpu.tab_bar.remove_tab(drag.tab_id)
                                    {
                                        if let Some(shell) =
                                            state.shells.remove(&drag.tab_id)
                                        {
                                            let content_hash = state
                                                .content_hashes
                                                .remove(&drag.tab_id)
                                                .unwrap_or(0);
                                            state.render.dirty = true;
                                            state.window.request_redraw();

                                            // Close source window if it's now empty
                                            if state.gpu.tab_bar.tab_count() == 0 {
                                                self.pending_close_empty =
                                                    Some(drag.source_window_id);
                                            }
                                            self.pending_merge = Some(MergePayload {
                                                tab,
                                                shell,
                                                content_hash,
                                                target_window_id,
                                                insert_index,
                                            });
                                            log::debug!(
                                                "Tab {} merging into window {:?} at index {}",
                                                drag.tab_id,
                                                target_window_id,
                                                insert_index
                                            );
                                        }
                                    }
                                }
                                drag::DragDropTarget::Pending => {
                                    // Should not happen for active drags
                                }
                            }
                        } else {
                            // Threshold not exceeded — treat as normal click
                            let (x, y) = state.interaction.cursor_position;
                            handle_tab_click(state, x, y, Instant::now());
                        }
                        handled_by_drag = true;
                    }
                }

                if !handled_by_drag {
                    handle_mouse_input(state, button, button_state, &self.modifiers);
                    // Check for pending theme change from context menu
                    if let Some(theme_name) = state.ui.pending_theme.take() {
                        if let Some(theme) =
                            self.theme_registry.get_theme(&theme_name).cloned()
                        {
                            super::apply_theme_to_window(
                                state,
                                self.shared_gpu.as_ref(),
                                &theme_name,
                                &theme,
                            );
                        } else {
                            log::warn!("Theme '{}' not found in registry", theme_name);
                        }
                    }
                }
            }

            WindowEvent::MouseWheel { delta, .. } => {
                handle_mouse_wheel(state, delta);
            }

            WindowEvent::RedrawRequested => {
                // Set drag visual feedback before rendering
                let feedback = self.drag_state.as_ref().and_then(|ds| {
                    if !ds.drag_active {
                        return None;
                    }
                    use crt_renderer::tab_bar::{DragFeedback, DragMode};
                    let mode = match &ds.drop_target {
                        drag::DragDropTarget::Reorder { .. } => DragMode::Reorder,
                        drag::DragDropTarget::Merge { .. } => DragMode::Merge,
                        drag::DragDropTarget::Detach => DragMode::Detach,
                        drag::DragDropTarget::Pending => return None,
                    };
                    let insertion_index = match &ds.drop_target {
                        drag::DragDropTarget::Reorder { insert_index } => Some(*insert_index),
                        drag::DragDropTarget::Merge { insert_index, .. }
                            if ds.source_window_id != id =>
                        {
                            // Show caret on target window, not source
                            None
                        }
                        _ => None,
                    };
                    let ghost_position = if ds.source_window_id == id {
                        Some((ds.current_pos.x as f32, ds.current_pos.y as f32))
                    } else {
                        None
                    };
                    log::debug!(
                        "Drag feedback: mode={:?}, insertion={:?}, ghost={:?}",
                        mode, insertion_index, ghost_position
                    );
                    Some(DragFeedback {
                        dragged_tab_id: ds.tab_id,
                        insertion_index,
                        ghost_position,
                        mode,
                    })
                });
                state.gpu.tab_bar.set_drag_feedback(feedback);

                let shared = self.shared_gpu.as_mut().unwrap();
                render_frame(state, shared);
            }

            _ => {}
        }

    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // Create drag overlay if drag just activated
        if let Some(ref ds) = self.drag_state {
            if ds.drag_active && self.drag_overlay.is_none() {
                // Get tab title from source window
                let title = self
                    .windows
                    .get(&ds.source_window_id)
                    .and_then(|w| w.gpu.tab_bar.get_tab_title(ds.tab_id).map(|s| s.to_string()))
                    .unwrap_or_else(|| "Tab".to_string());
                let x = ds.current_pos.x as i32;
                let y = ds.current_pos.y as i32;
                self.create_drag_overlay(event_loop, &title, x, y);
            }
        }

        // Close empty windows from tab extraction (must happen before creating new ones)
        if let Some(window_id) = self.pending_close_empty.take() {
            log::info!("Closing empty window {:?} after tab extraction", window_id);
            self.close_window(window_id);
        }

        // Execute deferred tab operations (saved from window_event to avoid borrow conflicts)
        if let Some(payload) = self.pending_detach.take() {
            self.create_window_for_detach(event_loop, payload);
        }
        if let Some(payload) = self.pending_merge.take() {
            if let Some(target) = self.windows.get_mut(&payload.target_window_id) {
                let tab_id = payload.tab.id;
                target.gpu.tab_bar.insert_existing_tab(payload.tab, payload.insert_index);
                let mut shell = payload.shell;
                shell.resize(crt_core::Size::new(target.cols, target.rows));
                target.shells.insert(tab_id, shell);
                target.content_hashes.insert(tab_id, payload.content_hash);
                target.gpu.tab_bar.select_tab(tab_id);
                target.render.dirty = true;
                target.content_hashes.insert(tab_id, 0); // Force re-render
                target.window.request_redraw();
                target.window.focus_window();
                self.focused_window = Some(payload.target_window_id);
                log::info!("Tab {} merged into window {:?}", tab_id, payload.target_window_id);
            }
        }

        #[cfg(target_os = "macos")]
        if let Some(ids) = &self.menu_ids
            && let Ok(event) = MenuEvent::receiver().try_recv()
            && let Some(action) = menu_id_to_action(event.id(), ids)
        {
            self.handle_menu_action(action, event_loop);
        }

        // Check for config/theme file changes - collect events first to avoid borrow issues
        let events: Vec<_> = self
            .config_watcher
            .as_mut()
            .map(|w| std::iter::from_fn(|| w.poll()).collect())
            .unwrap_or_default();

        for event in events {
            match event {
                crate::watcher::ConfigEvent::ConfigChanged => self.reload_config(),
                crate::watcher::ConfigEvent::ThemeChanged => self.reload_theme(),
            }
        }

        if self.pending_new_window {
            self.pending_new_window = false;
            self.create_window(event_loop);
        }

        // FRAME THROTTLING - Critical fix for Metal/wgpu memory leak (November 2024)
        //
        // WHY: wgpu/Metal on macOS has a bug where IOAccelerator drawable allocations grow
        // unboundedly when frames are rendered at high rates. With ControlFlow::Poll and
        // continuous request_redraw(), we were seeing 1500+ GPU allocations per second,
        // causing memory to balloon from ~130MB to 4-9GB within minutes.
        //
        // WHAT: Limits redraws to ~60fps by only calling request_redraw() when at least
        // 16.6ms has elapsed since the last frame. This keeps IOAccelerator regions
        // at ~500 instead of 40,000+ and memory stable at ~150-220MB.
        //
        // RE-EVALUATE WHEN:
        // - wgpu updates to 24.x+ (check if Metal backend fixes drawable allocation)
        // - Testing on non-macOS platforms (this may not be needed on Windows/Linux)
        // - If we need variable refresh rate support (would need smarter throttling)
        // - If Apple fixes the IOAccelerator memory management in a future macOS version
        //
        // Related: https://github.com/gfx-rs/wgpu/issues/3292 (Metal memory growth issues)
        const TARGET_FRAME_TIME: std::time::Duration =
            std::time::Duration::from_micros(16666); // ~60fps
        let elapsed = self.last_frame_time.elapsed();

        // Focused window: 60fps for smooth effects
        if elapsed >= TARGET_FRAME_TIME {
            self.last_frame_time = Instant::now();

            if let Some(focused_id) = self.focused_window
                && let Some(state) = self.windows.get(&focused_id)
                && !state.render.occluded
            {
                state.window.request_redraw();
            }
        }

        // Unfocused windows: 10fps for PTY output updates (saves GPU work)
        const UNFOCUSED_FRAME_TIME: std::time::Duration =
            std::time::Duration::from_millis(100);
        let unfocused_elapsed = self.last_unfocused_frame_time.elapsed();

        if unfocused_elapsed >= UNFOCUSED_FRAME_TIME {
            self.last_unfocused_frame_time = Instant::now();

            for (id, state) in self.windows.iter() {
                // Skip focused window (handled above) and occluded windows
                if Some(*id) != self.focused_window && !state.render.occluded {
                    state.window.request_redraw();
                }
            }
        }
    }
}
