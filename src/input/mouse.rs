//! Pure mouse input handling
//!
//! This module provides pure functions for converting mouse coordinates to
//! terminal grid positions and handling selection logic. All functions are
//! side-effect free and can be easily unit tested.

use crt_core::{Column, Line, Point};

/// Cell size and positioning metrics for coordinate conversion
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CellMetrics {
    /// Width of a single cell in pixels
    pub cell_width: f32,
    /// Height of a single cell (line height) in pixels
    pub cell_height: f32,
    /// Padding from left edge of content area in pixels
    pub padding_left: f32,
    /// Padding from top edge of content area in pixels
    pub padding_top: f32,
}

impl CellMetrics {
    /// Create new cell metrics with uniform padding
    pub fn new(cell_width: f32, cell_height: f32, padding: f32) -> Self {
        Self {
            cell_width,
            cell_height,
            padding_left: padding,
            padding_top: padding,
        }
    }

    /// Create new cell metrics with separate padding values
    pub fn with_padding(
        cell_width: f32,
        cell_height: f32,
        padding_left: f32,
        padding_top: f32,
    ) -> Self {
        Self {
            cell_width,
            cell_height,
            padding_left,
            padding_top,
        }
    }
}

impl Default for CellMetrics {
    fn default() -> Self {
        Self {
            cell_width: 10.0,
            cell_height: 20.0,
            padding_left: 10.0,
            padding_top: 10.0,
        }
    }
}

/// Convert pixel coordinates to grid cell position
///
/// Returns the grid cell (column, line) at the given pixel coordinates.
/// Coordinates outside the grid area will be clamped to the nearest edge.
///
/// # Arguments
/// * `x` - X coordinate in pixels (from left edge of content area)
/// * `y` - Y coordinate in pixels (from top edge of content area)
/// * `metrics` - Cell sizing and padding information
pub fn pixel_to_cell(x: f32, y: f32, metrics: &CellMetrics) -> Point {
    let col = ((x - metrics.padding_left) / metrics.cell_width)
        .max(0.0)
        .floor() as usize;
    let row = ((y - metrics.padding_top) / metrics.cell_height)
        .max(0.0)
        .floor() as i32;

    Point::new(Line(row), Column(col))
}

/// Convert pixel coordinates to grid cell, clamped to grid bounds
///
/// Same as `pixel_to_cell` but ensures the result is within the grid dimensions.
///
/// # Arguments
/// * `x` - X coordinate in pixels
/// * `y` - Y coordinate in pixels
/// * `metrics` - Cell sizing and padding information
/// * `cols` - Number of columns in the grid
/// * `rows` - Number of rows in the grid
pub fn pixel_to_cell_clamped(
    x: f32,
    y: f32,
    metrics: &CellMetrics,
    cols: usize,
    rows: usize,
) -> Point {
    let point = pixel_to_cell(x, y, metrics);
    let col = point.column.0.min(cols.saturating_sub(1));
    let row = point.line.0.max(0).min((rows.saturating_sub(1)) as i32);
    Point::new(Line(row), Column(col))
}

/// Convert grid cell to pixel coordinates (top-left corner of cell)
///
/// Returns the pixel coordinates of the top-left corner of the given cell.
///
/// # Arguments
/// * `point` - The grid cell position
/// * `metrics` - Cell sizing and padding information
pub fn cell_to_pixel(point: Point, metrics: &CellMetrics) -> (f32, f32) {
    let x = metrics.padding_left + (point.column.0 as f32 * metrics.cell_width);
    let y = metrics.padding_top + (point.line.0 as f32 * metrics.cell_height);
    (x, y)
}

/// Convert grid cell to pixel coordinates (center of cell)
///
/// Returns the pixel coordinates of the center of the given cell.
///
/// # Arguments
/// * `point` - The grid cell position
/// * `metrics` - Cell sizing and padding information
pub fn cell_to_pixel_center(point: Point, metrics: &CellMetrics) -> (f32, f32) {
    let (x, y) = cell_to_pixel(point, metrics);
    (x + metrics.cell_width / 2.0, y + metrics.cell_height / 2.0)
}

/// Check if pixel coordinates are within the terminal grid area
///
/// Returns true if the coordinates fall within the grid bounds.
///
/// # Arguments
/// * `x` - X coordinate in pixels
/// * `y` - Y coordinate in pixels
/// * `metrics` - Cell sizing and padding information
/// * `cols` - Number of columns in the grid
/// * `rows` - Number of rows in the grid
pub fn is_in_grid(x: f32, y: f32, metrics: &CellMetrics, cols: usize, rows: usize) -> bool {
    let grid_right = metrics.padding_left + (cols as f32 * metrics.cell_width);
    let grid_bottom = metrics.padding_top + (rows as f32 * metrics.cell_height);

    x >= metrics.padding_left && x < grid_right && y >= metrics.padding_top && y < grid_bottom
}

/// Selection range with ordered start and end points
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionRange {
    /// Starting point of selection (always <= end)
    pub start: Point,
    /// Ending point of selection (always >= start)
    pub end: Point,
}

impl SelectionRange {
    /// Create a new selection range from two points
    ///
    /// The points are automatically ordered so that start is always before
    /// or equal to end (in reading order: top-to-bottom, left-to-right).
    pub fn new(a: Point, b: Point) -> Self {
        if a.line < b.line || (a.line == b.line && a.column <= b.column) {
            Self { start: a, end: b }
        } else {
            Self { start: b, end: a }
        }
    }

    /// Create a selection range for a single cell
    pub fn single(point: Point) -> Self {
        Self {
            start: point,
            end: point,
        }
    }

    /// Check if a point is within this selection
    pub fn contains(&self, point: Point) -> bool {
        if point.line < self.start.line || point.line > self.end.line {
            return false;
        }
        if point.line == self.start.line && point.column < self.start.column {
            return false;
        }
        if point.line == self.end.line && point.column > self.end.column {
            return false;
        }
        true
    }

    /// Check if this selection spans multiple lines
    pub fn is_multiline(&self) -> bool {
        self.start.line != self.end.line
    }

    /// Check if this selection is empty (start == end)
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// Get the number of lines in this selection
    pub fn line_count(&self) -> usize {
        (self.end.line.0 - self.start.line.0 + 1) as usize
    }
}

/// Expand a column position to word boundaries
///
/// Returns (start_col, end_col) for the word at the given column position.
/// Word characters are alphanumeric characters and underscore.
///
/// # Arguments
/// * `line_content` - The text content of the line
/// * `col` - The column position to expand from
pub fn expand_to_word(line_content: &str, col: usize) -> (usize, usize) {
    let chars: Vec<char> = line_content.chars().collect();

    if col >= chars.len() {
        return (col, col);
    }

    let is_word_char = |c: char| c.is_alphanumeric() || c == '_';

    // If we're not on a word character, don't expand
    if !is_word_char(chars[col]) {
        return (col, col + 1);
    }

    // Find start of word
    let mut start = col;
    while start > 0 && is_word_char(chars[start - 1]) {
        start -= 1;
    }

    // Find end of word
    let mut end = col;
    while end < chars.len() && is_word_char(chars[end]) {
        end += 1;
    }

    (start, end)
}

/// Expand a line number to the full line boundaries
///
/// Returns the column range (0, line_length) for selecting an entire line.
///
/// # Arguments
/// * `line_content` - The text content of the line
pub fn expand_to_line(line_content: &str) -> (usize, usize) {
    (0, line_content.chars().count())
}

/// Determine click type based on timing and position
///
/// Returns the click count (1 = single, 2 = double, 3 = triple).
///
/// # Arguments
/// * `now` - Current timestamp in milliseconds
/// * `last_click_time` - Timestamp of last click in milliseconds (or None)
/// * `last_click_pos` - Position of last click (col, line) (or None)
/// * `current_pos` - Position of current click (col, line)
/// * `previous_click_count` - The click count from the previous click
/// * `multi_click_threshold_ms` - Maximum time between clicks for multi-click
/// * `multi_click_distance` - Maximum distance (in cells) for multi-click
pub fn determine_click_count(
    now: u64,
    last_click_time: Option<u64>,
    last_click_pos: Option<(usize, usize)>,
    current_pos: (usize, usize),
    previous_click_count: u8,
    multi_click_threshold_ms: u64,
    multi_click_distance: usize,
) -> u8 {
    if let (Some(last_time), Some((last_col, last_line))) = (last_click_time, last_click_pos) {
        let time_ok = now.saturating_sub(last_time) < multi_click_threshold_ms;
        let col_diff = current_pos.0.abs_diff(last_col);
        let line_diff = current_pos.1.abs_diff(last_line);
        let pos_ok = col_diff <= multi_click_distance && line_diff <= multi_click_distance;

        if time_ok && pos_ok {
            // Cycle through 1, 2, 3, then back to 1
            return (previous_click_count % 3) + 1;
        }
    }

    1 // Single click
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_metrics() -> CellMetrics {
        CellMetrics::new(10.0, 20.0, 5.0)
    }

    // === CellMetrics tests ===

    #[test]
    fn test_cell_metrics_new() {
        let m = CellMetrics::new(10.0, 20.0, 5.0);
        assert_eq!(m.cell_width, 10.0);
        assert_eq!(m.cell_height, 20.0);
        assert_eq!(m.padding_left, 5.0);
        assert_eq!(m.padding_top, 5.0);
    }

    #[test]
    fn test_cell_metrics_with_padding() {
        let m = CellMetrics::with_padding(10.0, 20.0, 5.0, 15.0);
        assert_eq!(m.padding_left, 5.0);
        assert_eq!(m.padding_top, 15.0);
    }

    #[test]
    fn test_cell_metrics_default() {
        let m = CellMetrics::default();
        assert_eq!(m.cell_width, 10.0);
        assert_eq!(m.cell_height, 20.0);
    }

    // === pixel_to_cell tests ===

    #[test]
    fn test_pixel_to_cell_origin() {
        let m = test_metrics();
        // At padding boundary (5, 5)
        let p = pixel_to_cell(5.0, 5.0, &m);
        assert_eq!(p, Point::new(Line(0), Column(0)));
    }

    #[test]
    fn test_pixel_to_cell_first_cell() {
        let m = test_metrics();
        // Inside first cell
        let p = pixel_to_cell(10.0, 15.0, &m);
        assert_eq!(p, Point::new(Line(0), Column(0)));
    }

    #[test]
    fn test_pixel_to_cell_offset() {
        let m = test_metrics();
        // x=25 -> (25-5)/10 = 2, y=45 -> (45-5)/20 = 2
        let p = pixel_to_cell(25.0, 45.0, &m);
        assert_eq!(p, Point::new(Line(2), Column(2)));
    }

    #[test]
    fn test_pixel_to_cell_negative_coords() {
        let m = test_metrics();
        // Before padding should clamp to 0
        let p = pixel_to_cell(0.0, 0.0, &m);
        assert_eq!(p, Point::new(Line(0), Column(0)));
    }

    #[test]
    fn test_pixel_to_cell_clamped() {
        let m = test_metrics();
        // Way outside grid
        let p = pixel_to_cell_clamped(1000.0, 1000.0, &m, 80, 24);
        assert_eq!(p.column.0, 79);
        assert_eq!(p.line.0, 23);
    }

    // === cell_to_pixel tests ===

    #[test]
    fn test_cell_to_pixel_origin() {
        let m = test_metrics();
        let (x, y) = cell_to_pixel(Point::new(Line(0), Column(0)), &m);
        assert_eq!(x, 5.0);
        assert_eq!(y, 5.0);
    }

    #[test]
    fn test_cell_to_pixel_offset() {
        let m = test_metrics();
        let (x, y) = cell_to_pixel(Point::new(Line(1), Column(3)), &m);
        assert_eq!(x, 35.0); // 5 + 3*10
        assert_eq!(y, 25.0); // 5 + 1*20
    }

    #[test]
    fn test_cell_to_pixel_center() {
        let m = test_metrics();
        let (x, y) = cell_to_pixel_center(Point::new(Line(0), Column(0)), &m);
        assert_eq!(x, 10.0); // 5 + 10/2
        assert_eq!(y, 15.0); // 5 + 20/2
    }

    // === is_in_grid tests ===

    #[test]
    fn test_is_in_grid_inside() {
        let m = test_metrics();
        assert!(is_in_grid(10.0, 10.0, &m, 80, 24));
    }

    #[test]
    fn test_is_in_grid_at_edge() {
        let m = test_metrics();
        // At left padding edge
        assert!(is_in_grid(5.0, 10.0, &m, 80, 24));
    }

    #[test]
    fn test_is_in_grid_in_padding() {
        let m = test_metrics();
        // Before padding
        assert!(!is_in_grid(0.0, 0.0, &m, 80, 24));
        assert!(!is_in_grid(4.9, 10.0, &m, 80, 24));
    }

    #[test]
    fn test_is_in_grid_past_boundary() {
        let m = test_metrics();
        // Past right edge (5 + 80*10 = 805)
        assert!(!is_in_grid(805.0, 10.0, &m, 80, 24));
        // Past bottom edge (5 + 24*20 = 485)
        assert!(!is_in_grid(10.0, 485.0, &m, 80, 24));
    }

    // === SelectionRange tests ===

    #[test]
    fn test_selection_range_ordering_same_line() {
        let a = Point::new(Line(5), Column(10));
        let b = Point::new(Line(5), Column(5));
        let range = SelectionRange::new(a, b);
        assert_eq!(range.start, b);
        assert_eq!(range.end, a);
    }

    #[test]
    fn test_selection_range_ordering_different_lines() {
        let a = Point::new(Line(5), Column(10));
        let b = Point::new(Line(2), Column(5));
        let range = SelectionRange::new(a, b);
        assert_eq!(range.start, b);
        assert_eq!(range.end, a);
    }

    #[test]
    fn test_selection_range_ordering_already_ordered() {
        let a = Point::new(Line(1), Column(5));
        let b = Point::new(Line(3), Column(10));
        let range = SelectionRange::new(a, b);
        assert_eq!(range.start, a);
        assert_eq!(range.end, b);
    }

    #[test]
    fn test_selection_range_single() {
        let p = Point::new(Line(5), Column(10));
        let range = SelectionRange::single(p);
        assert_eq!(range.start, p);
        assert_eq!(range.end, p);
        assert!(range.is_empty());
    }

    #[test]
    fn test_selection_range_contains() {
        let range = SelectionRange::new(
            Point::new(Line(1), Column(5)),
            Point::new(Line(3), Column(10)),
        );

        // Inside
        assert!(range.contains(Point::new(Line(2), Column(0))));
        assert!(range.contains(Point::new(Line(1), Column(5))));
        assert!(range.contains(Point::new(Line(3), Column(10))));

        // Outside
        assert!(!range.contains(Point::new(Line(0), Column(5))));
        assert!(!range.contains(Point::new(Line(1), Column(4))));
        assert!(!range.contains(Point::new(Line(3), Column(11))));
        assert!(!range.contains(Point::new(Line(4), Column(0))));
    }

    #[test]
    fn test_selection_range_is_multiline() {
        let single_line = SelectionRange::new(
            Point::new(Line(1), Column(0)),
            Point::new(Line(1), Column(10)),
        );
        assert!(!single_line.is_multiline());

        let multi_line = SelectionRange::new(
            Point::new(Line(1), Column(5)),
            Point::new(Line(3), Column(10)),
        );
        assert!(multi_line.is_multiline());
    }

    #[test]
    fn test_selection_range_line_count() {
        let single = SelectionRange::new(
            Point::new(Line(5), Column(0)),
            Point::new(Line(5), Column(10)),
        );
        assert_eq!(single.line_count(), 1);

        let multi = SelectionRange::new(
            Point::new(Line(2), Column(0)),
            Point::new(Line(5), Column(10)),
        );
        assert_eq!(multi.line_count(), 4);
    }

    // === expand_to_word tests ===

    #[test]
    fn test_expand_to_word_middle() {
        let line = "hello world test";
        // 'o' in "world" at position 7
        assert_eq!(expand_to_word(line, 7), (6, 11));
    }

    #[test]
    fn test_expand_to_word_start() {
        let line = "hello world test";
        assert_eq!(expand_to_word(line, 0), (0, 5)); // "hello"
    }

    #[test]
    fn test_expand_to_word_end() {
        let line = "hello world test";
        assert_eq!(expand_to_word(line, 14), (12, 16)); // "test"
    }

    #[test]
    fn test_expand_to_word_with_underscore() {
        let line = "foo_bar_baz";
        assert_eq!(expand_to_word(line, 5), (0, 11));
    }

    #[test]
    fn test_expand_to_word_on_space() {
        let line = "hello world";
        // Space at position 5
        assert_eq!(expand_to_word(line, 5), (5, 6));
    }

    #[test]
    fn test_expand_to_word_past_end() {
        let line = "hello";
        assert_eq!(expand_to_word(line, 10), (10, 10));
    }

    #[test]
    fn test_expand_to_word_with_numbers() {
        let line = "test123word";
        assert_eq!(expand_to_word(line, 5), (0, 11));
    }

    // === expand_to_line tests ===

    #[test]
    fn test_expand_to_line() {
        let line = "hello world";
        assert_eq!(expand_to_line(line), (0, 11));
    }

    #[test]
    fn test_expand_to_line_empty() {
        assert_eq!(expand_to_line(""), (0, 0));
    }

    #[test]
    fn test_expand_to_line_unicode() {
        let line = "hello world";
        assert_eq!(expand_to_line(line), (0, line.chars().count()));
    }

    // === determine_click_count tests ===

    #[test]
    fn test_determine_click_count_single() {
        let count = determine_click_count(1000, None, None, (5, 5), 0, 400, 1);
        assert_eq!(count, 1);
    }

    #[test]
    fn test_determine_click_count_double() {
        let count = determine_click_count(
            1200, // 200ms after last click
            Some(1000),
            Some((5, 5)),
            (5, 5),
            1,   // previous was single
            400, // threshold
            1,   // distance
        );
        assert_eq!(count, 2);
    }

    #[test]
    fn test_determine_click_count_triple() {
        let count = determine_click_count(
            1200,
            Some(1000),
            Some((5, 5)),
            (5, 5),
            2, // previous was double
            400,
            1,
        );
        assert_eq!(count, 3);
    }

    #[test]
    fn test_determine_click_count_wraps_to_single() {
        let count = determine_click_count(
            1200,
            Some(1000),
            Some((5, 5)),
            (5, 5),
            3, // previous was triple
            400,
            1,
        );
        assert_eq!(count, 1); // wraps back
    }

    #[test]
    fn test_determine_click_count_too_slow() {
        let count = determine_click_count(
            2000, // 1000ms after last click
            Some(1000),
            Some((5, 5)),
            (5, 5),
            1,
            400, // only 400ms threshold
            1,
        );
        assert_eq!(count, 1); // resets due to time
    }

    #[test]
    fn test_determine_click_count_too_far() {
        let count = determine_click_count(
            1200,
            Some(1000),
            Some((5, 5)),
            (10, 10), // 5 cells away
            1,
            400,
            1, // only 1 cell threshold
        );
        assert_eq!(count, 1); // resets due to distance
    }
}
