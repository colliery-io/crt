//! Default Keybindings
//!
//! Hardcoded keybindings for tab and window operations.
//! These are not yet configurable via config.toml.
//!
//! Platform-specific modifier keys:
//! - macOS: Cmd (logo key) is used as the primary modifier
//! - Linux/Windows: Ctrl is used as the primary modifier
//!
//! The `logo` field in `Modifiers` represents Cmd on macOS.
//! At runtime, the application should check the platform and use
//! the appropriate modifier key.

use std::collections::HashMap;

/// Actions that can be triggered by keybindings
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    // Tab operations
    NewTab,
    CloseTab,
    NextTab,
    PreviousTab,
    SelectTab1,
    SelectTab2,
    SelectTab3,
    SelectTab4,
    SelectTab5,
    SelectTab6,
    SelectTab7,
    SelectTab8,
    SelectTab9,
    LastTab,

    // Window operations
    NewWindow,
    CloseWindow,

    // Terminal operations
    Copy,
    Paste,
    SelectAll,
    ClearScrollback,
    ResetTerminal,

    // Font size
    IncreaseFontSize,
    DecreaseFontSize,
    ResetFontSize,

    // Scrolling
    ScrollUp,
    ScrollDown,
    ScrollPageUp,
    ScrollPageDown,
    ScrollToTop,
    ScrollToBottom,

    // Search
    Find,
    FindNext,
    FindPrevious,
}

/// Modifier keys
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Modifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub logo: bool, // Cmd on macOS, Win on Windows
}

impl Modifiers {
    pub const fn none() -> Self {
        Self {
            ctrl: false,
            alt: false,
            shift: false,
            logo: false,
        }
    }

    pub const fn ctrl() -> Self {
        Self {
            ctrl: true,
            alt: false,
            shift: false,
            logo: false,
        }
    }

    pub const fn logo() -> Self {
        Self {
            ctrl: false,
            alt: false,
            shift: false,
            logo: true,
        }
    }

    pub const fn logo_shift() -> Self {
        Self {
            ctrl: false,
            alt: false,
            shift: true,
            logo: true,
        }
    }

    pub const fn shift() -> Self {
        Self {
            ctrl: false,
            alt: false,
            shift: true,
            logo: false,
        }
    }

    pub const fn ctrl_shift() -> Self {
        Self {
            ctrl: true,
            alt: false,
            shift: true,
            logo: false,
        }
    }
}

/// A key code (simplified, maps to winit VirtualKeyCode)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    // Letters
    A, B, C, D, E, F, G, H, I, J, K, L, M,
    N, O, P, Q, R, S, T, U, V, W, X, Y, Z,

    // Numbers
    Key0, Key1, Key2, Key3, Key4, Key5, Key6, Key7, Key8, Key9,

    // Function keys
    F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12,

    // Special keys
    Escape,
    Tab,
    Backspace,
    Enter,
    Space,
    Insert,
    Delete,
    Home,
    End,
    PageUp,
    PageDown,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,

    // Punctuation
    Minus,
    Equal,
    Plus,
    BracketLeft,
    BracketRight,
    Backslash,
    Semicolon,
    Apostrophe,
    Comma,
    Period,
    Slash,
    Grave,
}

/// A complete keybinding (modifiers + key)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Keybinding {
    pub modifiers: Modifiers,
    pub key: Key,
}

impl Keybinding {
    pub const fn new(modifiers: Modifiers, key: Key) -> Self {
        Self { modifiers, key }
    }
}

/// Default keybindings configuration
pub struct Keybindings {
    bindings: HashMap<Keybinding, Action>,
}

impl Default for Keybindings {
    fn default() -> Self {
        Self::new()
    }
}

impl Keybindings {
    /// Create the default keybindings
    pub fn new() -> Self {
        let mut bindings = HashMap::new();

        // Tab operations (Cmd+key on macOS)
        bindings.insert(
            Keybinding::new(Modifiers::logo(), Key::T),
            Action::NewTab,
        );
        bindings.insert(
            Keybinding::new(Modifiers::logo(), Key::W),
            Action::CloseTab,
        );
        bindings.insert(
            Keybinding::new(Modifiers::logo_shift(), Key::BracketRight),
            Action::NextTab,
        );
        bindings.insert(
            Keybinding::new(Modifiers::logo_shift(), Key::BracketLeft),
            Action::PreviousTab,
        );

        // Tab selection (Cmd+1-9)
        bindings.insert(
            Keybinding::new(Modifiers::logo(), Key::Key1),
            Action::SelectTab1,
        );
        bindings.insert(
            Keybinding::new(Modifiers::logo(), Key::Key2),
            Action::SelectTab2,
        );
        bindings.insert(
            Keybinding::new(Modifiers::logo(), Key::Key3),
            Action::SelectTab3,
        );
        bindings.insert(
            Keybinding::new(Modifiers::logo(), Key::Key4),
            Action::SelectTab4,
        );
        bindings.insert(
            Keybinding::new(Modifiers::logo(), Key::Key5),
            Action::SelectTab5,
        );
        bindings.insert(
            Keybinding::new(Modifiers::logo(), Key::Key6),
            Action::SelectTab6,
        );
        bindings.insert(
            Keybinding::new(Modifiers::logo(), Key::Key7),
            Action::SelectTab7,
        );
        bindings.insert(
            Keybinding::new(Modifiers::logo(), Key::Key8),
            Action::SelectTab8,
        );
        bindings.insert(
            Keybinding::new(Modifiers::logo(), Key::Key9),
            Action::SelectTab9,
        );

        // Last tab (Cmd+0 or Cmd+9 can be used)
        bindings.insert(
            Keybinding::new(Modifiers::logo(), Key::Key0),
            Action::LastTab,
        );

        // Window operations
        bindings.insert(
            Keybinding::new(Modifiers::logo(), Key::N),
            Action::NewWindow,
        );
        bindings.insert(
            Keybinding::new(Modifiers::logo_shift(), Key::W),
            Action::CloseWindow,
        );

        // Copy/Paste (standard macOS)
        bindings.insert(
            Keybinding::new(Modifiers::logo(), Key::C),
            Action::Copy,
        );
        bindings.insert(
            Keybinding::new(Modifiers::logo(), Key::V),
            Action::Paste,
        );
        bindings.insert(
            Keybinding::new(Modifiers::logo(), Key::A),
            Action::SelectAll,
        );

        // Terminal operations
        bindings.insert(
            Keybinding::new(Modifiers::logo(), Key::K),
            Action::ClearScrollback,
        );

        // Font size (Cmd++/Cmd+-)
        bindings.insert(
            Keybinding::new(Modifiers::logo(), Key::Equal),
            Action::IncreaseFontSize,
        );
        bindings.insert(
            Keybinding::new(Modifiers::logo_shift(), Key::Equal),
            Action::IncreaseFontSize, // Cmd+Shift+= is also +
        );
        bindings.insert(
            Keybinding::new(Modifiers::logo(), Key::Minus),
            Action::DecreaseFontSize,
        );
        bindings.insert(
            Keybinding::new(Modifiers::logo(), Key::Key0),
            Action::ResetFontSize,
        );

        // Scrolling
        bindings.insert(
            Keybinding::new(Modifiers::shift(), Key::PageUp),
            Action::ScrollPageUp,
        );
        bindings.insert(
            Keybinding::new(Modifiers::shift(), Key::PageDown),
            Action::ScrollPageDown,
        );
        bindings.insert(
            Keybinding::new(Modifiers::logo(), Key::Home),
            Action::ScrollToTop,
        );
        bindings.insert(
            Keybinding::new(Modifiers::logo(), Key::End),
            Action::ScrollToBottom,
        );

        // Search
        bindings.insert(
            Keybinding::new(Modifiers::logo(), Key::F),
            Action::Find,
        );
        bindings.insert(
            Keybinding::new(Modifiers::logo(), Key::G),
            Action::FindNext,
        );
        bindings.insert(
            Keybinding::new(Modifiers::logo_shift(), Key::G),
            Action::FindPrevious,
        );

        Self { bindings }
    }

    /// Look up an action for a keybinding
    pub fn get_action(&self, keybinding: &Keybinding) -> Option<Action> {
        self.bindings.get(keybinding).copied()
    }

    /// Check if a keybinding matches any action
    pub fn matches(&self, modifiers: Modifiers, key: Key) -> Option<Action> {
        self.get_action(&Keybinding::new(modifiers, key))
    }

    /// Get all keybindings for a specific action
    pub fn get_bindings_for_action(&self, action: Action) -> Vec<Keybinding> {
        self.bindings
            .iter()
            .filter(|&(_, a)| *a == action)
            .map(|(&kb, _)| kb)
            .collect()
    }

    /// List all registered keybindings
    pub fn all_bindings(&self) -> impl Iterator<Item = (&Keybinding, &Action)> {
        self.bindings.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_keybindings() {
        let kb = Keybindings::new();

        // Test Cmd+T opens new tab
        assert_eq!(
            kb.matches(Modifiers::logo(), Key::T),
            Some(Action::NewTab)
        );

        // Test Cmd+W closes tab
        assert_eq!(
            kb.matches(Modifiers::logo(), Key::W),
            Some(Action::CloseTab)
        );

        // Test Cmd+N opens new window
        assert_eq!(
            kb.matches(Modifiers::logo(), Key::N),
            Some(Action::NewWindow)
        );
    }

    #[test]
    fn test_tab_selection() {
        let kb = Keybindings::new();

        assert_eq!(
            kb.matches(Modifiers::logo(), Key::Key1),
            Some(Action::SelectTab1)
        );
        assert_eq!(
            kb.matches(Modifiers::logo(), Key::Key5),
            Some(Action::SelectTab5)
        );
        assert_eq!(
            kb.matches(Modifiers::logo(), Key::Key9),
            Some(Action::SelectTab9)
        );
    }

    #[test]
    fn test_copy_paste() {
        let kb = Keybindings::new();

        assert_eq!(
            kb.matches(Modifiers::logo(), Key::C),
            Some(Action::Copy)
        );
        assert_eq!(
            kb.matches(Modifiers::logo(), Key::V),
            Some(Action::Paste)
        );
    }

    #[test]
    fn test_no_match() {
        let kb = Keybindings::new();

        // Random key with no binding
        assert_eq!(kb.matches(Modifiers::none(), Key::Z), None);

        // Key with wrong modifiers
        assert_eq!(kb.matches(Modifiers::ctrl(), Key::T), None);
    }

    #[test]
    fn test_get_bindings_for_action() {
        let kb = Keybindings::new();

        let copy_bindings = kb.get_bindings_for_action(Action::Copy);
        assert!(!copy_bindings.is_empty());
        assert!(copy_bindings.contains(&Keybinding::new(Modifiers::logo(), Key::C)));
    }

    #[test]
    fn test_modifiers() {
        assert_eq!(Modifiers::none(), Modifiers { ctrl: false, alt: false, shift: false, logo: false });
        assert!(Modifiers::logo().logo);
        assert!(Modifiers::ctrl().ctrl);
        assert!(Modifiers::shift().shift);
        assert!(Modifiers::logo_shift().logo);
        assert!(Modifiers::logo_shift().shift);
    }
}
