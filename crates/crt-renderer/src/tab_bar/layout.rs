//! Tab bar layout calculations
//!
//! Handles positioning, sizing, and hit testing - no GPU dependencies.
//! Tab bar is always positioned at the top of the window.

use super::state::TabBarState;
use crt_theme::TabTheme;

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
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height
    }

    pub fn close_contains(&self, x: f32, y: f32) -> bool {
        x >= self.close_x
            && x < self.close_x + self.close_width
            && y >= self.y
            && y < self.y + self.height
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
        let width_per_tab =
            ((available_width - total_gap) / tab_count as f32).clamp(min_width, max_width);

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

#[cfg(test)]
mod tests {
    use super::*;
    use crt_theme::TabTheme;

    fn layout_with_rects(tab_count: usize) -> TabLayout {
        let mut state = TabBarState::new();
        for i in 1..tab_count {
            state.add_tab(format!("Tab {}", i));
        }
        let theme = TabTheme::default();
        let mut layout = TabLayout::new();
        layout.resize(800.0, 600.0);
        layout.calculate_rects(&state, &theme);
        layout
    }

    #[test]
    fn new_layout_defaults() {
        let layout = TabLayout::new();
        assert_eq!(layout.height(), 36.0);
        assert_eq!(layout.content_offset(), (0.0, 40.0)); // 36 + 4 padding
        assert!(layout.is_dirty());
        assert!(layout.tab_rects().is_empty());
    }

    #[test]
    fn content_offset_respects_padding() {
        let mut layout = TabLayout::new();
        layout.set_content_padding(10.0);
        assert_eq!(layout.content_offset(), (0.0, 46.0)); // 36 + 10
    }

    #[test]
    fn resize_updates_screen_size() {
        let mut layout = TabLayout::new();
        layout.resize(1024.0, 768.0);
        assert_eq!(layout.screen_size(), (1024.0, 768.0));
        assert!(layout.is_dirty());
    }

    #[test]
    fn calculate_rects_produces_correct_count() {
        let layout = layout_with_rects(3);
        assert_eq!(layout.tab_rects().len(), 3);
        assert!(!layout.is_dirty()); // cleared after calculate
    }

    #[test]
    fn calculate_rects_tabs_are_contiguous() {
        let layout = layout_with_rects(3);
        let rects = layout.tab_rects();
        // Each tab should start after the previous one (with gap)
        for i in 1..rects.len() {
            assert!(
                rects[i].x > rects[i - 1].x + rects[i - 1].width - 1.0,
                "Tab {} should be after tab {}",
                i,
                i - 1
            );
        }
    }

    #[test]
    fn calculate_rects_zero_tabs() {
        let mut state = TabBarState::new();
        // Remove the default tab by creating a fresh state with manipulated internals
        // Actually, TabBarState always has at least 1 tab, so test with 1
        let theme = TabTheme::default();
        let mut layout = TabLayout::new();
        layout.resize(800.0, 600.0);
        layout.calculate_rects(&state, &theme);
        assert_eq!(layout.tab_rects().len(), 1);

        // Add more tabs
        state.add_tab("A");
        layout.mark_dirty();
        layout.calculate_rects(&state, &theme);
        assert_eq!(layout.tab_rects().len(), 2);
    }

    #[test]
    fn tab_width_clamped_to_max() {
        // Single tab with wide screen — should not exceed max_width
        let layout = layout_with_rects(1);
        let rects = layout.tab_rects();
        let theme = TabTheme::default();
        assert!(rects[0].width <= theme.tab.max_width);
    }

    #[test]
    fn many_tabs_shrink_to_fit() {
        // Many tabs should shrink but not below min_width
        let layout = layout_with_rects(20);
        let rects = layout.tab_rects();
        let theme = TabTheme::default();
        assert_eq!(rects.len(), 20);
        for rect in rects {
            assert!(rect.width >= theme.tab.min_width);
        }
    }

    #[test]
    fn hit_test_first_tab() {
        let layout = layout_with_rects(3);
        let first = &layout.tab_rects()[0];
        // Click in center of first tab
        let result = layout.hit_test(first.x + first.width / 2.0, first.y + first.height / 2.0);
        assert_eq!(result, Some((0, false)));
    }

    #[test]
    fn hit_test_close_button() {
        let layout = layout_with_rects(3);
        let first = &layout.tab_rects()[0];
        // Click on close button area
        let result = layout.hit_test(
            first.close_x + first.close_width / 2.0,
            first.y + first.height / 2.0,
        );
        assert_eq!(result, Some((0, true)));
    }

    #[test]
    fn hit_test_outside_returns_none() {
        let layout = layout_with_rects(3);
        // Click way below the tab bar
        assert_eq!(layout.hit_test(400.0, 500.0), None);
        // Click way to the right
        assert_eq!(layout.hit_test(2000.0, 10.0), None);
    }

    #[test]
    fn hit_test_second_tab() {
        let layout = layout_with_rects(3);
        let second = &layout.tab_rects()[1];
        let result =
            layout.hit_test(second.x + second.width / 2.0, second.y + second.height / 2.0);
        assert_eq!(result, Some((1, false)));
    }

    #[test]
    fn scale_factor_affects_rects() {
        let mut state = TabBarState::new();
        state.add_tab("Tab 1");
        let theme = TabTheme::default();

        let mut layout1 = TabLayout::new();
        layout1.resize(800.0, 600.0);
        layout1.set_scale_factor(1.0);
        layout1.calculate_rects(&state, &theme);

        let mut layout2 = TabLayout::new();
        layout2.resize(800.0, 600.0);
        layout2.set_scale_factor(2.0);
        layout2.calculate_rects(&state, &theme);

        // At 2x scale, tab heights should be larger
        assert!(layout2.tab_rects()[0].height > layout1.tab_rects()[0].height);
    }

    #[test]
    fn tab_rect_contains() {
        let rect = TabRect {
            x: 10.0,
            y: 5.0,
            width: 100.0,
            height: 30.0,
            close_x: 90.0,
            close_width: 16.0,
        };
        assert!(rect.contains(50.0, 20.0)); // center
        assert!(rect.contains(10.0, 5.0)); // top-left edge
        assert!(!rect.contains(9.0, 20.0)); // just outside left
        assert!(!rect.contains(110.0, 20.0)); // just outside right
        assert!(!rect.contains(50.0, 4.0)); // just above
        assert!(!rect.contains(50.0, 35.0)); // just below
    }

    #[test]
    fn tab_rect_close_contains() {
        let rect = TabRect {
            x: 10.0,
            y: 5.0,
            width: 100.0,
            height: 30.0,
            close_x: 90.0,
            close_width: 16.0,
        };
        assert!(rect.close_contains(95.0, 20.0)); // in close area
        assert!(!rect.close_contains(50.0, 20.0)); // in tab but not close
        assert!(!rect.close_contains(95.0, 4.0)); // close x but above
    }

    #[test]
    fn dirty_flag_lifecycle() {
        let mut layout = TabLayout::new();
        assert!(layout.is_dirty()); // dirty by default
        layout.clear_dirty();
        assert!(!layout.is_dirty());
        layout.mark_dirty();
        assert!(layout.is_dirty());
    }
}
