//! Aggregated UI state module
//!
//! This module provides a unified UI state struct that aggregates all
//! UI-related state (selection, search, context menu, bell, etc.) into
//! a single testable structure without GPU dependencies.

use super::SelectionState;
use crate::input::UrlMatch;

/// State for context menu visibility and position
#[derive(Debug, Clone, Default)]
pub struct ContextMenuState {
    /// Whether the context menu is visible
    pub visible: bool,
    /// Position of the menu (x, y in pixels)
    pub position: (f32, f32),
    /// Currently selected/hovered item index
    pub selected_index: usize,
}

impl ContextMenuState {
    /// Create a new hidden context menu state
    pub fn new() -> Self {
        Self::default()
    }

    /// Show the context menu at the given position
    pub fn show(&mut self, x: f32, y: f32) {
        self.visible = true;
        self.position = (x, y);
        self.selected_index = 0;
    }

    /// Hide the context menu
    pub fn hide(&mut self) {
        self.visible = false;
    }

    /// Toggle visibility at the given position
    pub fn toggle(&mut self, x: f32, y: f32) {
        if self.visible {
            self.hide();
        } else {
            self.show(x, y);
        }
    }

    /// Select the next menu item
    pub fn select_next(&mut self, max_items: usize) {
        if max_items > 0 && self.selected_index < max_items - 1 {
            self.selected_index += 1;
        }
    }

    /// Select the previous menu item
    pub fn select_previous(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }

    /// Check if a position is within the menu bounds
    pub fn contains(&self, x: f32, y: f32, item_height: f32, num_items: usize) -> bool {
        if !self.visible {
            return false;
        }
        let (mx, my) = self.position;
        let menu_width = 150.0; // Approximate menu width
        let menu_height = item_height * num_items as f32;
        x >= mx && x < mx + menu_width && y >= my && y < my + menu_height
    }
}

/// Visual bell animation state
#[derive(Debug, Clone, Default)]
pub struct BellState {
    /// Current bell intensity (0.0 to 1.0)
    pub intensity: f32,
    /// Whether the bell animation is active
    pub active: bool,
}

impl BellState {
    /// Create a new inactive bell state
    pub fn new() -> Self {
        Self::default()
    }

    /// Trigger the visual bell
    pub fn trigger(&mut self) {
        self.intensity = 1.0;
        self.active = true;
    }

    /// Update the bell animation
    ///
    /// # Arguments
    /// * `dt` - Delta time in seconds
    pub fn update(&mut self, dt: f32) {
        if self.active {
            self.intensity -= dt * 4.0; // Fade over ~250ms
            if self.intensity <= 0.0 {
                self.intensity = 0.0;
                self.active = false;
            }
        }
    }

    /// Check if the bell is currently visible
    pub fn is_visible(&self) -> bool {
        self.active && self.intensity > 0.0
    }
}

/// A search match location in the terminal
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchMatch {
    /// Line number (can be negative for scrollback history)
    pub line: i32,
    /// Starting column (inclusive)
    pub start_col: usize,
    /// Ending column (exclusive)
    pub end_col: usize,
}

impl SearchMatch {
    /// Create a new search match
    pub fn new(line: i32, start_col: usize, end_col: usize) -> Self {
        Self {
            line,
            start_col,
            end_col,
        }
    }

    /// Check if a position is within this match
    pub fn contains(&self, line: i32, col: usize) -> bool {
        self.line == line && col >= self.start_col && col < self.end_col
    }
}

/// Search/find state
#[derive(Debug, Clone, Default)]
pub struct SearchState {
    /// Whether search mode is active
    pub active: bool,
    /// Current search query
    pub query: String,
    /// List of matches found
    pub matches: Vec<SearchMatch>,
    /// Index of the current/focused match
    pub current_match: usize,
}

impl SearchState {
    /// Create a new inactive search state
    pub fn new() -> Self {
        Self::default()
    }

    /// Open/activate search mode
    pub fn open(&mut self) {
        self.active = true;
    }

    /// Close search mode and clear state
    pub fn close(&mut self) {
        self.active = false;
        self.query.clear();
        self.matches.clear();
        self.current_match = 0;
    }

    /// Set the search query
    pub fn set_query(&mut self, query: impl Into<String>) {
        self.query = query.into();
    }

    /// Set the search matches
    pub fn set_matches(&mut self, matches: Vec<SearchMatch>) {
        self.matches = matches;
        self.current_match = 0;
    }

    /// Navigate to the next match
    pub fn next_match(&mut self) {
        if !self.matches.is_empty() {
            self.current_match = (self.current_match + 1) % self.matches.len();
        }
    }

    /// Navigate to the previous match
    pub fn previous_match(&mut self) {
        if !self.matches.is_empty() {
            self.current_match = if self.current_match == 0 {
                self.matches.len() - 1
            } else {
                self.current_match - 1
            };
        }
    }

    /// Get the current match, if any
    pub fn current(&self) -> Option<&SearchMatch> {
        self.matches.get(self.current_match)
    }

    /// Get the number of matches
    pub fn match_count(&self) -> usize {
        self.matches.len()
    }

    /// Check if there are any matches
    pub fn has_matches(&self) -> bool {
        !self.matches.is_empty()
    }
}

/// Aggregated UI state for a window
///
/// This struct consolidates all UI-related state that doesn't depend on
/// GPU or window system resources, making it fully testable.
#[derive(Debug, Default)]
pub struct UiState {
    /// Text selection state
    pub selection: SelectionState,

    /// Search/find state
    pub search: SearchState,

    /// Right-click context menu state
    pub context_menu: ContextMenuState,

    /// Visual bell animation state
    pub bell: BellState,

    /// Current mouse cursor position in pixels
    pub cursor_position: (f32, f32),

    /// Detected URLs in the current viewport
    pub detected_urls: Vec<UrlMatch>,

    /// Index of URL currently being hovered, if any
    pub hovered_url_index: Option<usize>,

    /// Whether the window content needs redraw
    pub dirty: bool,
}

impl UiState {
    /// Create a new UI state with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Mark the view as needing redraw
    pub fn invalidate(&mut self) {
        self.dirty = true;
    }

    /// Clear the dirty flag after redraw
    pub fn mark_clean(&mut self) {
        self.dirty = false;
    }

    /// Update cursor position
    pub fn set_cursor_position(&mut self, x: f32, y: f32) {
        self.cursor_position = (x, y);
    }

    /// Update detected URLs for the current viewport
    pub fn set_detected_urls(&mut self, urls: Vec<UrlMatch>) {
        self.detected_urls = urls;
        self.hovered_url_index = None;
    }

    /// Update the hovered URL based on column position
    pub fn update_hovered_url(&mut self, col: usize) {
        self.hovered_url_index = self
            .detected_urls
            .iter()
            .position(|u| u.contains_column(col));
    }

    /// Get the currently hovered URL, if any
    pub fn hovered_url(&self) -> Option<&UrlMatch> {
        self.hovered_url_index
            .and_then(|i| self.detected_urls.get(i))
    }

    /// Clear all URL hover state
    pub fn clear_url_hover(&mut self) {
        self.hovered_url_index = None;
    }

    /// Update the bell animation with delta time
    pub fn update_bell(&mut self, dt: f32) {
        self.bell.update(dt);
        if self.bell.is_visible() {
            self.invalidate();
        }
    }

    /// Trigger the visual bell
    pub fn trigger_bell(&mut self) {
        self.bell.trigger();
        self.invalidate();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // === ContextMenuState tests ===

    #[test]
    fn test_context_menu_new() {
        let menu = ContextMenuState::new();
        assert!(!menu.visible);
        assert_eq!(menu.selected_index, 0);
    }

    #[test]
    fn test_context_menu_show_hide() {
        let mut menu = ContextMenuState::new();

        menu.show(100.0, 200.0);
        assert!(menu.visible);
        assert_eq!(menu.position, (100.0, 200.0));
        assert_eq!(menu.selected_index, 0);

        menu.hide();
        assert!(!menu.visible);
    }

    #[test]
    fn test_context_menu_toggle() {
        let mut menu = ContextMenuState::new();

        menu.toggle(50.0, 50.0);
        assert!(menu.visible);

        menu.toggle(50.0, 50.0);
        assert!(!menu.visible);
    }

    #[test]
    fn test_context_menu_navigation() {
        let mut menu = ContextMenuState::new();
        menu.show(0.0, 0.0);

        menu.select_next(3);
        assert_eq!(menu.selected_index, 1);

        menu.select_next(3);
        assert_eq!(menu.selected_index, 2);

        menu.select_next(3); // At max, shouldn't increment
        assert_eq!(menu.selected_index, 2);

        menu.select_previous();
        assert_eq!(menu.selected_index, 1);

        menu.select_previous();
        assert_eq!(menu.selected_index, 0);

        menu.select_previous(); // At 0, shouldn't decrement
        assert_eq!(menu.selected_index, 0);
    }

    // === BellState tests ===

    #[test]
    fn test_bell_new() {
        let bell = BellState::new();
        assert!(!bell.active);
        assert_eq!(bell.intensity, 0.0);
        assert!(!bell.is_visible());
    }

    #[test]
    fn test_bell_trigger() {
        let mut bell = BellState::new();
        bell.trigger();

        assert!(bell.active);
        assert_eq!(bell.intensity, 1.0);
        assert!(bell.is_visible());
    }

    #[test]
    fn test_bell_update() {
        let mut bell = BellState::new();
        bell.trigger();

        // Partial fade
        bell.update(0.1); // Should reduce by 0.4
        assert!(bell.active);
        assert!(bell.intensity < 1.0);
        assert!(bell.intensity > 0.0);

        // Complete fade
        bell.update(1.0); // Should completely fade
        assert!(!bell.active);
        assert_eq!(bell.intensity, 0.0);
        assert!(!bell.is_visible());
    }

    // === SearchMatch tests ===

    #[test]
    fn test_search_match_new() {
        let m = SearchMatch::new(5, 10, 15);
        assert_eq!(m.line, 5);
        assert_eq!(m.start_col, 10);
        assert_eq!(m.end_col, 15);
    }

    #[test]
    fn test_search_match_contains() {
        let m = SearchMatch::new(5, 10, 15);

        assert!(m.contains(5, 10));
        assert!(m.contains(5, 12));
        assert!(m.contains(5, 14));
        assert!(!m.contains(5, 9));
        assert!(!m.contains(5, 15)); // exclusive
        assert!(!m.contains(4, 12));
        assert!(!m.contains(6, 12));
    }

    // === SearchState tests ===

    #[test]
    fn test_search_new() {
        let search = SearchState::new();
        assert!(!search.active);
        assert!(search.query.is_empty());
        assert!(search.matches.is_empty());
    }

    #[test]
    fn test_search_open_close() {
        let mut search = SearchState::new();

        search.open();
        assert!(search.active);

        search.set_query("test");
        search.set_matches(vec![SearchMatch::new(0, 0, 4)]);

        search.close();
        assert!(!search.active);
        assert!(search.query.is_empty());
        assert!(search.matches.is_empty());
    }

    #[test]
    fn test_search_navigation() {
        let mut search = SearchState::new();
        search.set_matches(vec![
            SearchMatch::new(0, 0, 4),
            SearchMatch::new(1, 5, 9),
            SearchMatch::new(2, 0, 4),
        ]);

        assert_eq!(search.current_match, 0);

        search.next_match();
        assert_eq!(search.current_match, 1);

        search.next_match();
        assert_eq!(search.current_match, 2);

        search.next_match(); // Wraps
        assert_eq!(search.current_match, 0);

        search.previous_match(); // Wraps
        assert_eq!(search.current_match, 2);

        search.previous_match();
        assert_eq!(search.current_match, 1);
    }

    #[test]
    fn test_search_empty_navigation() {
        let mut search = SearchState::new();

        search.next_match(); // Should not panic
        search.previous_match(); // Should not panic
        assert_eq!(search.current_match, 0);
    }

    #[test]
    fn test_search_current() {
        let mut search = SearchState::new();
        assert!(search.current().is_none());

        search.set_matches(vec![SearchMatch::new(5, 0, 4)]);
        let current = search.current().unwrap();
        assert_eq!(current.line, 5);
    }

    // === UiState tests ===

    #[test]
    fn test_ui_state_new() {
        let state = UiState::new();
        assert!(!state.dirty);
        assert!(!state.selection.is_active());
        assert!(!state.search.active);
        assert!(!state.context_menu.visible);
        assert!(!state.bell.active);
    }

    #[test]
    fn test_ui_state_dirty() {
        let mut state = UiState::new();

        state.invalidate();
        assert!(state.dirty);

        state.mark_clean();
        assert!(!state.dirty);
    }

    #[test]
    fn test_ui_state_cursor() {
        let mut state = UiState::new();

        state.set_cursor_position(100.0, 200.0);
        assert_eq!(state.cursor_position, (100.0, 200.0));
    }

    #[test]
    fn test_ui_state_urls() {
        let mut state = UiState::new();

        let urls = vec![UrlMatch {
            url: "https://example.com".to_string(),
            start_col: 5,
            end_col: 24,
        }];

        state.set_detected_urls(urls);
        assert_eq!(state.detected_urls.len(), 1);
        assert!(state.hovered_url_index.is_none());

        state.update_hovered_url(10);
        assert_eq!(state.hovered_url_index, Some(0));

        let hovered = state.hovered_url().unwrap();
        assert_eq!(hovered.url, "https://example.com");

        state.update_hovered_url(0); // Outside URL
        assert!(state.hovered_url_index.is_none());
    }

    #[test]
    fn test_ui_state_bell() {
        let mut state = UiState::new();

        state.trigger_bell();
        assert!(state.bell.is_visible());
        assert!(state.dirty);

        state.mark_clean();
        state.update_bell(0.1);
        assert!(state.dirty); // Bell animation invalidates
    }
}
