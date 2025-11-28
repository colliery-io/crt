//! Window lifecycle management
//!
//! This module provides centralized management of application windows,
//! including creation, destruction, focus tracking, and iteration.

use std::collections::HashMap;
use winit::window::WindowId;

use crate::window::WindowState;

/// Manages all application windows and their lifecycle
///
/// This struct centralizes window management that was previously scattered
/// in the main App struct, providing a cleaner API for multi-window operations.
#[derive(Default)]
pub struct WindowManager {
    /// Map of window ID to window state
    windows: HashMap<WindowId, WindowState>,
    /// Currently focused window ID
    focused: Option<WindowId>,
}

impl WindowManager {
    /// Create a new empty window manager
    pub fn new() -> Self {
        Self {
            windows: HashMap::new(),
            focused: None,
        }
    }

    /// Insert a window state with its ID
    ///
    /// This is called after window creation to register the window.
    /// The window becomes focused automatically.
    pub fn insert(&mut self, id: WindowId, state: WindowState) {
        self.windows.insert(id, state);
        self.focused = Some(id);
    }

    /// Remove a window by ID
    ///
    /// Returns the removed window state if it existed.
    /// If the removed window was focused, focus moves to another window.
    pub fn remove(&mut self, id: WindowId) -> Option<WindowState> {
        let state = self.windows.remove(&id);
        if self.focused == Some(id) {
            // Move focus to another window if available
            self.focused = self.windows.keys().next().copied();
        }
        state
    }

    /// Close a window by ID
    ///
    /// Returns `true` if this was the last window (app should exit).
    pub fn close(&mut self, id: WindowId) -> bool {
        self.remove(id);
        self.windows.is_empty()
    }

    /// Get the focused window ID
    pub fn focused_id(&self) -> Option<WindowId> {
        self.focused
    }

    /// Get the focused window
    pub fn focused(&self) -> Option<&WindowState> {
        self.focused.and_then(|id| self.windows.get(&id))
    }

    /// Get the focused window mutably
    pub fn focused_mut(&mut self) -> Option<&mut WindowState> {
        self.focused.and_then(|id| self.windows.get_mut(&id))
    }

    /// Set which window has focus
    ///
    /// Only sets focus if the window ID exists in the manager.
    pub fn set_focused(&mut self, id: WindowId) {
        if self.windows.contains_key(&id) {
            self.focused = Some(id);
        }
    }

    /// Check if a window ID has focus
    pub fn is_focused(&self, id: WindowId) -> bool {
        self.focused == Some(id)
    }

    /// Get a window by ID
    pub fn get(&self, id: WindowId) -> Option<&WindowState> {
        self.windows.get(&id)
    }

    /// Get a window mutably by ID
    pub fn get_mut(&mut self, id: WindowId) -> Option<&mut WindowState> {
        self.windows.get_mut(&id)
    }

    /// Check if a window exists
    pub fn contains(&self, id: WindowId) -> bool {
        self.windows.contains_key(&id)
    }

    /// Iterate over all windows
    pub fn iter(&self) -> impl Iterator<Item = (&WindowId, &WindowState)> {
        self.windows.iter()
    }

    /// Iterate mutably over all windows
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&WindowId, &mut WindowState)> {
        self.windows.iter_mut()
    }

    /// Get all window IDs
    pub fn ids(&self) -> impl Iterator<Item = &WindowId> {
        self.windows.keys()
    }

    /// Get window count
    pub fn len(&self) -> usize {
        self.windows.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.windows.is_empty()
    }

    /// Get all windows as a slice (for batch operations)
    pub fn values(&self) -> impl Iterator<Item = &WindowState> {
        self.windows.values()
    }

    /// Get all windows mutably
    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut WindowState> {
        self.windows.values_mut()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: We can't fully test WindowManager without mocking WindowId,
    // but we can test the basic logic with unsafe ID creation for testing.

    fn mock_window_id(id: u64) -> WindowId {
        // WindowId is opaque, so we use this workaround for testing
        // This works because WindowId is just a wrapper around a platform-specific ID
        unsafe { std::mem::transmute(id) }
    }

    #[test]
    fn test_new() {
        let manager = WindowManager::new();
        assert!(manager.is_empty());
        assert_eq!(manager.len(), 0);
        assert!(manager.focused_id().is_none());
    }

    #[test]
    fn test_default() {
        let manager = WindowManager::default();
        assert!(manager.is_empty());
    }

    // Note: Full integration tests would require actual WindowState instances,
    // which need GPU context. The struct design is validated by compilation
    // and the basic logic tests above.
}
