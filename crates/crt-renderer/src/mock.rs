//! Mock renderer for testing
//!
//! Provides a MockRenderer that implements all renderer traits and records
//! all render calls for test assertions, without requiring GPU context.

use crate::traits::*;

/// Record of a render call for test inspection
#[derive(Debug, Clone)]
pub enum RenderCall {
    // Text rendering calls
    /// Cells were rendered
    Cells(Vec<CellContent>),
    /// Cursor was rendered
    Cursor(CursorInfo),
    /// Selection ranges were rendered
    Selection(Vec<SelectionRange>),
    /// Text buffer was cleared
    ClearText,

    // UI rendering calls
    /// Tab bar was rendered
    Tabs {
        tabs: Vec<TabRenderInfo>,
        bounds: Rect,
    },
    /// Search matches were rendered
    SearchMatches {
        matches: Vec<SearchHighlight>,
        highlight_color: Color,
    },
    /// Context menu was rendered
    ContextMenu {
        position: (f32, f32),
        items: Vec<ContextMenuItem>,
    },
    /// Bell flash was rendered
    BellFlash(f32),

    // Effects rendering calls
    /// Effects were updated
    EffectsUpdate(f32),
    /// Effects were rendered
    EffectsRender,
}

/// A mock renderer that records all render calls for testing
///
/// This renderer implements all rendering traits but doesn't actually
/// render anything - instead it records all calls in a vector that
/// can be inspected in tests.
#[derive(Debug, Default)]
pub struct MockRenderer {
    /// All render calls made to this renderer
    pub calls: Vec<RenderCall>,
    /// Whether this renderer reports needing animation
    pub needs_animation: bool,
}

impl MockRenderer {
    /// Create a new mock renderer
    pub fn new() -> Self {
        Self::default()
    }

    /// Clear all recorded calls
    pub fn clear_calls(&mut self) {
        self.calls.clear();
    }

    /// Get the number of recorded calls
    pub fn call_count(&self) -> usize {
        self.calls.len()
    }

    // === Assertion helpers ===

    /// Check if a specific character was rendered at a position
    pub fn has_cell(&self, character: char, row: i32, col: usize) -> bool {
        self.calls.iter().any(|call| {
            if let RenderCall::Cells(cells) = call {
                cells.iter().any(|c| {
                    c.character == character
                        && c.position.row == row
                        && c.position.col == col
                })
            } else {
                false
            }
        })
    }

    /// Check if cursor was rendered at a specific position
    pub fn has_cursor_at(&self, row: i32, col: usize) -> bool {
        self.calls.iter().any(|call| {
            if let RenderCall::Cursor(cursor) = call {
                cursor.position.row == row && cursor.position.col == col
            } else {
                false
            }
        })
    }

    /// Check if any selection was rendered
    pub fn has_selection(&self) -> bool {
        self.calls.iter().any(|call| {
            matches!(call, RenderCall::Selection(ranges) if !ranges.is_empty())
        })
    }

    /// Count total cells rendered across all calls
    pub fn total_cells_rendered(&self) -> usize {
        self.calls
            .iter()
            .filter_map(|call| {
                if let RenderCall::Cells(cells) = call {
                    Some(cells.len())
                } else {
                    None
                }
            })
            .sum()
    }

    /// Check if tabs were rendered
    pub fn has_tabs(&self) -> bool {
        self.calls.iter().any(|call| matches!(call, RenderCall::Tabs { .. }))
    }

    /// Get the last rendered tabs if any
    pub fn last_tabs(&self) -> Option<&[TabRenderInfo]> {
        self.calls.iter().rev().find_map(|call| {
            if let RenderCall::Tabs { tabs, .. } = call {
                Some(tabs.as_slice())
            } else {
                None
            }
        })
    }

    /// Check if context menu was rendered
    pub fn has_context_menu(&self) -> bool {
        self.calls.iter().any(|call| matches!(call, RenderCall::ContextMenu { .. }))
    }

    /// Check if bell flash was rendered
    pub fn has_bell_flash(&self) -> bool {
        self.calls.iter().any(|call| matches!(call, RenderCall::BellFlash(_)))
    }

    /// Get the last bell flash intensity if any
    pub fn last_bell_intensity(&self) -> Option<f32> {
        self.calls.iter().rev().find_map(|call| {
            if let RenderCall::BellFlash(intensity) = call {
                Some(*intensity)
            } else {
                None
            }
        })
    }

    /// Check if text was cleared
    pub fn was_cleared(&self) -> bool {
        self.calls.iter().any(|call| matches!(call, RenderCall::ClearText))
    }

    /// Check if effects update was called
    pub fn has_effects_update(&self) -> bool {
        self.calls.iter().any(|call| matches!(call, RenderCall::EffectsUpdate(_)))
    }

    /// Get the last effects update delta time if any
    pub fn last_effects_dt(&self) -> Option<f32> {
        self.calls.iter().rev().find_map(|call| {
            if let RenderCall::EffectsUpdate(dt) = call {
                Some(*dt)
            } else {
                None
            }
        })
    }

    /// Get all rendered characters as a string (for simple content assertions)
    pub fn rendered_text(&self) -> String {
        let mut chars: Vec<(i32, usize, char)> = self
            .calls
            .iter()
            .filter_map(|call| {
                if let RenderCall::Cells(cells) = call {
                    Some(cells.iter().map(|c| (c.position.row, c.position.col, c.character)))
                } else {
                    None
                }
            })
            .flatten()
            .collect();

        // Sort by row, then column
        chars.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));

        let mut result = String::new();
        let mut last_row = None;
        for (row, _, ch) in chars {
            if last_row != Some(row) {
                if last_row.is_some() {
                    result.push('\n');
                }
                last_row = Some(row);
            }
            result.push(ch);
        }
        result
    }
}

impl TextRenderer for MockRenderer {
    fn render_cells(&mut self, cells: &[CellContent]) {
        self.calls.push(RenderCall::Cells(cells.to_vec()));
    }

    fn render_cursor(&mut self, cursor: &CursorInfo) {
        self.calls.push(RenderCall::Cursor(*cursor));
    }

    fn render_selection(&mut self, ranges: &[SelectionRange]) {
        self.calls.push(RenderCall::Selection(ranges.to_vec()));
    }

    fn clear(&mut self) {
        self.calls.push(RenderCall::ClearText);
    }
}

impl UiRenderer for MockRenderer {
    fn render_tabs(&mut self, tabs: &[TabRenderInfo], bar_bounds: Rect) {
        self.calls.push(RenderCall::Tabs {
            tabs: tabs.to_vec(),
            bounds: bar_bounds,
        });
    }

    fn render_search_matches(&mut self, matches: &[SearchHighlight], highlight_color: Color) {
        self.calls.push(RenderCall::SearchMatches {
            matches: matches.to_vec(),
            highlight_color,
        });
    }

    fn render_context_menu(&mut self, position: (f32, f32), items: &[ContextMenuItem]) {
        self.calls.push(RenderCall::ContextMenu {
            position,
            items: items.to_vec(),
        });
    }

    fn render_bell_flash(&mut self, intensity: f32) {
        self.calls.push(RenderCall::BellFlash(intensity));
    }
}

impl BackdropRenderer for MockRenderer {
    fn update(&mut self, dt: f32) {
        self.calls.push(RenderCall::EffectsUpdate(dt));
    }

    fn render(&mut self) {
        self.calls.push(RenderCall::EffectsRender);
    }

    fn needs_animation(&self) -> bool {
        self.needs_animation
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_mock_is_empty() {
        let mock = MockRenderer::new();
        assert_eq!(mock.call_count(), 0);
        assert!(mock.calls.is_empty());
    }

    #[test]
    fn test_record_cells() {
        let mut mock = MockRenderer::new();

        let cells = vec![
            CellContent::simple('H', 0, 0),
            CellContent::simple('i', 0, 1),
        ];

        mock.render_cells(&cells);

        assert_eq!(mock.call_count(), 1);
        assert!(mock.has_cell('H', 0, 0));
        assert!(mock.has_cell('i', 0, 1));
        assert!(!mock.has_cell('X', 0, 0));
    }

    #[test]
    fn test_total_cells() {
        let mut mock = MockRenderer::new();

        mock.render_cells(&[CellContent::simple('A', 0, 0)]);
        mock.render_cells(&[
            CellContent::simple('B', 0, 1),
            CellContent::simple('C', 0, 2),
        ]);

        assert_eq!(mock.total_cells_rendered(), 3);
    }

    #[test]
    fn test_record_cursor() {
        let mut mock = MockRenderer::new();

        let cursor = CursorInfo {
            position: GridPosition::new(5, 10),
            shape: CursorShape::Block,
            color: Color::WHITE,
            visible: true,
        };

        mock.render_cursor(&cursor);

        assert!(mock.has_cursor_at(5, 10));
        assert!(!mock.has_cursor_at(0, 0));
    }

    #[test]
    fn test_record_selection() {
        let mut mock = MockRenderer::new();

        let ranges = vec![SelectionRange {
            start: GridPosition::new(0, 0),
            end: GridPosition::new(0, 10),
            color: Color::rgba(0.0, 0.0, 1.0, 0.5),
        }];

        mock.render_selection(&ranges);

        assert!(mock.has_selection());
    }

    #[test]
    fn test_no_selection() {
        let mut mock = MockRenderer::new();
        mock.render_selection(&[]);
        assert!(!mock.has_selection());
    }

    #[test]
    fn test_record_clear() {
        let mut mock = MockRenderer::new();
        mock.clear();
        assert!(mock.was_cleared());
    }

    #[test]
    fn test_record_tabs() {
        let mut mock = MockRenderer::new();

        let tabs = vec![
            TabRenderInfo {
                title: "Tab 1".to_string(),
                active: true,
                has_activity: false,
                bounds: Rect::new(0.0, 0.0, 100.0, 30.0),
            },
            TabRenderInfo {
                title: "Tab 2".to_string(),
                active: false,
                has_activity: true,
                bounds: Rect::new(100.0, 0.0, 100.0, 30.0),
            },
        ];

        mock.render_tabs(&tabs, Rect::new(0.0, 0.0, 800.0, 30.0));

        assert!(mock.has_tabs());
        let rendered = mock.last_tabs().unwrap();
        assert_eq!(rendered.len(), 2);
        assert_eq!(rendered[0].title, "Tab 1");
    }

    #[test]
    fn test_record_context_menu() {
        let mut mock = MockRenderer::new();

        let items = vec![
            ContextMenuItem {
                label: "Copy".to_string(),
                enabled: true,
                selected: false,
            },
            ContextMenuItem {
                label: "Paste".to_string(),
                enabled: true,
                selected: true,
            },
        ];

        mock.render_context_menu((100.0, 200.0), &items);

        assert!(mock.has_context_menu());
    }

    #[test]
    fn test_record_bell() {
        let mut mock = MockRenderer::new();

        mock.render_bell_flash(0.75);

        assert!(mock.has_bell_flash());
        assert_eq!(mock.last_bell_intensity(), Some(0.75));
    }

    #[test]
    fn test_record_effects() {
        let mut mock = MockRenderer::new();

        mock.update(0.016);
        mock.render();

        assert!(mock.has_effects_update());
        assert_eq!(mock.last_effects_dt(), Some(0.016));
    }

    #[test]
    fn test_needs_animation() {
        let mut mock = MockRenderer::new();
        assert!(!mock.needs_animation());

        mock.needs_animation = true;
        assert!(mock.needs_animation());
    }

    #[test]
    fn test_clear_calls() {
        let mut mock = MockRenderer::new();
        mock.render_cells(&[CellContent::simple('A', 0, 0)]);
        mock.render_cursor(&CursorInfo {
            position: GridPosition::new(0, 0),
            shape: CursorShape::Block,
            color: Color::WHITE,
            visible: true,
        });

        assert_eq!(mock.call_count(), 2);

        mock.clear_calls();

        assert_eq!(mock.call_count(), 0);
    }

    #[test]
    fn test_rendered_text() {
        let mut mock = MockRenderer::new();

        mock.render_cells(&[
            CellContent::simple('H', 0, 0),
            CellContent::simple('i', 0, 1),
        ]);
        mock.render_cells(&[
            CellContent::simple('W', 1, 0),
            CellContent::simple('o', 1, 1),
            CellContent::simple('r', 1, 2),
            CellContent::simple('l', 1, 3),
            CellContent::simple('d', 1, 4),
        ]);

        let text = mock.rendered_text();
        assert_eq!(text, "Hi\nWorld");
    }

    #[test]
    fn test_render_call_debug() {
        let call = RenderCall::ClearText;
        let debug = format!("{:?}", call);
        assert!(debug.contains("ClearText"));
    }
}
