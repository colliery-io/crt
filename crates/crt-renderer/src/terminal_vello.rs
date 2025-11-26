//! Terminal vello renderer for cursor and selection shapes
//!
//! Renders terminal UI elements (cursor, selection rectangles) via vello.
//! These elements can have glow applied via the CSS text-shadow property.

use vello::{peniko, kurbo, Scene};

/// Cursor shape style
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorShape {
    /// Block cursor (full cell)
    Block,
    /// Vertical bar cursor
    Bar,
    /// Underline cursor
    Underline,
}

impl Default for CursorShape {
    fn default() -> Self {
        Self::Block
    }
}

/// Terminal vello renderer state
pub struct TerminalVelloRenderer {
    cursor_shape: CursorShape,
    cursor_color: [f32; 4],
    selection_color: [f32; 4],
}

impl Default for TerminalVelloRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl TerminalVelloRenderer {
    pub fn new() -> Self {
        Self {
            cursor_shape: CursorShape::Block,
            cursor_color: [0.8, 0.8, 0.2, 0.9],
            selection_color: [0.3, 0.4, 0.6, 0.5],
        }
    }

    /// Set the cursor shape
    pub fn set_cursor_shape(&mut self, shape: CursorShape) {
        self.cursor_shape = shape;
    }

    /// Set the cursor color
    pub fn set_cursor_color(&mut self, color: [f32; 4]) {
        self.cursor_color = color;
    }

    /// Set the selection highlight color
    pub fn set_selection_color(&mut self, color: [f32; 4]) {
        self.selection_color = color;
    }

    /// Get the current cursor color
    pub fn cursor_color(&self) -> [f32; 4] {
        self.cursor_color
    }

    /// Render cursor at the given position
    ///
    /// # Arguments
    /// * `scene` - Vello scene to draw into
    /// * `x` - X position in pixels
    /// * `y` - Y position in pixels
    /// * `cell_width` - Width of a cell in pixels
    /// * `cell_height` - Height of a cell in pixels
    pub fn render_cursor(
        &self,
        scene: &mut Scene,
        x: f32,
        y: f32,
        cell_width: f32,
        cell_height: f32,
    ) {
        let brush = peniko::Brush::Solid(color_from_f32(
            self.cursor_color[0],
            self.cursor_color[1],
            self.cursor_color[2],
            self.cursor_color[3],
        ));

        let rect = match self.cursor_shape {
            CursorShape::Block => {
                kurbo::Rect::new(
                    x as f64,
                    y as f64,
                    (x + cell_width) as f64,
                    (y + cell_height) as f64,
                )
            }
            CursorShape::Bar => {
                // 2-pixel wide bar on the left
                kurbo::Rect::new(
                    x as f64,
                    y as f64,
                    (x + 2.0) as f64,
                    (y + cell_height) as f64,
                )
            }
            CursorShape::Underline => {
                // 2-pixel tall underline at the bottom
                kurbo::Rect::new(
                    x as f64,
                    (y + cell_height - 2.0) as f64,
                    (x + cell_width) as f64,
                    (y + cell_height) as f64,
                )
            }
        };

        scene.fill(
            peniko::Fill::NonZero,
            kurbo::Affine::IDENTITY,
            &brush,
            None,
            &rect,
        );
    }

    /// Render a selection rectangle for a single cell
    pub fn render_selection_cell(
        &self,
        scene: &mut Scene,
        x: f32,
        y: f32,
        cell_width: f32,
        cell_height: f32,
    ) {
        let rect = kurbo::Rect::new(
            x as f64,
            y as f64,
            (x + cell_width) as f64,
            (y + cell_height) as f64,
        );
        let brush = peniko::Brush::Solid(color_from_f32(
            self.selection_color[0],
            self.selection_color[1],
            self.selection_color[2],
            self.selection_color[3],
        ));

        scene.fill(
            peniko::Fill::NonZero,
            kurbo::Affine::IDENTITY,
            &brush,
            None,
            &rect,
        );
    }

    /// Render a selection spanning multiple cells
    ///
    /// # Arguments
    /// * `scene` - Vello scene to draw into
    /// * `start_x` - Start X position in pixels
    /// * `y` - Y position in pixels
    /// * `num_cells` - Number of cells to highlight
    /// * `cell_width` - Width of a cell in pixels
    /// * `cell_height` - Height of a cell in pixels
    pub fn render_selection_row(
        &self,
        scene: &mut Scene,
        start_x: f32,
        y: f32,
        num_cells: usize,
        cell_width: f32,
        cell_height: f32,
    ) {
        if num_cells == 0 {
            return;
        }

        let rect = kurbo::Rect::new(
            start_x as f64,
            y as f64,
            (start_x + cell_width * num_cells as f32) as f64,
            (y + cell_height) as f64,
        );
        let brush = peniko::Brush::Solid(color_from_f32(
            self.selection_color[0],
            self.selection_color[1],
            self.selection_color[2],
            self.selection_color[3],
        ));

        scene.fill(
            peniko::Fill::NonZero,
            kurbo::Affine::IDENTITY,
            &brush,
            None,
            &rect,
        );
    }
}

/// Helper to create a peniko Color from f32 RGBA components (0.0-1.0)
fn color_from_f32(r: f32, g: f32, b: f32, a: f32) -> peniko::Color {
    peniko::Color::from_rgba8(
        (r * 255.0) as u8,
        (g * 255.0) as u8,
        (b * 255.0) as u8,
        (a * 255.0) as u8,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cursor_shapes() {
        let mut renderer = TerminalVelloRenderer::new();
        assert_eq!(renderer.cursor_shape, CursorShape::Block);

        renderer.set_cursor_shape(CursorShape::Bar);
        assert_eq!(renderer.cursor_shape, CursorShape::Bar);

        renderer.set_cursor_shape(CursorShape::Underline);
        assert_eq!(renderer.cursor_shape, CursorShape::Underline);
    }

    #[test]
    fn test_cursor_color() {
        let mut renderer = TerminalVelloRenderer::new();
        let new_color = [1.0, 0.0, 0.0, 1.0];
        renderer.set_cursor_color(new_color);
        assert_eq!(renderer.cursor_color(), new_color);
    }
}
