//! Event handling utilities
//!
//! This module provides event handling functions that can be used
//! to process winit events and convert them to application commands.
//! The design enables incremental migration from the existing main.rs
//! event handling.

use winit::event::{ElementState, MouseButton};
use winit::keyboard::{Key, NamedKey};

use crate::input::Command;

/// Keyboard modifiers state
#[derive(Debug, Clone, Copy, Default)]
pub struct ModifierState {
    pub command: bool,
    pub control: bool,
    pub shift: bool,
    pub alt: bool,
}

impl ModifierState {
    /// Create from winit Modifiers
    pub fn from_winit(mods: &winit::event::Modifiers) -> Self {
        let state = mods.state();
        Self {
            command: state.super_key(),
            control: state.control_key(),
            shift: state.shift_key(),
            alt: state.alt_key(),
        }
    }

    /// Check if command/super key is pressed
    pub fn is_command(&self) -> bool {
        self.command
    }

    /// Check if control key is pressed
    pub fn is_control(&self) -> bool {
        self.control
    }

    /// Check if no modifiers are pressed
    pub fn is_empty(&self) -> bool {
        !self.command && !self.control && !self.shift && !self.alt
    }
}

/// Result of processing a keyboard event
#[derive(Debug, Clone)]
pub enum KeyboardResult {
    /// Event was an application shortcut, execute this command
    Shortcut(Command),
    /// Event should be sent to terminal as bytes
    TerminalInput(Vec<u8>),
    /// Event was handled but no action needed
    Handled,
    /// Event was not handled
    Ignored,
}

/// Result of processing a mouse event
#[derive(Debug, Clone)]
pub enum MouseResult {
    /// Execute these commands
    Commands(Vec<Command>),
    /// No action needed
    None,
}

/// Event handler utilities
///
/// This struct provides static methods for processing events.
/// The design allows for gradual migration from existing handlers.
pub struct EventHandler;

impl EventHandler {
    /// Check if a key press is an application shortcut
    pub fn is_app_shortcut(key: &Key, modifiers: ModifierState) -> bool {
        if !modifiers.is_command() {
            return false;
        }

        match key {
            Key::Character(c) => {
                let c = c.to_lowercase();
                matches!(
                    c.as_str(),
                    "c" | "v" | "t" | "w" | "n" | "q" | "f" | "0" | "=" | "-"
                        | "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9"
                )
            }
            Key::Named(NamedKey::Tab) => true,
            _ => false,
        }
    }

    /// Convert an application shortcut to a command
    pub fn shortcut_to_command(key: &Key, modifiers: ModifierState) -> Option<Command> {
        if !modifiers.is_command() {
            return None;
        }

        match key {
            Key::Character(c) => {
                let c = c.to_lowercase();
                match c.as_str() {
                    "c" => Some(Command::Copy(String::new())), // Text filled in by caller
                    "v" => Some(Command::Paste),
                    "t" => Some(Command::NewTab),
                    "w" => {
                        if modifiers.shift {
                            Some(Command::CloseWindow)
                        } else {
                            Some(Command::CloseCurrentTab)
                        }
                    }
                    "n" => Some(Command::NewWindow),
                    "f" => Some(Command::OpenSearch),
                    "0" => Some(Command::ResetFontScale),
                    "=" => Some(Command::AdjustFontScale(0.1)),
                    "-" => Some(Command::AdjustFontScale(-0.1)),
                    _ => None,
                }
            }
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

    /// Convert a key press to terminal bytes (for non-shortcut keys)
    pub fn key_to_terminal_bytes(key: &Key, modifiers: ModifierState) -> Option<Vec<u8>> {
        match key {
            Key::Named(named) => Self::named_key_to_bytes(named, modifiers),
            Key::Character(c) => Self::character_to_bytes(c, modifiers),
            _ => None,
        }
    }

    /// Convert named keys to terminal byte sequences
    fn named_key_to_bytes(key: &NamedKey, _modifiers: ModifierState) -> Option<Vec<u8>> {
        match key {
            NamedKey::Enter => Some(b"\r".to_vec()),
            NamedKey::Backspace => Some(b"\x7f".to_vec()),
            NamedKey::Tab => Some(b"\t".to_vec()),
            NamedKey::Escape => Some(b"\x1b".to_vec()),
            NamedKey::ArrowUp => Some(b"\x1b[A".to_vec()),
            NamedKey::ArrowDown => Some(b"\x1b[B".to_vec()),
            NamedKey::ArrowRight => Some(b"\x1b[C".to_vec()),
            NamedKey::ArrowLeft => Some(b"\x1b[D".to_vec()),
            NamedKey::Home => Some(b"\x1b[H".to_vec()),
            NamedKey::End => Some(b"\x1b[F".to_vec()),
            NamedKey::PageUp => Some(b"\x1b[5~".to_vec()),
            NamedKey::PageDown => Some(b"\x1b[6~".to_vec()),
            NamedKey::Insert => Some(b"\x1b[2~".to_vec()),
            NamedKey::Delete => Some(b"\x1b[3~".to_vec()),
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

    /// Convert character input to terminal bytes
    fn character_to_bytes(c: &str, modifiers: ModifierState) -> Option<Vec<u8>> {
        if modifiers.is_command() {
            // Command key shortcuts are handled separately
            return None;
        }

        if modifiers.is_control() {
            // Control key sequences
            if let Some(ch) = c.chars().next() {
                let ctrl_char = match ch.to_ascii_lowercase() {
                    'a'..='z' => Some((ch.to_ascii_lowercase() as u8) - b'a' + 1),
                    '[' => Some(27), // Escape
                    '\\' => Some(28),
                    ']' => Some(29),
                    '^' => Some(30),
                    '_' => Some(31),
                    _ => None,
                };
                return ctrl_char.map(|b| vec![b]);
            }
        }

        // Regular character input
        Some(c.as_bytes().to_vec())
    }

    /// Determine if a mouse button press starts a selection
    pub fn is_selection_start(button: MouseButton, state: ElementState) -> bool {
        button == MouseButton::Left && state == ElementState::Pressed
    }

    /// Determine if a mouse button release ends a selection
    pub fn is_selection_end(button: MouseButton, state: ElementState) -> bool {
        button == MouseButton::Left && state == ElementState::Released
    }

    /// Check if a mouse event is a context menu trigger (right-click)
    pub fn is_context_menu_trigger(button: MouseButton, state: ElementState) -> bool {
        button == MouseButton::Right && state == ElementState::Pressed
    }

    /// Check if middle mouse button was pressed (paste on Linux)
    pub fn is_middle_click_paste(button: MouseButton, state: ElementState) -> bool {
        button == MouseButton::Middle && state == ElementState::Pressed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // === ModifierState tests ===

    #[test]
    fn test_modifier_state_default() {
        let mods = ModifierState::default();
        assert!(!mods.command);
        assert!(!mods.control);
        assert!(!mods.shift);
        assert!(!mods.alt);
        assert!(mods.is_empty());
    }

    #[test]
    fn test_modifier_state_command() {
        let mods = ModifierState {
            command: true,
            ..Default::default()
        };
        assert!(mods.is_command());
        assert!(!mods.is_control());
        assert!(!mods.is_empty());
    }

    #[test]
    fn test_modifier_state_control() {
        let mods = ModifierState {
            control: true,
            ..Default::default()
        };
        assert!(!mods.is_command());
        assert!(mods.is_control());
        assert!(!mods.is_empty());
    }

    // === Shortcut detection tests ===

    #[test]
    fn test_is_app_shortcut_cmd_c() {
        let mods = ModifierState {
            command: true,
            ..Default::default()
        };
        let key = Key::Character("c".into());
        assert!(EventHandler::is_app_shortcut(&key, mods));
    }

    #[test]
    fn test_is_app_shortcut_cmd_v() {
        let mods = ModifierState {
            command: true,
            ..Default::default()
        };
        let key = Key::Character("v".into());
        assert!(EventHandler::is_app_shortcut(&key, mods));
    }

    #[test]
    fn test_is_app_shortcut_without_cmd() {
        let mods = ModifierState::default();
        let key = Key::Character("c".into());
        assert!(!EventHandler::is_app_shortcut(&key, mods));
    }

    #[test]
    fn test_is_app_shortcut_cmd_tab() {
        let mods = ModifierState {
            command: true,
            ..Default::default()
        };
        let key = Key::Named(NamedKey::Tab);
        assert!(EventHandler::is_app_shortcut(&key, mods));
    }

    // === Shortcut to command tests ===

    #[test]
    fn test_shortcut_copy() {
        let mods = ModifierState {
            command: true,
            ..Default::default()
        };
        let key = Key::Character("c".into());
        let cmd = EventHandler::shortcut_to_command(&key, mods);
        assert!(matches!(cmd, Some(Command::Copy(_))));
    }

    #[test]
    fn test_shortcut_paste() {
        let mods = ModifierState {
            command: true,
            ..Default::default()
        };
        let key = Key::Character("v".into());
        let cmd = EventHandler::shortcut_to_command(&key, mods);
        assert!(matches!(cmd, Some(Command::Paste)));
    }

    #[test]
    fn test_shortcut_new_tab() {
        let mods = ModifierState {
            command: true,
            ..Default::default()
        };
        let key = Key::Character("t".into());
        let cmd = EventHandler::shortcut_to_command(&key, mods);
        assert!(matches!(cmd, Some(Command::NewTab)));
    }

    #[test]
    fn test_shortcut_close_tab() {
        let mods = ModifierState {
            command: true,
            ..Default::default()
        };
        let key = Key::Character("w".into());
        let cmd = EventHandler::shortcut_to_command(&key, mods);
        assert!(matches!(cmd, Some(Command::CloseCurrentTab)));
    }

    #[test]
    fn test_shortcut_close_window() {
        let mods = ModifierState {
            command: true,
            shift: true,
            ..Default::default()
        };
        let key = Key::Character("w".into());
        let cmd = EventHandler::shortcut_to_command(&key, mods);
        assert!(matches!(cmd, Some(Command::CloseWindow)));
    }

    #[test]
    fn test_shortcut_next_tab() {
        let mods = ModifierState {
            command: true,
            ..Default::default()
        };
        let key = Key::Named(NamedKey::Tab);
        let cmd = EventHandler::shortcut_to_command(&key, mods);
        assert!(matches!(cmd, Some(Command::NextTab)));
    }

    #[test]
    fn test_shortcut_prev_tab() {
        let mods = ModifierState {
            command: true,
            shift: true,
            ..Default::default()
        };
        let key = Key::Named(NamedKey::Tab);
        let cmd = EventHandler::shortcut_to_command(&key, mods);
        assert!(matches!(cmd, Some(Command::PreviousTab)));
    }

    #[test]
    fn test_shortcut_zoom_in() {
        let mods = ModifierState {
            command: true,
            ..Default::default()
        };
        let key = Key::Character("=".into());
        let cmd = EventHandler::shortcut_to_command(&key, mods);
        assert!(matches!(cmd, Some(Command::AdjustFontScale(delta)) if delta > 0.0));
    }

    #[test]
    fn test_shortcut_zoom_out() {
        let mods = ModifierState {
            command: true,
            ..Default::default()
        };
        let key = Key::Character("-".into());
        let cmd = EventHandler::shortcut_to_command(&key, mods);
        assert!(matches!(cmd, Some(Command::AdjustFontScale(delta)) if delta < 0.0));
    }

    #[test]
    fn test_shortcut_reset_zoom() {
        let mods = ModifierState {
            command: true,
            ..Default::default()
        };
        let key = Key::Character("0".into());
        let cmd = EventHandler::shortcut_to_command(&key, mods);
        assert!(matches!(cmd, Some(Command::ResetFontScale)));
    }

    // === Key to bytes tests ===

    #[test]
    fn test_key_enter() {
        let mods = ModifierState::default();
        let key = Key::Named(NamedKey::Enter);
        let bytes = EventHandler::key_to_terminal_bytes(&key, mods);
        assert_eq!(bytes, Some(b"\r".to_vec()));
    }

    #[test]
    fn test_key_backspace() {
        let mods = ModifierState::default();
        let key = Key::Named(NamedKey::Backspace);
        let bytes = EventHandler::key_to_terminal_bytes(&key, mods);
        assert_eq!(bytes, Some(b"\x7f".to_vec()));
    }

    #[test]
    fn test_key_escape() {
        let mods = ModifierState::default();
        let key = Key::Named(NamedKey::Escape);
        let bytes = EventHandler::key_to_terminal_bytes(&key, mods);
        assert_eq!(bytes, Some(b"\x1b".to_vec()));
    }

    #[test]
    fn test_key_arrow_up() {
        let mods = ModifierState::default();
        let key = Key::Named(NamedKey::ArrowUp);
        let bytes = EventHandler::key_to_terminal_bytes(&key, mods);
        assert_eq!(bytes, Some(b"\x1b[A".to_vec()));
    }

    #[test]
    fn test_key_character() {
        let mods = ModifierState::default();
        let key = Key::Character("a".into());
        let bytes = EventHandler::key_to_terminal_bytes(&key, mods);
        assert_eq!(bytes, Some(b"a".to_vec()));
    }

    #[test]
    fn test_ctrl_c() {
        let mods = ModifierState {
            control: true,
            ..Default::default()
        };
        let key = Key::Character("c".into());
        let bytes = EventHandler::key_to_terminal_bytes(&key, mods);
        assert_eq!(bytes, Some(vec![3])); // ETX (Ctrl+C)
    }

    #[test]
    fn test_ctrl_a() {
        let mods = ModifierState {
            control: true,
            ..Default::default()
        };
        let key = Key::Character("a".into());
        let bytes = EventHandler::key_to_terminal_bytes(&key, mods);
        assert_eq!(bytes, Some(vec![1])); // SOH (Ctrl+A)
    }

    #[test]
    fn test_ctrl_z() {
        let mods = ModifierState {
            control: true,
            ..Default::default()
        };
        let key = Key::Character("z".into());
        let bytes = EventHandler::key_to_terminal_bytes(&key, mods);
        assert_eq!(bytes, Some(vec![26])); // SUB (Ctrl+Z)
    }

    #[test]
    fn test_cmd_blocks_terminal_bytes() {
        let mods = ModifierState {
            command: true,
            ..Default::default()
        };
        let key = Key::Character("c".into());
        let bytes = EventHandler::key_to_terminal_bytes(&key, mods);
        assert_eq!(bytes, None);
    }

    // === Mouse event tests ===

    #[test]
    fn test_selection_start() {
        assert!(EventHandler::is_selection_start(
            MouseButton::Left,
            ElementState::Pressed
        ));
        assert!(!EventHandler::is_selection_start(
            MouseButton::Left,
            ElementState::Released
        ));
        assert!(!EventHandler::is_selection_start(
            MouseButton::Right,
            ElementState::Pressed
        ));
    }

    #[test]
    fn test_selection_end() {
        assert!(EventHandler::is_selection_end(
            MouseButton::Left,
            ElementState::Released
        ));
        assert!(!EventHandler::is_selection_end(
            MouseButton::Left,
            ElementState::Pressed
        ));
    }

    #[test]
    fn test_context_menu_trigger() {
        assert!(EventHandler::is_context_menu_trigger(
            MouseButton::Right,
            ElementState::Pressed
        ));
        assert!(!EventHandler::is_context_menu_trigger(
            MouseButton::Right,
            ElementState::Released
        ));
        assert!(!EventHandler::is_context_menu_trigger(
            MouseButton::Left,
            ElementState::Pressed
        ));
    }

    #[test]
    fn test_middle_click_paste() {
        assert!(EventHandler::is_middle_click_paste(
            MouseButton::Middle,
            ElementState::Pressed
        ));
        assert!(!EventHandler::is_middle_click_paste(
            MouseButton::Middle,
            ElementState::Released
        ));
    }

    // === Keyboard result and mouse result tests ===

    #[test]
    fn test_keyboard_result_debug() {
        let result = KeyboardResult::Handled;
        let debug = format!("{:?}", result);
        assert!(debug.contains("Handled"));
    }

    #[test]
    fn test_mouse_result_debug() {
        let result = MouseResult::None;
        let debug = format!("{:?}", result);
        assert!(debug.contains("None"));
    }
}
