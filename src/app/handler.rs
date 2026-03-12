//! ApplicationHandler implementation for winit event loop.
//!
//! Handles window events, keyboard/mouse input, and frame timing.

use std::time::Instant;

use crate::input::{
    handle_cursor_moved, handle_keyboard_input, handle_mouse_input, handle_mouse_wheel,
    handle_resize, KeyboardAction,
};
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

                        if let Some(state) = self.windows.get_mut(&id) {
                            let cwd = state.active_shell_cwd();
                            let tab_num = state.gpu.tab_bar.tab_count() + 1;
                            let tab_id =
                                state.gpu.tab_bar.add_tab(format!("Terminal {}", tab_num));
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
                handle_cursor_moved(state, position.x as f32, position.y as f32);
            }

            WindowEvent::MouseInput {
                state: button_state,
                button,
                ..
            } => {
                handle_mouse_input(state, button, button_state, &self.modifiers);
                // Check for pending theme change from context menu
                if let Some(theme_name) = state.ui.pending_theme.take() {
                    if let Some(theme) = self.theme_registry.get_theme(&theme_name).cloned() {
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

            WindowEvent::MouseWheel { delta, .. } => {
                handle_mouse_wheel(state, delta);
            }

            WindowEvent::RedrawRequested => {
                let shared = self.shared_gpu.as_mut().unwrap();
                render_frame(state, shared);
            }

            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
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
