//! Selection rendering
//!
//! Renders terminal text selection highlights.

use crate::window::WindowState;
use crt_core::SelectionRange;

/// Render selection rectangles via RectRenderer (direct, no intermediate texture)
/// Selection coordinates are in grid space (negative = scrollback history, 0+ = visible screen)
/// display_offset converts grid coordinates to viewport coordinates for rendering
pub fn render_selection_rects(
    state: &mut WindowState,
    selection: &SelectionRange,
    display_offset: i32,
) {
    let cell_width = state.gpu.glyph_cache.cell_width();
    let line_height = state.gpu.glyph_cache.line_height();
    let (offset_x, offset_y) = state.gpu.tab_bar.content_offset();
    let padding = 10.0 * state.scale_factor;
    let screen_lines = state.rows as i32;

    // Selection highlight color (semi-transparent blue)
    let selection_color = [0.3, 0.4, 0.6, 0.5];

    // Selection coordinates are in grid space, convert to viewport space for rendering
    // viewport_line = grid_line + display_offset
    let start_grid_line = selection.start.line.0;
    let end_grid_line = selection.end.line.0;
    let start_col = selection.start.column.0;
    let end_col = selection.end.column.0;

    // Convert grid lines to viewport lines
    let start_viewport_line = start_grid_line + display_offset;
    let end_viewport_line = end_grid_line + display_offset;

    // Clamp to visible viewport (0 to screen_lines-1)
    let visible_start = start_viewport_line.max(0);
    let visible_end = end_viewport_line.min(screen_lines - 1);

    // If selection is entirely outside visible area, skip rendering
    if visible_start > screen_lines - 1 || visible_end < 0 {
        return;
    }

    if selection.is_block {
        // Block selection: rectangle from start to end
        let min_col = start_col.min(end_col);
        let max_col = start_col.max(end_col);

        for viewport_line in visible_start..=visible_end {
            let y = offset_y + padding + (viewport_line as f32 * line_height);
            let x = offset_x + padding + (min_col as f32 * cell_width);
            let num_cells = max_col - min_col + 1;
            let width = num_cells as f32 * cell_width;
            state
                .gpu
                .overlay_rect_renderer
                .push_rect(x, y, width, line_height, selection_color);
        }
    } else {
        // Normal selection: spans from start point to end point
        for viewport_line in visible_start..=visible_end {
            let y = offset_y + padding + (viewport_line as f32 * line_height);

            // Convert back to grid line to compare with selection boundaries
            let grid_line = viewport_line - display_offset;

            let (line_start_col, line_end_col) = if start_grid_line == end_grid_line {
                // Single line selection - normalize columns for right-to-left selection
                (start_col.min(end_col), start_col.max(end_col))
            } else if grid_line == start_grid_line {
                // First line: from start column to end of line
                (start_col, 999)
            } else if grid_line == end_grid_line {
                // Last line: from start of line to end column
                (0, end_col)
            } else {
                // Middle line: full line
                (0, 999)
            };

            let x = offset_x + padding + (line_start_col as f32 * cell_width);
            let num_cells = (line_end_col - line_start_col + 1).min(500);
            let width = num_cells as f32 * cell_width;
            state
                .gpu
                .overlay_rect_renderer
                .push_rect(x, y, width, line_height, selection_color);
        }
    }
}
