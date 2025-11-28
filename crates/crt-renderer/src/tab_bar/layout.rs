//! Tab bar layout calculations
//!
//! Handles positioning, sizing, and hit testing - no GPU dependencies.
//! Tab bar is always positioned at the top of the window.

use crt_theme::TabTheme;
use super::state::TabBarState;

/// Rectangle for a tab (for hit testing and rendering)
#[derive(Debug, Clone, Copy, Default)]
pub struct TabRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub close_x: f32,
    pub close_width: f32,
}

impl TabRect {
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.x + self.width &&
        y >= self.y && y < self.y + self.height
    }

    pub fn close_contains(&self, x: f32, y: f32) -> bool {
        x >= self.close_x && x < self.close_x + self.close_width &&
        y >= self.y && y < self.y + self.height
    }
}

/// Tab bar layout - manages dimensions and positioning
/// Tab bar is always at the top of the window.
pub struct TabLayout {
    pub(crate) tab_rects: Vec<TabRect>,
    pub(crate) bar_height: f32,
    pub(crate) content_padding: f32,
    pub(crate) screen_width: f32,
    pub(crate) screen_height: f32,
    pub(crate) scale_factor: f32,
    pub(crate) dirty: bool,
}

impl Default for TabLayout {
    fn default() -> Self {
        Self::new()
    }
}

impl TabLayout {
    pub fn new() -> Self {
        Self {
            tab_rects: Vec::new(),
            bar_height: 36.0,
            content_padding: 4.0,
            screen_width: 800.0,
            screen_height: 600.0,
            scale_factor: 1.0,
            dirty: true,
        }
    }

    /// Set scale factor for HiDPI displays
    pub fn set_scale_factor(&mut self, scale_factor: f32) {
        self.scale_factor = scale_factor;
        self.dirty = true;
    }

    /// Get scale factor
    pub fn scale_factor(&self) -> f32 {
        self.scale_factor
    }

    /// Get current tab bar height (in logical pixels)
    pub fn height(&self) -> f32 {
        self.bar_height
    }

    /// Get the content offset (x, y) in logical pixels
    /// Content starts below the tab bar plus content padding
    pub fn content_offset(&self) -> (f32, f32) {
        (0.0, self.bar_height + self.content_padding)
    }

    /// Set content padding from theme
    pub fn set_content_padding(&mut self, padding: f32) {
        self.content_padding = padding;
        self.dirty = true;
    }

    /// Update screen size (in physical pixels)
    pub fn resize(&mut self, width: f32, height: f32) {
        self.screen_width = width;
        self.screen_height = height;
        self.dirty = true;
    }

    /// Get screen dimensions
    pub fn screen_size(&self) -> (f32, f32) {
        (self.screen_width, self.screen_height)
    }

    /// Set bar height from theme
    pub fn set_bar_height(&mut self, height: f32) {
        self.bar_height = height;
        self.dirty = true;
    }

    /// Mark layout as dirty (needs rebuild)
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Check if layout needs rebuild
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Clear dirty flag
    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }

    /// Get tab rectangles
    pub fn tab_rects(&self) -> &[TabRect] {
        &self.tab_rects
    }

    /// Hit test - returns (tab_index, is_close_button) if hit
    pub fn hit_test(&self, x: f32, y: f32) -> Option<(usize, bool)> {
        for (i, rect) in self.tab_rects.iter().enumerate() {
            if rect.contains(x, y) {
                let is_close = rect.close_contains(x, y);
                return Some((i, is_close));
            }
        }
        None
    }

    /// Calculate tab rectangles based on current state and theme
    /// Tab bar is always at the top
    pub fn calculate_rects(&mut self, state: &TabBarState, theme: &TabTheme) {
        self.tab_rects.clear();

        let s = self.scale_factor;
        let padding = theme.bar.padding * s;
        let tab_padding_x = theme.tab.padding_x * s;
        let bar_height = self.bar_height * s;
        let tab_gap = 4.0 * s;
        let tab_count = state.tab_count();

        if tab_count == 0 {
            self.dirty = false;
            return;
        }

        let tab_height = bar_height - padding * 2.0;
        let available_width = self.screen_width - padding * 2.0;
        let total_gap = tab_gap * (tab_count.saturating_sub(1)) as f32;
        let min_width = theme.tab.min_width * s;
        let max_width = theme.tab.max_width * s;
        let width_per_tab = ((available_width - total_gap) / tab_count as f32)
            .clamp(min_width, max_width);

        let mut x = padding;
        for _ in 0..tab_count {
            let close_width = theme.close.size * s;
            self.tab_rects.push(TabRect {
                x,
                y: padding,
                width: width_per_tab,
                height: tab_height,
                close_x: x + width_per_tab - close_width - tab_padding_x,
                close_width,
            });
            x += width_per_tab + tab_gap;
        }

        self.dirty = false;
    }
}
