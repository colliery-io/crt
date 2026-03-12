//! Interaction state types.
//!
//! Groups state related to user interaction, mouse handling, search, and context menus.

use std::time::Instant;

use super::types::TabId;

/// Search match position in terminal
#[derive(Debug, Clone, Copy)]
pub struct SearchMatch {
    /// Line number (grid-relative: negative = history, 0+ = visible)
    pub line: i32,
    /// Starting column
    pub start_col: usize,
    /// Ending column (exclusive)
    pub end_col: usize,
}

/// Search state for find-in-terminal functionality
#[derive(Debug, Clone, Default)]
pub struct SearchState {
    /// Whether search mode is active
    pub active: bool,
    /// Current search query
    pub query: String,
    /// All matches found
    pub matches: Vec<SearchMatch>,
    /// Index of current/focused match
    pub current_match: usize,
}

/// Context menu item
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContextMenuItem {
    Copy,
    Paste,
    SelectAll,
    Separator,
    /// Parent item that shows submenu on hover
    Themes,
    /// Individual theme (shown in submenu)
    Theme(String),
}

impl ContextMenuItem {
    /// Get the display label for this menu item
    pub fn label(&self) -> String {
        match self {
            ContextMenuItem::Copy => "Copy".to_string(),
            ContextMenuItem::Paste => "Paste".to_string(),
            ContextMenuItem::SelectAll => "Select All".to_string(),
            ContextMenuItem::Separator => String::new(),
            ContextMenuItem::Themes => "Theme".to_string(),
            ContextMenuItem::Theme(name) => name.clone(),
        }
    }

    /// Get the keyboard shortcut hint (or arrow indicator for submenu)
    pub fn shortcut(&self) -> &'static str {
        #[cfg(target_os = "macos")]
        match self {
            ContextMenuItem::Copy => "Cmd+C",
            ContextMenuItem::Paste => "Cmd+V",
            ContextMenuItem::SelectAll => "Cmd+A",
            ContextMenuItem::Themes => "\u{25B6}", // Right-pointing triangle for submenu
            ContextMenuItem::Separator | ContextMenuItem::Theme(_) => "",
        }
        #[cfg(not(target_os = "macos"))]
        match self {
            ContextMenuItem::Copy => "Ctrl+C",
            ContextMenuItem::Paste => "Ctrl+V",
            ContextMenuItem::SelectAll => "Ctrl+A",
            ContextMenuItem::Themes => "\u{25B6}", // Right-pointing triangle for submenu
            ContextMenuItem::Separator | ContextMenuItem::Theme(_) => "",
        }
    }

    /// Returns true if this is a submenu parent
    pub fn has_submenu(&self) -> bool {
        matches!(self, ContextMenuItem::Themes)
    }

    /// Returns true if this is a separator item
    pub fn is_separator(&self) -> bool {
        matches!(self, ContextMenuItem::Separator)
    }

    /// Returns true if this is a selectable (non-separator) item
    pub fn is_selectable(&self) -> bool {
        !self.is_separator()
    }

    /// Get the base edit menu items
    pub fn edit_items() -> Vec<ContextMenuItem> {
        vec![
            ContextMenuItem::Copy,
            ContextMenuItem::Paste,
            ContextMenuItem::SelectAll,
        ]
    }
}

/// Interaction state (cursor, mouse, selection, URLs)
///
/// Groups state related to user interaction and mouse handling.
#[derive(Default)]
pub struct InteractionState {
    /// Current cursor position in pixels
    pub cursor_position: (f32, f32),
    /// Last click time for double-click detection
    pub last_click_time: Option<Instant>,
    /// Last clicked tab for double-click detection
    pub last_click_tab: Option<TabId>,
    /// Whether mouse button is currently pressed
    pub mouse_pressed: bool,
    /// Click count for multi-click selection (1=single, 2=word, 3=line)
    pub selection_click_count: u8,
    /// Last selection click time for multi-click detection
    pub last_selection_click_time: Option<Instant>,
    /// Last selection click position (col, line) for multi-click detection
    pub last_selection_click_pos: Option<(usize, usize)>,
    /// Detected URLs in current viewport
    pub detected_urls: Vec<crate::input::DetectedUrl>,
    /// Index of currently hovered URL (for hover underline effect)
    pub hovered_url_index: Option<usize>,
}

/// Context menu state
#[derive(Debug, Clone, Default)]
pub struct ContextMenu {
    /// Whether the context menu is visible
    pub visible: bool,
    /// Position of the menu (top-left corner)
    pub x: f32,
    pub y: f32,
    /// Currently hovered item index (from mouse)
    pub hovered_item: Option<usize>,
    /// Currently focused item index (from keyboard navigation)
    pub focused_item: Option<usize>,
    /// Menu dimensions (computed during render)
    pub width: f32,
    pub height: f32,
    pub item_height: f32,
    /// Available themes for theme picker section
    pub themes: Vec<String>,
    /// Currently active theme name
    pub current_theme: String,
    /// Whether the theme submenu is visible
    pub submenu_visible: bool,
    /// Submenu position (top-left corner)
    pub submenu_x: f32,
    pub submenu_y: f32,
    /// Submenu dimensions
    pub submenu_width: f32,
    pub submenu_height: f32,
    /// Hovered item in submenu
    pub submenu_hovered_item: Option<usize>,
}

impl ContextMenu {
    /// Build the main menu items (Themes shown as a parent item, not expanded)
    pub fn items(&self) -> Vec<ContextMenuItem> {
        let mut items = ContextMenuItem::edit_items();
        if !self.themes.is_empty() {
            items.push(ContextMenuItem::Separator);
            items.push(ContextMenuItem::Themes);
        }
        items
    }

    /// Build the theme submenu items
    pub fn theme_items(&self) -> Vec<ContextMenuItem> {
        self.themes
            .iter()
            .map(|name| ContextMenuItem::Theme(name.clone()))
            .collect()
    }

    /// Show the context menu at the given position
    pub fn show(&mut self, x: f32, y: f32) {
        self.visible = true;
        self.x = x;
        self.y = y;
        self.hovered_item = None;
        self.focused_item = Some(0); // Focus first item for keyboard accessibility
        self.submenu_visible = false;
        self.submenu_hovered_item = None;
    }

    /// Hide the context menu
    pub fn hide(&mut self) {
        self.visible = false;
        self.hovered_item = None;
        self.focused_item = None;
        self.submenu_visible = false;
        self.submenu_hovered_item = None;
    }

    /// Move focus to the next selectable item (wraps around, skips separators)
    pub fn focus_next(&mut self) {
        if !self.visible {
            return;
        }
        let items = self.items();
        let item_count = items.len();
        if item_count == 0 {
            return;
        }

        let start = self.focused_item.unwrap_or(0);
        for i in 1..=item_count {
            let idx = (start + i) % item_count;
            if items[idx].is_selectable() {
                self.focused_item = Some(idx);
                break;
            }
        }
        // Clear hover when using keyboard
        self.hovered_item = None;
    }

    /// Move focus to the previous selectable item (wraps around, skips separators)
    pub fn focus_prev(&mut self) {
        if !self.visible {
            return;
        }
        let items = self.items();
        let item_count = items.len();
        if item_count == 0 {
            return;
        }

        let start = self.focused_item.unwrap_or(0);
        for i in 1..=item_count {
            let idx = (start + item_count - i) % item_count;
            if items[idx].is_selectable() {
                self.focused_item = Some(idx);
                break;
            }
        }
        // Clear hover when using keyboard
        self.hovered_item = None;
    }

    /// Get the currently focused item
    pub fn get_focused_item(&self) -> Option<ContextMenuItem> {
        let items = self.items();
        self.focused_item.and_then(|idx| items.get(idx).cloned())
    }

    /// Check if a point is inside the main menu
    pub fn contains(&self, x: f32, y: f32) -> bool {
        if !self.visible {
            return false;
        }
        x >= self.x && x <= self.x + self.width && y >= self.y && y <= self.y + self.height
    }

    /// Check if a point is inside the submenu
    pub fn contains_submenu(&self, x: f32, y: f32) -> bool {
        if !self.submenu_visible {
            return false;
        }
        x >= self.submenu_x
            && x <= self.submenu_x + self.submenu_width
            && y >= self.submenu_y
            && y <= self.submenu_y + self.submenu_height
    }

    /// Get the menu item at the given position (main menu only)
    pub fn item_at(&self, x: f32, y: f32) -> Option<ContextMenuItem> {
        if !self.contains(x, y) {
            return None;
        }
        let items = self.items();
        let rel_y = y - self.y;
        let index = (rel_y / self.item_height) as usize;
        items.get(index).cloned()
    }

    /// Get the submenu item at the given position
    pub fn submenu_item_at(&self, x: f32, y: f32) -> Option<ContextMenuItem> {
        if !self.contains_submenu(x, y) {
            return None;
        }
        let items = self.theme_items();
        let rel_y = y - self.submenu_y;
        let index = (rel_y / self.item_height) as usize;
        items.get(index).cloned()
    }

    /// Check if a theme name is the currently active theme
    pub fn is_current_theme(&self, name: &str) -> bool {
        self.current_theme == name
    }

    /// Update hover state based on mouse position
    pub fn update_hover(&mut self, x: f32, y: f32) {
        if !self.visible {
            self.hovered_item = None;
            self.submenu_hovered_item = None;
            return;
        }

        // Check submenu first
        if self.submenu_visible && self.contains_submenu(x, y) {
            let rel_y = y - self.submenu_y;
            self.submenu_hovered_item = Some((rel_y / self.item_height) as usize);
            // Keep main menu item hovered (the Themes item)
            return;
        } else {
            self.submenu_hovered_item = None;
        }

        // Check main menu
        if self.contains(x, y) {
            let rel_y = y - self.y;
            self.hovered_item = Some((rel_y / self.item_height) as usize);
        } else {
            self.hovered_item = None;
            // Hide submenu when mouse leaves both menus
            if !self.contains_submenu(x, y) {
                self.submenu_visible = false;
            }
        }
    }

    /// Get the index of the Themes item in the main menu
    pub fn themes_item_index(&self) -> Option<usize> {
        self.items()
            .iter()
            .position(|item| matches!(item, ContextMenuItem::Themes))
    }
}
