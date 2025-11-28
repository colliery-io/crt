//! Tab state management
//!
//! This module provides testable tab management without requiring GPU or window
//! dependencies. Tab state tracks the list of tabs, active tab, and metadata.

use std::collections::HashMap;

/// Unique identifier for a tab
pub type TabId = u64;

/// Metadata about a single tab (no GPU resources)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabInfo {
    /// Unique identifier for this tab
    pub id: TabId,
    /// Display title for the tab
    pub title: String,
    /// Whether this tab has unseen activity (for visual indicator)
    pub has_activity: bool,
    /// Whether this tab is currently being edited (title rename)
    pub is_editing: bool,
}

impl TabInfo {
    /// Create a new tab with the given ID and title
    pub fn new(id: TabId, title: impl Into<String>) -> Self {
        Self {
            id,
            title: title.into(),
            has_activity: false,
            is_editing: false,
        }
    }
}

/// Manages tab list and active tab selection
///
/// This struct handles all tab-related state without any rendering or
/// window-specific logic, making it fully testable.
#[derive(Debug, Clone)]
pub struct TabState {
    /// List of all tabs
    tabs: Vec<TabInfo>,
    /// Index of the currently active tab
    active_index: usize,
    /// Next ID to assign to a new tab
    next_id: TabId,
    /// Content hash per tab (used to detect changes and skip unnecessary redraws)
    content_hashes: HashMap<TabId, u64>,
}

impl Default for TabState {
    fn default() -> Self {
        Self::new()
    }
}

impl TabState {
    /// Create a new empty tab state
    pub fn new() -> Self {
        Self {
            tabs: Vec::new(),
            active_index: 0,
            next_id: 1,
            content_hashes: HashMap::new(),
        }
    }

    /// Create with a custom starting ID (useful for testing)
    pub fn with_start_id(start_id: TabId) -> Self {
        Self {
            tabs: Vec::new(),
            active_index: 0,
            next_id: start_id,
            content_hashes: HashMap::new(),
        }
    }

    /// Add a new tab with the given title and return its ID
    ///
    /// The tab is added to the end of the list but is not automatically activated.
    pub fn add_tab(&mut self, title: impl Into<String>) -> TabId {
        let id = self.next_id;
        self.next_id += 1;
        self.tabs.push(TabInfo::new(id, title));
        id
    }

    /// Add a tab and make it the active tab
    pub fn add_tab_and_activate(&mut self, title: impl Into<String>) -> TabId {
        let id = self.add_tab(title);
        self.switch_to(id);
        id
    }

    /// Close a tab by ID
    ///
    /// Returns the closed tab info if found. After closing:
    /// - If the closed tab was active, the new active tab will be the previous tab
    ///   or the first tab if there is no previous tab.
    /// - The active index is adjusted to maintain the same active tab when possible.
    pub fn close_tab(&mut self, id: TabId) -> Option<TabInfo> {
        let index = self.tabs.iter().position(|t| t.id == id)?;
        let tab = self.tabs.remove(index);

        // Adjust active index if needed
        if self.tabs.is_empty() {
            self.active_index = 0;
        } else if index < self.active_index {
            // Closed tab was before active, shift active index down
            self.active_index -= 1;
        } else if index == self.active_index {
            // Closed the active tab
            if self.active_index >= self.tabs.len() {
                // Was last tab, move to new last
                self.active_index = self.tabs.len().saturating_sub(1);
            }
            // Otherwise keep same index (now pointing to next tab)
        }
        // If index > active_index, no adjustment needed

        Some(tab)
    }

    /// Close the currently active tab
    ///
    /// Returns the closed tab info if there was an active tab.
    pub fn close_active(&mut self) -> Option<TabInfo> {
        let id = self.active_id()?;
        self.close_tab(id)
    }

    /// Switch to a tab by ID
    ///
    /// Returns true if the tab was found and activated, false otherwise.
    pub fn switch_to(&mut self, id: TabId) -> bool {
        if let Some(index) = self.tabs.iter().position(|t| t.id == id) {
            self.active_index = index;
            // Clear activity indicator when switching to a tab
            self.tabs[index].has_activity = false;
            true
        } else {
            false
        }
    }

    /// Switch to a tab by index
    ///
    /// Returns true if the index was valid, false otherwise.
    pub fn switch_to_index(&mut self, index: usize) -> bool {
        if index < self.tabs.len() {
            self.active_index = index;
            self.tabs[index].has_activity = false;
            true
        } else {
            false
        }
    }

    /// Switch to the next tab (wraps around)
    pub fn next_tab(&mut self) {
        if !self.tabs.is_empty() {
            self.active_index = (self.active_index + 1) % self.tabs.len();
            self.tabs[self.active_index].has_activity = false;
        }
    }

    /// Switch to the previous tab (wraps around)
    pub fn previous_tab(&mut self) {
        if !self.tabs.is_empty() {
            self.active_index = if self.active_index == 0 {
                self.tabs.len() - 1
            } else {
                self.active_index - 1
            };
            self.tabs[self.active_index].has_activity = false;
        }
    }

    /// Get a reference to the currently active tab
    pub fn active(&self) -> Option<&TabInfo> {
        self.tabs.get(self.active_index)
    }

    /// Get a mutable reference to the currently active tab
    pub fn active_mut(&mut self) -> Option<&mut TabInfo> {
        self.tabs.get_mut(self.active_index)
    }

    /// Get the active tab ID
    pub fn active_id(&self) -> Option<TabId> {
        self.active().map(|t| t.id)
    }

    /// Get the active tab index
    pub fn active_index(&self) -> usize {
        self.active_index
    }

    /// Get a reference to all tabs
    pub fn tabs(&self) -> &[TabInfo] {
        &self.tabs
    }

    /// Get a mutable reference to all tabs
    pub fn tabs_mut(&mut self) -> &mut [TabInfo] {
        &mut self.tabs
    }

    /// Get a tab by ID
    pub fn get(&self, id: TabId) -> Option<&TabInfo> {
        self.tabs.iter().find(|t| t.id == id)
    }

    /// Get a mutable tab by ID
    pub fn get_mut(&mut self, id: TabId) -> Option<&mut TabInfo> {
        self.tabs.iter_mut().find(|t| t.id == id)
    }

    /// Get the index of a tab by ID
    pub fn index_of(&self, id: TabId) -> Option<usize> {
        self.tabs.iter().position(|t| t.id == id)
    }

    /// Get tab count
    pub fn len(&self) -> usize {
        self.tabs.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }

    /// Update a tab's title
    pub fn set_title(&mut self, id: TabId, title: impl Into<String>) {
        if let Some(tab) = self.get_mut(id) {
            tab.title = title.into();
        }
    }

    /// Mark a tab as having activity (for visual indicator)
    ///
    /// Note: Activity is automatically cleared when switching to a tab.
    pub fn set_activity(&mut self, id: TabId, has_activity: bool) {
        if let Some(tab) = self.get_mut(id) {
            tab.has_activity = has_activity;
        }
    }

    /// Start editing a tab's title
    pub fn start_editing(&mut self, id: TabId) -> bool {
        // First clear any other editing state
        for tab in &mut self.tabs {
            tab.is_editing = false;
        }

        if let Some(tab) = self.get_mut(id) {
            tab.is_editing = true;
            true
        } else {
            false
        }
    }

    /// Stop editing a tab's title
    pub fn stop_editing(&mut self, id: TabId) {
        if let Some(tab) = self.get_mut(id) {
            tab.is_editing = false;
        }
    }

    /// Check if any tab is being edited
    pub fn is_editing(&self) -> bool {
        self.tabs.iter().any(|t| t.is_editing)
    }

    /// Get the ID of the tab being edited, if any
    pub fn editing_id(&self) -> Option<TabId> {
        self.tabs.iter().find(|t| t.is_editing).map(|t| t.id)
    }

    /// Move a tab from one index to another (for drag reordering)
    pub fn move_tab(&mut self, from_index: usize, to_index: usize) -> bool {
        if from_index >= self.tabs.len() || to_index >= self.tabs.len() {
            return false;
        }

        let tab = self.tabs.remove(from_index);

        // Adjust to_index if we removed before it
        let adjusted_to = if from_index < to_index {
            to_index - 1
        } else {
            to_index
        };

        self.tabs.insert(adjusted_to.min(self.tabs.len()), tab);

        // Adjust active index
        if from_index == self.active_index {
            self.active_index = adjusted_to.min(self.tabs.len() - 1);
        } else if from_index < self.active_index && adjusted_to >= self.active_index {
            self.active_index -= 1;
        } else if from_index > self.active_index && adjusted_to <= self.active_index {
            self.active_index += 1;
        }

        true
    }

    // === Content hash management ===

    /// Get the content hash for a tab
    pub fn content_hash(&self, tab_id: TabId) -> Option<u64> {
        self.content_hashes.get(&tab_id).copied()
    }

    /// Set the content hash for a tab
    pub fn set_content_hash(&mut self, tab_id: TabId, hash: u64) {
        self.content_hashes.insert(tab_id, hash);
    }

    /// Invalidate (clear) the content hash for a tab, forcing a redraw
    pub fn invalidate_content_hash(&mut self, tab_id: TabId) {
        self.content_hashes.insert(tab_id, 0);
    }

    /// Invalidate content hashes for all tabs
    pub fn invalidate_all_content_hashes(&mut self) {
        for hash in self.content_hashes.values_mut() {
            *hash = 0;
        }
    }

    /// Remove content hash for a tab (when tab is closed)
    pub fn remove_content_hash(&mut self, tab_id: TabId) {
        self.content_hashes.remove(&tab_id);
    }

    /// Invalidate the active tab's content hash, forcing a redraw
    /// Returns true if there was an active tab to invalidate
    pub fn invalidate_active_tab_content(&mut self) -> bool {
        if let Some(tab_id) = self.active_id() {
            self.content_hashes.insert(tab_id, 0);
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // === Basic operations ===

    #[test]
    fn test_new() {
        let state = TabState::new();
        assert!(state.is_empty());
        assert_eq!(state.len(), 0);
        assert!(state.active().is_none());
        assert!(state.active_id().is_none());
    }

    #[test]
    fn test_with_start_id() {
        let mut state = TabState::with_start_id(100);
        let id = state.add_tab("Tab");
        assert_eq!(id, 100);
    }

    #[test]
    fn test_add_tab() {
        let mut state = TabState::new();
        let id = state.add_tab("Tab 1");

        assert_eq!(state.len(), 1);
        assert_eq!(state.tabs()[0].title, "Tab 1");
        assert_eq!(id, 1);
        // First tab becomes active
        assert_eq!(state.active_index(), 0);
    }

    #[test]
    fn test_add_multiple_tabs() {
        let mut state = TabState::new();
        let id1 = state.add_tab("Tab 1");
        let id2 = state.add_tab("Tab 2");
        let id3 = state.add_tab("Tab 3");

        assert_eq!(state.len(), 3);
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
        assert_eq!(id3, 3);
    }

    #[test]
    fn test_add_tab_and_activate() {
        let mut state = TabState::new();
        state.add_tab("Tab 1");
        let id2 = state.add_tab_and_activate("Tab 2");

        assert_eq!(state.active_id(), Some(id2));
    }

    // === Switch operations ===

    #[test]
    fn test_switch_to() {
        let mut state = TabState::new();
        let id1 = state.add_tab("Tab 1");
        let id2 = state.add_tab("Tab 2");

        state.switch_to(id2);
        assert_eq!(state.active_id(), Some(id2));

        state.switch_to(id1);
        assert_eq!(state.active_id(), Some(id1));
    }

    #[test]
    fn test_switch_to_invalid() {
        let mut state = TabState::new();
        state.add_tab("Tab 1");

        let result = state.switch_to(999);
        assert!(!result);
        assert_eq!(state.active_index(), 0);
    }

    #[test]
    fn test_switch_to_index() {
        let mut state = TabState::new();
        state.add_tab("Tab 1");
        state.add_tab("Tab 2");
        state.add_tab("Tab 3");

        assert!(state.switch_to_index(2));
        assert_eq!(state.active_index(), 2);

        assert!(!state.switch_to_index(5));
        assert_eq!(state.active_index(), 2); // unchanged
    }

    #[test]
    fn test_switch_clears_activity() {
        let mut state = TabState::new();
        let id1 = state.add_tab("Tab 1");
        let id2 = state.add_tab("Tab 2");

        state.set_activity(id2, true);
        assert!(state.get(id2).unwrap().has_activity);

        state.switch_to(id2);
        assert!(!state.get(id2).unwrap().has_activity);

        state.set_activity(id1, true);
        state.switch_to_index(0);
        assert!(!state.get(id1).unwrap().has_activity);
    }

    // === Next/Previous ===

    #[test]
    fn test_next_tab() {
        let mut state = TabState::new();
        state.add_tab("Tab 1");
        state.add_tab("Tab 2");
        state.add_tab("Tab 3");

        assert_eq!(state.active_index(), 0);

        state.next_tab();
        assert_eq!(state.active_index(), 1);

        state.next_tab();
        assert_eq!(state.active_index(), 2);

        state.next_tab(); // Wraps to 0
        assert_eq!(state.active_index(), 0);
    }

    #[test]
    fn test_previous_tab() {
        let mut state = TabState::new();
        state.add_tab("Tab 1");
        state.add_tab("Tab 2");
        state.add_tab("Tab 3");

        assert_eq!(state.active_index(), 0);

        state.previous_tab(); // Wraps to 2
        assert_eq!(state.active_index(), 2);

        state.previous_tab();
        assert_eq!(state.active_index(), 1);

        state.previous_tab();
        assert_eq!(state.active_index(), 0);
    }

    #[test]
    fn test_next_previous_empty() {
        let mut state = TabState::new();
        state.next_tab(); // Should not panic
        state.previous_tab(); // Should not panic
        assert_eq!(state.active_index(), 0);
    }

    #[test]
    fn test_next_previous_single_tab() {
        let mut state = TabState::new();
        state.add_tab("Tab 1");

        state.next_tab();
        assert_eq!(state.active_index(), 0);

        state.previous_tab();
        assert_eq!(state.active_index(), 0);
    }

    // === Close operations ===

    #[test]
    fn test_close_tab() {
        let mut state = TabState::new();
        let id1 = state.add_tab("Tab 1");
        state.add_tab("Tab 2");

        let closed = state.close_tab(id1);
        assert!(closed.is_some());
        assert_eq!(closed.unwrap().title, "Tab 1");
        assert_eq!(state.len(), 1);
    }

    #[test]
    fn test_close_active_tab() {
        let mut state = TabState::new();
        let id1 = state.add_tab("Tab 1");
        let id2 = state.add_tab("Tab 2");
        state.switch_to(id2);

        state.close_tab(id2);

        assert_eq!(state.len(), 1);
        assert_eq!(state.active_id(), Some(id1));
    }

    #[test]
    fn test_close_tab_before_active() {
        let mut state = TabState::new();
        let id1 = state.add_tab("Tab 1");
        let id2 = state.add_tab("Tab 2");
        state.switch_to(id2);

        state.close_tab(id1);

        assert_eq!(state.active_id(), Some(id2));
        assert_eq!(state.active_index(), 0); // Index adjusted
    }

    #[test]
    fn test_close_tab_after_active() {
        let mut state = TabState::new();
        let id1 = state.add_tab("Tab 1");
        let id2 = state.add_tab("Tab 2");

        state.close_tab(id2);

        assert_eq!(state.active_id(), Some(id1));
        assert_eq!(state.active_index(), 0);
    }

    #[test]
    fn test_close_last_remaining_tab() {
        let mut state = TabState::new();
        let id = state.add_tab("Tab 1");

        state.close_tab(id);

        assert!(state.is_empty());
        assert!(state.active().is_none());
    }

    #[test]
    fn test_close_active() {
        let mut state = TabState::new();
        state.add_tab("Tab 1");
        let id2 = state.add_tab("Tab 2");
        state.switch_to(id2);

        let closed = state.close_active();
        assert_eq!(closed.unwrap().id, id2);
    }

    #[test]
    fn test_close_invalid() {
        let mut state = TabState::new();
        state.add_tab("Tab 1");

        let closed = state.close_tab(999);
        assert!(closed.is_none());
        assert_eq!(state.len(), 1);
    }

    // === Title and activity ===

    #[test]
    fn test_set_title() {
        let mut state = TabState::new();
        let id = state.add_tab("Original");

        state.set_title(id, "New Title");
        assert_eq!(state.get(id).unwrap().title, "New Title");
    }

    #[test]
    fn test_set_activity() {
        let mut state = TabState::new();
        let id = state.add_tab("Tab 1");

        assert!(!state.get(id).unwrap().has_activity);

        state.set_activity(id, true);
        assert!(state.get(id).unwrap().has_activity);

        state.set_activity(id, false);
        assert!(!state.get(id).unwrap().has_activity);
    }

    // === Editing ===

    #[test]
    fn test_editing() {
        let mut state = TabState::new();
        let id1 = state.add_tab("Tab 1");
        let id2 = state.add_tab("Tab 2");

        assert!(!state.is_editing());
        assert!(state.editing_id().is_none());

        state.start_editing(id1);
        assert!(state.is_editing());
        assert_eq!(state.editing_id(), Some(id1));

        // Starting edit on another tab stops previous
        state.start_editing(id2);
        assert!(state.is_editing());
        assert_eq!(state.editing_id(), Some(id2));
        assert!(!state.get(id1).unwrap().is_editing);

        state.stop_editing(id2);
        assert!(!state.is_editing());
    }

    // === Get operations ===

    #[test]
    fn test_get() {
        let mut state = TabState::new();
        let id = state.add_tab("Tab 1");

        assert!(state.get(id).is_some());
        assert!(state.get(999).is_none());
    }

    #[test]
    fn test_index_of() {
        let mut state = TabState::new();
        let id1 = state.add_tab("Tab 1");
        let id2 = state.add_tab("Tab 2");

        assert_eq!(state.index_of(id1), Some(0));
        assert_eq!(state.index_of(id2), Some(1));
        assert_eq!(state.index_of(999), None);
    }

    // === Move operations ===

    #[test]
    fn test_move_tab_forward() {
        let mut state = TabState::new();
        let id1 = state.add_tab("Tab 1");
        let id2 = state.add_tab("Tab 2");
        let id3 = state.add_tab("Tab 3");

        state.move_tab(0, 2);

        assert_eq!(state.tabs()[0].id, id2);
        assert_eq!(state.tabs()[1].id, id1);
        assert_eq!(state.tabs()[2].id, id3);
    }

    #[test]
    fn test_move_tab_backward() {
        let mut state = TabState::new();
        let id1 = state.add_tab("Tab 1");
        let id2 = state.add_tab("Tab 2");
        let id3 = state.add_tab("Tab 3");

        state.move_tab(2, 0);

        assert_eq!(state.tabs()[0].id, id3);
        assert_eq!(state.tabs()[1].id, id1);
        assert_eq!(state.tabs()[2].id, id2);
    }

    #[test]
    fn test_move_active_tab() {
        let mut state = TabState::new();
        state.add_tab("Tab 1");
        state.add_tab("Tab 2");
        state.add_tab("Tab 3");

        // Active is at 0
        state.move_tab(0, 2);
        assert_eq!(state.active_index(), 1); // Moved with the tab
    }

    #[test]
    fn test_move_tab_invalid() {
        let mut state = TabState::new();
        state.add_tab("Tab 1");

        assert!(!state.move_tab(0, 5));
        assert!(!state.move_tab(5, 0));
    }

    // === Clone ===

    #[test]
    fn test_clone() {
        let mut state = TabState::new();
        state.add_tab("Tab 1");
        state.add_tab("Tab 2");
        state.switch_to_index(1);

        let cloned = state.clone();
        assert_eq!(cloned.len(), 2);
        assert_eq!(cloned.active_index(), 1);
    }

    // === Content hash management ===

    #[test]
    fn test_content_hash_get_set() {
        let mut state = TabState::new();
        let id = state.add_tab("Tab 1");

        // Initially no hash
        assert_eq!(state.content_hash(id), None);

        // Set hash
        state.set_content_hash(id, 12345);
        assert_eq!(state.content_hash(id), Some(12345));

        // Update hash
        state.set_content_hash(id, 67890);
        assert_eq!(state.content_hash(id), Some(67890));
    }

    #[test]
    fn test_content_hash_invalidate() {
        let mut state = TabState::new();
        let id = state.add_tab("Tab 1");
        state.set_content_hash(id, 12345);

        state.invalidate_content_hash(id);
        assert_eq!(state.content_hash(id), Some(0));
    }

    #[test]
    fn test_content_hash_invalidate_all() {
        let mut state = TabState::new();
        let id1 = state.add_tab("Tab 1");
        let id2 = state.add_tab("Tab 2");
        state.set_content_hash(id1, 111);
        state.set_content_hash(id2, 222);

        state.invalidate_all_content_hashes();
        assert_eq!(state.content_hash(id1), Some(0));
        assert_eq!(state.content_hash(id2), Some(0));
    }

    #[test]
    fn test_content_hash_remove() {
        let mut state = TabState::new();
        let id = state.add_tab("Tab 1");
        state.set_content_hash(id, 12345);

        state.remove_content_hash(id);
        assert_eq!(state.content_hash(id), None);
    }

    #[test]
    fn test_invalidate_active_tab_content() {
        let mut state = TabState::new();

        // No active tab
        assert!(!state.invalidate_active_tab_content());

        // With active tab
        let id1 = state.add_tab_and_activate("Tab 1");
        let id2 = state.add_tab("Tab 2");
        state.set_content_hash(id1, 111);
        state.set_content_hash(id2, 222);

        assert!(state.invalidate_active_tab_content());
        assert_eq!(state.content_hash(id1), Some(0)); // Active tab invalidated
        assert_eq!(state.content_hash(id2), Some(222)); // Other tab unchanged
    }

    #[test]
    fn test_invalidate_active_after_close() {
        let mut state = TabState::new();
        let id1 = state.add_tab_and_activate("Tab 1");
        let id2 = state.add_tab("Tab 2");
        state.set_content_hash(id1, 111);
        state.set_content_hash(id2, 222);

        // Close active tab - simulates the tab close flow
        state.close_tab(id1);
        state.remove_content_hash(id1);

        // Now tab 2 is active, invalidate it
        assert!(state.invalidate_active_tab_content());
        assert_eq!(state.content_hash(id2), Some(0));
    }
}
