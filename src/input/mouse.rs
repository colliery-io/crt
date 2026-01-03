//! Mouse input handling
//!
//! Extracts mouse event handling logic from main.rs for better modularity.

use std::time::Instant;

use crt_core::Scroll;
use winit::event::{ElementState, Modifiers, MouseButton, MouseScrollDelta};

use crate::window::{ContextMenuItem, WindowState};

use super::{
    MOUSE_BUTTON_LEFT, MOUSE_BUTTON_MIDDLE, MOUSE_BUTTON_RIGHT, find_url_at_position,
    find_url_index_at_position, get_clipboard_content, get_terminal_selection_text,
    handle_tab_click, handle_terminal_mouse_button, handle_terminal_mouse_move,
    handle_terminal_mouse_release, handle_terminal_scroll, open_url, paste_to_terminal,
    set_clipboard_content,
};

/// Handle cursor moved event
///
/// Updates cursor position, context menu hover, URL hover, and selection drag.
pub fn handle_cursor_moved(state: &mut WindowState, x: f32, y: f32) {
    state.interaction.cursor_position = (x, y);

    // Update context menu hover state
    if state.ui.context_menu.visible {
        let old_hover = state.ui.context_menu.hovered_item;
        state.ui.context_menu.update_hover(x, y);
        if old_hover != state.ui.context_menu.hovered_item {
            state.render.dirty = true;
            state.window.request_redraw();
        }
    }

    // Update selection if dragging
    handle_terminal_mouse_move(state, x, y);

    // Check for URL hover and update underline state
    let (offset_x, offset_y) = state.gpu.tab_bar.content_offset();
    let padding = 10.0 * state.scale_factor;
    let cell_width = state.gpu.glyph_cache.cell_width();
    let line_height = state.gpu.glyph_cache.line_height();

    let rel_x = x - offset_x - padding;
    let rel_y = y - offset_y - padding;

    let new_hovered = if rel_x >= 0.0 && rel_y >= 0.0 {
        let col = (rel_x / cell_width) as usize;
        let line = (rel_y / line_height) as usize;
        find_url_index_at_position(&state.interaction.detected_urls, col, line)
    } else {
        None
    };

    // Redraw if hover state changed
    if new_hovered != state.interaction.hovered_url_index {
        state.interaction.hovered_url_index = new_hovered;
        // Force content re-render to update decorations
        state.force_active_tab_redraw();
    }
}

/// Handle mouse button event
///
/// Returns true if the event was fully handled.
pub fn handle_mouse_input(
    state: &mut WindowState,
    button: MouseButton,
    button_state: ElementState,
    modifiers: &Modifiers,
) -> bool {
    let (x, y) = state.interaction.cursor_position;

    // Check for Cmd+click (Super on macOS, Ctrl on Linux) to open URLs
    #[cfg(target_os = "macos")]
    let cmd_pressed = modifiers.state().super_key();
    #[cfg(not(target_os = "macos"))]
    let cmd_pressed = modifiers.state().control_key();

    if cmd_pressed && button == MouseButton::Left && button_state == ElementState::Pressed {
        // Calculate cell position from pixel coordinates
        let (offset_x, offset_y) = state.gpu.tab_bar.content_offset();
        let padding = 10.0 * state.scale_factor;
        let cell_width = state.gpu.glyph_cache.cell_width();
        let line_height = state.gpu.glyph_cache.line_height();

        let rel_x = x - offset_x - padding;
        let rel_y = y - offset_y - padding;

        if rel_x >= 0.0 && rel_y >= 0.0 {
            let col = (rel_x / cell_width) as usize;
            let line = (rel_y / line_height) as usize;

            // Check if there's a URL at this position
            if let Some(url) = find_url_at_position(&state.interaction.detected_urls, col, line) {
                log::info!("Opening URL: {}", url.url);
                open_url(&url.url);
                return true;
            }
        }
    }

    // Handle context menu interactions first
    if state.ui.context_menu.visible {
        match (button, button_state) {
            (MouseButton::Left, ElementState::Pressed) => {
                // Check submenu first
                if let Some(item) = state.ui.context_menu.submenu_item_at(x, y) {
                    handle_context_menu_action(state, item);
                    state.ui.context_menu.hide();
                    state.render.dirty = true;
                    state.window.request_redraw();
                    return true;
                }
                // Check main menu
                if let Some(item) = state.ui.context_menu.item_at(x, y) {
                    // Themes item doesn't do anything on click (submenu shows on hover)
                    if !item.has_submenu() {
                        handle_context_menu_action(state, item);
                        state.ui.context_menu.hide();
                        state.render.dirty = true;
                        state.window.request_redraw();
                        return true;
                    }
                    // Clicking on Themes just keeps it highlighted
                    return true;
                }
                // Clicking outside the menu dismisses it
                state.ui.context_menu.hide();
                state.render.dirty = true;
                state.window.request_redraw();
                // Fall through to normal click handling
            }
            (MouseButton::Right, ElementState::Pressed) => {
                // Right-click while menu is open moves the menu
                state.ui.context_menu.show(x, y);
                state.render.dirty = true;
                state.window.request_redraw();
                return true;
            }
            _ => {}
        }
    }

    // Right-click shows context menu
    if button == MouseButton::Right && button_state == ElementState::Pressed {
        state.ui.context_menu.show(x, y);
        state.render.dirty = true;
        state.window.request_redraw();
        return true;
    }

    let mouse_button = match button {
        MouseButton::Left => Some(MOUSE_BUTTON_LEFT),
        MouseButton::Middle => Some(MOUSE_BUTTON_MIDDLE),
        MouseButton::Right => Some(MOUSE_BUTTON_RIGHT),
        _ => None,
    };

    if let Some(btn) = mouse_button {
        match button_state {
            ElementState::Pressed => {
                // Try terminal (mouse reporting or selection) first, then tab bar
                if !handle_terminal_mouse_button(state, x, y, Instant::now(), btn, true)
                    && btn == MOUSE_BUTTON_LEFT
                {
                    handle_tab_click(state, x, y, Instant::now());
                }
            }
            ElementState::Released => {
                handle_terminal_mouse_release(state, x, y);
            }
        }
    }

    false
}

/// Handle mouse wheel event
pub fn handle_mouse_wheel(state: &mut WindowState, delta: MouseScrollDelta) {
    let (x, y) = state.interaction.cursor_position;
    let delta_y = match delta {
        MouseScrollDelta::LineDelta(_, y) => y,
        MouseScrollDelta::PixelDelta(pos) => {
            let line_height = state.gpu.glyph_cache.line_height();
            (pos.y / line_height as f64) as f32
        }
    };

    // Check if mouse reporting should handle this
    if handle_terminal_scroll(state, x, y, delta_y) {
        // Mouse reporting handled the scroll
        return;
    }

    // Fall back to local scrollback
    let tab_id = state.gpu.tab_bar.active_tab_id();
    if let Some(tab_id) = tab_id
        && let Some(shell) = state.shells.get_mut(&tab_id)
    {
        let lines = delta_y as i32;
        if lines != 0 {
            shell.scroll(Scroll::Delta(lines));
            state.render.dirty = true;
            state.content_hashes.insert(tab_id, 0);
            state.window.request_redraw();
        }
    }
}

/// Handle context menu action (copy, paste, select all)
pub(super) fn handle_context_menu_action(state: &mut WindowState, item: ContextMenuItem) {
    match item {
        ContextMenuItem::Copy => {
            if let Some(text) = get_terminal_selection_text(state) {
                set_clipboard_content(&text);
                state.ui.copy_indicator.trigger();
            }
        }
        ContextMenuItem::Paste => {
            if let Some(content) = get_clipboard_content() {
                paste_to_terminal(state, &content);
            }
        }
        ContextMenuItem::SelectAll => {
            // Select all visible content
            if let Some(tab_id) = state.gpu.tab_bar.active_tab_id()
                && let Some(shell) = state.shells.get_mut(&tab_id)
            {
                use crt_core::{Column, Line, Point, SelectionType};
                let terminal = shell.terminal_mut();
                let screen_lines = terminal.screen_lines();
                let columns = terminal.columns();

                // Start selection at top-left
                terminal.start_selection(
                    Point {
                        line: Line(0),
                        column: Column(0),
                    },
                    SelectionType::Simple,
                );
                // Extend to bottom-right
                terminal.update_selection(Point {
                    line: Line(screen_lines as i32 - 1),
                    column: Column(columns - 1),
                });
                state.render.dirty = true;
            }
        }
        ContextMenuItem::Separator | ContextMenuItem::Themes => {
            // Separator and Themes parent items are not clickable
        }
        ContextMenuItem::Theme(name) => {
            // Store pending theme change for main loop to process
            state.ui.pending_theme = Some(name);
        }
    }
}
