//! Tab bar layout calculations
//!
//! Handles positioning, sizing, and hit testing - no GPU dependencies.

use crt_theme::TabTheme;
use super::state::TabBarState;

/// Tab bar position on the screen
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TabPosition {
    #[default]
    Top,
    Bottom,
    Left,
    Right,
}

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
pub struct TabLayout {
    pub(crate) position: TabPosition,
    pub(crate) tab_rects: Vec<TabRect>,
    pub(crate) bar_height: f32,
    pub(crate) bar_width: f32,
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
            position: TabPosition::Top,
            tab_rects: Vec::new(),
            bar_height: 36.0,
            bar_width: 180.0,
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

    /// Set tab bar position
    pub fn set_position(&mut self, position: TabPosition) {
        self.position = position;
        self.dirty = true;
    }

    /// Get current tab bar position
    pub fn position(&self) -> TabPosition {
        self.position
    }

    /// Check if tab bar is horizontal (top/bottom)
    pub fn is_horizontal(&self) -> bool {
        matches!(self.position, TabPosition::Top | TabPosition::Bottom)
    }

    /// Get current tab bar height (in logical pixels) - for top/bottom positioning
    pub fn height(&self) -> f32 {
        self.bar_height
    }

    /// Get current tab bar width (in logical pixels) - for left/right positioning
    pub fn width(&self) -> f32 {
        self.bar_width
    }

    /// Get the dimension that affects content layout (height for top/bottom, width for left/right)
    pub fn size(&self) -> f32 {
        if self.is_horizontal() {
            self.bar_height
        } else {
            self.bar_width
        }
    }

    /// Get the content offset (x, y) in logical pixels based on tab bar position
    pub fn content_offset(&self) -> (f32, f32) {
        match self.position {
            TabPosition::Top => (0.0, self.bar_height),
            TabPosition::Bottom => (0.0, 0.0),
            TabPosition::Left => (self.bar_width, 0.0),
            TabPosition::Right => (0.0, 0.0),
        }
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
    pub fn calculate_rects(&mut self, state: &TabBarState, theme: &TabTheme) {
        self.tab_rects.clear();

        let s = self.scale_factor;
        let padding = theme.bar.padding * s;
        let tab_padding_x = theme.tab.padding_x * s;
        let bar_height = self.bar_height * s;
        let bar_width = self.bar_width * s;
        let tab_gap = 4.0 * s;
        let tab_count = state.tab_count();

        match self.position {
            TabPosition::Top => {
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
            }

            TabPosition::Bottom => {
                let tab_height = bar_height - padding * 2.0;
                let bar_y = self.screen_height - bar_height;
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
                        y: bar_y + padding,
                        width: width_per_tab,
                        height: tab_height,
                        close_x: x + width_per_tab - close_width - tab_padding_x,
                        close_width,
                    });
                    x += width_per_tab + tab_gap;
                }
            }

            TabPosition::Left => {
                let tab_width = bar_width - padding * 2.0;
                let available_height = self.screen_height - padding * 2.0;
                let total_gap = tab_gap * (tab_count.saturating_sub(1)) as f32;
                let min_height = 24.0 * s;
                let max_height = 32.0 * s;
                let height_per_tab = ((available_height - total_gap) / tab_count as f32)
                    .clamp(min_height, max_height);

                let mut y = padding;
                for _ in 0..tab_count {
                    let close_width = theme.close.size * s;
                    self.tab_rects.push(TabRect {
                        x: padding,
                        y,
                        width: tab_width,
                        height: height_per_tab,
                        close_x: padding + tab_width - close_width - tab_padding_x,
                        close_width,
                    });
                    y += height_per_tab + tab_gap;
                }
            }

            TabPosition::Right => {
                let tab_width = bar_width - padding * 2.0;
                let bar_x = self.screen_width - bar_width;
                let available_height = self.screen_height - padding * 2.0;
                let total_gap = tab_gap * (tab_count.saturating_sub(1)) as f32;
                let min_height = 24.0 * s;
                let max_height = 32.0 * s;
                let height_per_tab = ((available_height - total_gap) / tab_count as f32)
                    .clamp(min_height, max_height);

                let mut y = padding;
                for _ in 0..tab_count {
                    let close_width = theme.close.size * s;
                    self.tab_rects.push(TabRect {
                        x: bar_x + padding,
                        y,
                        width: tab_width,
                        height: height_per_tab,
                        close_x: bar_x + padding + tab_width - close_width - tab_padding_x,
                        close_width,
                    });
                    y += height_per_tab + tab_gap;
                }
            }
        }

        self.dirty = false;
    }
}
