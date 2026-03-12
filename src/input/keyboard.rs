//! Keyboard input handling
//!
//! Extracts keyboard event handling logic from main.rs for better modularity.
//! Returns actions that main.rs applies, keeping ownership/lifetime concerns there.

use crt_core::Scroll;
use winit::event::Modifiers;
use winit::keyboard::{Key, NamedKey};

use crate::window::{TabId, WindowState};

use super::{
    TabEditResult, clear_terminal_selection, get_clipboard_content, get_terminal_selection_text,
    handle_shell_input, handle_tab_editing, paste_to_terminal, set_clipboard_content,
};

/// Result of keyboard event handling
#[derive(Debug)]
#[allow(dead_code)] // Variants exist for API completeness
pub enum KeyboardAction {
    /// No action needed (event fully handled)
    Handled,
    /// Event was not handled, continue default processing
    NotHandled,
    /// Close the current window
    CloseWindow,
    /// Close a specific tab
    CloseTab(TabId),
    /// Request a new window
    NewWindow,
    /// Request a new tab (main.rs handles creation with spawn options)
    NewTab,
    /// Quit the application
    Quit,
    /// Scroll the terminal
    Scroll(Scroll),
    /// Copy selection to clipboard
    Copy,
    /// Paste from clipboard
    Paste,
    /// Toggle search mode
    ToggleSearch,
    /// Navigate to next/previous search match (true = previous)
    SearchNavigate { reverse: bool },
    /// Switch to previous tab
    PrevTab,
    /// Switch to next tab
    NextTab,
    /// Select a specific tab by index (0-based)
    SelectTab(usize),
}

/// Read-only context for keyboard action determination.
///
/// Captures the minimal state needed to decide which action a key combination
/// should produce, without requiring access to the full `WindowState`.
pub struct InputContext {
    /// Whether the context menu is currently visible
    pub context_menu_visible: bool,
    /// Whether a tab is being renamed
    pub tab_editing_active: bool,
    /// Whether the window rename dialog is active
    pub window_rename_active: bool,
    /// Whether search mode is active
    pub search_active: bool,
    /// Number of search matches (for navigation decisions)
    pub search_match_count: usize,
    /// Number of open tabs
    pub tab_count: usize,
    /// Active tab ID (if any)
    pub active_tab_id: Option<TabId>,
}

impl InputContext {
    /// Extract input context from window state
    pub fn from_state(state: &WindowState) -> Self {
        Self {
            context_menu_visible: state.ui.context_menu.visible,
            tab_editing_active: state.gpu.tab_bar.is_editing(),
            window_rename_active: state.ui.window_rename.active,
            search_active: state.ui.search.active,
            search_match_count: state.ui.search.matches.len(),
            tab_count: state.gpu.tab_bar.tab_count(),
            active_tab_id: state.gpu.tab_bar.active_tab_id(),
        }
    }
}

/// Determine the keyboard action for a command shortcut (Cmd/Ctrl + key).
///
/// Pure function — no side effects, no state mutation. Returns `None` if the
/// key combination is not a recognized shortcut.
pub fn determine_command_shortcut(
    key: &Key,
    shift_pressed: bool,
    ctx: &InputContext,
) -> Option<KeyboardAction> {
    match key {
        Key::Character(c) if c.as_str() == "c" => Some(KeyboardAction::Copy),
        Key::Character(c) if c.as_str() == "v" => Some(KeyboardAction::Paste),
        Key::Character(c) if c.as_str() == "q" => Some(KeyboardAction::Quit),
        Key::Character(c) if c.as_str() == "w" => {
            if ctx.tab_count > 1 {
                if let Some(tab_id) = ctx.active_tab_id {
                    return Some(KeyboardAction::CloseTab(tab_id));
                }
            }
            Some(KeyboardAction::CloseWindow)
        }
        Key::Character(c) if c.as_str() == "n" => Some(KeyboardAction::NewWindow),
        Key::Character(c) if c.as_str() == "t" => Some(KeyboardAction::NewTab),
        Key::Character(c) if c.as_str() == "f" => Some(KeyboardAction::ToggleSearch),
        Key::Character(c) if c.as_str() == "g" => {
            if ctx.search_active && ctx.search_match_count > 0 {
                Some(KeyboardAction::SearchNavigate {
                    reverse: shift_pressed,
                })
            } else {
                Some(KeyboardAction::Handled)
            }
        }
        Key::Character(c) if c.as_str() == "[" && shift_pressed => Some(KeyboardAction::PrevTab),
        Key::Character(c) if c.as_str() == "]" && shift_pressed => Some(KeyboardAction::NextTab),
        Key::Character(c) if c.len() == 1 => {
            if let Some(digit) = c.chars().next().and_then(|ch| ch.to_digit(10))
                && (1..=9).contains(&digit)
            {
                return Some(KeyboardAction::SelectTab((digit - 1) as usize));
            }
            None
        }
        _ => None,
    }
}

/// Determine the scroll action for a key combination.
///
/// Pure function — returns the scroll action if the key is a scroll shortcut.
pub fn determine_scroll_action(
    key: &Key,
    mod_pressed: bool,
    shift_pressed: bool,
) -> Option<Scroll> {
    if !shift_pressed {
        return None;
    }

    match key {
        Key::Named(NamedKey::PageUp) if !mod_pressed => Some(Scroll::PageUp),
        Key::Named(NamedKey::PageDown) if !mod_pressed => Some(Scroll::PageDown),
        Key::Named(NamedKey::Home) if !mod_pressed => Some(Scroll::Top),
        Key::Named(NamedKey::End) if !mod_pressed => Some(Scroll::Bottom),
        #[cfg(target_os = "macos")]
        Key::Named(NamedKey::ArrowLeft) if mod_pressed => Some(Scroll::Top),
        #[cfg(target_os = "macos")]
        Key::Named(NamedKey::ArrowRight) if mod_pressed => Some(Scroll::Bottom),
        _ => None,
    }
}

/// Handle keyboard input event
///
/// Returns the action that main.rs should take, if any.
pub fn handle_keyboard_input(
    state: &mut WindowState,
    key: &Key,
    text: Option<&str>,
    modifiers: &Modifiers,
) -> KeyboardAction {
    #[cfg(target_os = "macos")]
    let mod_pressed = modifiers.state().super_key();
    #[cfg(not(target_os = "macos"))]
    let mod_pressed = modifiers.state().control_key();

    let shift_pressed = modifiers.state().shift_key();
    let ctrl_pressed = modifiers.state().control_key();
    let alt_pressed = modifiers.state().alt_key();

    // Handle scroll shortcuts (Shift+PageUp/PageDown/Home/End)
    if let Some(action) = handle_scroll_shortcuts(state, key, mod_pressed, shift_pressed) {
        return action;
    }

    // Handle context menu keyboard navigation
    if state.ui.context_menu.visible {
        match key {
            Key::Named(NamedKey::Escape) => {
                state.ui.context_menu.hide();
                state.render.dirty = true;
                state.window.request_redraw();
                return KeyboardAction::Handled;
            }
            Key::Named(NamedKey::ArrowDown) => {
                state.ui.context_menu.focus_next();
                state.render.dirty = true;
                state.window.request_redraw();
                return KeyboardAction::Handled;
            }
            Key::Named(NamedKey::ArrowUp) => {
                state.ui.context_menu.focus_prev();
                state.render.dirty = true;
                state.window.request_redraw();
                return KeyboardAction::Handled;
            }
            Key::Named(NamedKey::Enter) => {
                if let Some(item) = state.ui.context_menu.get_focused_item() {
                    super::mouse::handle_context_menu_action(state, item);
                    state.ui.context_menu.hide();
                    state.render.dirty = true;
                    state.window.request_redraw();
                }
                return KeyboardAction::Handled;
            }
            _ => {}
        }
    }

    // Handle tab editing
    if let TabEditResult::Handled = handle_tab_editing(state, key, mod_pressed) {
        return KeyboardAction::Handled;
    }

    // Handle window rename input
    if state.ui.window_rename.active {
        match key {
            Key::Named(NamedKey::Escape) => {
                state.ui.window_rename.cancel();
                state.render.dirty = true;
                state.window.request_redraw();
                return KeyboardAction::Handled;
            }
            Key::Named(NamedKey::Enter) => {
                if let Some(new_title) = state.ui.window_rename.confirm() {
                    state.custom_title = Some(new_title.clone());
                    state.window.set_title(&new_title);
                } else {
                    // Empty = reset to default
                    state.custom_title = None;
                    state.window.set_title("CRT Terminal");
                }
                state.render.dirty = true;
                state.window.request_redraw();
                return KeyboardAction::Handled;
            }
            Key::Named(NamedKey::Backspace) => {
                state.ui.window_rename.input.pop();
                state.render.dirty = true;
                state.window.request_redraw();
                return KeyboardAction::Handled;
            }
            Key::Character(c) => {
                for ch in c.chars() {
                    if !ch.is_control() {
                        state.ui.window_rename.input.push(ch);
                    }
                }
                state.render.dirty = true;
                state.window.request_redraw();
                return KeyboardAction::Handled;
            }
            Key::Named(NamedKey::Space) => {
                state.ui.window_rename.input.push(' ');
                state.render.dirty = true;
                state.window.request_redraw();
                return KeyboardAction::Handled;
            }
            _ => return KeyboardAction::Handled, // Consume all keys in rename mode
        }
    }

    // Handle search input when search is active
    if let Some(action) = handle_search_input(state, key, mod_pressed) {
        return action;
    }

    // Handle keyboard shortcuts (Cmd/Ctrl + key)
    if mod_pressed && let Some(action) = handle_command_shortcuts(state, key, shift_pressed) {
        return action;
    }

    // Send to shell (clears selection on input)
    if handle_shell_input(
        state,
        key,
        text,
        mod_pressed,
        ctrl_pressed,
        shift_pressed,
        alt_pressed,
    ) {
        clear_terminal_selection(state);
    }

    KeyboardAction::Handled
}

/// Handle scroll shortcuts (Shift+PageUp/PageDown/Home/End)
fn handle_scroll_shortcuts(
    state: &mut WindowState,
    key: &Key,
    mod_pressed: bool,
    shift_pressed: bool,
) -> Option<KeyboardAction> {
    let scroll = determine_scroll_action(key, mod_pressed, shift_pressed)?;

    let tab_id = state.gpu.tab_bar.active_tab_id();
    if let Some(tab_id) = tab_id
        && let Some(shell) = state.shells.get_mut(&tab_id)
    {
        shell.scroll(scroll);
        state.render.dirty = true;
        state.content_hashes.insert(tab_id, 0);
        state.window.request_redraw();
    }
    Some(KeyboardAction::Handled)
}

/// Handle search mode input
fn handle_search_input(
    state: &mut WindowState,
    key: &Key,
    mod_pressed: bool,
) -> Option<KeyboardAction> {
    if !state.ui.search.active {
        return None;
    }

    match key {
        Key::Named(NamedKey::Escape) => {
            // Close search
            state.ui.search.active = false;
            state.ui.search.query.clear();
            state.ui.search.matches.clear();
            state.ui.search.current_match = 0;
            state.force_active_tab_redraw();
            state.window.request_redraw();
            Some(KeyboardAction::Handled)
        }
        Key::Named(NamedKey::Enter) => {
            // Next match on Enter
            if !state.ui.search.matches.is_empty() {
                state.ui.search.current_match =
                    (state.ui.search.current_match + 1) % state.ui.search.matches.len();
                super::scroll_to_current_match(state);
                state.force_active_tab_redraw();
                state.window.request_redraw();
            }
            Some(KeyboardAction::Handled)
        }
        Key::Named(NamedKey::Backspace) => {
            // Delete last char from query
            state.ui.search.query.pop();
            super::update_search_matches(state);
            state.force_active_tab_redraw();
            state.window.request_redraw();
            Some(KeyboardAction::Handled)
        }
        Key::Character(c) if !mod_pressed => {
            // Add character to query
            state.ui.search.query.push_str(c.as_str());
            super::update_search_matches(state);
            state.force_active_tab_redraw();
            state.window.request_redraw();
            Some(KeyboardAction::Handled)
        }
        _ => None,
    }
}

/// Handle command shortcuts (Cmd/Ctrl + key)
fn handle_command_shortcuts(
    state: &mut WindowState,
    key: &Key,
    shift_pressed: bool,
) -> Option<KeyboardAction> {
    // Confirm any tab editing in progress
    if state.gpu.tab_bar.is_editing() {
        state.gpu.tab_bar.confirm_editing();
        state.render.dirty = true;
    }

    let ctx = InputContext::from_state(state);
    let action = determine_command_shortcut(key, shift_pressed, &ctx)?;

    // Apply side effects for actions that need them
    match &action {
        KeyboardAction::Copy => {
            if let Some(text) = get_terminal_selection_text(state) {
                set_clipboard_content(&text);
                state.ui.copy_indicator.trigger();
            }
            return Some(KeyboardAction::Handled);
        }
        KeyboardAction::Paste => {
            if let Some(content) = get_clipboard_content() {
                paste_to_terminal(state, &content);
            }
            return Some(KeyboardAction::Handled);
        }
        KeyboardAction::CloseTab(tab_id) => {
            let tab_id = *tab_id;
            state.gpu.tab_bar.close_tab(tab_id);
            state.remove_shell_for_tab(tab_id);
            state.force_active_tab_redraw();
            state.window.request_redraw();
            return Some(KeyboardAction::Handled);
        }
        KeyboardAction::ToggleSearch => {
            state.ui.search.active = !state.ui.search.active;
            if !state.ui.search.active {
                state.ui.search.query.clear();
                state.ui.search.matches.clear();
                state.ui.search.current_match = 0;
            }
            state.force_active_tab_redraw();
            state.window.request_redraw();
            return Some(KeyboardAction::Handled);
        }
        KeyboardAction::SearchNavigate { reverse } => {
            if !state.ui.search.matches.is_empty() {
                if *reverse {
                    if state.ui.search.current_match == 0 {
                        state.ui.search.current_match = state.ui.search.matches.len() - 1;
                    } else {
                        state.ui.search.current_match -= 1;
                    }
                } else {
                    state.ui.search.current_match =
                        (state.ui.search.current_match + 1) % state.ui.search.matches.len();
                }
                super::scroll_to_current_match(state);
                state.force_active_tab_redraw();
                state.window.request_redraw();
            }
            return Some(KeyboardAction::Handled);
        }
        KeyboardAction::PrevTab => {
            state.gpu.tab_bar.prev_tab();
            state.force_active_tab_redraw();
            state.window.request_redraw();
            return Some(KeyboardAction::Handled);
        }
        KeyboardAction::NextTab => {
            state.gpu.tab_bar.next_tab();
            state.force_active_tab_redraw();
            state.window.request_redraw();
            return Some(KeyboardAction::Handled);
        }
        KeyboardAction::SelectTab(index) => {
            state.gpu.tab_bar.select_tab_index(*index);
            state.force_active_tab_redraw();
            state.window.request_redraw();
            return Some(KeyboardAction::Handled);
        }
        // Actions that don't need local side effects (handled by caller)
        _ => {}
    }

    Some(action)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_ctx() -> InputContext {
        InputContext {
            context_menu_visible: false,
            tab_editing_active: false,
            window_rename_active: false,
            search_active: false,
            search_match_count: 0,
            tab_count: 1,
            active_tab_id: Some(1),
        }
    }

    #[test]
    fn test_cmd_q_returns_quit() {
        let ctx = default_ctx();
        let key = Key::Character("q".into());
        let result = determine_command_shortcut(&key, false, &ctx);
        assert!(matches!(result, Some(KeyboardAction::Quit)));
    }

    #[test]
    fn test_cmd_t_returns_new_tab() {
        let ctx = default_ctx();
        let key = Key::Character("t".into());
        let result = determine_command_shortcut(&key, false, &ctx);
        assert!(matches!(result, Some(KeyboardAction::NewTab)));
    }

    #[test]
    fn test_cmd_n_returns_new_window() {
        let ctx = default_ctx();
        let key = Key::Character("n".into());
        let result = determine_command_shortcut(&key, false, &ctx);
        assert!(matches!(result, Some(KeyboardAction::NewWindow)));
    }

    #[test]
    fn test_cmd_w_single_tab_returns_close_window() {
        let ctx = InputContext {
            tab_count: 1,
            active_tab_id: Some(1),
            ..default_ctx()
        };
        let key = Key::Character("w".into());
        let result = determine_command_shortcut(&key, false, &ctx);
        assert!(matches!(result, Some(KeyboardAction::CloseWindow)));
    }

    #[test]
    fn test_cmd_w_multiple_tabs_returns_close_tab() {
        let ctx = InputContext {
            tab_count: 3,
            active_tab_id: Some(42),
            ..default_ctx()
        };
        let key = Key::Character("w".into());
        let result = determine_command_shortcut(&key, false, &ctx);
        assert!(matches!(result, Some(KeyboardAction::CloseTab(42))));
    }

    #[test]
    fn test_cmd_c_returns_copy() {
        let ctx = default_ctx();
        let key = Key::Character("c".into());
        let result = determine_command_shortcut(&key, false, &ctx);
        assert!(matches!(result, Some(KeyboardAction::Copy)));
    }

    #[test]
    fn test_cmd_f_returns_toggle_search() {
        let ctx = default_ctx();
        let key = Key::Character("f".into());
        let result = determine_command_shortcut(&key, false, &ctx);
        assert!(matches!(result, Some(KeyboardAction::ToggleSearch)));
    }

    #[test]
    fn test_cmd_1_returns_select_tab_0() {
        let ctx = default_ctx();
        let key = Key::Character("1".into());
        let result = determine_command_shortcut(&key, false, &ctx);
        assert!(matches!(result, Some(KeyboardAction::SelectTab(0))));
    }

    #[test]
    fn test_cmd_shift_bracket_prev_tab() {
        let ctx = default_ctx();
        let key = Key::Character("[".into());
        let result = determine_command_shortcut(&key, true, &ctx);
        assert!(matches!(result, Some(KeyboardAction::PrevTab)));
    }

    #[test]
    fn test_scroll_shift_pageup() {
        let key = Key::Named(NamedKey::PageUp);
        let result = determine_scroll_action(&key, false, true);
        assert!(matches!(result, Some(Scroll::PageUp)));
    }

    #[test]
    fn test_scroll_no_shift_returns_none() {
        let key = Key::Named(NamedKey::PageUp);
        let result = determine_scroll_action(&key, false, false);
        assert!(result.is_none());
    }
}
