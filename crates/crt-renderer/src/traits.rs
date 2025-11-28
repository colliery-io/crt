//! Renderer trait abstractions for testing
//!
//! This module defines GPU-agnostic traits for rendering operations.
//! These traits enable creating mock renderers for testing without
//! requiring actual GPU context.

/// RGBA color as floats (0.0 - 1.0)
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    /// Create a new opaque color
    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    /// Create a new color with alpha
    pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// Create from 8-bit components
    pub fn from_u8(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self {
            r: r as f32 / 255.0,
            g: g as f32 / 255.0,
            b: b as f32 / 255.0,
            a: a as f32 / 255.0,
        }
    }

    /// White color
    pub const WHITE: Self = Self::rgb(1.0, 1.0, 1.0);
    /// Black color
    pub const BLACK: Self = Self::rgb(0.0, 0.0, 0.0);
    /// Transparent
    pub const TRANSPARENT: Self = Self::rgba(0.0, 0.0, 0.0, 0.0);
}

/// Rectangle with position and size
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    /// Create a new rectangle
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self { x, y, width, height }
    }

    /// Check if a point is inside this rectangle
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px < self.x + self.width && py >= self.y && py < self.y + self.height
    }

    /// Get the right edge x coordinate
    pub fn right(&self) -> f32 {
        self.x + self.width
    }

    /// Get the bottom edge y coordinate
    pub fn bottom(&self) -> f32 {
        self.y + self.height
    }
}

/// Grid position (row, column)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GridPosition {
    pub row: i32,
    pub col: usize,
}

impl GridPosition {
    pub const fn new(row: i32, col: usize) -> Self {
        Self { row, col }
    }
}

/// A single cell's content for rendering
#[derive(Debug, Clone)]
pub struct CellContent {
    /// The character to display
    pub character: char,
    /// Grid position
    pub position: GridPosition,
    /// Foreground color
    pub fg_color: Color,
    /// Background color (None for transparent)
    pub bg_color: Option<Color>,
    /// Bold style
    pub bold: bool,
    /// Italic style
    pub italic: bool,
    /// Underline style
    pub underline: bool,
    /// Strikethrough style
    pub strikethrough: bool,
}

impl CellContent {
    /// Create a simple cell with just character and position
    pub fn simple(character: char, row: i32, col: usize) -> Self {
        Self {
            character,
            position: GridPosition::new(row, col),
            fg_color: Color::WHITE,
            bg_color: None,
            bold: false,
            italic: false,
            underline: false,
            strikethrough: false,
        }
    }
}

/// Cursor shape styles
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CursorShape {
    #[default]
    Block,
    Underline,
    Beam,
}

/// Cursor information for rendering
#[derive(Debug, Clone, Copy)]
pub struct CursorInfo {
    /// Grid position
    pub position: GridPosition,
    /// Cursor shape
    pub shape: CursorShape,
    /// Cursor color
    pub color: Color,
    /// Whether cursor is visible
    pub visible: bool,
}

/// Selection range for highlighting
#[derive(Debug, Clone, Copy)]
pub struct SelectionRange {
    /// Start position
    pub start: GridPosition,
    /// End position (exclusive)
    pub end: GridPosition,
    /// Selection highlight color
    pub color: Color,
}

/// Tab rendering information
#[derive(Debug, Clone)]
pub struct TabRenderInfo {
    /// Tab title text
    pub title: String,
    /// Whether this tab is active
    pub active: bool,
    /// Whether tab has new activity
    pub has_activity: bool,
    /// Tab bounds in pixels
    pub bounds: Rect,
}

/// Search match highlight
#[derive(Debug, Clone, Copy)]
pub struct SearchHighlight {
    /// Match bounds in pixels
    pub bounds: Rect,
    /// Whether this is the current/focused match
    pub is_current: bool,
}

/// Context menu item
#[derive(Debug, Clone)]
pub struct ContextMenuItem {
    /// Item label
    pub label: String,
    /// Whether item is enabled
    pub enabled: bool,
    /// Whether item is selected/hovered
    pub selected: bool,
}

/// Trait for rendering terminal text content
///
/// Implementors render the terminal grid including cells, cursor, and selection.
pub trait TextRenderer {
    /// Render a batch of cells
    fn render_cells(&mut self, cells: &[CellContent]);

    /// Render the cursor
    fn render_cursor(&mut self, cursor: &CursorInfo);

    /// Render selection highlighting
    fn render_selection(&mut self, ranges: &[SelectionRange]);

    /// Clear the text buffer
    fn clear(&mut self);
}

/// Trait for rendering UI elements
///
/// Implementors render UI overlays like tab bar, search, and context menu.
pub trait UiRenderer {
    /// Render the tab bar
    fn render_tabs(&mut self, tabs: &[TabRenderInfo], bar_bounds: Rect);

    /// Render search match highlights
    fn render_search_matches(&mut self, matches: &[SearchHighlight], highlight_color: Color);

    /// Render context menu
    fn render_context_menu(&mut self, position: (f32, f32), items: &[ContextMenuItem]);

    /// Render visual bell flash overlay
    fn render_bell_flash(&mut self, intensity: f32);
}

/// Trait for rendering backdrop effects
///
/// Implementors render animated background effects like starfield, rain, etc.
pub trait BackdropRenderer {
    /// Update effect animations with delta time
    fn update(&mut self, dt: f32);

    /// Render all active effects
    fn render(&mut self);

    /// Check if any effects need continuous animation
    fn needs_animation(&self) -> bool;
}

#[cfg(test)]
mod tests {
    use super::*;

    // === Color tests ===

    #[test]
    fn test_color_rgb() {
        let c = Color::rgb(1.0, 0.5, 0.0);
        assert_eq!(c.r, 1.0);
        assert_eq!(c.g, 0.5);
        assert_eq!(c.b, 0.0);
        assert_eq!(c.a, 1.0);
    }

    #[test]
    fn test_color_rgba() {
        let c = Color::rgba(1.0, 0.5, 0.0, 0.5);
        assert_eq!(c.a, 0.5);
    }

    #[test]
    fn test_color_from_u8() {
        let c = Color::from_u8(255, 128, 0, 255);
        assert!((c.r - 1.0).abs() < 0.01);
        assert!((c.g - 0.5).abs() < 0.01);
        assert_eq!(c.b, 0.0);
    }

    #[test]
    fn test_color_constants() {
        assert_eq!(Color::WHITE, Color::rgb(1.0, 1.0, 1.0));
        assert_eq!(Color::BLACK, Color::rgb(0.0, 0.0, 0.0));
        assert_eq!(Color::TRANSPARENT, Color::rgba(0.0, 0.0, 0.0, 0.0));
    }

    // === Rect tests ===

    #[test]
    fn test_rect_new() {
        let r = Rect::new(10.0, 20.0, 100.0, 50.0);
        assert_eq!(r.x, 10.0);
        assert_eq!(r.y, 20.0);
        assert_eq!(r.width, 100.0);
        assert_eq!(r.height, 50.0);
    }

    #[test]
    fn test_rect_contains() {
        let r = Rect::new(10.0, 10.0, 100.0, 50.0);
        assert!(r.contains(50.0, 30.0)); // inside
        assert!(r.contains(10.0, 10.0)); // top-left corner
        assert!(!r.contains(110.0, 30.0)); // right edge (exclusive)
        assert!(!r.contains(5.0, 30.0)); // left of rect
    }

    #[test]
    fn test_rect_edges() {
        let r = Rect::new(10.0, 20.0, 100.0, 50.0);
        assert_eq!(r.right(), 110.0);
        assert_eq!(r.bottom(), 70.0);
    }

    // === GridPosition tests ===

    #[test]
    fn test_grid_position() {
        let p = GridPosition::new(5, 10);
        assert_eq!(p.row, 5);
        assert_eq!(p.col, 10);
    }

    // === CellContent tests ===

    #[test]
    fn test_cell_content_simple() {
        let cell = CellContent::simple('A', 0, 5);
        assert_eq!(cell.character, 'A');
        assert_eq!(cell.position.row, 0);
        assert_eq!(cell.position.col, 5);
        assert!(!cell.bold);
        assert!(!cell.italic);
    }

    // === CursorShape tests ===

    #[test]
    fn test_cursor_shape_default() {
        let shape = CursorShape::default();
        assert_eq!(shape, CursorShape::Block);
    }

    // === CursorInfo tests ===

    #[test]
    fn test_cursor_info() {
        let cursor = CursorInfo {
            position: GridPosition::new(0, 0),
            shape: CursorShape::Beam,
            color: Color::WHITE,
            visible: true,
        };
        assert!(cursor.visible);
        assert_eq!(cursor.shape, CursorShape::Beam);
    }

    // === SelectionRange tests ===

    #[test]
    fn test_selection_range() {
        let sel = SelectionRange {
            start: GridPosition::new(0, 5),
            end: GridPosition::new(0, 10),
            color: Color::rgba(0.0, 0.0, 1.0, 0.5),
        };
        assert_eq!(sel.start.col, 5);
        assert_eq!(sel.end.col, 10);
    }

    // === TabRenderInfo tests ===

    #[test]
    fn test_tab_render_info() {
        let tab = TabRenderInfo {
            title: "Terminal".to_string(),
            active: true,
            has_activity: false,
            bounds: Rect::new(0.0, 0.0, 100.0, 30.0),
        };
        assert!(tab.active);
        assert!(!tab.has_activity);
    }

    // === SearchHighlight tests ===

    #[test]
    fn test_search_highlight() {
        let highlight = SearchHighlight {
            bounds: Rect::new(10.0, 20.0, 50.0, 15.0),
            is_current: true,
        };
        assert!(highlight.is_current);
    }

    // === ContextMenuItem tests ===

    #[test]
    fn test_context_menu_item() {
        let item = ContextMenuItem {
            label: "Copy".to_string(),
            enabled: true,
            selected: false,
        };
        assert!(item.enabled);
        assert!(!item.selected);
    }
}
