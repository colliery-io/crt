//! Menu action handling.
//!
//! Processes macOS menu bar actions (new tab, close, theme switching, etc.).

use crate::input::{get_clipboard_content, get_terminal_selection_text, paste_to_terminal, set_clipboard_content};
use crate::menu::MenuAction;
use winit::event_loop::ActiveEventLoop;

use super::{App, FONT_SCALE_STEP};

#[cfg(target_os = "macos")]
impl App {
    pub(crate) fn handle_menu_action(&mut self, action: MenuAction, event_loop: &ActiveEventLoop) {
        match action {
            MenuAction::OpenConfig => self.open_config_file(),
            MenuAction::NewTab => self.open_new_tab(),
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
            MenuAction::ToggleFullScreen => self.toggle_fullscreen_focused(),
            MenuAction::IncreaseFontSize => self.adjust_font_scale(FONT_SCALE_STEP),
            MenuAction::DecreaseFontSize => self.adjust_font_scale(-FONT_SCALE_STEP),
            MenuAction::ResetFontSize => self.reset_font_scale(),
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
                // Reflect the new state in the menu item label.
                if let Some(ids) = self.menu_ids.as_ref() {
                    ids.toggle_profiling_item.set_text(if enabled {
                        "Stop Profiling"
                    } else {
                        "Start Profiling"
                    });
                }
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
                    // Persist so the choice survives a restart.
                    self.persist_theme_choice(theme_name);
                } else {
                    log::warn!("Theme '{}' not found in registry", theme_name);
                }
            }
            _ => log::info!("{:?} not yet implemented", action),
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
