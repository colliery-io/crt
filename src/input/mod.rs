//! Input handling
//!
//! Keyboard and mouse input processing for terminal and tab bar.

mod key_encoder;

pub use key_encoder::encode_key;

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
        key, text, mod_pressed, ctrl_pressed, shift_pressed, alt_pressed
    );

    let mut input_sent = false;

    // macOS-specific word navigation shortcuts (Option+Arrow, Cmd+Arrow, Option+Backspace)
    // These override standard encoding because macOS users expect this behavior
    #[cfg(target_os = "macos")]
    {
        match key {
            Key::Named(NamedKey::Backspace) if alt_pressed => {
                shell.send_input(b"\x1b\x7f"); // ESC DEL = delete word backward
                input_sent = true;
            }
            Key::Named(NamedKey::ArrowRight) if mod_pressed => {
                shell.send_input(b"\x1b[F"); // End of line
                input_sent = true;
            }
            Key::Named(NamedKey::ArrowRight) if alt_pressed => {
                shell.send_input(b"\x1bf"); // ESC f = forward word
                input_sent = true;
            }
            Key::Named(NamedKey::ArrowLeft) if mod_pressed => {
                shell.send_input(b"\x1b[H"); // Beginning of line
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
        if !mod_pressed {
            if let Some(bytes) = encode_key(key, ctrl_pressed, shift_pressed, alt_pressed) {
                shell.send_input(&bytes);
                input_sent = true;
                log::debug!("Sent via termwiz: {:?}", bytes);
            }
        }
    }

    // Final fallback: use the text field from the key event
    // This catches any keys termwiz doesn't handle
    if !input_sent {
        if let Some(t) = text {
            if !t.is_empty() && !mod_pressed {
                shell.send_input(t.as_bytes());
                input_sent = true;
                log::debug!("Forwarded via text field: {:?}", t);
            }
        }
    }

    if input_sent {
        // Scroll to bottom when user types (show live output)
        if shell.is_scrolled_back() {
            shell.scroll_to_bottom();
        }
        // Always invalidate content hash when input is sent to ensure re-render
        // even if PTY output hasn't arrived yet (handles TUI apps like Claude Code)
        state.content_hashes.insert(tab_id, 0);
        state.dirty = true;
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
    } else {
        if let Some((tab_id, is_close)) = state.gpu.tab_bar.hit_test(x, y) {
            if is_close {
                if state.gpu.tab_bar.tab_count() > 1 {
                    state.gpu.tab_bar.close_tab(tab_id);
                    tab_closed = Some(tab_id);
                    tab_switched = true;
                }
            } else {
                let is_double_click = state
                    .last_click_time
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
    state
        .gpu
        .surface
        .configure(&shared.device, &state.gpu.config);

    // Recreate text texture for glow effect from pool (old texture returns to pool)
    let text_texture = shared.texture_pool.checkout(new_width, new_height, state.gpu.config.format);
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

    state.dirty = true;
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
            state.mouse_pressed = pressed;
        }

        state.dirty = true;
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
            state.last_selection_click_time,
            state.last_selection_click_pos,
        ) {
            let time_ok = now.duration_since(last_time) < MULTI_CLICK_THRESHOLD;
            let pos_ok = col.abs_diff(last_col) <= MULTI_CLICK_DISTANCE
                && line.abs_diff(last_line) <= MULTI_CLICK_DISTANCE;

            if time_ok && pos_ok {
                (state.selection_click_count % 3) + 1
            } else {
                1
            }
        } else {
            1
        };

        state.selection_click_count = click_count;
        state.last_selection_click_time = Some(now);
        state.last_selection_click_pos = Some((col, line));
        state.mouse_pressed = true;

        let point = Point::new(Line(line as i32), Column(col));

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
        state.mouse_pressed = false;
    }

    state.dirty = true;
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
    if should_report_motion(shell, state.mouse_pressed) {
        let sgr = is_sgr_mouse_mode(shell);
        // Button code: 32 + button for motion with button, or just 35 for motion without
        let button = if state.mouse_pressed {
            MOUSE_BUTTON_MOTION + MOUSE_BUTTON_LEFT // 32 = motion with left button
        } else {
            MOUSE_BUTTON_MOTION + MOUSE_BUTTON_RELEASE // 35 = motion without button
        };
        let seq = mouse_report(button, col, line, true, sgr);
        shell.send_input(&seq);

        state.dirty = true;
        state.window.request_redraw();
        return;
    }

    // Local selection handling
    if !state.mouse_pressed {
        return;
    }

    let point = Point::new(Line(line as i32), Column(col));
    shell.update_selection(point);

    state.dirty = true;
    state.window.request_redraw();
}

/// Handle mouse release for terminal selection or mouse reporting
pub fn handle_terminal_mouse_release(state: &mut WindowState, x: f32, y: f32) {
    let Some((col, line)) = screen_to_cell(state, x, y) else {
        state.mouse_pressed = false;
        return;
    };

    let tab_id = state.gpu.tab_bar.active_tab_id();
    let Some(tab_id) = tab_id else {
        state.mouse_pressed = false;
        return;
    };
    let Some(shell) = state.shells.get_mut(&tab_id) else {
        state.mouse_pressed = false;
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

    state.mouse_pressed = false;
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

        state.dirty = true;
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
        state.dirty = true;
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
pub fn get_clipboard_content() -> Option<String> {
    arboard::Clipboard::new().ok()?.get_text().ok()
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

    state.dirty = true;
    state.window.request_redraw();
}
