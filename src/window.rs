//! Window state management
//!
//! Per-window state including shells, GPU resources, and interaction state.

use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::Hasher;
use std::sync::Arc;
use std::time::Instant;

use crt_core::{ShellTerminal, Size};
use winit::window::Window;

use crate::gpu::{SharedGpuState, WindowGpuState};

/// Per-window state containing window handle, GPU state, shells, and interaction state
pub struct WindowState {
    pub window: Arc<Window>,
    pub gpu: WindowGpuState,
    // Map of tab_id -> shell (each window has its own tabs)
    pub shells: HashMap<u64, ShellTerminal>,
    // Content hash to skip reshaping when unchanged (per tab)
    pub content_hashes: HashMap<u64, u64>,
    // Window-specific sizing
    pub cols: usize,
    pub rows: usize,
    pub scale_factor: f32,
    // User font scale multiplier (1.0 = default)
    pub font_scale: f32,
    // Rendering state
    pub dirty: bool,
    pub frame_count: u32,
    // Interaction state
    pub cursor_position: (f32, f32),
    pub last_click_time: Option<Instant>,
    pub last_click_tab: Option<u64>,
}

impl WindowState {
    /// Update text buffer for this window's active shell
    pub fn update_text_buffer(&mut self, shared_gpu: &SharedGpuState) -> bool {
        let active_tab_id = self.gpu.tab_bar.active_tab_id();
        let shell = active_tab_id.and_then(|id| self.shells.get(&id));

        if shell.is_none() {
            return false;
        }
        let shell = shell.unwrap();
        let terminal = shell.terminal();

        // Compute content hash to avoid re-rendering unchanged content
        let mut hasher = DefaultHasher::new();
        let content = terminal.renderable_content();
        hasher.write_i32(content.cursor.point.line.0);
        hasher.write_usize(content.cursor.point.column.0);
        for cell in content.display_iter {
            hasher.write_u32(cell.c as u32);
        }
        let content_hash = hasher.finish();

        // Check if content changed
        let tab_id = active_tab_id.unwrap();
        let prev_hash = self.content_hashes.get(&tab_id).copied().unwrap_or(0);
        if content_hash == prev_hash && prev_hash != 0 {
            return false; // No changes
        }
        self.content_hashes.insert(tab_id, content_hash);

        // Get content offset (excluding tab bar)
        let (offset_x, offset_y) = self.gpu.tab_bar.content_offset();

        // Re-read content since we consumed it above
        let content = terminal.renderable_content();
        self.gpu.grid_renderer.clear();

        let cell_width = self.gpu.glyph_cache.cell_width();
        let line_height = self.gpu.glyph_cache.line_height();
        let padding = 10.0 * self.scale_factor;

        // Cursor info
        let cursor = content.cursor;
        let cursor_point = cursor.point;

        // Render cells
        for cell in content.display_iter {
            let col = cell.point.column.0;
            let row = cell.point.line.0;

            let is_cursor = cell.point.column == cursor_point.column
                && cell.point.line == cursor_point.line;

            let x = offset_x + padding + (col as f32 * cell_width);
            let y = offset_y + padding + (row as f32 * line_height);

            let c = cell.c;
            if c == ' ' && !is_cursor {
                continue;
            }

            // Default text color (could be extended to support ANSI colors)
            let color = [0.9, 0.9, 0.9, 1.0];

            if let Some(glyph) = self.gpu.glyph_cache.position_char(c, x, y) {
                self.gpu.grid_renderer.push_glyphs(&[glyph], color);
            }

            // Render cursor as a highlighted space
            if is_cursor {
                let cursor_color = [0.8, 0.8, 0.2, 0.8];
                // Use a block character for cursor visualization
                if let Some(glyph) = self.gpu.glyph_cache.position_char('\u{2588}', x, y) {
                    self.gpu.grid_renderer.push_glyphs(&[glyph], cursor_color);
                }
            }
        }

        self.gpu.glyph_cache.flush(&shared_gpu.queue);
        true
    }

    /// Create a shell for a new tab
    pub fn create_shell_for_tab(&mut self, tab_id: u64) {
        match ShellTerminal::new(Size::new(self.cols, self.rows)) {
            Ok(shell) => {
                log::info!("Shell spawned for tab {}", tab_id);
                self.shells.insert(tab_id, shell);
                self.content_hashes.insert(tab_id, 0);
            }
            Err(e) => {
                log::error!("Failed to spawn shell for tab {}: {}", tab_id, e);
            }
        }
    }

    /// Remove shell for a closed tab
    pub fn remove_shell_for_tab(&mut self, tab_id: u64) {
        self.shells.remove(&tab_id);
        self.content_hashes.remove(&tab_id);
        log::info!("Removed shell for tab {}", tab_id);
    }

    /// Force redraw of active tab by clearing its content hash
    pub fn force_active_tab_redraw(&mut self) {
        if let Some(tab_id) = self.gpu.tab_bar.active_tab_id() {
            self.content_hashes.insert(tab_id, 0);
            self.dirty = true;
        }
    }
}
