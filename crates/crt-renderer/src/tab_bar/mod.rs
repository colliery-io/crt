//! Tab Bar Rendering
//!
//! GPU-accelerated tab bar with theme support, separated into:
//! - state: Tab data and management (no GPU)
//! - layout: Positioning and hit testing (no GPU)
//! - vello_renderer: Shape rendering via vello (backgrounds, borders)
//!
//! Text and glow effects are rendered separately via the text pipeline,
//! triggered by CSS properties like `text-shadow`.

mod layout;
mod state;
mod vello_renderer;

pub use layout::{TabLayout, TabRect};
pub use state::{EditState, Tab, TabBarState};
pub use vello_renderer::VelloTabBarRenderer;

use crt_theme::TabTheme;

/// Tab bar facade - combines state, layout, and rendering
///
/// Rendering architecture:
/// - Vello: Renders all shapes (backgrounds, borders, rounded corners)
/// - Shader pipeline: Renders text with effects based on CSS properties
///   (e.g., text-shadow triggers glow shader pass)
pub struct TabBar {
    state: TabBarState,
    layout: TabLayout,
    vello_renderer: VelloTabBarRenderer,
    theme: TabTheme,
}

impl TabBar {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        Self {
            state: TabBarState::new(),
            layout: TabLayout::new(),
            vello_renderer: VelloTabBarRenderer::new(device, format),
            theme: TabTheme::default(),
        }
    }

    // ---- Theme ----

    /// Set the tab theme
    pub fn set_theme(&mut self, theme: TabTheme) {
        self.layout.set_bar_height(theme.bar.height);
        self.layout.set_content_padding(theme.bar.content_padding);
        self.theme = theme;
        self.layout.mark_dirty();
    }

    // ---- Layout delegation ----

    /// Set scale factor for HiDPI displays
    pub fn set_scale_factor(&mut self, scale_factor: f32) {
        self.layout.set_scale_factor(scale_factor);
    }

    /// Get current tab bar height (in logical pixels)
    pub fn height(&self) -> f32 {
        self.layout.height()
    }

    /// Get the content offset (x, y) in logical pixels
    /// Content starts below the tab bar
    pub fn content_offset(&self) -> (f32, f32) {
        self.layout.content_offset()
    }

    /// Update screen size (in physical pixels)
    pub fn resize(&mut self, width: f32, height: f32) {
        self.layout.resize(width, height);
    }

    // ---- State delegation ----

    /// Add a new tab
    pub fn add_tab(&mut self, title: impl Into<String>) -> u64 {
        let id = self.state.add_tab(title);
        self.layout.mark_dirty();
        id
    }

    /// Close a tab by ID
    pub fn close_tab(&mut self, id: u64) -> bool {
        let result = self.state.close_tab(id);
        if result {
            self.layout.mark_dirty();
        }
        result
    }

    /// Select a tab by ID
    pub fn select_tab(&mut self, id: u64) -> bool {
        let result = self.state.select_tab(id);
        if result {
            self.layout.mark_dirty();
        }
        result
    }

    /// Select tab by index (0-based)
    pub fn select_tab_index(&mut self, index: usize) -> bool {
        let result = self.state.select_tab_index(index);
        if result {
            self.layout.mark_dirty();
        }
        result
    }

    /// Select next tab
    pub fn next_tab(&mut self) {
        self.state.next_tab();
        self.layout.mark_dirty();
    }

    /// Select previous tab
    pub fn prev_tab(&mut self) {
        self.state.prev_tab();
        self.layout.mark_dirty();
    }

    /// Get active tab ID
    pub fn active_tab_id(&self) -> Option<u64> {
        self.state.active_tab_id()
    }

    /// Get active tab rectangle (for focus indicator rendering)
    /// Returns (x, y, width, height) in physical pixels
    pub fn active_tab_rect(&self) -> Option<(f32, f32, f32, f32)> {
        let active_idx = self.state.active_tab_index();
        self.layout
            .tab_rects()
            .get(active_idx)
            .map(|r| (r.x, r.y, r.width, r.height))
    }

    /// Get number of tabs
    pub fn tab_count(&self) -> usize {
        self.state.tab_count()
    }

    /// Hit test - returns (tab_id, is_close_button) if hit
    pub fn hit_test(&self, x: f32, y: f32) -> Option<(u64, bool)> {
        let tabs = self.state.tabs();
        self.layout
            .hit_test(x, y)
            .map(|(idx, is_close)| (tabs[idx].id, is_close))
    }

    /// Update a tab's title by ID (from OSC escape sequences)
    pub fn set_tab_title(&mut self, id: u64, title: impl Into<String>) -> bool {
        let result = self.state.set_tab_title(id, title);
        if result {
            self.layout.mark_dirty();
        }
        result
    }

    /// Set a custom title for a tab (user-initiated)
    pub fn set_custom_tab_title(&mut self, id: u64, title: impl Into<String>) -> bool {
        let result = self.state.set_custom_tab_title(id, title);
        if result {
            self.layout.mark_dirty();
        }
        result
    }

    /// Clear custom title flag
    pub fn clear_custom_title(&mut self, id: u64) {
        self.state.clear_custom_title(id);
    }

    /// Check if a tab has a custom title
    pub fn has_custom_title(&self, id: u64) -> bool {
        self.state.has_custom_title(id)
    }

    /// Get a tab's title by ID
    pub fn get_tab_title(&self, id: u64) -> Option<&str> {
        self.state.get_tab_title(id)
    }

    // ---- Inline Editing ----

    /// Check if currently editing a tab
    pub fn is_editing(&self) -> bool {
        self.state.is_editing()
    }

    /// Get the tab ID being edited (if any)
    pub fn editing_tab_id(&self) -> Option<u64> {
        self.state.editing_tab_id()
    }

    /// Start editing a tab's title
    pub fn start_editing(&mut self, id: u64) -> bool {
        let result = self.state.start_editing(id);
        if result {
            self.layout.mark_dirty();
        }
        result
    }

    /// Cancel editing without saving
    pub fn cancel_editing(&mut self) {
        self.state.cancel_editing();
        self.layout.mark_dirty();
    }

    /// Confirm editing and save the new title
    pub fn confirm_editing(&mut self) -> bool {
        let result = self.state.confirm_editing();
        self.layout.mark_dirty();
        result
    }

    /// Handle a character input during editing
    pub fn edit_insert_char(&mut self, c: char) {
        self.state.edit_insert_char(c);
        self.layout.mark_dirty();
    }

    /// Handle backspace during editing
    pub fn edit_backspace(&mut self) {
        self.state.edit_backspace();
        self.layout.mark_dirty();
    }

    /// Handle delete during editing
    pub fn edit_delete(&mut self) {
        self.state.edit_delete();
        self.layout.mark_dirty();
    }

    /// Move cursor left during editing
    pub fn edit_cursor_left(&mut self) {
        self.state.edit_cursor_left();
        self.layout.mark_dirty();
    }

    /// Move cursor right during editing
    pub fn edit_cursor_right(&mut self) {
        self.state.edit_cursor_right();
        self.layout.mark_dirty();
    }

    /// Move cursor to start during editing
    pub fn edit_cursor_home(&mut self) {
        self.state.edit_cursor_home();
        self.layout.mark_dirty();
    }

    /// Move cursor to end during editing
    pub fn edit_cursor_end(&mut self) {
        self.state.edit_cursor_end();
        self.layout.mark_dirty();
    }

    // ---- Theme colors ----

    /// Get the foreground color for inactive tabs
    pub fn inactive_tab_color(&self) -> [f32; 4] {
        color_to_array(&self.theme.tab.foreground)
    }

    /// Get the foreground color for active tabs
    pub fn active_tab_color(&self) -> [f32; 4] {
        color_to_array(&self.theme.active.foreground)
    }

    /// Get text shadow for inactive tabs (if any)
    pub fn inactive_tab_text_shadow(&self) -> Option<(f32, [f32; 4])> {
        self.theme
            .tab
            .text_shadow
            .map(|s| (s.radius, color_to_array(&s.color)))
    }

    /// Get text shadow for active tabs (if any)
    pub fn active_tab_text_shadow(&self) -> Option<(f32, [f32; 4])> {
        self.theme
            .active
            .text_shadow
            .map(|s| (s.radius, color_to_array(&s.color)))
    }

    // ---- Rendering ----

    /// Get tab titles for text rendering (returns position and title in physical pixels)
    pub fn get_tab_labels(&self) -> Vec<(f32, f32, String, bool, bool)> {
        let s = self.layout.scale_factor();
        let tab_padding_x = self.theme.tab.padding_x * s;
        let font_height = 14.0 * s;
        let edit_state = self.state.edit_state();

        self.layout
            .tab_rects()
            .iter()
            .zip(self.state.tabs().iter())
            .enumerate()
            .map(|(i, (rect, tab))| {
                let text_x = rect.x + tab_padding_x;
                let text_y = rect.y + (rect.height - font_height) / 2.0;
                let is_active = i == self.state.active_tab_index();

                let (display_text, is_editing) = if edit_state.tab_id == Some(tab.id) {
                    let mut text = edit_state.text.clone();
                    // Convert character index to byte index for insertion
                    let byte_idx = text
                        .char_indices()
                        .nth(edit_state.cursor)
                        .map(|(i, _)| i)
                        .unwrap_or(text.len());
                    text.insert(byte_idx, '|');
                    (text, true)
                } else {
                    (tab.title.clone(), false)
                };

                (text_x, text_y, display_text, is_active, is_editing)
            })
            .collect()
    }

    /// Get close button positions for text rendering (returns x, y position for 'x' glyph)
    pub fn get_close_button_labels(&self) -> Vec<(f32, f32)> {
        let s = self.layout.scale_factor();
        let font_height = 14.0 * s;

        self.layout
            .tab_rects()
            .iter()
            .map(|rect| {
                // Center the 'x' character in the close button area
                let x = rect.close_x + (self.theme.close.size * s - font_height * 0.5) * 0.5;
                let y = rect.y + (rect.height - font_height) / 2.0;
                (x, y)
            })
            .collect()
    }

    /// Prepare the tab bar for rendering (builds vello scene)
    pub fn prepare(&mut self, device: &wgpu::Device, _queue: &wgpu::Queue) {
        // Recalculate layout if dirty
        if self.layout.is_dirty() {
            self.layout.calculate_rects(&self.state, &self.theme);
        }

        // Build vello scene for shapes
        self.vello_renderer
            .prepare(device, &self.state, &self.layout, &self.theme);
    }

    /// Render vello scene to internal texture using shared renderer
    pub fn render_vello(
        &mut self,
        renderer: &mut vello::Renderer,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<(), vello::Error> {
        self.vello_renderer
            .render_to_texture(renderer, device, queue)
    }

    /// Get vello texture view for compositing
    pub fn vello_texture_view(&self) -> Option<&wgpu::TextureView> {
        self.vello_renderer.texture_view()
    }
}

fn color_to_array(color: &crt_theme::Color) -> [f32; 4] {
    [color.r, color.g, color.b, color.a]
}

impl TabBar {
    /// Render tab bar shapes using RectRenderer (sharp corners, no Vello needed)
    ///
    /// This is the lightweight path - call this instead of render_vello() when
    /// you don't need rounded corners or complex effects.
    pub fn render_shapes_to_rects(&self, rect_renderer: &mut crate::RectRenderer) {
        let s = self.layout.scale_factor();
        let bar_height = self.layout.height() * s;
        let (screen_width, _) = self.layout.screen_size();

        let tab_rects = self.layout.tab_rects();
        let active_tab = self.state.active_tab_index();

        // Tab bar background
        let bar_bg = color_to_array(&self.theme.bar.background);
        rect_renderer.push_rect(0.0, 0.0, screen_width, bar_height, bar_bg);

        // Bottom border
        let border_color = color_to_array(&self.theme.bar.border_color);
        rect_renderer.push_rect(0.0, bar_height - s, screen_width, s, border_color);

        // Draw individual tabs
        for (i, rect) in tab_rects.iter().enumerate() {
            let is_active = i == active_tab;
            let bg_color = if is_active {
                color_to_array(&self.theme.active.background)
            } else {
                color_to_array(&self.theme.tab.background)
            };

            // Tab background (sharp corners)
            rect_renderer.push_rect(rect.x, rect.y, rect.width, rect.height, bg_color);

            // Tab border (top and sides)
            let border = color_to_array(&self.theme.bar.border_color);
            // Top
            rect_renderer.push_rect(rect.x, rect.y, rect.width, s, border);
            // Left
            rect_renderer.push_rect(rect.x, rect.y, s, rect.height, border);
            // Right
            rect_renderer.push_rect(rect.x + rect.width - s, rect.y, s, rect.height, border);

            // Active tab accent line at bottom
            if is_active {
                let accent = color_to_array(&self.theme.active.accent);
                let accent_height = 2.0 * s;
                rect_renderer.push_rect(
                    rect.x,
                    rect.y + rect.height - accent_height,
                    rect.width,
                    accent_height,
                    accent,
                );
            }
        }

        // Close buttons are rendered as text glyphs in render_tab_titles
    }
}
