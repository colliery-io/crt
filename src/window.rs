//! Window state management
//!
//! Per-window state including shells, GPU resources, and interaction state.

use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::Hasher;
use std::sync::Arc;
use std::time::Instant;

use crt_core::{AnsiColor, CellFlags, ShellTerminal, Size};
use crt_renderer::GlyphStyle;
use crt_theme::AnsiPalette;
use winit::window::Window;

use crate::gpu::{SharedGpuState, WindowGpuState};

/// Map alacritty_terminal AnsiColor to RGBA array using theme palette
fn ansi_color_to_rgba(color: AnsiColor, palette: &AnsiPalette, default_color: [f32; 4]) -> [f32; 4] {
    use crt_core::AnsiColor::*;
    use crt_core::NamedColor;

    match color {
        // Named colors (0-7 normal, 8-15 bright)
        Named(named) => {
            let c = match named {
                NamedColor::Black => palette.black,
                NamedColor::Red => palette.red,
                NamedColor::Green => palette.green,
                NamedColor::Yellow => palette.yellow,
                NamedColor::Blue => palette.blue,
                NamedColor::Magenta => palette.magenta,
                NamedColor::Cyan => palette.cyan,
                NamedColor::White => palette.white,
                NamedColor::BrightBlack => palette.bright_black,
                NamedColor::BrightRed => palette.bright_red,
                NamedColor::BrightGreen => palette.bright_green,
                NamedColor::BrightYellow => palette.bright_yellow,
                NamedColor::BrightBlue => palette.bright_blue,
                NamedColor::BrightMagenta => palette.bright_magenta,
                NamedColor::BrightCyan => palette.bright_cyan,
                NamedColor::BrightWhite => palette.bright_white,
                // Foreground/Background use default
                NamedColor::Foreground | NamedColor::Background => return default_color,
                // Dim variants use regular colors
                NamedColor::DimBlack => palette.black,
                NamedColor::DimRed => palette.red,
                NamedColor::DimGreen => palette.green,
                NamedColor::DimYellow => palette.yellow,
                NamedColor::DimBlue => palette.blue,
                NamedColor::DimMagenta => palette.magenta,
                NamedColor::DimCyan => palette.cyan,
                NamedColor::DimWhite => palette.white,
                // Cursor color
                NamedColor::Cursor => return default_color,
                // Bright foreground
                NamedColor::BrightForeground => palette.bright_white,
                NamedColor::DimForeground => palette.white,
            };
            c.to_array()
        }
        // Indexed colors (0-255)
        Indexed(idx) => {
            if idx < 16 {
                // First 16 are the ANSI palette
                palette.get(idx).to_array()
            } else if idx < 232 {
                // 216 color cube (16-231)
                let idx = idx - 16;
                let r = (idx / 36) % 6;
                let g = (idx / 6) % 6;
                let b = idx % 6;
                let to_float = |v: u8| if v == 0 { 0.0 } else { (v as f32 * 40.0 + 55.0) / 255.0 };
                [to_float(r), to_float(g), to_float(b), 1.0]
            } else {
                // Grayscale (232-255)
                let gray = ((idx - 232) as f32 * 10.0 + 8.0) / 255.0;
                [gray, gray, gray, 1.0]
            }
        }
        // Direct RGB color
        Spec(rgb) => [
            rgb.r as f32 / 255.0,
            rgb.g as f32 / 255.0,
            rgb.b as f32 / 255.0,
            1.0,
        ],
    }
}

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
    // Terminal selection state
    pub mouse_pressed: bool,
    pub selection_click_count: u8,
    pub last_selection_click_time: Option<Instant>,
    pub last_selection_click_pos: Option<(usize, usize)>,
}

/// Cursor position info returned from text buffer update
#[derive(Debug, Clone, Copy)]
pub struct CursorInfo {
    /// X position in pixels
    pub x: f32,
    /// Y position in pixels
    pub y: f32,
    /// Cell width in pixels
    pub cell_width: f32,
    /// Cell height in pixels
    pub cell_height: f32,
}

/// Text decoration (underline or strikethrough)
#[derive(Debug, Clone, Copy)]
pub struct TextDecoration {
    /// X position in pixels
    pub x: f32,
    /// Y position in pixels
    pub y: f32,
    /// Cell width in pixels
    pub cell_width: f32,
    /// Cell height in pixels
    pub cell_height: f32,
    /// Decoration color
    pub color: [f32; 4],
    /// Decoration type
    pub kind: DecorationKind,
}

/// Types of text decoration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecorationKind {
    Underline,
    Strikethrough,
}

/// Result from text buffer update
pub struct TextBufferUpdateResult {
    pub cursor: CursorInfo,
    pub decorations: Vec<TextDecoration>,
}

impl WindowState {
    /// Update text buffer for this window's active shell
    ///
    /// Returns cursor position and decorations if content changed, None otherwise
    pub fn update_text_buffer(&mut self, shared_gpu: &SharedGpuState) -> Option<TextBufferUpdateResult> {
        let active_tab_id = self.gpu.tab_bar.active_tab_id();
        let shell = active_tab_id.and_then(|id| self.shells.get(&id));

        if shell.is_none() {
            return None;
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
            return None; // No changes
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

        // Get display offset to convert grid lines to viewport lines
        let display_offset = terminal.display_offset() as i32;

        // Cursor info
        let cursor = content.cursor;
        let cursor_point = cursor.point;

        // Compute cursor position (adjust for scroll offset)
        let cursor_viewport_line = cursor_point.line.0 + display_offset;
        let cursor_x = offset_x + padding + (cursor_point.column.0 as f32 * cell_width);
        let cursor_y = offset_y + padding + (cursor_viewport_line as f32 * line_height);

        // Collect decorations (underline, strikethrough)
        let mut decorations = Vec::new();

        // Render cells (text only, no cursor)
        for cell in content.display_iter {
            let col = cell.point.column.0;
            // Convert grid line to viewport line (grid lines can be negative for history)
            let grid_line = cell.point.line.0;
            let viewport_line = grid_line + display_offset;

            let x = offset_x + padding + (col as f32 * cell_width);
            let y = offset_y + padding + (viewport_line as f32 * line_height);

            // Get foreground color from cell, mapped through theme palette
            let palette = &self.gpu.effect_pipeline.theme().palette;
            let default_fg = self.gpu.effect_pipeline.theme().foreground.to_array();
            let mut color = ansi_color_to_rgba(cell.fg, palette, default_fg);

            // Apply DIM flag by reducing alpha
            let flags = cell.flags;
            if flags.contains(CellFlags::DIM) {
                color[3] *= 0.5;
            }

            // Collect underline decorations
            if flags.intersects(CellFlags::UNDERLINE | CellFlags::DOUBLE_UNDERLINE) {
                decorations.push(TextDecoration {
                    x,
                    y,
                    cell_width,
                    cell_height: line_height,
                    color,
                    kind: DecorationKind::Underline,
                });
            }

            // Collect strikethrough decorations
            if flags.contains(CellFlags::STRIKEOUT) {
                decorations.push(TextDecoration {
                    x,
                    y,
                    cell_width,
                    cell_height: line_height,
                    color,
                    kind: DecorationKind::Strikethrough,
                });
            }

            let c = cell.c;
            if c == ' ' {
                continue;
            }

            // Get style from cell flags
            let style = GlyphStyle::new(
                flags.contains(CellFlags::BOLD),
                flags.contains(CellFlags::ITALIC),
            );

            if let Some(glyph) = self.gpu.glyph_cache.position_char_styled(c, x, y, style) {
                self.gpu.grid_renderer.push_glyphs(&[glyph], color);
            }
        }

        self.gpu.glyph_cache.flush(&shared_gpu.queue);

        Some(TextBufferUpdateResult {
            cursor: CursorInfo {
                x: cursor_x,
                y: cursor_y,
                cell_width,
                cell_height: line_height,
            },
            decorations,
        })
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
