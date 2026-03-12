//! Mouse input handling
//!
//! Extracts mouse event handling logic from main.rs for better modularity.
//!
//! Pure decision functions (no side effects, fully testable):
//! - `screen_to_grid_position` — pixel-to-cell coordinate conversion
//! - `determine_click_target` — decides what a mouse click hits
//! - `compute_click_count` — multi-click detection (single/double/triple)
//! - `normalize_scroll_delta` — converts pixel scroll delta to line count

use std::time::{Duration, Instant};

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

// ── Pure decision functions (no side effects) ──────────────────────────────

/// Layout parameters needed for coordinate conversion
#[derive(Debug, Clone, Copy)]
pub struct GridLayout {
    /// X offset of the content area (e.g., tab bar offset)
    pub content_offset_x: f32,
    /// Y offset of the content area
    pub content_offset_y: f32,
    /// Padding around the terminal content
    pub padding: f32,
    /// Width of a single character cell
    pub cell_width: f32,
    /// Height of a single line
    pub line_height: f32,
    /// Maximum number of columns
    pub max_cols: usize,
    /// Maximum number of rows
    pub max_rows: usize,
}

/// Convert screen pixel coordinates to terminal grid position (col, line).
///
/// Returns `None` if the position is outside the terminal content area
/// (negative relative coordinates after subtracting offsets and padding).
/// Clamps to grid bounds when inside the content area.
pub fn screen_to_grid_position(x: f32, y: f32, layout: &GridLayout) -> Option<(usize, usize)> {
    let rel_x = x - layout.content_offset_x - layout.padding;
    let rel_y = y - layout.content_offset_y - layout.padding;

    if rel_x < 0.0 || rel_y < 0.0 {
        return None;
    }

    let col = (rel_x / layout.cell_width) as usize;
    let line = (rel_y / layout.line_height) as usize;

    let col = col.min(layout.max_cols.saturating_sub(1));
    let line = line.min(layout.max_rows.saturating_sub(1));

    Some((col, line))
}

/// What a mouse click targets
#[derive(Debug, Clone, PartialEq)]
pub enum MouseClickTarget {
    /// Cmd+click on a URL at grid position (col, line)
    OpenUrl { col: usize, line: usize },
    /// Left-click on a context menu submenu item
    ContextSubmenuItem,
    /// Left-click on a context menu main item
    ContextMenuItem,
    /// Left-click on a context menu item that has a submenu (no-op click)
    ContextSubmenuParent,
    /// Click outside the context menu (dismisses it)
    DismissContextMenu,
    /// Right-click while context menu is open (repositions it)
    MoveContextMenu,
    /// Right-click to open context menu
    ShowContextMenu,
    /// Left-click in terminal/tab area (normal click handling)
    Terminal { button: u8 },
    /// Mouse button release
    Release,
    /// Unhandled button or state
    None,
}

/// Determine what a mouse click targets, without performing any side effects.
///
/// This is the pure decision function for `handle_mouse_input`. The caller
/// is responsible for executing the appropriate side effects based on the result.
pub fn determine_click_target(
    button: MouseButton,
    button_state: ElementState,
    cmd_pressed: bool,
    context_menu_visible: bool,
    has_grid_position: bool,
) -> MouseClickTarget {
    // Cmd+click to open URL
    if cmd_pressed && button == MouseButton::Left && button_state == ElementState::Pressed {
        // Caller must check if there's actually a URL at the position
        if has_grid_position {
            return MouseClickTarget::OpenUrl { col: 0, line: 0 };
        }
    }

    // Context menu interactions
    if context_menu_visible {
        match (button, button_state) {
            (MouseButton::Left, ElementState::Pressed) => {
                // Caller determines which item was hit and fills in the variant
                // This returns a hint that context menu should be checked
                return MouseClickTarget::ContextMenuItem;
            }
            (MouseButton::Right, ElementState::Pressed) => {
                return MouseClickTarget::MoveContextMenu;
            }
            _ => {}
        }
    }

    // Right-click shows context menu
    if button == MouseButton::Right && button_state == ElementState::Pressed {
        return MouseClickTarget::ShowContextMenu;
    }

    // Map winit button to terminal button code
    let mouse_button = match button {
        MouseButton::Left => Some(MOUSE_BUTTON_LEFT),
        MouseButton::Middle => Some(MOUSE_BUTTON_MIDDLE),
        MouseButton::Right => Some(MOUSE_BUTTON_RIGHT),
        _ => None,
    };

    match (mouse_button, button_state) {
        (Some(btn), ElementState::Pressed) => MouseClickTarget::Terminal { button: btn },
        (Some(_), ElementState::Released) => MouseClickTarget::Release,
        _ => MouseClickTarget::None,
    }
}

/// Compute click count for multi-click detection (single, double, triple).
///
/// Returns 1 for single click, 2 for double, 3 for triple. Cycles back to 1
/// after triple click. Resets to 1 if the click is too far from the previous
/// click or too much time has elapsed.
pub fn compute_click_count(
    now: Instant,
    last_click_time: Option<Instant>,
    last_click_pos: Option<(usize, usize)>,
    current_pos: (usize, usize),
    previous_count: usize,
    threshold: Duration,
    max_distance: usize,
) -> usize {
    if let (Some(last_time), Some((last_col, last_line))) = (last_click_time, last_click_pos) {
        let time_ok = now.duration_since(last_time) < threshold;
        let pos_ok = current_pos.0.abs_diff(last_col) <= max_distance
            && current_pos.1.abs_diff(last_line) <= max_distance;

        if time_ok && pos_ok {
            (previous_count % 3) + 1
        } else {
            1
        }
    } else {
        1
    }
}

/// Normalize a pixel scroll delta to a line count.
///
/// For `LineDelta`, returns the Y component directly.
/// For `PixelDelta`, divides by line height to convert pixels to lines.
pub fn normalize_scroll_delta(delta: &MouseScrollDelta, line_height: f32) -> f32 {
    match delta {
        MouseScrollDelta::LineDelta(_, y) => *y,
        MouseScrollDelta::PixelDelta(pos) => (pos.y / line_height as f64) as f32,
    }
}

// ── Side-effectful handler functions ───────────────────────────────────────

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
    let layout = grid_layout_from_state(state);
    let new_hovered = screen_to_grid_position(x, y, &layout)
        .and_then(|(col, line)| {
            find_url_index_at_position(&state.interaction.detected_urls, col, line)
        });

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
        let layout = grid_layout_from_state(state);
        if let Some((col, line)) = screen_to_grid_position(x, y, &layout) {
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
    let line_height = state.gpu.glyph_cache.line_height();
    let delta_y = normalize_scroll_delta(&delta, line_height);

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

/// Build a `GridLayout` from the current window state.
///
/// Extracts the layout parameters needed by pure coordinate-conversion functions.
fn grid_layout_from_state(state: &WindowState) -> GridLayout {
    let (content_offset_x, content_offset_y) = state.gpu.tab_bar.content_offset();
    GridLayout {
        content_offset_x,
        content_offset_y,
        padding: 10.0 * state.scale_factor,
        cell_width: state.gpu.glyph_cache.cell_width(),
        line_height: state.gpu.glyph_cache.line_height(),
        max_cols: state.cols,
        max_rows: state.rows,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn test_layout() -> GridLayout {
        GridLayout {
            content_offset_x: 0.0,
            content_offset_y: 30.0, // tab bar height
            padding: 10.0,
            cell_width: 8.0,
            line_height: 16.0,
            max_cols: 80,
            max_rows: 24,
        }
    }

    // ── screen_to_grid_position tests ──────────────────────────────────

    #[test]
    fn grid_position_at_origin() {
        let layout = test_layout();
        // First cell: x just past offset+padding, y just past offset+padding
        let pos = screen_to_grid_position(10.5, 40.5, &layout);
        assert_eq!(pos, Some((0, 0)));
    }

    #[test]
    fn grid_position_mid_screen() {
        let layout = test_layout();
        // x = 0 + 10 + 8*10 = 90, y = 30 + 10 + 16*5 = 120
        let pos = screen_to_grid_position(90.0, 120.0, &layout);
        assert_eq!(pos, Some((10, 5)));
    }

    #[test]
    fn grid_position_outside_returns_none() {
        let layout = test_layout();
        // x is in the padding zone (before content starts)
        assert_eq!(screen_to_grid_position(5.0, 50.0, &layout), None);
        // y is above the content area
        assert_eq!(screen_to_grid_position(50.0, 25.0, &layout), None);
    }

    #[test]
    fn grid_position_clamps_to_bounds() {
        let layout = test_layout();
        // Very far right — should clamp to max_cols - 1
        let pos = screen_to_grid_position(10000.0, 50.0, &layout);
        assert_eq!(pos, Some((79, 0)));
        // Very far down — should clamp to max_rows - 1
        let pos = screen_to_grid_position(50.0, 10000.0, &layout);
        assert_eq!(pos, Some((5, 23)));
    }

    // ── compute_click_count tests ──────────────────────────────────────

    #[test]
    fn click_count_single_on_first_click() {
        let now = Instant::now();
        let count = compute_click_count(
            now,
            None,  // no previous click
            None,
            (5, 5),
            0,
            Duration::from_millis(400),
            1,
        );
        assert_eq!(count, 1);
    }

    #[test]
    fn click_count_double_on_rapid_same_position() {
        let first = Instant::now();
        let second = first + Duration::from_millis(200);
        let count = compute_click_count(
            second,
            Some(first),
            Some((5, 5)),
            (5, 5),
            1, // previous count was 1
            Duration::from_millis(400),
            1,
        );
        assert_eq!(count, 2);
    }

    #[test]
    fn click_count_triple_then_wraps() {
        let first = Instant::now();
        let second = first + Duration::from_millis(200);
        // Third click with previous count 2 → should give 3
        let count = compute_click_count(
            second,
            Some(first),
            Some((5, 5)),
            (5, 5),
            2,
            Duration::from_millis(400),
            1,
        );
        assert_eq!(count, 3);

        // Fourth click with previous count 3 → wraps back to 1
        let third = second + Duration::from_millis(200);
        let count = compute_click_count(
            third,
            Some(second),
            Some((5, 5)),
            (5, 5),
            3,
            Duration::from_millis(400),
            1,
        );
        assert_eq!(count, 1);
    }

    #[test]
    fn click_count_resets_on_timeout() {
        let first = Instant::now();
        let second = first + Duration::from_millis(500); // beyond 400ms threshold
        let count = compute_click_count(
            second,
            Some(first),
            Some((5, 5)),
            (5, 5),
            1,
            Duration::from_millis(400),
            1,
        );
        assert_eq!(count, 1);
    }

    #[test]
    fn click_count_resets_on_distance() {
        let first = Instant::now();
        let second = first + Duration::from_millis(200);
        let count = compute_click_count(
            second,
            Some(first),
            Some((5, 5)),
            (10, 10), // far from previous
            1,
            Duration::from_millis(400),
            1,
        );
        assert_eq!(count, 1);
    }

    // ── determine_click_target tests ───────────────────────────────────

    #[test]
    fn click_target_cmd_click_url() {
        let target = determine_click_target(
            MouseButton::Left,
            ElementState::Pressed,
            true, // cmd pressed
            false,
            true, // has grid position
        );
        assert!(matches!(target, MouseClickTarget::OpenUrl { .. }));
    }

    #[test]
    fn click_target_right_click_shows_menu() {
        let target = determine_click_target(
            MouseButton::Right,
            ElementState::Pressed,
            false,
            false, // no context menu visible
            true,
        );
        assert_eq!(target, MouseClickTarget::ShowContextMenu);
    }

    #[test]
    fn click_target_right_click_moves_menu_when_visible() {
        let target = determine_click_target(
            MouseButton::Right,
            ElementState::Pressed,
            false,
            true, // context menu visible
            true,
        );
        assert_eq!(target, MouseClickTarget::MoveContextMenu);
    }

    #[test]
    fn click_target_left_press_terminal() {
        let target = determine_click_target(
            MouseButton::Left,
            ElementState::Pressed,
            false,
            false,
            true,
        );
        assert_eq!(target, MouseClickTarget::Terminal { button: MOUSE_BUTTON_LEFT });
    }

    #[test]
    fn click_target_release() {
        let target = determine_click_target(
            MouseButton::Left,
            ElementState::Released,
            false,
            false,
            true,
        );
        assert_eq!(target, MouseClickTarget::Release);
    }

    // ── normalize_scroll_delta tests ───────────────────────────────────

    #[test]
    fn scroll_delta_line_passthrough() {
        let delta = MouseScrollDelta::LineDelta(0.0, 3.0);
        assert_eq!(normalize_scroll_delta(&delta, 16.0), 3.0);
    }

    #[test]
    fn scroll_delta_pixel_converts_to_lines() {
        let delta = MouseScrollDelta::PixelDelta(winit::dpi::PhysicalPosition::new(0.0, 32.0));
        let result = normalize_scroll_delta(&delta, 16.0);
        assert!((result - 2.0).abs() < 0.01);
    }
}
