//! Pure keyboard input handling
//!
//! This module provides pure functions for converting keyboard events to
//! terminal byte sequences and application commands. All functions are
//! side-effect free and can be easily unit tested.

use winit::keyboard::{Key, NamedKey};

use super::Command;

/// Modifiers state for keyboard input
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Modifiers {
    /// Command/Super key (Cmd on macOS, Win on Windows)
    pub command: bool,
    /// Control key
    pub control: bool,
    /// Shift key
    pub shift: bool,
    /// Alt/Option key
    pub alt: bool,
}

impl Modifiers {
    /// Create a new Modifiers with all keys released
    pub fn empty() -> Self {
        Self::default()
    }

    /// Create modifiers with only command key pressed
    pub fn command_only() -> Self {
        Self {
            command: true,
            ..Self::default()
        }
    }

    /// Create modifiers with only control key pressed
    pub fn control_only() -> Self {
        Self {
            control: true,
            ..Self::default()
        }
    }

    /// Create modifiers with only shift key pressed
    pub fn shift_only() -> Self {
        Self {
            shift: true,
            ..Self::default()
        }
    }
}

/// Convert a key press to terminal byte sequence
///
/// Returns `None` if the key should not be sent to the PTY (e.g., modifier-only
/// keys or application shortcuts).
///
/// # Arguments
/// * `key` - The key that was pressed
/// * `modifiers` - Current modifier key state
///
/// # Examples
/// ```ignore
/// let bytes = key_to_terminal_bytes(&Key::Named(NamedKey::Enter), Modifiers::empty());
/// assert_eq!(bytes, Some(vec![b'\r']));
/// ```
pub fn key_to_terminal_bytes(key: &Key, modifiers: Modifiers) -> Option<Vec<u8>> {
    // Don't send to terminal if command key is pressed (app shortcuts)
    if modifiers.command {
        return None;
    }

    match key {
        Key::Named(named) => named_key_to_bytes(named, modifiers),
        Key::Character(c) => character_to_bytes(c, modifiers),
        _ => None,
    }
}

/// Convert a named key to terminal bytes
fn named_key_to_bytes(key: &NamedKey, _modifiers: Modifiers) -> Option<Vec<u8>> {
    match key {
        NamedKey::Enter => Some(vec![b'\r']),
        NamedKey::Backspace => Some(vec![0x7f]),
        NamedKey::Tab => Some(vec![b'\t']),
        NamedKey::Escape => Some(vec![0x1b]),
        NamedKey::Space => Some(vec![b' ']),

        // Arrow keys
        NamedKey::ArrowUp => Some(b"\x1b[A".to_vec()),
        NamedKey::ArrowDown => Some(b"\x1b[B".to_vec()),
        NamedKey::ArrowRight => Some(b"\x1b[C".to_vec()),
        NamedKey::ArrowLeft => Some(b"\x1b[D".to_vec()),

        // Navigation keys
        NamedKey::Home => Some(b"\x1b[H".to_vec()),
        NamedKey::End => Some(b"\x1b[F".to_vec()),
        NamedKey::PageUp => Some(b"\x1b[5~".to_vec()),
        NamedKey::PageDown => Some(b"\x1b[6~".to_vec()),
        NamedKey::Insert => Some(b"\x1b[2~".to_vec()),
        NamedKey::Delete => Some(b"\x1b[3~".to_vec()),

        // Function keys
        NamedKey::F1 => Some(b"\x1bOP".to_vec()),
        NamedKey::F2 => Some(b"\x1bOQ".to_vec()),
        NamedKey::F3 => Some(b"\x1bOR".to_vec()),
        NamedKey::F4 => Some(b"\x1bOS".to_vec()),
        NamedKey::F5 => Some(b"\x1b[15~".to_vec()),
        NamedKey::F6 => Some(b"\x1b[17~".to_vec()),
        NamedKey::F7 => Some(b"\x1b[18~".to_vec()),
        NamedKey::F8 => Some(b"\x1b[19~".to_vec()),
        NamedKey::F9 => Some(b"\x1b[20~".to_vec()),
        NamedKey::F10 => Some(b"\x1b[21~".to_vec()),
        NamedKey::F11 => Some(b"\x1b[23~".to_vec()),
        NamedKey::F12 => Some(b"\x1b[24~".to_vec()),

        _ => None,
    }
}

/// Convert a character key to terminal bytes, handling Ctrl combinations
fn character_to_bytes(c: &str, modifiers: Modifiers) -> Option<Vec<u8>> {
    if modifiers.control {
        // Convert Ctrl+letter to control character (ASCII 1-26)
        if let Some(ch) = c.chars().next() {
            let ctrl_char = match ch.to_ascii_lowercase() {
                'a'..='z' => Some((ch.to_ascii_lowercase() as u8) - b'a' + 1),
                '[' => Some(0x1b), // Ctrl+[ = Escape
                '\\' => Some(0x1c), // Ctrl+\
                ']' => Some(0x1d),  // Ctrl+]
                '^' => Some(0x1e),  // Ctrl+^
                '_' => Some(0x1f),  // Ctrl+_
                '@' => Some(0x00),  // Ctrl+@ = NUL
                _ => None,
            };
            return ctrl_char.map(|byte| vec![byte]);
        }
    }

    // Regular character
    Some(c.as_bytes().to_vec())
}

/// Check if key combination is an application shortcut
///
/// Application shortcuts are key combinations that should be handled by the
/// application rather than sent to the terminal (e.g., Cmd+C for copy).
pub fn is_app_shortcut(key: &Key, modifiers: Modifiers) -> bool {
    if !modifiers.command {
        return false;
    }

    match key {
        Key::Character(c) => matches!(
            c.as_str(),
            "c" | "v" | "x" | "a" | "f" | "n" | "t" | "w" | "q" | "+" | "=" | "-" | "0"
        ),
        Key::Named(NamedKey::Tab) => true, // Cmd+Tab for tab switching
        _ => false,
    }
}

/// Convert an application shortcut to a Command
///
/// Returns `None` if the key combination is not a recognized shortcut.
pub fn shortcut_to_command(key: &Key, modifiers: Modifiers) -> Option<Command> {
    if !modifiers.command {
        return None;
    }

    match key {
        Key::Character(c) => match c.as_str() {
            "c" => Some(Command::Copy(String::new())), // Text filled by caller
            "v" => Some(Command::Paste),
            "f" => Some(Command::OpenSearch),
            "n" => Some(Command::NewWindow),
            "t" => Some(Command::NewTab),
            "w" => {
                if modifiers.shift {
                    Some(Command::CloseWindow)
                } else {
                    Some(Command::CloseCurrentTab)
                }
            }
            "+" | "=" => Some(Command::AdjustFontScale(0.1)),
            "-" => Some(Command::AdjustFontScale(-0.1)),
            "0" => Some(Command::ResetFontScale),
            _ => None,
        },
        Key::Named(NamedKey::Tab) => {
            if modifiers.shift {
                Some(Command::PreviousTab)
            } else {
                Some(Command::NextTab)
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // === key_to_terminal_bytes tests ===

    #[test]
    fn test_enter_key() {
        let key = Key::Named(NamedKey::Enter);
        let result = key_to_terminal_bytes(&key, Modifiers::empty());
        assert_eq!(result, Some(vec![b'\r']));
    }

    #[test]
    fn test_backspace_key() {
        let key = Key::Named(NamedKey::Backspace);
        let result = key_to_terminal_bytes(&key, Modifiers::empty());
        assert_eq!(result, Some(vec![0x7f]));
    }

    #[test]
    fn test_escape_key() {
        let key = Key::Named(NamedKey::Escape);
        let result = key_to_terminal_bytes(&key, Modifiers::empty());
        assert_eq!(result, Some(vec![0x1b]));
    }

    #[test]
    fn test_arrow_keys() {
        assert_eq!(
            key_to_terminal_bytes(&Key::Named(NamedKey::ArrowUp), Modifiers::empty()),
            Some(b"\x1b[A".to_vec())
        );
        assert_eq!(
            key_to_terminal_bytes(&Key::Named(NamedKey::ArrowDown), Modifiers::empty()),
            Some(b"\x1b[B".to_vec())
        );
        assert_eq!(
            key_to_terminal_bytes(&Key::Named(NamedKey::ArrowRight), Modifiers::empty()),
            Some(b"\x1b[C".to_vec())
        );
        assert_eq!(
            key_to_terminal_bytes(&Key::Named(NamedKey::ArrowLeft), Modifiers::empty()),
            Some(b"\x1b[D".to_vec())
        );
    }

    #[test]
    fn test_function_keys() {
        assert_eq!(
            key_to_terminal_bytes(&Key::Named(NamedKey::F1), Modifiers::empty()),
            Some(b"\x1bOP".to_vec())
        );
        assert_eq!(
            key_to_terminal_bytes(&Key::Named(NamedKey::F5), Modifiers::empty()),
            Some(b"\x1b[15~".to_vec())
        );
    }

    #[test]
    fn test_regular_character() {
        let key = Key::Character("a".into());
        let result = key_to_terminal_bytes(&key, Modifiers::empty());
        assert_eq!(result, Some(vec![b'a']));
    }

    #[test]
    fn test_ctrl_c() {
        let key = Key::Character("c".into());
        let result = key_to_terminal_bytes(&key, Modifiers::control_only());
        assert_eq!(result, Some(vec![0x03])); // ETX (End of Text)
    }

    #[test]
    fn test_ctrl_a() {
        let key = Key::Character("a".into());
        let result = key_to_terminal_bytes(&key, Modifiers::control_only());
        assert_eq!(result, Some(vec![0x01])); // SOH (Start of Heading)
    }

    #[test]
    fn test_ctrl_z() {
        let key = Key::Character("z".into());
        let result = key_to_terminal_bytes(&key, Modifiers::control_only());
        assert_eq!(result, Some(vec![0x1a])); // SUB (Substitute)
    }

    #[test]
    fn test_ctrl_bracket() {
        let key = Key::Character("[".into());
        let result = key_to_terminal_bytes(&key, Modifiers::control_only());
        assert_eq!(result, Some(vec![0x1b])); // ESC
    }

    #[test]
    fn test_cmd_key_blocks_terminal() {
        let key = Key::Character("c".into());
        let result = key_to_terminal_bytes(&key, Modifiers::command_only());
        assert_eq!(result, None); // Should not send to terminal
    }

    // === is_app_shortcut tests ===

    #[test]
    fn test_cmd_c_is_shortcut() {
        let key = Key::Character("c".into());
        assert!(is_app_shortcut(&key, Modifiers::command_only()));
    }

    #[test]
    fn test_cmd_v_is_shortcut() {
        let key = Key::Character("v".into());
        assert!(is_app_shortcut(&key, Modifiers::command_only()));
    }

    #[test]
    fn test_cmd_t_is_shortcut() {
        let key = Key::Character("t".into());
        assert!(is_app_shortcut(&key, Modifiers::command_only()));
    }

    #[test]
    fn test_regular_c_not_shortcut() {
        let key = Key::Character("c".into());
        assert!(!is_app_shortcut(&key, Modifiers::empty()));
    }

    #[test]
    fn test_ctrl_c_not_app_shortcut() {
        let key = Key::Character("c".into());
        assert!(!is_app_shortcut(&key, Modifiers::control_only()));
    }

    #[test]
    fn test_cmd_tab_is_shortcut() {
        let key = Key::Named(NamedKey::Tab);
        assert!(is_app_shortcut(&key, Modifiers::command_only()));
    }

    // === shortcut_to_command tests ===

    #[test]
    fn test_cmd_c_to_copy() {
        let key = Key::Character("c".into());
        let cmd = shortcut_to_command(&key, Modifiers::command_only());
        assert!(matches!(cmd, Some(Command::Copy(_))));
    }

    #[test]
    fn test_cmd_v_to_paste() {
        let key = Key::Character("v".into());
        let cmd = shortcut_to_command(&key, Modifiers::command_only());
        assert_eq!(cmd, Some(Command::Paste));
    }

    #[test]
    fn test_cmd_t_to_new_tab() {
        let key = Key::Character("t".into());
        let cmd = shortcut_to_command(&key, Modifiers::command_only());
        assert_eq!(cmd, Some(Command::NewTab));
    }

    #[test]
    fn test_cmd_w_to_close_tab() {
        let key = Key::Character("w".into());
        let cmd = shortcut_to_command(&key, Modifiers::command_only());
        assert_eq!(cmd, Some(Command::CloseCurrentTab));
    }

    #[test]
    fn test_cmd_shift_w_to_close_window() {
        let key = Key::Character("w".into());
        let mods = Modifiers {
            command: true,
            shift: true,
            ..Modifiers::empty()
        };
        let cmd = shortcut_to_command(&key, mods);
        assert_eq!(cmd, Some(Command::CloseWindow));
    }

    #[test]
    fn test_cmd_plus_to_zoom_in() {
        let key = Key::Character("+".into());
        let cmd = shortcut_to_command(&key, Modifiers::command_only());
        assert_eq!(cmd, Some(Command::AdjustFontScale(0.1)));
    }

    #[test]
    fn test_cmd_minus_to_zoom_out() {
        let key = Key::Character("-".into());
        let cmd = shortcut_to_command(&key, Modifiers::command_only());
        assert_eq!(cmd, Some(Command::AdjustFontScale(-0.1)));
    }

    #[test]
    fn test_cmd_0_to_reset_zoom() {
        let key = Key::Character("0".into());
        let cmd = shortcut_to_command(&key, Modifiers::command_only());
        assert_eq!(cmd, Some(Command::ResetFontScale));
    }

    #[test]
    fn test_cmd_tab_to_next_tab() {
        let key = Key::Named(NamedKey::Tab);
        let cmd = shortcut_to_command(&key, Modifiers::command_only());
        assert_eq!(cmd, Some(Command::NextTab));
    }

    #[test]
    fn test_cmd_shift_tab_to_prev_tab() {
        let key = Key::Named(NamedKey::Tab);
        let mods = Modifiers {
            command: true,
            shift: true,
            ..Modifiers::empty()
        };
        let cmd = shortcut_to_command(&key, mods);
        assert_eq!(cmd, Some(Command::PreviousTab));
    }

    #[test]
    fn test_no_modifier_no_command() {
        let key = Key::Character("c".into());
        let cmd = shortcut_to_command(&key, Modifiers::empty());
        assert_eq!(cmd, None);
    }
}
