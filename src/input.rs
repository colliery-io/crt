//! Input handling
//!
//! Keyboard and mouse input processing for terminal and tab bar.

use winit::keyboard::{Key, NamedKey};

use crate::window::WindowState;

/// Result of handling tab editing input
pub enum TabEditResult {
    /// Input was handled by tab editing
    Handled,
    /// Input was not handled (not in edit mode or not an edit key)
    NotHandled,
}

/// Handle keyboard input for tab title editing
pub fn handle_tab_editing(
    state: &mut WindowState,
    key: &Key,
    mod_pressed: bool,
) -> TabEditResult {
    if !state.gpu.tab_bar.is_editing() || mod_pressed {
        return TabEditResult::NotHandled;
    }

    let mut handled = true;
    let mut need_redraw = true;

    match key {
        Key::Named(NamedKey::Enter) => {
            state.gpu.tab_bar.confirm_editing();
        }
        Key::Named(NamedKey::Escape) => {
            state.gpu.tab_bar.cancel_editing();
        }
        Key::Named(NamedKey::Backspace) => {
            state.gpu.tab_bar.edit_backspace();
        }
        Key::Named(NamedKey::Delete) => {
            state.gpu.tab_bar.edit_delete();
        }
        Key::Named(NamedKey::ArrowLeft) => {
            state.gpu.tab_bar.edit_cursor_left();
        }
        Key::Named(NamedKey::ArrowRight) => {
            state.gpu.tab_bar.edit_cursor_right();
        }
        Key::Named(NamedKey::Home) => {
            state.gpu.tab_bar.edit_cursor_home();
        }
        Key::Named(NamedKey::End) => {
            state.gpu.tab_bar.edit_cursor_end();
        }
        Key::Named(NamedKey::Space) => {
            state.gpu.tab_bar.edit_insert_char(' ');
        }
        Key::Character(c) => {
            for ch in c.chars() {
                if !ch.is_control() {
                    state.gpu.tab_bar.edit_insert_char(ch);
                }
            }
        }
        _ => {
            handled = false;
            need_redraw = false;
        }
    }

    if need_redraw {
        state.dirty = true;
        state.window.request_redraw();
    }

    if handled {
        TabEditResult::Handled
    } else {
        TabEditResult::NotHandled
    }
}

/// Handle shell input (send to PTY)
pub fn handle_shell_input(
    state: &mut WindowState,
    key: &Key,
    mod_pressed: bool,
) -> bool {
    let tab_id = state.gpu.tab_bar.active_tab_id();
    let Some(tab_id) = tab_id else { return false };
    let Some(shell) = state.shells.get(&tab_id) else { return false };

    let mut input_sent = false;
    match key {
        Key::Named(NamedKey::Escape) => {
            shell.send_input(b"\x1b");
            input_sent = true;
        }
        Key::Named(NamedKey::Enter) => {
            shell.send_input(b"\r");
            input_sent = true;
        }
        Key::Named(NamedKey::Backspace) => {
            shell.send_input(b"\x7f");
            input_sent = true;
        }
        Key::Named(NamedKey::Tab) => {
            shell.send_input(b"\t");
            input_sent = true;
        }
        Key::Named(NamedKey::ArrowUp) => {
            shell.send_input(b"\x1b[A");
            input_sent = true;
        }
        Key::Named(NamedKey::ArrowDown) => {
            shell.send_input(b"\x1b[B");
            input_sent = true;
        }
        Key::Named(NamedKey::ArrowRight) => {
            shell.send_input(b"\x1b[C");
            input_sent = true;
        }
        Key::Named(NamedKey::ArrowLeft) => {
            shell.send_input(b"\x1b[D");
            input_sent = true;
        }
        Key::Named(NamedKey::Space) => {
            shell.send_input(b" ");
            input_sent = true;
        }
        Key::Character(c) => {
            if !mod_pressed {
                shell.send_input(c.as_bytes());
                input_sent = true;
            }
        }
        _ => {}
    }

    if input_sent {
        state.dirty = true;
        state.window.request_redraw();
    }

    input_sent
}

/// Handle mouse click on tab bar
pub fn handle_tab_click(
    state: &mut WindowState,
    x: f32,
    y: f32,
    now: std::time::Instant,
) -> bool {
    let double_click_threshold = std::time::Duration::from_millis(400);

    let mut tab_closed = None;
    let mut tab_switched = false;
    let mut started_editing = false;

    if state.gpu.tab_bar.is_editing() {
        if let Some(editing_id) = state.gpu.tab_bar.editing_tab_id() {
            if let Some((tab_id, _)) = state.gpu.tab_bar.hit_test(x, y) {
                if tab_id != editing_id {
                    state.gpu.tab_bar.confirm_editing();
                    state.gpu.tab_bar.select_tab(tab_id);
                    tab_switched = true;
                }
            } else {
                state.gpu.tab_bar.confirm_editing();
            }
        }
    } else {
        if let Some((tab_id, is_close)) = state.gpu.tab_bar.hit_test(x, y) {
            if is_close {
                if state.gpu.tab_bar.tab_count() > 1 {
                    state.gpu.tab_bar.close_tab(tab_id);
                    tab_closed = Some(tab_id);
                    tab_switched = true;
                }
            } else {
                let is_double_click = state.last_click_time
                    .map(|t| now.duration_since(t) < double_click_threshold)
                    .unwrap_or(false)
                    && state.last_click_tab == Some(tab_id);

                if is_double_click {
                    state.gpu.tab_bar.start_editing(tab_id);
                    started_editing = true;
                    state.last_click_time = None;
                    state.last_click_tab = None;
                } else {
                    state.gpu.tab_bar.select_tab(tab_id);
                    tab_switched = true;
                    state.last_click_time = Some(now);
                    state.last_click_tab = Some(tab_id);
                }
            }
        } else {
            state.last_click_time = None;
            state.last_click_tab = None;
        }
    }

    if let Some(tab_id) = tab_closed {
        state.remove_shell_for_tab(tab_id);
    }

    if tab_switched || started_editing {
        state.force_active_tab_redraw();
        state.window.request_redraw();
        true
    } else {
        false
    }
}

/// Handle window resize
pub fn handle_resize(
    state: &mut WindowState,
    shared: &crate::gpu::SharedGpuState,
    new_width: u32,
    new_height: u32,
) {
    use crt_core::Size;

    if new_width < 100 || new_height < 80 {
        return;
    }

    let scale_factor = state.scale_factor;
    let cell_width = state.gpu.glyph_cache.cell_width();
    let line_height = state.gpu.glyph_cache.line_height();
    let tab_bar_height = state.gpu.tab_bar.height();

    let padding_physical = 20.0 * scale_factor;
    let tab_bar_physical = tab_bar_height * scale_factor;

    // Tab bar is always at top, so subtract its height from content area
    let content_width = (new_width as f32 - padding_physical).max(60.0);
    let content_height = (new_height as f32 - padding_physical - tab_bar_physical).max(40.0);

    let new_cols = ((content_width / cell_width) as usize).max(10);
    let new_rows = ((content_height / line_height) as usize).max(4);

    state.cols = new_cols;
    state.rows = new_rows;

    // Resize all shells in this window
    for shell in state.shells.values_mut() {
        shell.resize(Size::new(new_cols, new_rows));
    }

    // Update GPU resources
    state.gpu.config.width = new_width;
    state.gpu.config.height = new_height;
    state.gpu.surface.configure(&shared.device, &state.gpu.config);

    state.gpu.grid_renderer.update_screen_size(
        &shared.queue,
        new_width as f32,
        new_height as f32,
    );
    state.gpu.tab_title_renderer.update_screen_size(
        &shared.queue,
        new_width as f32,
        new_height as f32,
    );

    state.gpu.tab_bar.resize(new_width as f32, new_height as f32);
    state.gpu.text_target.resize(&shared.device, new_width, new_height, state.gpu.config.format);
    state.gpu.composite_bind_group = Some(
        state.gpu.effect_pipeline.create_bind_group(&shared.device, &state.gpu.text_target.view)
    );

    state.dirty = true;
    for hash in state.content_hashes.values_mut() {
        *hash = 0;
    }
    state.window.request_redraw();
}
