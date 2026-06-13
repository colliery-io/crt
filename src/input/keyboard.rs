//! Keyboard input handling
//!
//! Extracts keyboard event handling logic from main.rs for better modularity.
//! Returns actions that main.rs applies, keeping ownership/lifetime concerns there.

use crt_core::Scroll;
use winit::event::Modifiers;
use winit::keyboard::{Key, NamedKey};

use crate::config::{KeyAction, KeybindingsConfig};
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
    /// Increase font size
    IncreaseFontSize,
    /// Decrease font size
    DecreaseFontSize,
    /// Reset font size to default
    ResetFontSize,
    /// Toggle fullscreen mode
    ToggleFullscreen,
    /// Open the config file in the default editor
    OpenConfig,
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

/// Normalized modifier signature used to match key events against configured
/// bindings. `primary` is the platform's command modifier (Cmd on macOS,
/// Ctrl on other platforms); `ctrl_extra` is a Control press distinct from the
/// primary modifier (only meaningful on macOS, where Cmd and Ctrl differ).
#[derive(Debug, Default, PartialEq, Eq)]
struct ModSignature {
    primary: bool,
    shift: bool,
    alt: bool,
    ctrl_extra: bool,
}

/// Build the modifier signature for an incoming key event.
fn event_mod_signature(modifiers: &Modifiers) -> ModSignature {
    let s = modifiers.state();
    #[cfg(target_os = "macos")]
    {
        ModSignature {
            primary: s.super_key(),
            shift: s.shift_key(),
            alt: s.alt_key(),
            ctrl_extra: s.control_key(),
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        ModSignature {
            primary: s.control_key(),
            shift: s.shift_key(),
            alt: s.alt_key(),
            ctrl_extra: false,
        }
    }
}

/// Build the modifier signature a configured binding requires.
///
/// `"super"` maps to the platform command modifier. On non-macOS platforms a
/// literal `"ctrl"` is treated as the primary modifier too (Ctrl *is* the
/// command key there), so the default `super`-based bindings and explicit
/// `ctrl` bindings both resolve naturally.
fn binding_mod_signature(mods: &[String]) -> ModSignature {
    let mut sig = ModSignature::default();
    for m in mods {
        match m.to_ascii_lowercase().as_str() {
            "super" | "cmd" | "command" | "meta" | "win" => sig.primary = true,
            "ctrl" | "control" => {
                #[cfg(target_os = "macos")]
                {
                    sig.ctrl_extra = true;
                }
                #[cfg(not(target_os = "macos"))]
                {
                    sig.primary = true;
                }
            }
            "shift" => sig.shift = true,
            "alt" | "option" | "opt" => sig.alt = true,
            _ => {}
        }
    }
    sig
}

/// Normalize a key token to a canonical lowercase form so that, e.g., `"="`,
/// `"+"` and `"equal"` all compare equal.
fn normalize_key_token(token: &str) -> String {
    match token {
        "=" | "+" => "equal".to_string(),
        "-" | "_" => "minus".to_string(),
        "{" => "[".to_string(),
        "}" => "]".to_string(),
        "comma" => ",".to_string(),
        other => other.to_ascii_lowercase(),
    }
}

/// Extract a normalized key token from a winit key, or `None` for keys that
/// can't be bound (e.g. plain modifier presses).
fn event_key_token(key: &Key) -> Option<String> {
    match key {
        Key::Character(c) => Some(normalize_key_token(c.as_str())),
        Key::Named(NamedKey::Space) => Some("space".to_string()),
        Key::Named(NamedKey::Tab) => Some("tab".to_string()),
        Key::Named(NamedKey::Enter) => Some("enter".to_string()),
        Key::Named(named) => {
            // Function keys (F1..F35) serialize as "F1", "F2", ... via Debug.
            let s = format!("{named:?}");
            if let Some(num) = s.strip_prefix('F')
                && num.chars().all(|ch| ch.is_ascii_digit())
                && !num.is_empty()
            {
                Some(format!("f{num}"))
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Resolve an incoming key event to a configured [`KeyAction`], if any binding
/// matches both the key and the exact modifier combination.
pub fn resolve_keybinding(
    keybindings: &KeybindingsConfig,
    key: &Key,
    modifiers: &Modifiers,
) -> Option<KeyAction> {
    let token = event_key_token(key)?;
    let event_sig = event_mod_signature(modifiers);
    keybindings
        .bindings
        .iter()
        .find(|b| {
            normalize_key_token(&b.key) == token && binding_mod_signature(&b.mods) == event_sig
        })
        .map(|b| b.action.clone())
}

/// Translate a configured [`KeyAction`] into a [`KeyboardAction`], given the
/// current input context (used to decide close-tab vs. close-window).
pub fn key_action_to_keyboard_action(action: &KeyAction, ctx: &InputContext) -> KeyboardAction {
    match action {
        KeyAction::Copy => KeyboardAction::Copy,
        KeyAction::Paste => KeyboardAction::Paste,
        KeyAction::Quit => KeyboardAction::Quit,
        KeyAction::NewTab => KeyboardAction::NewTab,
        KeyAction::CloseTab => {
            if ctx.tab_count > 1 {
                if let Some(tab_id) = ctx.active_tab_id {
                    return KeyboardAction::CloseTab(tab_id);
                }
            }
            KeyboardAction::CloseWindow
        }
        KeyAction::NextTab => KeyboardAction::NextTab,
        KeyAction::PrevTab => KeyboardAction::PrevTab,
        KeyAction::SelectTab1
        | KeyAction::SelectTab2
        | KeyAction::SelectTab3
        | KeyAction::SelectTab4
        | KeyAction::SelectTab5
        | KeyAction::SelectTab6
        | KeyAction::SelectTab7
        | KeyAction::SelectTab8
        | KeyAction::SelectTab9 => {
            KeyboardAction::SelectTab(action.tab_index().unwrap_or(0))
        }
        KeyAction::IncreaseFontSize => KeyboardAction::IncreaseFontSize,
        KeyAction::DecreaseFontSize => KeyboardAction::DecreaseFontSize,
        KeyAction::ResetFontSize => KeyboardAction::ResetFontSize,
        KeyAction::ToggleFullscreen => KeyboardAction::ToggleFullscreen,
        KeyAction::OpenConfig => KeyboardAction::OpenConfig,
    }
}

/// Determine a hardcoded, non-configurable command shortcut (Cmd/Ctrl + key).
///
/// These actions have no [`KeyAction`] equivalent and are always available
/// regardless of the user's keybindings: new window, search, and search
/// navigation. Configurable shortcuts are resolved separately via
/// [`resolve_keybinding`]. Pure function — no side effects.
pub fn determine_command_shortcut(
    key: &Key,
    shift_pressed: bool,
    ctx: &InputContext,
) -> Option<KeyboardAction> {
    match key {
        Key::Character(c) if c.as_str() == "n" => Some(KeyboardAction::NewWindow),
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
    keybindings: &KeybindingsConfig,
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

    // Resolve user-configurable keybindings first (these may include
    // no-modifier bindings such as F11 for fullscreen).
    if let Some(action) = handle_configured_keybinding(state, key, modifiers, keybindings) {
        return action;
    }

    // Handle hardcoded, non-configurable shortcuts (Cmd/Ctrl + key)
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

/// Resolve and dispatch a user-configurable keybinding.
///
/// Returns `Some` if a binding matched the key event (applying any local side
/// effects), or `None` if no binding matched.
fn handle_configured_keybinding(
    state: &mut WindowState,
    key: &Key,
    modifiers: &Modifiers,
    keybindings: &KeybindingsConfig,
) -> Option<KeyboardAction> {
    let key_action = resolve_keybinding(keybindings, key, modifiers)?;

    // Confirm any tab editing in progress before acting on the shortcut.
    if state.gpu.tab_bar.is_editing() {
        state.gpu.tab_bar.confirm_editing();
        state.render.dirty = true;
    }

    let ctx = InputContext::from_state(state);
    let action = key_action_to_keyboard_action(&key_action, &ctx);
    Some(apply_keyboard_action(state, action))
}

/// Handle hardcoded, non-configurable command shortcuts (Cmd/Ctrl + key).
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
    Some(apply_keyboard_action(state, action))
}

/// Apply any local (window-scoped) side effects for an action and return the
/// resulting [`KeyboardAction`]. Actions that require app-level access (quit,
/// new window/tab, font size, fullscreen) are returned unchanged for the
/// caller in `handler.rs` to process.
fn apply_keyboard_action(state: &mut WindowState, action: KeyboardAction) -> KeyboardAction {
    match action {
        KeyboardAction::Copy => {
            if let Some(text) = get_terminal_selection_text(state) {
                set_clipboard_content(&text);
                state.ui.copy_indicator.trigger();
            }
            KeyboardAction::Handled
        }
        KeyboardAction::Paste => {
            if let Some(content) = get_clipboard_content() {
                paste_to_terminal(state, &content);
            }
            KeyboardAction::Handled
        }
        KeyboardAction::CloseTab(tab_id) => {
            state.gpu.tab_bar.close_tab(tab_id);
            state.remove_shell_for_tab(tab_id);
            state.force_active_tab_redraw();
            state.window.request_redraw();
            KeyboardAction::Handled
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
            KeyboardAction::Handled
        }
        KeyboardAction::SearchNavigate { reverse } => {
            if !state.ui.search.matches.is_empty() {
                if reverse {
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
            KeyboardAction::Handled
        }
        KeyboardAction::PrevTab => {
            state.gpu.tab_bar.prev_tab();
            state.force_active_tab_redraw();
            state.window.request_redraw();
            KeyboardAction::Handled
        }
        KeyboardAction::NextTab => {
            state.gpu.tab_bar.next_tab();
            state.force_active_tab_redraw();
            state.window.request_redraw();
            KeyboardAction::Handled
        }
        KeyboardAction::SelectTab(index) => {
            state.gpu.tab_bar.select_tab_index(index);
            state.force_active_tab_redraw();
            state.window.request_redraw();
            KeyboardAction::Handled
        }
        // Actions that don't need local side effects (handled by caller)
        other => other,
    }
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

    /// Build a `Modifiers` with only the platform primary command modifier held.
    fn primary_mods() -> Modifiers {
        use winit::keyboard::ModifiersState;
        #[cfg(target_os = "macos")]
        let state = ModifiersState::SUPER;
        #[cfg(not(target_os = "macos"))]
        let state = ModifiersState::CONTROL;
        Modifiers::from(state)
    }

    /// Build a `Modifiers` with the primary command modifier plus shift.
    fn primary_shift_mods() -> Modifiers {
        use winit::keyboard::ModifiersState;
        #[cfg(target_os = "macos")]
        let state = ModifiersState::SUPER | ModifiersState::SHIFT;
        #[cfg(not(target_os = "macos"))]
        let state = ModifiersState::CONTROL | ModifiersState::SHIFT;
        Modifiers::from(state)
    }

    #[test]
    fn test_default_binding_quit() {
        let kb = KeybindingsConfig::default();
        let key = Key::Character("q".into());
        assert_eq!(
            resolve_keybinding(&kb, &key, &primary_mods()),
            Some(KeyAction::Quit)
        );
    }

    #[test]
    fn test_default_binding_new_tab() {
        let kb = KeybindingsConfig::default();
        let key = Key::Character("t".into());
        assert_eq!(
            resolve_keybinding(&kb, &key, &primary_mods()),
            Some(KeyAction::NewTab)
        );
    }

    #[test]
    fn test_default_binding_copy() {
        let kb = KeybindingsConfig::default();
        let key = Key::Character("c".into());
        assert_eq!(
            resolve_keybinding(&kb, &key, &primary_mods()),
            Some(KeyAction::Copy)
        );
    }

    #[test]
    fn test_default_binding_select_tab1() {
        let kb = KeybindingsConfig::default();
        let key = Key::Character("1".into());
        assert_eq!(
            resolve_keybinding(&kb, &key, &primary_mods()),
            Some(KeyAction::SelectTab1)
        );
    }

    #[test]
    fn test_default_binding_prev_tab_needs_shift() {
        let kb = KeybindingsConfig::default();
        let key = Key::Character("[".into());
        // Shift is required for prev/next tab.
        assert_eq!(
            resolve_keybinding(&kb, &key, &primary_shift_mods()),
            Some(KeyAction::PrevTab)
        );
        // Without shift, no binding matches.
        assert_eq!(resolve_keybinding(&kb, &key, &primary_mods()), None);
    }

    #[test]
    fn test_no_modifier_does_not_match_primary_binding() {
        let kb = KeybindingsConfig::default();
        let key = Key::Character("t".into());
        assert_eq!(resolve_keybinding(&kb, &key, &Modifiers::default()), None);
    }

    #[test]
    fn test_equal_token_normalization() {
        // A binding on "equal" matches the "=" character event.
        let kb = KeybindingsConfig::default();
        let key = Key::Character("=".into());
        assert_eq!(
            resolve_keybinding(&kb, &key, &primary_mods()),
            Some(KeyAction::IncreaseFontSize)
        );
    }

    #[test]
    fn test_custom_binding_resolves() {
        use crate::config::Keybinding;
        let kb = KeybindingsConfig {
            bindings: vec![Keybinding {
                key: "F11".to_string(),
                mods: vec![],
                action: KeyAction::ToggleFullscreen,
            }],
        };
        let key = Key::Named(NamedKey::F11);
        assert_eq!(
            resolve_keybinding(&kb, &key, &Modifiers::default()),
            Some(KeyAction::ToggleFullscreen)
        );
    }

    #[test]
    fn test_key_action_close_tab_single_tab_closes_window() {
        let ctx = InputContext {
            tab_count: 1,
            active_tab_id: Some(1),
            ..default_ctx()
        };
        let action = key_action_to_keyboard_action(&KeyAction::CloseTab, &ctx);
        assert!(matches!(action, KeyboardAction::CloseWindow));
    }

    #[test]
    fn test_key_action_close_tab_multiple_tabs_closes_tab() {
        let ctx = InputContext {
            tab_count: 3,
            active_tab_id: Some(42),
            ..default_ctx()
        };
        let action = key_action_to_keyboard_action(&KeyAction::CloseTab, &ctx);
        assert!(matches!(action, KeyboardAction::CloseTab(42)));
    }

    #[test]
    fn test_hardcoded_cmd_n_returns_new_window() {
        let ctx = default_ctx();
        let key = Key::Character("n".into());
        let result = determine_command_shortcut(&key, false, &ctx);
        assert!(matches!(result, Some(KeyboardAction::NewWindow)));
    }

    #[test]
    fn test_hardcoded_cmd_f_returns_toggle_search() {
        let ctx = default_ctx();
        let key = Key::Character("f".into());
        let result = determine_command_shortcut(&key, false, &ctx);
        assert!(matches!(result, Some(KeyboardAction::ToggleSearch)));
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
