//! Selection state management
//!
//! This module provides a testable selection state machine that tracks
//! terminal text selection without requiring GPU or window dependencies.

use crt_core::{Column, Line, Point};

/// Selection mode determining how text is selected
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionMode {
    /// Character-by-character selection (single click + drag)
    Simple,
    /// Word selection (double-click)
    Word,
    /// Line selection (triple-click)
    Line,
    /// Block/rectangular selection (Alt + drag)
    Block,
}

impl Default for SelectionMode {
    fn default() -> Self {
        Self::Simple
    }
}

/// Represents the current state of text selection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionState {
    /// No selection active
    None,
    /// Currently selecting (mouse is pressed and dragging)
    Selecting {
        /// Starting point of the selection (where mouse was pressed)
        start: Point,
        /// Current endpoint of the selection (current mouse position)
        end: Point,
        /// Mode of selection
        mode: SelectionMode,
    },
    /// Selection complete (mouse released with valid selection)
    Selected {
        /// Starting point of the selection
        start: Point,
        /// Ending point of the selection
        end: Point,
        /// Mode of selection
        mode: SelectionMode,
    },
}

impl Default for SelectionState {
    fn default() -> Self {
        Self::None
    }
}

impl SelectionState {
    /// Create a new selection state with no active selection
    pub fn new() -> Self {
        Self::None
    }

    /// Start a new selection at the given point
    ///
    /// This clears any existing selection and begins a new one.
    /// The selection is in "Selecting" state until `finish()` is called.
    pub fn start(&mut self, point: Point, mode: SelectionMode) {
        *self = Self::Selecting {
            start: point,
            end: point,
            mode,
        };
    }

    /// Update the selection endpoint during drag
    ///
    /// Only has effect if currently in "Selecting" state.
    pub fn update(&mut self, point: Point) {
        if let Self::Selecting { start, mode, .. } = *self {
            *self = Self::Selecting {
                start,
                end: point,
                mode,
            };
        }
    }

    /// Finish the selection (called on mouse release)
    ///
    /// If the start and end points are the same (single click without drag),
    /// the selection is cleared. Otherwise, transitions to "Selected" state.
    pub fn finish(&mut self) {
        if let Self::Selecting { start, end, mode } = *self {
            if start == end {
                // Single click without movement = no selection
                *self = Self::None;
            } else {
                *self = Self::Selected { start, end, mode };
            }
        }
    }

    /// Clear any active selection
    pub fn clear(&mut self) {
        *self = Self::None;
    }

    /// Check if there's any active selection (selecting or selected)
    pub fn is_active(&self) -> bool {
        !matches!(self, Self::None)
    }

    /// Check if currently in the middle of selecting (mouse down)
    pub fn is_selecting(&self) -> bool {
        matches!(self, Self::Selecting { .. })
    }

    /// Check if selection is complete (mouse released with valid selection)
    pub fn is_selected(&self) -> bool {
        matches!(self, Self::Selected { .. })
    }

    /// Get ordered start/end points (start always before end in reading order)
    ///
    /// Returns `None` if no selection is active.
    pub fn ordered_bounds(&self) -> Option<(Point, Point)> {
        match *self {
            Self::None => None,
            Self::Selecting { start, end, .. } | Self::Selected { start, end, .. } => {
                Some(Self::order_points(start, end))
            }
        }
    }

    /// Get the raw (unordered) start and end points
    ///
    /// Returns `None` if no selection is active.
    pub fn raw_bounds(&self) -> Option<(Point, Point)> {
        match *self {
            Self::None => None,
            Self::Selecting { start, end, .. } | Self::Selected { start, end, .. } => {
                Some((start, end))
            }
        }
    }

    /// Get the selection mode
    ///
    /// Returns `None` if no selection is active.
    pub fn mode(&self) -> Option<SelectionMode> {
        match *self {
            Self::None => None,
            Self::Selecting { mode, .. } | Self::Selected { mode, .. } => Some(mode),
        }
    }

    /// Get the starting point of the selection (where mouse was first pressed)
    ///
    /// Returns `None` if no selection is active.
    pub fn anchor(&self) -> Option<Point> {
        match *self {
            Self::None => None,
            Self::Selecting { start, .. } | Self::Selected { start, .. } => Some(start),
        }
    }

    /// Get the current endpoint of the selection
    ///
    /// Returns `None` if no selection is active.
    pub fn cursor(&self) -> Option<Point> {
        match *self {
            Self::None => None,
            Self::Selecting { end, .. } | Self::Selected { end, .. } => Some(end),
        }
    }

    /// Check if a point is within the selection
    ///
    /// Returns `false` if no selection is active.
    pub fn contains(&self, point: Point) -> bool {
        let Some((start, end)) = self.ordered_bounds() else {
            return false;
        };

        match self.mode() {
            Some(SelectionMode::Block) => {
                // Block selection: point must be within the rectangular bounds
                let min_col = start.column.min(end.column);
                let max_col = start.column.max(end.column);
                point.line >= start.line
                    && point.line <= end.line
                    && point.column >= min_col
                    && point.column <= max_col
            }
            _ => {
                // Linear selection: standard text selection
                if point.line < start.line || point.line > end.line {
                    return false;
                }
                if point.line == start.line && point.column < start.column {
                    return false;
                }
                if point.line == end.line && point.column > end.column {
                    return false;
                }
                true
            }
        }
    }

    /// Get the number of lines in the selection
    ///
    /// Returns `0` if no selection is active.
    pub fn line_count(&self) -> usize {
        match self.ordered_bounds() {
            Some((start, end)) => (end.line.0 - start.line.0 + 1) as usize,
            None => 0,
        }
    }

    /// Check if the selection spans multiple lines
    pub fn is_multiline(&self) -> bool {
        self.line_count() > 1
    }

    /// Order two points so that the first is before or equal to the second
    fn order_points(a: Point, b: Point) -> (Point, Point) {
        if a.line < b.line || (a.line == b.line && a.column <= b.column) {
            (a, b)
        } else {
            (b, a)
        }
    }
}

/// A range within a single line for rendering purposes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineRange {
    /// Line number
    pub line: Line,
    /// Starting column (inclusive)
    pub start_col: Column,
    /// Ending column (exclusive)
    pub end_col: Column,
}

impl LineRange {
    /// Create a new line range
    pub fn new(line: Line, start_col: Column, end_col: Column) -> Self {
        Self {
            line,
            start_col,
            end_col,
        }
    }

    /// Create a range spanning the entire line width
    pub fn full_line(line: Line, line_width: usize) -> Self {
        Self {
            line,
            start_col: Column(0),
            end_col: Column(line_width),
        }
    }

    /// Check if a column is within this range
    pub fn contains_column(&self, col: Column) -> bool {
        col >= self.start_col && col < self.end_col
    }

    /// Get the width of this range in columns
    pub fn width(&self) -> usize {
        self.end_col.0.saturating_sub(self.start_col.0)
    }
}

/// Convert a selection to line ranges for rendering
///
/// # Arguments
/// * `state` - The selection state to convert
/// * `line_width` - Width of each line in columns (for full-line ranges)
///
/// Returns a vector of `LineRange` representing the selected areas.
pub fn selection_to_ranges(state: &SelectionState, line_width: usize) -> Vec<LineRange> {
    let Some((start, end)) = state.ordered_bounds() else {
        return Vec::new();
    };

    let mode = state.mode().unwrap_or(SelectionMode::Simple);
    let mut ranges = Vec::new();

    match mode {
        SelectionMode::Block => {
            // Block selection: same columns on each line
            let min_col = start.column.min(end.column);
            let max_col = start.column.max(end.column);
            for line in start.line.0..=end.line.0 {
                ranges.push(LineRange::new(
                    Line(line),
                    min_col,
                    Column(max_col.0 + 1), // exclusive end
                ));
            }
        }
        SelectionMode::Line => {
            // Line selection: full lines
            for line in start.line.0..=end.line.0 {
                ranges.push(LineRange::full_line(Line(line), line_width));
            }
        }
        _ => {
            // Simple or Word: standard text selection
            if start.line == end.line {
                // Single line selection
                ranges.push(LineRange::new(
                    start.line,
                    start.column,
                    Column(end.column.0 + 1),
                ));
            } else {
                // Multi-line selection
                // First line: from start to end of line
                ranges.push(LineRange::new(
                    start.line,
                    start.column,
                    Column(line_width),
                ));

                // Middle lines: full lines
                for line in (start.line.0 + 1)..end.line.0 {
                    ranges.push(LineRange::full_line(Line(line), line_width));
                }

                // Last line: from start of line to end column
                ranges.push(LineRange::new(
                    end.line,
                    Column(0),
                    Column(end.column.0 + 1),
                ));
            }
        }
    }

    ranges
}

#[cfg(test)]
mod tests {
    use super::*;

    // === SelectionState basic tests ===

    #[test]
    fn test_initial_state() {
        let state = SelectionState::new();
        assert!(!state.is_active());
        assert!(!state.is_selecting());
        assert!(!state.is_selected());
        assert_eq!(state.mode(), None);
        assert_eq!(state.ordered_bounds(), None);
    }

    #[test]
    fn test_default_state() {
        let state = SelectionState::default();
        assert!(!state.is_active());
    }

    #[test]
    fn test_start_selection() {
        let mut state = SelectionState::new();
        let point = Point::new(Line(5), Column(10));
        state.start(point, SelectionMode::Simple);

        assert!(state.is_active());
        assert!(state.is_selecting());
        assert!(!state.is_selected());
        assert_eq!(state.mode(), Some(SelectionMode::Simple));
        assert_eq!(state.anchor(), Some(point));
        assert_eq!(state.cursor(), Some(point));
    }

    #[test]
    fn test_start_word_selection() {
        let mut state = SelectionState::new();
        let point = Point::new(Line(0), Column(0));
        state.start(point, SelectionMode::Word);

        assert_eq!(state.mode(), Some(SelectionMode::Word));
    }

    #[test]
    fn test_start_line_selection() {
        let mut state = SelectionState::new();
        let point = Point::new(Line(0), Column(0));
        state.start(point, SelectionMode::Line);

        assert_eq!(state.mode(), Some(SelectionMode::Line));
    }

    #[test]
    fn test_start_block_selection() {
        let mut state = SelectionState::new();
        let point = Point::new(Line(0), Column(0));
        state.start(point, SelectionMode::Block);

        assert_eq!(state.mode(), Some(SelectionMode::Block));
    }

    #[test]
    fn test_update_selection() {
        let mut state = SelectionState::new();
        let start = Point::new(Line(5), Column(10));
        let end = Point::new(Line(5), Column(20));

        state.start(start, SelectionMode::Simple);
        state.update(end);

        assert!(state.is_selecting());
        assert_eq!(state.anchor(), Some(start));
        assert_eq!(state.cursor(), Some(end));

        let bounds = state.ordered_bounds().unwrap();
        assert_eq!(bounds, (start, end));
    }

    #[test]
    fn test_update_no_effect_when_not_selecting() {
        let mut state = SelectionState::new();
        state.update(Point::new(Line(5), Column(10)));

        assert!(!state.is_active());
    }

    #[test]
    fn test_finish_selection() {
        let mut state = SelectionState::new();
        let start = Point::new(Line(5), Column(10));
        let end = Point::new(Line(5), Column(20));

        state.start(start, SelectionMode::Simple);
        state.update(end);
        state.finish();

        assert!(state.is_active());
        assert!(!state.is_selecting());
        assert!(state.is_selected());
    }

    #[test]
    fn test_single_click_clears() {
        let mut state = SelectionState::new();
        let point = Point::new(Line(5), Column(10));

        state.start(point, SelectionMode::Simple);
        state.finish(); // No movement = no selection

        assert!(!state.is_active());
    }

    #[test]
    fn test_clear_selection() {
        let mut state = SelectionState::new();
        state.start(Point::new(Line(0), Column(0)), SelectionMode::Simple);
        state.update(Point::new(Line(1), Column(5)));
        state.clear();

        assert!(!state.is_active());
        assert_eq!(state.ordered_bounds(), None);
    }

    #[test]
    fn test_clear_finished_selection() {
        let mut state = SelectionState::new();
        state.start(Point::new(Line(0), Column(0)), SelectionMode::Simple);
        state.update(Point::new(Line(1), Column(5)));
        state.finish();
        state.clear();

        assert!(!state.is_active());
    }

    // === Ordering tests ===

    #[test]
    fn test_ordered_bounds_same_line() {
        let mut state = SelectionState::new();
        let start = Point::new(Line(5), Column(20));
        let end = Point::new(Line(5), Column(10));

        state.start(start, SelectionMode::Simple);
        state.update(end);

        let (ordered_start, ordered_end) = state.ordered_bounds().unwrap();
        assert_eq!(ordered_start, end); // Earlier column
        assert_eq!(ordered_end, start); // Later column
    }

    #[test]
    fn test_ordered_bounds_different_lines() {
        let mut state = SelectionState::new();
        let start = Point::new(Line(10), Column(20));
        let end = Point::new(Line(5), Column(10));

        state.start(start, SelectionMode::Simple);
        state.update(end);

        let (ordered_start, ordered_end) = state.ordered_bounds().unwrap();
        assert_eq!(ordered_start, end); // Earlier line
        assert_eq!(ordered_end, start); // Later line
    }

    #[test]
    fn test_raw_bounds_preserves_order() {
        let mut state = SelectionState::new();
        let start = Point::new(Line(10), Column(20));
        let end = Point::new(Line(5), Column(10));

        state.start(start, SelectionMode::Simple);
        state.update(end);

        let (raw_start, raw_end) = state.raw_bounds().unwrap();
        assert_eq!(raw_start, start); // Original order preserved
        assert_eq!(raw_end, end);
    }

    // === Contains tests ===

    #[test]
    fn test_contains_single_line() {
        let mut state = SelectionState::new();
        state.start(Point::new(Line(5), Column(5)), SelectionMode::Simple);
        state.update(Point::new(Line(5), Column(15)));

        assert!(state.contains(Point::new(Line(5), Column(10))));
        assert!(state.contains(Point::new(Line(5), Column(5))));
        assert!(state.contains(Point::new(Line(5), Column(15))));
        assert!(!state.contains(Point::new(Line(5), Column(4))));
        assert!(!state.contains(Point::new(Line(5), Column(16))));
        assert!(!state.contains(Point::new(Line(4), Column(10))));
    }

    #[test]
    fn test_contains_multi_line() {
        let mut state = SelectionState::new();
        state.start(Point::new(Line(2), Column(10)), SelectionMode::Simple);
        state.update(Point::new(Line(5), Column(5)));

        // Middle line - any column should be included
        assert!(state.contains(Point::new(Line(3), Column(0))));
        assert!(state.contains(Point::new(Line(3), Column(100))));

        // First line - only from start column
        assert!(state.contains(Point::new(Line(2), Column(10))));
        assert!(state.contains(Point::new(Line(2), Column(50))));
        assert!(!state.contains(Point::new(Line(2), Column(9))));

        // Last line - only up to end column
        assert!(state.contains(Point::new(Line(5), Column(5))));
        assert!(state.contains(Point::new(Line(5), Column(0))));
        assert!(!state.contains(Point::new(Line(5), Column(6))));
    }

    #[test]
    fn test_contains_block_selection() {
        let mut state = SelectionState::new();
        state.start(Point::new(Line(2), Column(5)), SelectionMode::Block);
        state.update(Point::new(Line(5), Column(15)));

        // Inside block
        assert!(state.contains(Point::new(Line(3), Column(10))));

        // Outside block horizontally
        assert!(!state.contains(Point::new(Line(3), Column(4))));
        assert!(!state.contains(Point::new(Line(3), Column(16))));

        // Outside block vertically
        assert!(!state.contains(Point::new(Line(1), Column(10))));
        assert!(!state.contains(Point::new(Line(6), Column(10))));
    }

    #[test]
    fn test_contains_no_selection() {
        let state = SelectionState::new();
        assert!(!state.contains(Point::new(Line(0), Column(0))));
    }

    // === Line count tests ===

    #[test]
    fn test_line_count_single() {
        let mut state = SelectionState::new();
        state.start(Point::new(Line(5), Column(0)), SelectionMode::Simple);
        state.update(Point::new(Line(5), Column(10)));

        assert_eq!(state.line_count(), 1);
        assert!(!state.is_multiline());
    }

    #[test]
    fn test_line_count_multi() {
        let mut state = SelectionState::new();
        state.start(Point::new(Line(2), Column(0)), SelectionMode::Simple);
        state.update(Point::new(Line(5), Column(10)));

        assert_eq!(state.line_count(), 4);
        assert!(state.is_multiline());
    }

    #[test]
    fn test_line_count_none() {
        let state = SelectionState::new();
        assert_eq!(state.line_count(), 0);
        assert!(!state.is_multiline());
    }

    // === LineRange tests ===

    #[test]
    fn test_line_range_new() {
        let range = LineRange::new(Line(5), Column(10), Column(20));
        assert_eq!(range.line, Line(5));
        assert_eq!(range.start_col, Column(10));
        assert_eq!(range.end_col, Column(20));
    }

    #[test]
    fn test_line_range_full_line() {
        let range = LineRange::full_line(Line(5), 80);
        assert_eq!(range.start_col, Column(0));
        assert_eq!(range.end_col, Column(80));
    }

    #[test]
    fn test_line_range_contains_column() {
        let range = LineRange::new(Line(0), Column(10), Column(20));
        assert!(range.contains_column(Column(10)));
        assert!(range.contains_column(Column(15)));
        assert!(range.contains_column(Column(19)));
        assert!(!range.contains_column(Column(9)));
        assert!(!range.contains_column(Column(20))); // exclusive
    }

    #[test]
    fn test_line_range_width() {
        let range = LineRange::new(Line(0), Column(10), Column(20));
        assert_eq!(range.width(), 10);
    }

    // === selection_to_ranges tests ===

    #[test]
    fn test_ranges_no_selection() {
        let state = SelectionState::new();
        let ranges = selection_to_ranges(&state, 80);
        assert!(ranges.is_empty());
    }

    #[test]
    fn test_ranges_single_line() {
        let mut state = SelectionState::new();
        state.start(Point::new(Line(5), Column(10)), SelectionMode::Simple);
        state.update(Point::new(Line(5), Column(20)));

        let ranges = selection_to_ranges(&state, 80);
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0].line, Line(5));
        assert_eq!(ranges[0].start_col, Column(10));
        assert_eq!(ranges[0].end_col, Column(21)); // exclusive
    }

    #[test]
    fn test_ranges_multi_line() {
        let mut state = SelectionState::new();
        state.start(Point::new(Line(2), Column(10)), SelectionMode::Simple);
        state.update(Point::new(Line(4), Column(5)));

        let ranges = selection_to_ranges(&state, 80);
        assert_eq!(ranges.len(), 3);

        // First line: from column 10 to end
        assert_eq!(ranges[0].line, Line(2));
        assert_eq!(ranges[0].start_col, Column(10));
        assert_eq!(ranges[0].end_col, Column(80));

        // Middle line: full line
        assert_eq!(ranges[1].line, Line(3));
        assert_eq!(ranges[1].start_col, Column(0));
        assert_eq!(ranges[1].end_col, Column(80));

        // Last line: from start to column 5
        assert_eq!(ranges[2].line, Line(4));
        assert_eq!(ranges[2].start_col, Column(0));
        assert_eq!(ranges[2].end_col, Column(6));
    }

    #[test]
    fn test_ranges_line_mode() {
        let mut state = SelectionState::new();
        state.start(Point::new(Line(2), Column(10)), SelectionMode::Line);
        state.update(Point::new(Line(4), Column(5)));

        let ranges = selection_to_ranges(&state, 80);
        assert_eq!(ranges.len(), 3);

        // All lines should be full width
        for (i, range) in ranges.iter().enumerate() {
            assert_eq!(range.line, Line(2 + i as i32));
            assert_eq!(range.start_col, Column(0));
            assert_eq!(range.end_col, Column(80));
        }
    }

    #[test]
    fn test_ranges_block_mode() {
        let mut state = SelectionState::new();
        state.start(Point::new(Line(2), Column(10)), SelectionMode::Block);
        state.update(Point::new(Line(4), Column(20)));

        let ranges = selection_to_ranges(&state, 80);
        assert_eq!(ranges.len(), 3);

        // All lines should have same column range
        for (i, range) in ranges.iter().enumerate() {
            assert_eq!(range.line, Line(2 + i as i32));
            assert_eq!(range.start_col, Column(10));
            assert_eq!(range.end_col, Column(21));
        }
    }

    // === SelectionMode tests ===

    #[test]
    fn test_selection_mode_default() {
        assert_eq!(SelectionMode::default(), SelectionMode::Simple);
    }
}
