//! Input handling
//!
//! Keyboard and mouse input processing for terminal and tab bar.

mod key_encoder;
mod keyboard;
mod mouse;

pub use key_encoder::encode_key;
pub use keyboard::{KeyboardAction, handle_keyboard_input};
pub use mouse::{handle_cursor_moved, handle_mouse_input, handle_mouse_wheel};

use std::sync::OnceLock;
use std::time::{Duration, Instant};

use crt_core::{Column, Line, Point, SelectionType, ShellTerminal, TermMode};
use regex::Regex;
use winit::keyboard::{Key, NamedKey};

use crate::window::WindowState;

/// Detected URL with its position in the terminal
#[derive(Debug, Clone)]
pub struct DetectedUrl {
    /// The URL string
    pub url: String,
    /// Starting column (0-indexed)
    pub start_col: usize,
    /// Ending column (exclusive, 0-indexed)
    pub end_col: usize,
    /// Line number (0-indexed viewport line)
    pub line: usize,
}

/// Get the URL regex (compiled once)
fn url_regex() -> &'static Regex {
    static URL_REGEX: OnceLock<Regex> = OnceLock::new();
    URL_REGEX.get_or_init(|| {
        // Match http://, https://, file:// URLs
        // Also match bare domains like github.com/path
        Regex::new(
            r"(?x)
            (?:https?://|file://)  # Protocol
            [^\s<>\[\]{}|\\^`\x00-\x1f]+  # URL characters (no whitespace or special chars)
            |
            (?:www\.)  # www. prefix
            [^\s<>\[\]{}|\\^`\x00-\x1f]+  # URL characters
            ",
        )
        .expect("Invalid URL regex")
    })
}

/// Scan a line of text for URLs and return their positions
pub fn detect_urls_in_line(line_text: &str, line_num: usize) -> Vec<DetectedUrl> {
    let regex = url_regex();
    regex
        .find_iter(line_text)
        .map(|m| DetectedUrl {
            url: m.as_str().to_string(),
            start_col: m.start(),
            end_col: m.end(),
            line: line_num,
        })
        .collect()
}

/// Check if a position (col, line) is within a detected URL
pub fn find_url_at_position(urls: &[DetectedUrl], col: usize, line: usize) -> Option<&DetectedUrl> {
    urls.iter()
        .find(|url| url.line == line && col >= url.start_col && col < url.end_col)
}

/// Find the index of a URL at a given position
pub fn find_url_index_at_position(urls: &[DetectedUrl], col: usize, line: usize) -> Option<usize> {
    urls.iter()
        .position(|url| url.line == line && col >= url.start_col && col < url.end_col)
}

/// Open a URL in the default browser
pub fn open_url(url: &str) {
    // Ensure URL has a protocol
    let full_url = if url.starts_with("www.") {
        format!("https://{}", url)
    } else {
        url.to_string()
    };

    if let Err(e) = open::that(&full_url) {
        log::error!("Failed to open URL '{}': {}", full_url, e);
    }
}

/// Threshold for multi-click detection
const MULTI_CLICK_THRESHOLD: Duration = Duration::from_millis(400);
/// Maximum distance (in cells) for multi-click to register
const MULTI_CLICK_DISTANCE: usize = 1;

// Mouse button codes for mouse reporting protocol
pub const MOUSE_BUTTON_LEFT: u8 = 0;
pub const MOUSE_BUTTON_MIDDLE: u8 = 1;
pub const MOUSE_BUTTON_RIGHT: u8 = 2;
pub const MOUSE_BUTTON_RELEASE: u8 = 3;
pub const MOUSE_BUTTON_MOTION: u8 = 32;
pub const MOUSE_BUTTON_SCROLL_UP: u8 = 64;
pub const MOUSE_BUTTON_SCROLL_DOWN: u8 = 65;

/// Check if the terminal has mouse reporting enabled
pub fn should_report_mouse(shell: &ShellTerminal) -> bool {
    let mode = shell.terminal().inner().mode();
    mode.intersects(
        TermMode::MOUSE_REPORT_CLICK
            | TermMode::MOUSE_DRAG
            | TermMode::MOUSE_MOTION
            | TermMode::SGR_MOUSE,
    )
}

/// Check if the terminal is tracking mouse motion
pub fn should_report_motion(shell: &ShellTerminal, button_pressed: bool) -> bool {
    let mode = shell.terminal().inner().mode();
    // MOUSE_MOTION: report all motion
    // MOUSE_DRAG: report motion only when button is pressed
    mode.contains(TermMode::MOUSE_MOTION) || (mode.contains(TermMode::MOUSE_DRAG) && button_pressed)
}

/// Check if SGR extended mouse mode is enabled
pub fn is_sgr_mouse_mode(shell: &ShellTerminal) -> bool {
    shell
        .terminal()
        .inner()
        .mode()
        .contains(TermMode::SGR_MOUSE)
}

/// Generate mouse escape sequence for terminal
///
/// # Arguments
/// * `button` - Mouse button code (0=left, 1=middle, 2=right, 3=release, 32+=motion, 64/65=scroll)
/// * `col` - Column (0-indexed)
/// * `line` - Line (0-indexed)
/// * `pressed` - Whether this is a press event (for SGR mode)
/// * `sgr_mode` - Whether to use SGR extended encoding
pub fn mouse_report(button: u8, col: usize, line: usize, pressed: bool, sgr_mode: bool) -> Vec<u8> {
    if sgr_mode {
        // SGR extended mode: \x1b[<Btn;Col;RowM (press) or m (release)
        // Uses 1-indexed coordinates
        let suffix = if pressed { 'M' } else { 'm' };
        format!("\x1b[<{};{};{}{}", button, col + 1, line + 1, suffix).into_bytes()
    } else {
        // Legacy X10 mode: \x1b[M<btn+32><col+33><row+33>
        // Coordinates are offset by 32 and limited to 223 (255 - 32)
        let mut seq = vec![0x1b, b'[', b'M'];
        seq.push(button + 32);
        seq.push(((col + 33).min(255)) as u8);
        seq.push(((line + 33).min(255)) as u8);
        seq
    }
}

/// Result of handling tab editing input
pub enum TabEditResult {
    /// Input was handled by tab editing
    Handled,
    /// Input was not handled (not in edit mode or not an edit key)
    NotHandled,
}

/// Handle keyboard input for tab title editing
pub fn handle_tab_editing(state: &mut WindowState, key: &Key, mod_pressed: bool) -> TabEditResult {
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
        state.render.dirty = true;
        state.window.request_redraw();
    }

    if handled {
        TabEditResult::Handled
    } else {
        TabEditResult::NotHandled
    }
}

/// Handle shell input (send to PTY)
///
/// Uses termwiz for key-to-escape-sequence encoding, with platform-specific
/// overrides for macOS word navigation. Falls back to the event's text field
/// for any keys termwiz doesn't handle.
pub fn handle_shell_input(
    state: &mut WindowState,
    key: &Key,
    text: Option<&str>,
    mod_pressed: bool,
    ctrl_pressed: bool,
    shift_pressed: bool,
    alt_pressed: bool,
) -> bool {
    let tab_id = state.gpu.tab_bar.active_tab_id();
    let Some(tab_id) = tab_id else { return false };
    let Some(shell) = state.shells.get_mut(&tab_id) else {
        return false;
    };

    log::debug!(
        "Shell input: key={:?} text={:?} mod={} ctrl={} shift={} alt={}",
        key,
        text,
        mod_pressed,
        ctrl_pressed,
        shift_pressed,
        alt_pressed
    );

    let mut input_sent = false;

    // Handle Home/End keys explicitly using readline's native bindings
    // Ctrl-A (0x01) = beginning of line, Ctrl-E (0x05) = end of line
    // These work universally in bash, zsh, and other readline-based shells
    match key {
        Key::Named(NamedKey::Home) if !shift_pressed => {
            shell.send_input(b"\x01"); // Ctrl-A = beginning of line
            input_sent = true;
        }
        Key::Named(NamedKey::End) if !shift_pressed => {
            shell.send_input(b"\x05"); // Ctrl-E = end of line
            input_sent = true;
        }
        _ => {}
    }

    // macOS-specific word/line navigation shortcuts (Option+Arrow, Cmd+Arrow, Option+Backspace)
    // These override standard encoding because macOS users expect this behavior
    #[cfg(target_os = "macos")]
    if !input_sent {
        match key {
            Key::Named(NamedKey::Backspace) if alt_pressed => {
                shell.send_input(b"\x1b\x7f"); // ESC DEL = delete word backward
                input_sent = true;
            }
            // Cmd+Arrow = Home/End (same as Home/End keys above)
            // Use readline bindings for universal shell compatibility
            Key::Named(NamedKey::ArrowRight) if mod_pressed && !shift_pressed => {
                shell.send_input(b"\x05"); // Ctrl-E = end of line
                input_sent = true;
            }
            Key::Named(NamedKey::ArrowLeft) if mod_pressed && !shift_pressed => {
                shell.send_input(b"\x01"); // Ctrl-A = beginning of line
                input_sent = true;
            }
            // Option+Arrow = word navigation
            Key::Named(NamedKey::ArrowRight) if alt_pressed => {
                shell.send_input(b"\x1bf"); // ESC f = forward word
                input_sent = true;
            }
            Key::Named(NamedKey::ArrowLeft) if alt_pressed => {
                shell.send_input(b"\x1bb"); // ESC b = backward word
                input_sent = true;
            }
            _ => {}
        }
    }

    // If not handled by platform-specific code, use termwiz encoding
    if !input_sent {
        // Don't send Cmd+key combinations to terminal (they're app shortcuts)
        if !mod_pressed
            && let Some(bytes) = encode_key(key, ctrl_pressed, shift_pressed, alt_pressed)
        {
            shell.send_input(&bytes);
            input_sent = true;
            log::debug!("Sent via termwiz: {:?}", bytes);
        }
    }

    // Final fallback: use the text field from the key event
    // This catches any keys termwiz doesn't handle
    if !input_sent
        && let Some(t) = text
        && !t.is_empty()
        && !mod_pressed
    {
        shell.send_input(t.as_bytes());
        input_sent = true;
        log::debug!("Forwarded via text field: {:?}", t);
    }

    if input_sent {
        // Scroll to bottom when user types (show live output)
        if shell.is_scrolled_back() {
            shell.scroll_to_bottom();
        }
        // Always invalidate content hash when input is sent to ensure re-render
        // even if PTY output hasn't arrived yet (handles TUI apps like Claude Code)
        state.content_hashes.insert(tab_id, 0);
        state.render.dirty = true;
        state.window.request_redraw();
    }

    input_sent
}

/// Handle mouse click on tab bar
pub fn handle_tab_click(state: &mut WindowState, x: f32, y: f32, now: std::time::Instant) -> bool {
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
    } else if let Some((tab_id, is_close)) = state.gpu.tab_bar.hit_test(x, y) {
        if is_close {
            if state.gpu.tab_bar.tab_count() > 1 {
                state.gpu.tab_bar.close_tab(tab_id);
                tab_closed = Some(tab_id);
                tab_switched = true;
            }
        } else {
            let is_double_click = state
                .interaction
                .last_click_time
                .map(|t| now.duration_since(t) < double_click_threshold)
                .unwrap_or(false)
                && state.interaction.last_click_tab == Some(tab_id);

            if is_double_click {
                // Cancel window rename if active
                if state.ui.window_rename.active {
                    state.ui.window_rename.cancel();
                }
                state.gpu.tab_bar.start_editing(tab_id);
                started_editing = true;
                state.interaction.last_click_time = None;
                state.interaction.last_click_tab = None;
            } else {
                state.gpu.tab_bar.select_tab(tab_id);
                tab_switched = true;
                state.interaction.last_click_time = Some(now);
                state.interaction.last_click_tab = Some(tab_id);
            }
        }
    } else {
        state.interaction.last_click_time = None;
        state.interaction.last_click_tab = None;
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
    state
        .gpu
        .surface
        .configure(&shared.device, &state.gpu.config);

    // Recreate text texture for glow effect from pool (old texture returns to pool)
    let text_texture =
        match shared
            .texture_pool
            .checkout(new_width, new_height, state.gpu.config.format)
        {
            Some(t) => t,
            None => {
                log::error!(
                    "Failed to checkout texture from pool during resize - skipping texture update"
                );
                return;
            }
        };
    let composite_bind_group = state
        .gpu
        .effect_pipeline
        .composite
        .create_bind_group(&shared.device, text_texture.view());
    state.gpu.text_texture = text_texture;
    state.gpu.composite_bind_group = composite_bind_group;

    state
        .gpu
        .grid_renderer
        .update_screen_size(&shared.queue, new_width as f32, new_height as f32);
    state.gpu.output_grid_renderer.update_screen_size(
        &shared.queue,
        new_width as f32,
        new_height as f32,
    );
    state.gpu.tab_title_renderer.update_screen_size(
        &shared.queue,
        new_width as f32,
        new_height as f32,
    );

    state
        .gpu
        .tab_bar
        .resize(new_width as f32, new_height as f32);

    state.render.dirty = true;
    for hash in state.content_hashes.values_mut() {
        *hash = 0;
    }
    state.window.request_redraw();
}

/// Convert screen coordinates to terminal cell (column, line)
/// Returns None if the position is outside the terminal area
pub fn screen_to_cell(state: &WindowState, x: f32, y: f32) -> Option<(usize, usize)> {
    let (offset_x, offset_y) = state.gpu.tab_bar.content_offset();
    let padding = 10.0 * state.scale_factor;
    let cell_width = state.gpu.glyph_cache.cell_width();
    let line_height = state.gpu.glyph_cache.line_height();

    // Check if in terminal area
    let content_x = x - offset_x - padding;
    let content_y = y - offset_y - padding;

    if content_x < 0.0 || content_y < 0.0 {
        return None;
    }

    let col = (content_x / cell_width) as usize;
    let line = (content_y / line_height) as usize;

    // Clamp to terminal bounds
    let col = col.min(state.cols.saturating_sub(1));
    let line = line.min(state.rows.saturating_sub(1));

    Some((col, line))
}

/// Handle mouse press for terminal selection or mouse reporting
/// Returns true if the press was handled (was in terminal area)
#[allow(dead_code)]
pub fn handle_terminal_mouse_press(state: &mut WindowState, x: f32, y: f32, now: Instant) -> bool {
    handle_terminal_mouse_button(state, x, y, now, MOUSE_BUTTON_LEFT, true)
}

/// Handle mouse button press/release for any button
/// Returns true if the event was handled (was in terminal area)
pub fn handle_terminal_mouse_button(
    state: &mut WindowState,
    x: f32,
    y: f32,
    now: Instant,
    button: u8,
    pressed: bool,
) -> bool {
    // Check if click is in tab bar area first
    let tab_bar_height = state.gpu.tab_bar.height() * state.scale_factor;
    if y < tab_bar_height {
        return false; // Let tab bar handle it
    }

    let Some((col, line)) = screen_to_cell(state, x, y) else {
        return false;
    };

    // Get the active shell
    let tab_id = state.gpu.tab_bar.active_tab_id();
    let Some(tab_id) = tab_id else { return false };
    let Some(shell) = state.shells.get_mut(&tab_id) else {
        return false;
    };

    // Check if we should report mouse events to the terminal
    if should_report_mouse(shell) {
        let sgr = is_sgr_mouse_mode(shell);
        let report_button = if !pressed && !sgr {
            // In legacy mode, release is button 3
            MOUSE_BUTTON_RELEASE
        } else {
            button
        };
        let seq = mouse_report(report_button, col, line, pressed, sgr);
        shell.send_input(&seq);

        // Track button state for drag reporting
        if button == MOUSE_BUTTON_LEFT {
            state.interaction.mouse_pressed = pressed;
        }

        state.render.dirty = true;
        state.window.request_redraw();
        return true;
    }

    // Local selection handling (mouse mode not enabled)
    if button != MOUSE_BUTTON_LEFT {
        return false; // Only left button for selection
    }

    if pressed {
        // Determine click count for multi-click selection
        let click_count = if let (Some(last_time), Some((last_col, last_line))) = (
            state.interaction.last_selection_click_time,
            state.interaction.last_selection_click_pos,
        ) {
            let time_ok = now.duration_since(last_time) < MULTI_CLICK_THRESHOLD;
            let pos_ok = col.abs_diff(last_col) <= MULTI_CLICK_DISTANCE
                && line.abs_diff(last_line) <= MULTI_CLICK_DISTANCE;

            if time_ok && pos_ok {
                (state.interaction.selection_click_count % 3) + 1
            } else {
                1
            }
        } else {
            1
        };

        state.interaction.selection_click_count = click_count;
        state.interaction.last_selection_click_time = Some(now);
        state.interaction.last_selection_click_pos = Some((col, line));
        state.interaction.mouse_pressed = true;

        // Convert viewport coordinates to grid coordinates
        // Grid coordinates: negative = scrollback history, 0+ = visible screen
        // Viewport coordinates: 0 = top of visible area
        // When scrolled back, display_offset > 0, so grid_line = viewport_line - display_offset
        let display_offset = shell.terminal().display_offset() as i32;
        let grid_line = line as i32 - display_offset;
        let point = Point::new(Line(grid_line), Column(col));

        // Selection type based on click count
        let selection_type = match click_count {
            1 => SelectionType::Simple,
            2 => SelectionType::Semantic, // Word selection
            3 => SelectionType::Lines,    // Line selection
            _ => SelectionType::Simple,
        };

        shell.start_selection(point, selection_type);

        // For semantic and lines selection, also set the end point immediately
        // to show the full word/line
        if click_count > 1 {
            shell.update_selection(point);
        }
    } else {
        state.interaction.mouse_pressed = false;
    }

    state.render.dirty = true;
    state.window.request_redraw();
    true
}

/// Handle mouse move for terminal selection (dragging) or mouse motion reporting
pub fn handle_terminal_mouse_move(state: &mut WindowState, x: f32, y: f32) {
    let Some((col, line)) = screen_to_cell(state, x, y) else {
        return;
    };

    let tab_id = state.gpu.tab_bar.active_tab_id();
    let Some(tab_id) = tab_id else { return };
    let Some(shell) = state.shells.get_mut(&tab_id) else {
        return;
    };

    // Check if we should report motion to terminal
    if should_report_motion(shell, state.interaction.mouse_pressed) {
        let sgr = is_sgr_mouse_mode(shell);
        // Button code: 32 + button for motion with button, or just 35 for motion without
        let button = if state.interaction.mouse_pressed {
            MOUSE_BUTTON_MOTION + MOUSE_BUTTON_LEFT // 32 = motion with left button
        } else {
            MOUSE_BUTTON_MOTION + MOUSE_BUTTON_RELEASE // 35 = motion without button
        };
        let seq = mouse_report(button, col, line, true, sgr);
        shell.send_input(&seq);

        state.render.dirty = true;
        state.window.request_redraw();
        return;
    }

    // Local selection handling
    if !state.interaction.mouse_pressed {
        return;
    }

    // Convert viewport coordinates to grid coordinates (same as in mouse button handler)
    let display_offset = shell.terminal().display_offset() as i32;
    let grid_line = line as i32 - display_offset;
    let point = Point::new(Line(grid_line), Column(col));
    shell.update_selection(point);

    state.render.dirty = true;
    state.window.request_redraw();
}

/// Handle mouse release for terminal selection or mouse reporting
pub fn handle_terminal_mouse_release(state: &mut WindowState, x: f32, y: f32) {
    let Some((col, line)) = screen_to_cell(state, x, y) else {
        state.interaction.mouse_pressed = false;
        return;
    };

    let tab_id = state.gpu.tab_bar.active_tab_id();
    let Some(tab_id) = tab_id else {
        state.interaction.mouse_pressed = false;
        return;
    };
    let Some(shell) = state.shells.get_mut(&tab_id) else {
        state.interaction.mouse_pressed = false;
        return;
    };

    // Check if we should report the release to terminal
    if should_report_mouse(shell) {
        let sgr = is_sgr_mouse_mode(shell);
        let button = if sgr {
            MOUSE_BUTTON_LEFT // SGR mode sends button with release suffix 'm'
        } else {
            MOUSE_BUTTON_RELEASE // Legacy mode sends button 3 for release
        };
        let seq = mouse_report(button, col, line, false, sgr);
        shell.send_input(&seq);
    }

    state.interaction.mouse_pressed = false;
    // Selection remains if we were doing local selection - user can copy with Cmd+C
}

/// Handle mouse scroll wheel for terminal scrollback or mouse reporting
/// Returns true if the scroll was handled by mouse reporting
pub fn handle_terminal_scroll(state: &mut WindowState, x: f32, y: f32, delta_y: f32) -> bool {
    let Some((col, line)) = screen_to_cell(state, x, y) else {
        return false;
    };

    let tab_id = state.gpu.tab_bar.active_tab_id();
    let Some(tab_id) = tab_id else { return false };
    let Some(shell) = state.shells.get_mut(&tab_id) else {
        return false;
    };

    // Check if we should report scroll to terminal
    if should_report_mouse(shell) {
        let sgr = is_sgr_mouse_mode(shell);
        // Scroll up = 64, scroll down = 65
        let button = if delta_y > 0.0 {
            MOUSE_BUTTON_SCROLL_UP
        } else {
            MOUSE_BUTTON_SCROLL_DOWN
        };
        let seq = mouse_report(button, col, line, true, sgr);
        shell.send_input(&seq);

        state.render.dirty = true;
        state.window.request_redraw();
        return true;
    }

    false
}

/// Clear terminal selection (e.g., when user types or presses Escape)
pub fn clear_terminal_selection(state: &mut WindowState) {
    let tab_id = state.gpu.tab_bar.active_tab_id();
    let Some(tab_id) = tab_id else { return };
    let Some(shell) = state.shells.get_mut(&tab_id) else {
        return;
    };

    if shell.has_selection() {
        shell.clear_selection();
        state.render.dirty = true;
        state.window.request_redraw();
    }
}

/// Get selected text from terminal (for copy)
pub fn get_terminal_selection_text(state: &WindowState) -> Option<String> {
    let tab_id = state.gpu.tab_bar.active_tab_id()?;
    let shell = state.shells.get(&tab_id)?;
    shell.selection_to_string()
}

/// Get clipboard content from system clipboard
///
/// Handles three types of clipboard content:
/// 1. Text - returns the text directly
/// 2. Files (copied from Finder/Explorer) - returns the file path(s)
/// 3. Images (screenshots) - saves to temp file and returns the path
///
/// This allows pasting images into applications like Claude Code that accept file paths.
pub fn get_clipboard_content() -> Option<String> {
    let mut clipboard = arboard::Clipboard::new().ok()?;

    // First try to get text (most common case)
    if let Ok(text) = clipboard.get_text()
        && !text.is_empty()
    {
        return Some(text);
    }

    // Try to get file paths (files copied from Finder/Explorer)
    if let Ok(files) = clipboard_files::read()
        && !files.is_empty()
    {
        // Join multiple paths with spaces, quoting paths that contain spaces
        let paths: Vec<String> = files
            .into_iter()
            .map(|p| {
                let path_str = p.to_string_lossy().to_string();
                if path_str.contains(' ') {
                    format!("'{}'", path_str)
                } else {
                    path_str
                }
            })
            .collect();
        return Some(paths.join(" "));
    }

    // Try to get image data (screenshots, copied images)
    if let Ok(image_data) = clipboard.get_image()
        && let Some(path) = save_clipboard_image_to_temp(&image_data)
    {
        return Some(path);
    }

    None
}

/// Save clipboard image data to a temporary file and return the path
fn save_clipboard_image_to_temp(image_data: &arboard::ImageData) -> Option<String> {
    use image::ImageEncoder;
    use std::io::Write;

    // Create temp directory if it doesn't exist
    let temp_dir = std::env::temp_dir().join("crt_clipboard");
    std::fs::create_dir_all(&temp_dir).ok()?;

    // Generate unique filename with timestamp
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_millis();
    let filename = format!("clipboard_{}.png", timestamp);
    let path = temp_dir.join(&filename);

    // Convert RGBA bytes to PNG using the image crate
    let img = image::RgbaImage::from_raw(
        image_data.width as u32,
        image_data.height as u32,
        image_data.bytes.to_vec(),
    )?;

    // Save as PNG
    let mut file = std::fs::File::create(&path).ok()?;
    let encoder = image::codecs::png::PngEncoder::new(&mut file);
    encoder
        .write_image(
            &img,
            image_data.width as u32,
            image_data.height as u32,
            image::ExtendedColorType::Rgba8,
        )
        .ok()?;
    file.flush().ok()?;

    Some(path.to_string_lossy().to_string())
}

/// Set clipboard content
pub fn set_clipboard_content(text: &str) {
    if let Ok(mut clipboard) = arboard::Clipboard::new() {
        let _ = clipboard.set_text(text.to_string());
    }
}

/// Paste content to terminal with bracketed paste mode support
///
/// If the terminal has bracketed paste mode enabled, the content will be
/// wrapped with escape sequences to indicate a paste operation.
pub fn paste_to_terminal(state: &mut WindowState, content: &str) {
    let tab_id = state.gpu.tab_bar.active_tab_id();
    let Some(tab_id) = tab_id else { return };
    let Some(shell) = state.shells.get_mut(&tab_id) else {
        return;
    };

    // Check if bracketed paste mode is enabled
    let bracketed = shell.bracketed_paste_enabled();

    if bracketed {
        // Bracketed paste mode: wrap with escape sequences
        shell.send_input(b"\x1b[200~");
        shell.send_input(content.as_bytes());
        shell.send_input(b"\x1b[201~");
    } else {
        shell.send_input(content.as_bytes());
    }

    // Scroll to bottom and clear selection
    if shell.is_scrolled_back() {
        shell.scroll_to_bottom();
        state.content_hashes.insert(tab_id, 0);
    }
    clear_terminal_selection(state);

    state.render.dirty = true;
    state.window.request_redraw();
}

/// Scroll terminal to make current search match visible
pub fn scroll_to_current_match(state: &mut WindowState) {
    use crt_core::Scroll;

    if state.ui.search.matches.is_empty() {
        return;
    }

    let current_match = &state.ui.search.matches[state.ui.search.current_match];
    let match_line = current_match.line;

    // Get active shell
    let active_tab_id = state.gpu.tab_bar.active_tab_id();
    let shell = active_tab_id.and_then(|id| state.shells.get_mut(&id));
    let Some(shell) = shell else { return };

    let terminal = shell.terminal();
    let screen_lines = terminal.screen_lines() as i32;
    let display_offset = terminal.display_offset() as i32;

    // Calculate viewport line (what line would the match be at in current viewport)
    // viewport_line = grid_line + display_offset
    let viewport_line = match_line + display_offset;

    // If match is outside visible range (0 to screen_lines-1), scroll to center it
    if viewport_line < 0 || viewport_line >= screen_lines {
        // Target: put match roughly in the middle of the screen
        let target_viewport_line = screen_lines / 2;
        // New display_offset needed: match_line + new_offset = target_viewport_line
        // new_offset = target_viewport_line - match_line
        let new_offset = target_viewport_line - match_line;

        // The scroll delta is the change in display_offset
        // Positive delta scrolls up (increases display_offset)
        let scroll_delta = new_offset - display_offset;

        if scroll_delta != 0 {
            shell.scroll(Scroll::Delta(scroll_delta));
            if let Some(tab_id) = active_tab_id {
                state.content_hashes.insert(tab_id, 0);
            }
        }
    }
}

/// Update search matches based on current query
pub fn update_search_matches(state: &mut WindowState) {
    use crate::window::SearchMatch;

    state.ui.search.matches.clear();
    state.ui.search.current_match = 0;

    let query = &state.ui.search.query;
    if query.is_empty() {
        return;
    }

    // Get active shell's terminal content
    let active_tab_id = state.gpu.tab_bar.active_tab_id();
    let shell = active_tab_id.and_then(|id| state.shells.get(&id));
    let Some(shell) = shell else { return };

    let terminal = shell.terminal();

    // Get all lines including history
    let all_lines = terminal.all_lines_text();

    // Search each line for the query (case-insensitive)
    let query_lower = query.to_lowercase();
    for (line_idx, line_text) in &all_lines {
        let line_lower = line_text.to_lowercase();
        let mut start = 0;
        while let Some(pos) = line_lower[start..].find(&query_lower) {
            let match_start = start + pos;
            state.ui.search.matches.push(SearchMatch {
                line: *line_idx,
                start_col: match_start,
                end_col: match_start + query.len(),
            });
            start = match_start + 1;
        }
    }

    // Scroll to first match if any found
    if !state.ui.search.matches.is_empty() {
        scroll_to_current_match(state);
    }
}
