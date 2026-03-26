//! Tab bar state management
//!
//! Pure data structures for tab state - no GPU dependencies.

/// A single tab in the tab bar
#[derive(Debug, Clone)]
pub struct Tab {
    pub id: u64,
    pub title: String,
    pub is_active: bool,
    /// Whether this tab has a user-set custom title (prevents OSC overwrite)
    pub has_custom_title: bool,
}

impl Tab {
    pub fn new(id: u64, title: impl Into<String>) -> Self {
        Self {
            id,
            title: title.into(),
            is_active: false,
            has_custom_title: false,
        }
    }
}

/// State for inline tab title editing
#[derive(Debug, Clone, Default)]
pub struct EditState {
    /// Tab ID being edited (None if not editing)
    pub tab_id: Option<u64>,
    /// Current edit text
    pub text: String,
    /// Cursor position in the text
    pub cursor: usize,
}

/// Tab bar state - manages tabs without any GPU concerns
///
/// Tab IDs are provided externally (by `App`) to ensure global uniqueness
/// across all windows — required for cross-window tab moves.
pub struct TabBarState {
    pub(crate) tabs: Vec<Tab>,
    pub(crate) active_tab: usize,
    pub(crate) edit_state: EditState,
}

impl Default for TabBarState {
    fn default() -> Self {
        Self::new()
    }
}

impl TabBarState {
    pub fn new() -> Self {
        let initial_tab = Tab::new(0, "Terminal");
        Self {
            tabs: vec![initial_tab],
            active_tab: 0,
            edit_state: EditState::default(),
        }
    }

    /// Create a tab bar with a specific initial tab ID (for global ID support)
    pub fn with_initial_id(id: u64) -> Self {
        let initial_tab = Tab::new(id, "Terminal");
        Self {
            tabs: vec![initial_tab],
            active_tab: 0,
            edit_state: EditState::default(),
        }
    }

    /// Create an empty tab bar (no initial tab)
    pub fn empty() -> Self {
        Self {
            tabs: Vec::new(),
            active_tab: 0,
            edit_state: EditState::default(),
        }
    }

    /// Add a new tab with a caller-provided ID.
    ///
    /// IDs must be globally unique across all windows — the caller (typically `App`)
    /// is responsible for generating them from a global counter.
    pub fn add_tab(&mut self, id: u64, title: impl Into<String>) {
        self.tabs.push(Tab::new(id, title));
    }

    /// Remove a tab by ID and return it (for cross-window transfer).
    ///
    /// Adjusts `active_tab` if needed. Returns `None` if the tab doesn't exist.
    pub fn remove_tab(&mut self, id: u64) -> Option<Tab> {
        let idx = self.tabs.iter().position(|t| t.id == id)?;
        let tab = self.tabs.remove(idx);
        if !self.tabs.is_empty() {
            if self.active_tab >= self.tabs.len() {
                self.active_tab = self.tabs.len() - 1;
            } else if self.active_tab > idx {
                self.active_tab -= 1;
            }
        } else {
            self.active_tab = 0;
        }
        Some(tab)
    }

    /// Insert a pre-existing tab (preserving its ID) at the end.
    pub fn add_existing_tab(&mut self, tab: Tab) {
        self.tabs.push(tab);
    }

    /// Insert a pre-existing tab at a specific index.
    pub fn insert_existing_tab(&mut self, tab: Tab, index: usize) {
        let index = index.min(self.tabs.len());
        self.tabs.insert(index, tab);
        // Adjust active_tab if insertion shifted it
        if self.active_tab >= index {
            self.active_tab += 1;
        }
    }

    /// Move a tab from one index to another, updating active_tab to follow.
    ///
    /// If the active tab is the one being moved, it tracks to the new position.
    /// If the active tab is between `from` and `to`, it shifts by ±1.
    pub fn move_tab(&mut self, from: usize, to: usize) {
        if from == to || from >= self.tabs.len() || to >= self.tabs.len() {
            return;
        }
        let tab = self.tabs.remove(from);
        self.tabs.insert(to, tab);

        // Update active_tab to follow the move
        if self.active_tab == from {
            // The active tab was the one moved
            self.active_tab = to;
        } else if from < to {
            // Moved forward: tabs in (from, to] shift left by 1
            if self.active_tab > from && self.active_tab <= to {
                self.active_tab -= 1;
            }
        } else {
            // Moved backward: tabs in [to, from) shift right by 1
            if self.active_tab >= to && self.active_tab < from {
                self.active_tab += 1;
            }
        }
    }

    /// Close a tab by ID. Returns true if successful, false if it's the last tab
    pub fn close_tab(&mut self, id: u64) -> bool {
        if self.tabs.len() <= 1 {
            return false;
        }

        if let Some(idx) = self.tabs.iter().position(|t| t.id == id) {
            self.tabs.remove(idx);
            if self.active_tab >= self.tabs.len() {
                self.active_tab = self.tabs.len() - 1;
            }
            return true;
        }
        false
    }

    /// Select a tab by ID
    pub fn select_tab(&mut self, id: u64) -> bool {
        if let Some(idx) = self.tabs.iter().position(|t| t.id == id) {
            self.active_tab = idx;
            return true;
        }
        false
    }

    /// Select tab by index (0-based)
    pub fn select_tab_index(&mut self, index: usize) -> bool {
        if index < self.tabs.len() {
            self.active_tab = index;
            return true;
        }
        false
    }

    /// Select next tab (wraps around)
    pub fn next_tab(&mut self) {
        if !self.tabs.is_empty() {
            self.active_tab = (self.active_tab + 1) % self.tabs.len();
        }
    }

    /// Select previous tab (wraps around)
    pub fn prev_tab(&mut self) {
        if !self.tabs.is_empty() {
            self.active_tab = if self.active_tab == 0 {
                self.tabs.len() - 1
            } else {
                self.active_tab - 1
            };
        }
    }

    /// Get active tab ID
    pub fn active_tab_id(&self) -> Option<u64> {
        self.tabs.get(self.active_tab).map(|t| t.id)
    }

    /// Get active tab index
    pub fn active_tab_index(&self) -> usize {
        self.active_tab
    }

    /// Get number of tabs
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    /// Get tabs slice
    pub fn tabs(&self) -> &[Tab] {
        &self.tabs
    }

    /// Update a tab's title by ID (from OSC escape sequences)
    /// Will NOT update if the tab has a user-set custom title
    pub fn set_tab_title(&mut self, id: u64, title: impl Into<String>) -> bool {
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) {
            if tab.has_custom_title {
                return false;
            }

            let raw_title = title.into();
            let cleaned: String = raw_title
                .chars()
                .filter(|c| !c.is_control() && *c != '\x1b')
                .collect();
            let cleaned = cleaned.trim();

            if !cleaned.is_empty() {
                let final_title = if cleaned.chars().count() > 20 {
                    format!("{}...", cleaned.chars().take(17).collect::<String>())
                } else {
                    cleaned.to_string()
                };
                tab.title = final_title;
                return true;
            }
        }
        false
    }

    /// Set a custom title for a tab (user-initiated)
    pub fn set_custom_tab_title(&mut self, id: u64, title: impl Into<String>) -> bool {
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) {
            let raw_title = title.into();
            let trimmed = raw_title.trim();

            if !trimmed.is_empty() {
                let final_title = if trimmed.chars().count() > 20 {
                    format!("{}...", trimmed.chars().take(17).collect::<String>())
                } else {
                    trimmed.to_string()
                };
                tab.title = final_title;
                tab.has_custom_title = true;
                return true;
            }
        }
        false
    }

    /// Clear custom title flag (allows OSC to update title again)
    pub fn clear_custom_title(&mut self, id: u64) {
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) {
            tab.has_custom_title = false;
        }
    }

    /// Check if a tab has a custom title
    pub fn has_custom_title(&self, id: u64) -> bool {
        self.tabs
            .iter()
            .find(|t| t.id == id)
            .map(|t| t.has_custom_title)
            .unwrap_or(false)
    }

    /// Get a tab's title by ID
    pub fn get_tab_title(&self, id: u64) -> Option<&str> {
        self.tabs
            .iter()
            .find(|t| t.id == id)
            .map(|t| t.title.as_str())
    }

    // ---- Inline Editing Methods ----

    /// Check if currently editing a tab
    pub fn is_editing(&self) -> bool {
        self.edit_state.tab_id.is_some()
    }

    /// Get the tab ID being edited (if any)
    pub fn editing_tab_id(&self) -> Option<u64> {
        self.edit_state.tab_id
    }

    /// Start editing a tab's title
    pub fn start_editing(&mut self, id: u64) -> bool {
        if let Some(tab) = self.tabs.iter().find(|t| t.id == id) {
            self.edit_state = EditState {
                tab_id: Some(id),
                text: tab.title.clone(),
                cursor: tab.title.chars().count(), // Use character count, not byte length
            };
            return true;
        }
        false
    }

    /// Cancel editing without saving
    pub fn cancel_editing(&mut self) {
        self.edit_state = EditState::default();
    }

    /// Confirm editing and save the new title
    pub fn confirm_editing(&mut self) -> bool {
        if let Some(id) = self.edit_state.tab_id {
            let text = self.edit_state.text.clone();
            self.edit_state = EditState::default();
            return self.set_custom_tab_title(id, text);
        }
        false
    }

    /// Convert character index to byte index
    fn char_to_byte_index(text: &str, char_idx: usize) -> usize {
        text.char_indices()
            .nth(char_idx)
            .map(|(i, _)| i)
            .unwrap_or(text.len())
    }

    /// Get the number of characters in the edit text
    fn char_count(&self) -> usize {
        self.edit_state.text.chars().count()
    }

    /// Handle a character input during editing
    pub fn edit_insert_char(&mut self, c: char) {
        if self.edit_state.tab_id.is_some() && self.char_count() < 50 {
            let byte_idx = Self::char_to_byte_index(&self.edit_state.text, self.edit_state.cursor);
            self.edit_state.text.insert(byte_idx, c);
            self.edit_state.cursor += 1;
        }
    }

    /// Handle backspace during editing
    pub fn edit_backspace(&mut self) {
        if self.edit_state.tab_id.is_some() && self.edit_state.cursor > 0 {
            self.edit_state.cursor -= 1;
            let byte_idx = Self::char_to_byte_index(&self.edit_state.text, self.edit_state.cursor);
            self.edit_state.text.remove(byte_idx);
        }
    }

    /// Handle delete during editing
    pub fn edit_delete(&mut self) {
        if self.edit_state.tab_id.is_some() && self.edit_state.cursor < self.char_count() {
            let byte_idx = Self::char_to_byte_index(&self.edit_state.text, self.edit_state.cursor);
            self.edit_state.text.remove(byte_idx);
        }
    }

    /// Move cursor left during editing
    pub fn edit_cursor_left(&mut self) {
        if self.edit_state.tab_id.is_some() && self.edit_state.cursor > 0 {
            self.edit_state.cursor -= 1;
        }
    }

    /// Move cursor right during editing
    pub fn edit_cursor_right(&mut self) {
        if self.edit_state.tab_id.is_some() && self.edit_state.cursor < self.char_count() {
            self.edit_state.cursor += 1;
        }
    }

    /// Move cursor to start during editing
    pub fn edit_cursor_home(&mut self) {
        if self.edit_state.tab_id.is_some() {
            self.edit_state.cursor = 0;
        }
    }

    /// Move cursor to end during editing
    pub fn edit_cursor_end(&mut self) {
        if self.edit_state.tab_id.is_some() {
            self.edit_state.cursor = self.char_count();
        }
    }

    /// Get edit state for display purposes
    pub fn edit_state(&self) -> &EditState {
        &self.edit_state
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_state() {
        let state = TabBarState::new();
        assert_eq!(state.tab_count(), 1);
        assert_eq!(state.active_tab_id(), Some(0));
    }

    #[test]
    fn test_add_and_close_tabs() {
        let mut state = TabBarState::new();
        state.add_tab(1, "Tab 1");
        state.add_tab(2, "Tab 2");
        assert_eq!(state.tab_count(), 3);

        assert!(state.close_tab(1));
        assert_eq!(state.tab_count(), 2);

        assert!(state.close_tab(2));
        assert_eq!(state.tab_count(), 1);

        // Can't close last tab
        assert!(!state.close_tab(0));
        assert_eq!(state.tab_count(), 1);
    }

    #[test]
    fn test_navigation() {
        let mut state = TabBarState::new();
        state.add_tab(1, "Tab 1");
        state.add_tab(2, "Tab 2");

        assert_eq!(state.active_tab_index(), 0);

        state.next_tab();
        assert_eq!(state.active_tab_index(), 1);

        state.next_tab();
        assert_eq!(state.active_tab_index(), 2);

        state.next_tab(); // wraps
        assert_eq!(state.active_tab_index(), 0);

        state.prev_tab(); // wraps back
        assert_eq!(state.active_tab_index(), 2);
    }

    #[test]
    fn add_tab_uses_caller_provided_ids() {
        let mut state = TabBarState::new();
        state.add_tab(10, "A");
        state.add_tab(20, "B");
        state.add_tab(30, "C");
        assert_eq!(state.tabs()[1].id, 10);
        assert_eq!(state.tabs()[2].id, 20);
        assert_eq!(state.tabs()[3].id, 30);
    }

    #[test]
    fn with_initial_id_sets_first_tab_id() {
        let state = TabBarState::with_initial_id(42);
        assert_eq!(state.tab_count(), 1);
        assert_eq!(state.active_tab_id(), Some(42));
    }

    #[test]
    fn empty_creates_no_tabs() {
        let state = TabBarState::empty();
        assert_eq!(state.tab_count(), 0);
        assert_eq!(state.active_tab_id(), None);
    }

    #[test]
    fn select_tab_by_id() {
        let mut state = TabBarState::new();
        state.add_tab(1, "Tab 1");
        state.add_tab(2, "Tab 2");

        assert!(state.select_tab(1));
        assert_eq!(state.active_tab_id(), Some(1));

        // Invalid ID
        assert!(!state.select_tab(999));
        assert_eq!(state.active_tab_id(), Some(1)); // unchanged
    }

    #[test]
    fn select_tab_by_index() {
        let mut state = TabBarState::new();
        state.add_tab(1, "Tab 1");
        state.add_tab(2, "Tab 2");

        assert!(state.select_tab_index(2));
        assert_eq!(state.active_tab_index(), 2);

        // Out of range
        assert!(!state.select_tab_index(10));
        assert_eq!(state.active_tab_index(), 2); // unchanged
    }

    #[test]
    fn close_active_tab_selects_previous() {
        let mut state = TabBarState::new();
        state.add_tab(1, "Tab 1");
        state.add_tab(2, "Tab 2");

        // Select last tab then close it
        state.select_tab(2);
        assert_eq!(state.active_tab_index(), 2);
        assert!(state.close_tab(2));
        // Active index should clamp to last valid
        assert!(state.active_tab_index() < state.tab_count());

        // Close middle tab
        state.select_tab(1);
        assert!(state.close_tab(1));
        assert_eq!(state.tab_count(), 1);
    }

    #[test]
    fn close_nonexistent_tab_returns_false() {
        let mut state = TabBarState::new();
        state.add_tab(1, "Tab 1");
        assert!(!state.close_tab(999));
        assert_eq!(state.tab_count(), 2);
    }

    #[test]
    fn set_tab_title_from_osc() {
        let mut state = TabBarState::new();
        assert!(state.set_tab_title(0, "New Title"));
        assert_eq!(state.get_tab_title(0), Some("New Title"));
    }

    #[test]
    fn set_tab_title_truncates_long_titles() {
        let mut state = TabBarState::new();
        let long_title = "This is a very long tab title that exceeds 20 chars";
        assert!(state.set_tab_title(0, long_title));
        let title = state.get_tab_title(0).unwrap();
        assert!(title.ends_with("..."));
        assert!(title.chars().count() <= 20);
    }

    #[test]
    fn set_tab_title_strips_control_chars() {
        let mut state = TabBarState::new();
        assert!(state.set_tab_title(0, "Hello\x1b[0mWorld"));
        assert_eq!(state.get_tab_title(0), Some("Hello[0mWorld"));
    }

    #[test]
    fn set_tab_title_rejects_empty() {
        let mut state = TabBarState::new();
        assert!(!state.set_tab_title(0, ""));
        assert_eq!(state.get_tab_title(0), Some("Terminal")); // unchanged
    }

    #[test]
    fn custom_title_blocks_osc_updates() {
        let mut state = TabBarState::new();
        assert!(state.set_custom_tab_title(0, "My Tab"));
        assert!(state.has_custom_title(0));

        // OSC update should be blocked
        assert!(!state.set_tab_title(0, "From OSC"));
        assert_eq!(state.get_tab_title(0), Some("My Tab"));

        // Clear custom flag, OSC should work again
        state.clear_custom_title(0);
        assert!(!state.has_custom_title(0));
        assert!(state.set_tab_title(0, "From OSC"));
        assert_eq!(state.get_tab_title(0), Some("From OSC"));
    }

    #[test]
    fn get_tab_title_nonexistent_returns_none() {
        let state = TabBarState::new();
        assert_eq!(state.get_tab_title(999), None);
    }

    // ── Edit mode tests ────────────────────────────────────────────

    #[test]
    fn edit_start_and_cancel() {
        let mut state = TabBarState::new();
        assert!(!state.is_editing());

        assert!(state.start_editing(0));
        assert!(state.is_editing());
        assert_eq!(state.editing_tab_id(), Some(0));
        assert_eq!(state.edit_state().text, "Terminal");

        state.cancel_editing();
        assert!(!state.is_editing());
        assert_eq!(state.get_tab_title(0), Some("Terminal")); // unchanged
    }

    #[test]
    fn edit_start_nonexistent_tab_fails() {
        let mut state = TabBarState::new();
        assert!(!state.start_editing(999));
        assert!(!state.is_editing());
    }

    #[test]
    fn edit_insert_and_confirm() {
        let mut state = TabBarState::new();
        state.start_editing(0);

        // Clear existing text by backspacing
        for _ in 0.."Terminal".len() {
            state.edit_backspace();
        }
        assert_eq!(state.edit_state().text, "");

        // Type new title
        for c in "Hello".chars() {
            state.edit_insert_char(c);
        }
        assert_eq!(state.edit_state().text, "Hello");
        assert_eq!(state.edit_state().cursor, 5);

        assert!(state.confirm_editing());
        assert!(!state.is_editing());
        assert_eq!(state.get_tab_title(0), Some("Hello"));
        assert!(state.has_custom_title(0));
    }

    #[test]
    fn edit_cursor_movement() {
        let mut state = TabBarState::new();
        state.start_editing(0);
        // Cursor starts at end: "Terminal" len = 8
        assert_eq!(state.edit_state().cursor, 8);

        state.edit_cursor_home();
        assert_eq!(state.edit_state().cursor, 0);

        state.edit_cursor_end();
        assert_eq!(state.edit_state().cursor, 8);

        state.edit_cursor_left();
        assert_eq!(state.edit_state().cursor, 7);

        state.edit_cursor_right();
        assert_eq!(state.edit_state().cursor, 8);

        // Right at end should not go past
        state.edit_cursor_right();
        assert_eq!(state.edit_state().cursor, 8);

        // Left at 0 should not go negative
        state.edit_cursor_home();
        state.edit_cursor_left();
        assert_eq!(state.edit_state().cursor, 0);
    }

    #[test]
    fn edit_backspace_and_delete() {
        let mut state = TabBarState::new();
        state.start_editing(0);
        // Text is "Terminal", cursor at 8

        // Backspace removes last char
        state.edit_backspace();
        assert_eq!(state.edit_state().text, "Termina");
        assert_eq!(state.edit_state().cursor, 7);

        // Move to start and delete forward
        state.edit_cursor_home();
        state.edit_delete();
        assert_eq!(state.edit_state().text, "ermina");
        assert_eq!(state.edit_state().cursor, 0);

        // Backspace at position 0 does nothing
        state.edit_backspace();
        assert_eq!(state.edit_state().text, "ermina");

        // Delete at end does nothing
        state.edit_cursor_end();
        state.edit_delete();
        assert_eq!(state.edit_state().text, "ermina");
    }

    #[test]
    fn edit_insert_respects_max_length() {
        let mut state = TabBarState::new();
        state.start_editing(0);
        // Clear and fill to 50 chars
        for _ in 0.."Terminal".len() {
            state.edit_backspace();
        }
        for _ in 0..50 {
            state.edit_insert_char('x');
        }
        assert_eq!(state.edit_state().text.len(), 50);
        // 51st char should be rejected
        state.edit_insert_char('y');
        assert_eq!(state.edit_state().text.len(), 50);
    }

    #[test]
    fn edit_operations_noop_when_not_editing() {
        let mut state = TabBarState::new();
        // These should all be no-ops when not editing
        state.edit_insert_char('x');
        state.edit_backspace();
        state.edit_delete();
        state.edit_cursor_left();
        state.edit_cursor_right();
        state.edit_cursor_home();
        state.edit_cursor_end();
        // No crash, title unchanged
        assert_eq!(state.get_tab_title(0), Some("Terminal"));
    }

    #[test]
    fn confirm_empty_edit_does_not_save() {
        let mut state = TabBarState::new();
        state.start_editing(0);
        // Clear all text
        for _ in 0.."Terminal".len() {
            state.edit_backspace();
        }
        assert_eq!(state.edit_state().text, "");
        // Confirm with empty text should not change title
        assert!(!state.confirm_editing());
        assert_eq!(state.get_tab_title(0), Some("Terminal"));
    }

    #[test]
    fn prev_tab_single_tab_stays() {
        let mut state = TabBarState::new();
        assert_eq!(state.active_tab_index(), 0);
        state.prev_tab();
        assert_eq!(state.active_tab_index(), 0);
        state.next_tab();
        assert_eq!(state.active_tab_index(), 0);
    }

    #[test]
    fn tabs_slice_reflects_state() {
        let mut state = TabBarState::new();
        state.add_tab(1, "A");
        state.add_tab(2, "B");
        let tabs = state.tabs();
        assert_eq!(tabs.len(), 3);
        assert_eq!(tabs[0].title, "Terminal");
        assert_eq!(tabs[1].title, "A");
        assert_eq!(tabs[2].title, "B");
    }

    // ── move_tab tests ────────────────────────────────────────────

    fn make_state_with_tabs(names: &[&str]) -> TabBarState {
        let mut state = TabBarState::with_initial_id(0);
        for (i, name) in names.iter().enumerate().skip(1) {
            state.add_tab(i as u64, *name);
        }
        // Rename the first tab
        if !names.is_empty() {
            state.set_tab_title(0, names[0]);
        }
        state
    }

    fn tab_titles(state: &TabBarState) -> Vec<String> {
        state.tabs().iter().map(|t| t.title.clone()).collect()
    }

    #[test]
    fn move_tab_forward() {
        // [A, B, C, D] → move A(0) to index 2 → [B, C, A, D]
        let mut state = make_state_with_tabs(&["A", "B", "C", "D"]);
        state.move_tab(0, 2);
        assert_eq!(tab_titles(&state), vec!["B", "C", "A", "D"]);
    }

    #[test]
    fn move_tab_backward() {
        // [A, B, C, D] → move C(2) to index 0 → [C, A, B, D]
        let mut state = make_state_with_tabs(&["A", "B", "C", "D"]);
        state.move_tab(2, 0);
        assert_eq!(tab_titles(&state), vec!["C", "A", "B", "D"]);
    }

    #[test]
    fn move_tab_same_position_is_noop() {
        let mut state = make_state_with_tabs(&["A", "B", "C"]);
        state.move_tab(1, 1);
        assert_eq!(tab_titles(&state), vec!["A", "B", "C"]);
    }

    #[test]
    fn move_tab_to_start() {
        let mut state = make_state_with_tabs(&["A", "B", "C"]);
        state.move_tab(2, 0);
        assert_eq!(tab_titles(&state), vec!["C", "A", "B"]);
    }

    #[test]
    fn move_tab_to_end() {
        let mut state = make_state_with_tabs(&["A", "B", "C"]);
        state.move_tab(0, 2);
        assert_eq!(tab_titles(&state), vec!["B", "C", "A"]);
    }

    #[test]
    fn move_tab_active_follows_moved_tab() {
        let mut state = make_state_with_tabs(&["A", "B", "C", "D"]);
        state.select_tab_index(1); // B is active
        assert_eq!(state.active_tab_index(), 1);

        state.move_tab(1, 3); // Move B to end
        assert_eq!(state.active_tab_index(), 3); // B is still active
        assert_eq!(tab_titles(&state), vec!["A", "C", "D", "B"]);
    }

    #[test]
    fn move_tab_active_shifts_when_between_from_and_to() {
        let mut state = make_state_with_tabs(&["A", "B", "C", "D"]);
        state.select_tab_index(2); // C is active (index 2)

        state.move_tab(0, 3); // Move A to end: [B, C, D, A]
        // C was at index 2, A moved from 0 to 3, so C shifts left to index 1
        assert_eq!(state.active_tab_index(), 1);
        assert_eq!(state.tabs()[state.active_tab_index()].title, "C");
    }

    #[test]
    fn move_tab_active_shifts_backward_move() {
        let mut state = make_state_with_tabs(&["A", "B", "C", "D"]);
        state.select_tab_index(1); // B is active (index 1)

        state.move_tab(3, 0); // Move D to start: [D, A, B, C]
        // B was at index 1, D moved from 3 to 0, so B shifts right to index 2
        assert_eq!(state.active_tab_index(), 2);
        assert_eq!(state.tabs()[state.active_tab_index()].title, "B");
    }

    #[test]
    fn move_tab_out_of_bounds_is_noop() {
        let mut state = make_state_with_tabs(&["A", "B", "C"]);
        state.move_tab(0, 10); // to out of bounds
        assert_eq!(tab_titles(&state), vec!["A", "B", "C"]);
        state.move_tab(10, 0); // from out of bounds
        assert_eq!(tab_titles(&state), vec!["A", "B", "C"]);
    }

    #[test]
    fn move_tab_adjacent_swap() {
        let mut state = make_state_with_tabs(&["A", "B", "C"]);
        state.move_tab(0, 1); // Swap A and B: [B, A, C]
        assert_eq!(tab_titles(&state), vec!["B", "A", "C"]);
    }

    // ── remove_tab / add_existing_tab / insert_existing_tab tests ─

    #[test]
    fn remove_tab_returns_tab() {
        let mut state = make_state_with_tabs(&["A", "B", "C"]);
        let tab = state.remove_tab(1).unwrap(); // Remove B
        assert_eq!(tab.title, "B");
        assert_eq!(tab_titles(&state), vec!["A", "C"]);
    }

    #[test]
    fn remove_tab_nonexistent_returns_none() {
        let mut state = make_state_with_tabs(&["A", "B"]);
        assert!(state.remove_tab(999).is_none());
        assert_eq!(state.tab_count(), 2);
    }

    #[test]
    fn remove_tab_adjusts_active_index() {
        let mut state = make_state_with_tabs(&["A", "B", "C"]);
        state.select_tab_index(2); // C is active
        state.remove_tab(0); // Remove A, shifting active from 2→1
        assert_eq!(state.active_tab_index(), 1);
        assert_eq!(state.tabs()[state.active_tab_index()].title, "C");
    }

    #[test]
    fn remove_tab_active_clamps_when_last() {
        let mut state = make_state_with_tabs(&["A", "B", "C"]);
        state.select_tab_index(2); // C active at index 2
        state.remove_tab(2); // Remove C
        assert_eq!(state.active_tab_index(), 1); // Clamped to last valid
    }

    #[test]
    fn add_existing_tab_appends() {
        let mut state = make_state_with_tabs(&["A", "B"]);
        let tab = Tab::new(99, "Moved");
        state.add_existing_tab(tab);
        assert_eq!(tab_titles(&state), vec!["A", "B", "Moved"]);
        assert_eq!(state.tabs()[2].id, 99); // ID preserved
    }

    #[test]
    fn insert_existing_tab_at_beginning() {
        let mut state = make_state_with_tabs(&["A", "B"]);
        let tab = Tab::new(99, "Front");
        state.insert_existing_tab(tab, 0);
        assert_eq!(tab_titles(&state), vec!["Front", "A", "B"]);
    }

    #[test]
    fn insert_existing_tab_at_middle() {
        let mut state = make_state_with_tabs(&["A", "B", "C"]);
        let tab = Tab::new(99, "Mid");
        state.insert_existing_tab(tab, 1);
        assert_eq!(tab_titles(&state), vec!["A", "Mid", "B", "C"]);
    }

    #[test]
    fn insert_existing_tab_adjusts_active() {
        let mut state = make_state_with_tabs(&["A", "B", "C"]);
        state.select_tab_index(1); // B active at index 1
        let tab = Tab::new(99, "New");
        state.insert_existing_tab(tab, 0); // Insert before B
        // B should shift to index 2
        assert_eq!(state.active_tab_index(), 2);
        assert_eq!(state.tabs()[state.active_tab_index()].title, "B");
    }

    #[test]
    fn round_trip_remove_and_insert() {
        let mut source = make_state_with_tabs(&["A", "B", "C"]);
        let mut target = make_state_with_tabs(&["X", "Y"]);

        // Extract B from source
        let tab = source.remove_tab(1).unwrap();
        assert_eq!(tab_titles(&source), vec!["A", "C"]);

        // Insert into target at index 1
        target.insert_existing_tab(tab, 1);
        assert_eq!(tab_titles(&target), vec!["X", "B", "Y"]);
        assert_eq!(target.tabs()[1].id, 1); // ID preserved
    }
}
