//! Key encoding using termwiz
//!
//! Converts winit keyboard events to terminal escape sequences using termwiz's
//! KeyCode::encode() method. This provides comprehensive key handling without
//! maintaining manual escape sequence mappings.

use termwiz::input::{KeyCode, KeyCodeEncodeModes, KeyboardEncoding, Modifiers as TermwizModifiers};
use winit::keyboard::{Key, NamedKey};

/// Convert a winit Key to a termwiz KeyCode
fn winit_to_termwiz_keycode(key: &Key) -> Option<KeyCode> {
    match key {
        Key::Character(c) => {
            // Get the first character
            c.chars().next().map(KeyCode::Char)
        }
        Key::Named(named) => {
            match named {
                // Navigation
                NamedKey::ArrowUp => Some(KeyCode::UpArrow),
                NamedKey::ArrowDown => Some(KeyCode::DownArrow),
                NamedKey::ArrowLeft => Some(KeyCode::LeftArrow),
                NamedKey::ArrowRight => Some(KeyCode::RightArrow),
                NamedKey::Home => Some(KeyCode::Home),
                NamedKey::End => Some(KeyCode::End),
                NamedKey::PageUp => Some(KeyCode::PageUp),
                NamedKey::PageDown => Some(KeyCode::PageDown),

                // Editing
                NamedKey::Backspace => Some(KeyCode::Backspace),
                NamedKey::Delete => Some(KeyCode::Delete),
                NamedKey::Insert => Some(KeyCode::Insert),
                NamedKey::Enter => Some(KeyCode::Enter),
                NamedKey::Tab => Some(KeyCode::Tab),
                NamedKey::Escape => Some(KeyCode::Escape),
                NamedKey::Space => Some(KeyCode::Char(' ')),

                // Function keys
                NamedKey::F1 => Some(KeyCode::Function(1)),
                NamedKey::F2 => Some(KeyCode::Function(2)),
                NamedKey::F3 => Some(KeyCode::Function(3)),
                NamedKey::F4 => Some(KeyCode::Function(4)),
                NamedKey::F5 => Some(KeyCode::Function(5)),
                NamedKey::F6 => Some(KeyCode::Function(6)),
                NamedKey::F7 => Some(KeyCode::Function(7)),
                NamedKey::F8 => Some(KeyCode::Function(8)),
                NamedKey::F9 => Some(KeyCode::Function(9)),
                NamedKey::F10 => Some(KeyCode::Function(10)),
                NamedKey::F11 => Some(KeyCode::Function(11)),
                NamedKey::F12 => Some(KeyCode::Function(12)),
                NamedKey::F13 => Some(KeyCode::Function(13)),
                NamedKey::F14 => Some(KeyCode::Function(14)),
                NamedKey::F15 => Some(KeyCode::Function(15)),
                NamedKey::F16 => Some(KeyCode::Function(16)),
                NamedKey::F17 => Some(KeyCode::Function(17)),
                NamedKey::F18 => Some(KeyCode::Function(18)),
                NamedKey::F19 => Some(KeyCode::Function(19)),
                NamedKey::F20 => Some(KeyCode::Function(20)),

                // Other keys that don't produce escape sequences
                _ => None,
            }
        }
        _ => None,
    }
}

/// Build termwiz modifiers from individual modifier flags
fn build_modifiers(ctrl: bool, shift: bool, alt: bool) -> TermwizModifiers {
    let mut mods = TermwizModifiers::NONE;
    if ctrl {
        mods |= TermwizModifiers::CTRL;
    }
    if shift {
        mods |= TermwizModifiers::SHIFT;
    }
    if alt {
        mods |= TermwizModifiers::ALT;
    }
    mods
}

/// Encode a winit key event to terminal escape sequence bytes
///
/// Returns the bytes to send to the PTY, or None if the key cannot be encoded.
pub fn encode_key(
    key: &Key,
    ctrl_pressed: bool,
    shift_pressed: bool,
    alt_pressed: bool,
) -> Option<Vec<u8>> {
    let keycode = winit_to_termwiz_keycode(key)?;
    let modifiers = build_modifiers(ctrl_pressed, shift_pressed, alt_pressed);

    // Default encoding modes for a standard terminal
    let modes = KeyCodeEncodeModes {
        encoding: KeyboardEncoding::Xterm,
        application_cursor_keys: false,
        newline_mode: false,
        modify_other_keys: None,
    };

    // Encode the key - returns a String containing the escape sequence
    match keycode.encode(modifiers, modes, true) {
        Ok(encoded) => {
            if encoded.is_empty() {
                None
            } else {
                Some(encoded.into_bytes())
            }
        }
        Err(e) => {
            log::debug!("Failed to encode key {:?}: {}", key, e);
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_tab() {
        let result = encode_key(&Key::Named(NamedKey::Tab), false, false, false);
        assert_eq!(result, Some(b"\t".to_vec()));
    }

    #[test]
    fn test_encode_shift_tab() {
        let result = encode_key(&Key::Named(NamedKey::Tab), false, true, false);
        // Shift+Tab should produce backtab escape sequence
        assert!(result.is_some());
        let bytes = result.unwrap();
        assert!(!bytes.is_empty());
    }

    #[test]
    fn test_encode_arrow_keys() {
        let up = encode_key(&Key::Named(NamedKey::ArrowUp), false, false, false);
        assert_eq!(up, Some(b"\x1b[A".to_vec()));

        let down = encode_key(&Key::Named(NamedKey::ArrowDown), false, false, false);
        assert_eq!(down, Some(b"\x1b[B".to_vec()));
    }

    #[test]
    fn test_encode_function_keys() {
        let f1 = encode_key(&Key::Named(NamedKey::F1), false, false, false);
        assert!(f1.is_some());

        let f12 = encode_key(&Key::Named(NamedKey::F12), false, false, false);
        assert!(f12.is_some());
    }

    #[test]
    fn test_encode_character() {
        let a = encode_key(&Key::Character("a".into()), false, false, false);
        assert_eq!(a, Some(b"a".to_vec()));
    }

    #[test]
    fn test_encode_ctrl_c() {
        let ctrl_c = encode_key(&Key::Character("c".into()), true, false, false);
        // Ctrl+C should produce ETX (0x03)
        assert_eq!(ctrl_c, Some(vec![0x03]));
    }

    #[test]
    fn test_encode_home_end() {
        // Note: Home/End are now handled explicitly in handle_shell_input with PC-style
        // sequences (\x1b[1~ and \x1b[4~) because they work more universally with shells.
        // This test verifies termwiz behavior for reference.
        let home = encode_key(&Key::Named(NamedKey::Home), false, false, false);
        assert_eq!(home, Some(b"\x1b[H".to_vec()), "Home key encoding (termwiz)");

        let end = encode_key(&Key::Named(NamedKey::End), false, false, false);
        assert_eq!(end, Some(b"\x1b[F".to_vec()), "End key encoding (termwiz)");
    }
}
