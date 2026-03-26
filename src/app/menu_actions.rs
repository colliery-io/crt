//! Menu action handling.
//!
//! Processes macOS menu bar actions (new tab, close, theme switching, etc.).

use crate::config::Config;
use crate::input::{get_clipboard_content, get_terminal_selection_text, paste_to_terminal, set_clipboard_content};
use crate::menu::MenuAction;
use crt_core::SpawnOptions;
use winit::event_loop::ActiveEventLoop;
use winit::window::WindowId;

use super::{App, FONT_SCALE_STEP, MAX_FONT_SCALE, MIN_FONT_SCALE};

#[cfg(target_os = "macos")]
impl App {
    pub(crate) fn handle_menu_action(&mut self, action: MenuAction, event_loop: &ActiveEventLoop) {
        match action {
            MenuAction::NewTab => {
                // Extract config values before borrowing state mutably
                let shell_program = self.config.shell.program.clone();
                let semantic_prompts = self.config.shell.semantic_prompts;
                let shell_assets_dir = Config::shell_assets_dir();

                let new_tab_id = self.next_tab_id();
                if let Some(state) = self.focused_window_mut() {
                    // Get current shell's working directory for the new tab
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
            MenuAction::NewWindow => {
                self.pending_new_window = true;
            }
            MenuAction::RenameWindow => {
                if let Some(state) = self.focused_window_mut() {
                    let current_title = state
                        .custom_title
                        .clone()
                        .unwrap_or_else(|| "CRT Terminal".to_string());
                    // Cancel tab editing if active
                    if state.gpu.tab_bar.is_editing() {
                        state.gpu.tab_bar.cancel_editing();
                    }
                    state.ui.window_rename.start(&current_title);
                    state.render.dirty = true;
                    state.window.request_redraw();
                }
            }
            MenuAction::CloseTab => {
                let should_close = if let Some(state) = self.focused_window_mut() {
                    if state.gpu.tab_bar.tab_count() > 1 {
                        if let Some(id) = state.gpu.tab_bar.active_tab_id() {
                            state.gpu.tab_bar.close_tab(id);
                            state.remove_shell_for_tab(id);
                            // Force redraw of new active tab to clear stale cached render state
                            state.force_active_tab_redraw();
                            state.window.request_redraw();
                        }
                        false
                    } else {
                        true
                    }
                } else {
                    false
                };
                if should_close && let Some(id) = self.focused_window {
                    self.close_window(id);
                }
            }
            MenuAction::CloseWindow => {
                if let Some(id) = self.focused_window {
                    self.close_window(id);
                }
            }
            MenuAction::Quit => event_loop.exit(),
            MenuAction::ToggleFullScreen => {
                if let Some(state) = self.focused_window_mut() {
                    let fs = state.window.fullscreen().is_some();
                    state.window.set_fullscreen(if fs {
                        None
                    } else {
                        Some(winit::window::Fullscreen::Borderless(None))
                    });
                }
            }
            MenuAction::IncreaseFontSize => self.adjust_font_scale(FONT_SCALE_STEP),
            MenuAction::DecreaseFontSize => self.adjust_font_scale(-FONT_SCALE_STEP),
            MenuAction::ResetFontSize => {
                // Compute delta to get back to scale 1.0
                if let Some(state) = self
                    .windows
                    .get(&self.focused_window.unwrap_or(WindowId::dummy()))
                {
                    let delta = 1.0 - state.font_scale;
                    if delta.abs() > 0.001 {
                        self.adjust_font_scale(delta);
                    }
                }
            }
            MenuAction::Minimize => {
                if let Some(state) = self.focused_window_mut() {
                    state.window.set_minimized(true);
                }
            }
            MenuAction::NextTab => self.navigate_tab(true),
            MenuAction::PrevTab => self.navigate_tab(false),
            ref a if a.tab_index().is_some() => {
                self.select_tab_index(a.tab_index().unwrap());
            }
            MenuAction::Paste => {
                if let Some(state) = self.focused_window_mut()
                    && let Some(content) = get_clipboard_content()
                {
                    paste_to_terminal(state, &content);
                }
            }
            MenuAction::Copy => {
                if let Some(state) = self.focused_window_mut()
                    && let Some(text) = get_terminal_selection_text(state)
                {
                    set_clipboard_content(&text);
                    state.ui.copy_indicator.trigger();
                }
            }
            MenuAction::Find => {
                if let Some(state) = self.focused_window_mut() {
                    // Toggle search mode
                    state.ui.search.active = !state.ui.search.active;
                    if !state.ui.search.active {
                        // Clear search when closing
                        state.ui.search.query.clear();
                        state.ui.search.matches.clear();
                        state.ui.search.current_match = 0;
                    }
                    state.force_active_tab_redraw();
                    state.window.request_redraw();
                }
            }
            MenuAction::ToggleProfiling => {
                let (enabled, path) = crate::profiling::toggle();
                if enabled {
                    if let Some(p) = path {
                        log::info!("Profiling started: {}", p.display());
                    }
                } else if let Some(p) = path {
                    log::info!("Profiling stopped. Log: {}", p.display());
                }
            }
            MenuAction::SetTheme(ref theme_name) => {
                if let Some(theme) = self.theme_registry.get_theme(theme_name).cloned() {
                    if let Some(window_id) = self.focused_window
                        && let Some(state) = self.windows.get_mut(&window_id)
                    {
                        super::apply_theme_to_window(
                            state,
                            self.shared_gpu.as_ref(),
                            theme_name,
                            &theme,
                        );
                    }
                } else {
                    log::warn!("Theme '{}' not found in registry", theme_name);
                }
            }
            _ => log::info!("{:?} not yet implemented", action),
        }
    }

    pub(crate) fn adjust_font_scale(&mut self, delta: f32) {
        use crt_core::Size;

        let base_font_size = self.config.font.size;
        let focused_id = match self.focused_window {
            Some(id) => id,
            None => return,
        };

        let shared = match self.shared_gpu.as_ref() {
            Some(s) => s,
            None => return,
        };

        let state = match self.windows.get_mut(&focused_id) {
            Some(s) => s,
            None => return,
        };

        let new_scale = (state.font_scale + delta).clamp(MIN_FONT_SCALE, MAX_FONT_SCALE);
        if (new_scale - state.font_scale).abs() > 0.001 {
            state.font_scale = new_scale;

            // Update glyph cache with new font size
            let new_font_size = base_font_size * new_scale * state.scale_factor;
            state
                .gpu
                .glyph_cache
                .set_font_size(&shared.queue, new_font_size);
            state.gpu.glyph_cache.precache_ascii();
            state.gpu.glyph_cache.flush(&shared.queue);

            // Update grid renderers with new glyph cache
            state
                .gpu
                .grid_renderer
                .set_glyph_cache(&shared.device, &state.gpu.glyph_cache);
            state
                .gpu
                .output_grid_renderer
                .set_glyph_cache(&shared.device, &state.gpu.glyph_cache);

            // Recalculate terminal grid size (like resize does)
            let cell_width = state.gpu.glyph_cache.cell_width();
            let line_height = state.gpu.glyph_cache.line_height();
            let tab_bar_height = state.gpu.tab_bar.height();

            let padding_physical = 20.0 * state.scale_factor;
            let tab_bar_physical = tab_bar_height * state.scale_factor;

            let content_width = (state.gpu.config.width as f32 - padding_physical).max(60.0);
            let content_height =
                (state.gpu.config.height as f32 - padding_physical - tab_bar_physical).max(40.0);

            let new_cols = ((content_width / cell_width) as usize).max(10);
            let new_rows = ((content_height / line_height) as usize).max(4);

            state.cols = new_cols;
            state.rows = new_rows;

            // Resize all shells to match new grid size
            for shell in state.shells.values_mut() {
                shell.resize(Size::new(new_cols, new_rows));
            }

            // Trigger zoom indicator
            state.ui.zoom_indicator.trigger(new_scale);

            // Force full redraw
            state.render.dirty = true;
            for hash in state.content_hashes.values_mut() {
                *hash = 0;
            }
            state.window.request_redraw();
        }
    }

    pub(crate) fn navigate_tab(&mut self, next: bool) {
        if let Some(state) = self.focused_window_mut() {
            if next {
                state.gpu.tab_bar.next_tab();
            } else {
                state.gpu.tab_bar.prev_tab();
            }
            state.force_active_tab_redraw();
            state.window.request_redraw();
        }
    }

    pub(crate) fn select_tab_index(&mut self, index: usize) {
        if let Some(state) = self.focused_window_mut() {
            state.gpu.tab_bar.select_tab_index(index);
            state.force_active_tab_redraw();
            state.window.request_redraw();
        }
    }
}
