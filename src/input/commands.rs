//! Command enum for testable input handling
//!
//! Commands represent the intent of user input without side effects.
//! Input handlers return Commands which are then executed by the command dispatcher.

use crt_core::Point;

/// Selection mode for terminal text selection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionMode {
    /// Character-by-character selection
    Simple,
    /// Word selection (double-click)
    Word,
    /// Line selection (triple-click)
    Line,
    /// Block/rectangular selection
    Block,
}

/// Commands that can be returned by input handlers
///
/// These commands represent user intent without performing any side effects.
/// The command dispatcher is responsible for executing these commands.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    // === Terminal I/O ===
    /// Send bytes to the PTY
    SendToPty(Vec<u8>),
    /// Scroll by a number of lines (positive = up, negative = down)
    ScrollLines(i32),
    /// Scroll to the top of the scrollback buffer
    ScrollToTop,
    /// Scroll to the bottom (live output)
    ScrollToBottom,

    // === Selection ===
    /// Start a new selection at the given point with the specified mode
    StartSelection(Point, SelectionMode),
    /// Update the selection endpoint to the given point
    UpdateSelection(Point),
    /// End the current selection (mouse release)
    EndSelection,
    /// Clear the current selection
    ClearSelection,

    // === Clipboard ===
    /// Copy the given text to the clipboard
    Copy(String),
    /// Paste from clipboard to terminal
    Paste,

    // === Navigation ===
    /// Open a URL in the default browser
    OpenUrl(String),
    /// Switch to a specific tab by ID
    SwitchTab(u64),
    /// Switch to the next tab
    NextTab,
    /// Switch to the previous tab
    PreviousTab,
    /// Create a new tab
    NewTab,
    /// Close a specific tab by ID
    CloseTab(u64),
    /// Close the currently active tab
    CloseCurrentTab,

    // === Window ===
    /// Create a new window
    NewWindow,
    /// Close the current window
    CloseWindow,
    /// Toggle fullscreen mode
    ToggleFullscreen,

    // === Search ===
    /// Open the search UI
    OpenSearch,
    /// Close the search UI
    CloseSearch,
    /// Find the next search match
    FindNext,
    /// Find the previous search match
    FindPrevious,
    /// Set the search query
    SetSearchQuery(String),

    // === Display ===
    /// Request a redraw of the terminal
    RequestRedraw,
    /// Adjust font scale by a delta (positive = larger, negative = smaller)
    AdjustFontScale(f32),
    /// Reset font scale to default
    ResetFontScale,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crt_core::{Column, Line};

    #[test]
    fn command_is_debug() {
        let cmd = Command::SendToPty(vec![0x1b, b'[', b'A']);
        let debug_str = format!("{:?}", cmd);
        assert!(debug_str.contains("SendToPty"));
    }

    #[test]
    fn command_is_clone() {
        let cmd = Command::ScrollLines(5);
        let cloned = cmd.clone();
        assert_eq!(cmd, cloned);
    }

    #[test]
    fn command_is_partial_eq() {
        assert_eq!(Command::NewTab, Command::NewTab);
        assert_ne!(Command::NewTab, Command::CloseCurrentTab);
        assert_eq!(Command::ScrollLines(5), Command::ScrollLines(5));
        assert_ne!(Command::ScrollLines(5), Command::ScrollLines(10));
    }

    #[test]
    fn selection_mode_is_copy() {
        let mode = SelectionMode::Word;
        let copied = mode;
        assert_eq!(mode, copied);
    }

    #[test]
    fn selection_command_with_point() {
        let point = Point::new(Line(5), Column(10));
        let cmd = Command::StartSelection(point, SelectionMode::Simple);

        if let Command::StartSelection(p, m) = cmd {
            assert_eq!(p.line, Line(5));
            assert_eq!(p.column, Column(10));
            assert_eq!(m, SelectionMode::Simple);
        } else {
            panic!("Expected StartSelection command");
        }
    }
}
