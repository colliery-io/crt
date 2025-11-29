//! Window state management
//!
//! Per-window state including shells, GPU resources, and interaction state.

use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::Hasher;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crt_core::{AnsiColor, CellFlags, SemanticZone, ShellTerminal, Size};
use crt_renderer::GlyphStyle;
use crt_theme::AnsiPalette;
use winit::window::Window;

use crate::gpu::{SharedGpuState, WindowGpuState};
use crate::input::detect_urls_in_line;

/// Unique identifier for a terminal tab
pub type TabId = u64;

/// Map alacritty_terminal AnsiColor to RGBA array using theme palette
fn ansi_color_to_rgba(
    color: AnsiColor,
    palette: &AnsiPalette,
    default_fg: [f32; 4],
    default_bg: [f32; 4],
) -> [f32; 4] {
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
                // Foreground/Background use their actual theme colors
                NamedColor::Foreground => return default_fg,
                NamedColor::Background => return default_bg,
                // Dim variants use regular colors
                NamedColor::DimBlack => palette.black,
                NamedColor::DimRed => palette.red,
                NamedColor::DimGreen => palette.green,
                NamedColor::DimYellow => palette.yellow,
                NamedColor::DimBlue => palette.blue,
                NamedColor::DimMagenta => palette.magenta,
                NamedColor::DimCyan => palette.cyan,
                NamedColor::DimWhite => palette.white,
                // Cursor color - use foreground as default
                NamedColor::Cursor => return default_fg,
                // Bright foreground
                NamedColor::BrightForeground => palette.bright_white,
                NamedColor::DimForeground => palette.white,
            };
            c.to_array()
        }
        // Indexed colors (0-255)
        Indexed(idx) => {
            if idx < 16 {
                // First 16 are the base ANSI palette
                palette.get(idx).to_array()
            } else {
                // Extended colors (16-255): check for theme override first
                if let Some(color) = palette.get_extended(idx) {
                    color.to_array()
                } else {
                    // Fall back to calculated standard 256-color palette
                    AnsiPalette::calculate_extended(idx).to_array()
                }
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
    pub shells: HashMap<TabId, ShellTerminal>,
    // Content hash to skip reshaping when unchanged (per tab)
    pub content_hashes: HashMap<TabId, u64>,
    // Window-specific sizing
    pub cols: usize,
    pub rows: usize,
    pub scale_factor: f32,
    // User font scale multiplier (1.0 = default)
    pub font_scale: f32,
    // Rendering state
    pub dirty: bool,
    pub frame_count: u32,
    /// Window is occluded (hidden, minimized, or fully covered)
    pub occluded: bool,
    /// Window has keyboard focus (effects animate only when focused)
    pub focused: bool,
    // Interaction state
    pub cursor_position: (f32, f32),
    pub last_click_time: Option<Instant>,
    pub last_click_tab: Option<TabId>,
    // Terminal selection state
    pub mouse_pressed: bool,
    pub selection_click_count: u8,
    pub last_selection_click_time: Option<Instant>,
    pub last_selection_click_pos: Option<(usize, usize)>,
    // Cached render state (decorations persist across frames)
    pub cached_render: CachedRenderState,
    // Detected URLs in current viewport
    pub detected_urls: Vec<crate::input::DetectedUrl>,
    // Index of currently hovered URL (for hover underline effect)
    pub hovered_url_index: Option<usize>,
    // Search state
    pub search: SearchState,
    // Bell state for visual flash
    pub bell: BellState,
    // Context menu state
    pub context_menu: ContextMenu,

}

/// Bell visual flash state
#[derive(Debug, Clone)]
pub struct BellState {
    /// When the bell was triggered (None if not active)
    pub triggered_at: Option<Instant>,
    /// Duration of the visual flash
    pub flash_duration: Duration,
    /// Flash intensity multiplier (from config)
    pub intensity: f32,
    /// Whether visual bell is enabled
    pub enabled: bool,
}

impl Default for BellState {
    fn default() -> Self {
        Self {
            triggered_at: None,
            flash_duration: Duration::from_millis(100),
            intensity: 0.3,
            enabled: true,
        }
    }
}

impl BellState {
    /// Create from config
    pub fn from_config(config: &crate::config::BellConfig) -> Self {
        Self {
            triggered_at: None,
            flash_duration: Duration::from_millis(config.flash_duration_ms),
            intensity: config.flash_intensity,
            enabled: config.visual,
        }
    }

    /// Trigger the bell (start visual flash)
    pub fn trigger(&mut self) {
        if self.enabled {
            self.triggered_at = Some(Instant::now());
        }
    }

    /// Get the current flash intensity (0.0 = no flash, up to configured intensity)
    pub fn flash_intensity(&self) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        match self.triggered_at {
            Some(triggered) => {
                let elapsed = triggered.elapsed();
                if elapsed >= self.flash_duration {
                    0.0
                } else {
                    // Fade out linearly, scaled by intensity
                    let progress =
                        1.0 - (elapsed.as_secs_f32() / self.flash_duration.as_secs_f32());
                    progress * self.intensity
                }
            }
            None => 0.0,
        }
    }

    /// Check if flash is still active
    pub fn is_active(&self) -> bool {
        self.flash_intensity() > 0.0
    }
}

/// Search match position in terminal
#[derive(Debug, Clone, Copy)]
pub struct SearchMatch {
    /// Line number (grid-relative: negative = history, 0+ = visible)
    pub line: i32,
    /// Starting column
    pub start_col: usize,
    /// Ending column (exclusive)
    pub end_col: usize,
}

/// Search state for find-in-terminal functionality
#[derive(Debug, Clone, Default)]
pub struct SearchState {
    /// Whether search mode is active
    pub active: bool,
    /// Current search query
    pub query: String,
    /// All matches found
    pub matches: Vec<SearchMatch>,
    /// Index of current/focused match
    pub current_match: usize,
}

/// Context menu item
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextMenuItem {
    Copy,
    Paste,
    SelectAll,
}

impl ContextMenuItem {
    /// Get the display label for this menu item
    pub fn label(&self) -> &'static str {
        match self {
            ContextMenuItem::Copy => "Copy",
            ContextMenuItem::Paste => "Paste",
            ContextMenuItem::SelectAll => "Select All",
        }
    }

    /// Get the keyboard shortcut hint
    pub fn shortcut(&self) -> &'static str {
        #[cfg(target_os = "macos")]
        match self {
            ContextMenuItem::Copy => "Cmd+C",
            ContextMenuItem::Paste => "Cmd+V",
            ContextMenuItem::SelectAll => "Cmd+A",
        }
        #[cfg(not(target_os = "macos"))]
        match self {
            ContextMenuItem::Copy => "Ctrl+C",
            ContextMenuItem::Paste => "Ctrl+V",
            ContextMenuItem::SelectAll => "Ctrl+A",
        }
    }

    /// Get all menu items in order
    pub fn all() -> &'static [ContextMenuItem] {
        &[
            ContextMenuItem::Copy,
            ContextMenuItem::Paste,
            ContextMenuItem::SelectAll,
        ]
    }
}

/// Context menu state
#[derive(Debug, Clone, Default)]
pub struct ContextMenu {
    /// Whether the context menu is visible
    pub visible: bool,
    /// Position of the menu (top-left corner)
    pub x: f32,
    pub y: f32,
    /// Currently hovered item index
    pub hovered_item: Option<usize>,
    /// Menu dimensions (computed during render)
    pub width: f32,
    pub height: f32,
    pub item_height: f32,
}

impl ContextMenu {
    /// Show the context menu at the given position
    pub fn show(&mut self, x: f32, y: f32) {
        self.visible = true;
        self.x = x;
        self.y = y;
        self.hovered_item = None;
    }

    /// Hide the context menu
    pub fn hide(&mut self) {
        self.visible = false;
        self.hovered_item = None;
    }

    /// Check if a point is inside the menu
    pub fn contains(&self, x: f32, y: f32) -> bool {
        if !self.visible {
            return false;
        }
        x >= self.x && x <= self.x + self.width && y >= self.y && y <= self.y + self.height
    }

    /// Get the menu item at the given position, if any
    pub fn item_at(&self, x: f32, y: f32) -> Option<ContextMenuItem> {
        if !self.contains(x, y) {
            return None;
        }
        let rel_y = y - self.y;
        let index = (rel_y / self.item_height) as usize;
        ContextMenuItem::all().get(index).copied()
    }

    /// Update hover state based on mouse position
    pub fn update_hover(&mut self, x: f32, y: f32) {
        if !self.visible {
            self.hovered_item = None;
            return;
        }
        if self.contains(x, y) {
            let rel_y = y - self.y;
            self.hovered_item = Some((rel_y / self.item_height) as usize);
        } else {
            self.hovered_item = None;
        }
    }
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
    /// Whether the cursor should be visible (false if terminal hid it via escape sequence)
    pub visible: bool,
    /// Cursor shape requested by the terminal/application
    pub shape: crt_core::CursorShape,
}

/// Text decoration (underline, strikethrough, or background)
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
    /// Cell background color
    Background,
    /// Underline decoration
    Underline,
    /// Strikethrough decoration
    Strikethrough,
}

/// Result from text buffer update
pub struct TextBufferUpdateResult {
    pub cursor: CursorInfo,
    pub decorations: Vec<TextDecoration>,
}

/// Cached rendering state that persists across frames
#[derive(Default)]
pub struct CachedRenderState {
    /// Cached decorations from last content update
    pub decorations: Vec<TextDecoration>,
    /// Cached cursor info
    pub cursor: Option<CursorInfo>,
}

impl WindowState {
    /// Update text buffer for this window's active shell
    ///
    /// Returns cursor position and decorations if content changed, None otherwise
    pub fn update_text_buffer(
        &mut self,
        shared_gpu: &SharedGpuState,
    ) -> Option<TextBufferUpdateResult> {
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
        // Include cursor shape in hash - programs like Claude Code change cursor style
        let cursor_shape_discriminant = match content.cursor.shape {
            crt_core::CursorShape::Block => 0u8,
            crt_core::CursorShape::Underline => 1u8,
            crt_core::CursorShape::Beam => 2u8,
            crt_core::CursorShape::HollowBlock => 3u8,
            crt_core::CursorShape::Hidden => 4u8,
        };
        hasher.write_u8(cursor_shape_discriminant);
        // Include cursor visibility mode
        hasher.write_u8(if terminal.cursor_mode_visible() { 1 } else { 0 });
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
        self.gpu.output_grid_renderer.clear();

        let cell_width = self.gpu.glyph_cache.cell_width();
        let line_height = self.gpu.glyph_cache.line_height();
        let padding = 10.0 * self.scale_factor;

        // Get display offset to convert grid lines to viewport lines
        let display_offset = terminal.display_offset() as i32;

        // Cursor info
        let cursor = content.cursor;
        let cursor_point = cursor.point;
        // Check cursor visibility via TermMode::SHOW_CURSOR (CSI ?25h/l)
        let cursor_visible = terminal.cursor_mode_visible();

        // Compute cursor position (adjust for scroll offset)
        let cursor_viewport_line = cursor_point.line.0 + display_offset;
        let cursor_x = offset_x + padding + (cursor_point.column.0 as f32 * cell_width);
        let cursor_y = offset_y + padding + (cursor_viewport_line as f32 * line_height);

        // First pass: collect line text for URL detection
        let mut line_texts: std::collections::BTreeMap<i32, String> =
            std::collections::BTreeMap::new();
        for cell in content.display_iter {
            let viewport_line = cell.point.line.0 + display_offset;
            line_texts.entry(viewport_line).or_default().push(cell.c);
        }

        // Detect URLs before rendering so we can underline them with text color
        self.detected_urls.clear();
        for (viewport_line, line_text) in &line_texts {
            let urls = detect_urls_in_line(line_text, *viewport_line as usize);
            self.detected_urls.extend(urls);
        }

        // Check if shell supports OSC 133 semantic zones
        let has_semantic_zones = terminal.has_semantic_zones();

        // Re-read content for rendering pass
        let content = terminal.renderable_content();

        // Collect decorations (backgrounds, underline, strikethrough)
        let mut decorations: Vec<TextDecoration> = Vec::new();

        // Get theme colors
        let palette = &self.gpu.effect_pipeline.theme().palette;
        let default_fg = self.gpu.effect_pipeline.theme().foreground.to_array();
        // Use the bottom of the gradient as default background (typically the darker color)
        let default_bg = self
            .gpu
            .effect_pipeline
            .theme()
            .background
            .bottom
            .to_array();

        // Render cells (text only, no cursor)
        for cell in content.display_iter {
            let col = cell.point.column.0;
            // Convert grid line to viewport line (grid lines can be negative for history)
            let grid_line = cell.point.line.0;
            let viewport_line = grid_line + display_offset;

            let x = offset_x + padding + (col as f32 * cell_width);
            let y = offset_y + padding + (viewport_line as f32 * line_height);

            let flags = cell.flags;

            // Handle INVERSE flag - swap foreground and background colors
            let (fg_ansi, bg_ansi) = if flags.contains(CellFlags::INVERSE) {
                (cell.bg, cell.fg)
            } else {
                (cell.fg, cell.bg)
            };

            // Get foreground color
            let mut fg_color = ansi_color_to_rgba(fg_ansi, palette, default_fg, default_bg);

            // Apply DIM flag by reducing alpha
            if flags.contains(CellFlags::DIM) {
                fg_color[3] *= 0.5;
            }

            // Get background color and add decoration if non-default
            // Skip spacer cells (for wide characters) and hidden cells - they shouldn't have
            // their own background decorations as this causes visual artifacts
            let is_spacer = flags.intersects(
                CellFlags::WIDE_CHAR_SPACER | CellFlags::LEADING_WIDE_CHAR_SPACER,
            );
            let is_hidden = flags.contains(CellFlags::HIDDEN);

            if !is_spacer && !is_hidden {
                let bg_color = ansi_color_to_rgba(bg_ansi, palette, default_fg, default_bg);
                if bg_color != default_bg {
                    decorations.push(TextDecoration {
                        x,
                        y,
                        cell_width,
                        cell_height: line_height,
                        color: bg_color,
                        kind: DecorationKind::Background,
                    });
                }
            }

            // Collect underline decorations
            if flags.intersects(CellFlags::UNDERLINE | CellFlags::DOUBLE_UNDERLINE) {
                decorations.push(TextDecoration {
                    x,
                    y,
                    cell_width,
                    cell_height: line_height,
                    color: fg_color,
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
                    color: fg_color,
                    kind: DecorationKind::Strikethrough,
                });
            }

            // Check if this cell is part of the hovered URL and add underline
            if let Some(hovered_idx) = self.hovered_url_index {
                if let Some(hovered_url) = self.detected_urls.get(hovered_idx) {
                    if hovered_url.line == viewport_line as usize
                        && col >= hovered_url.start_col
                        && col < hovered_url.end_col
                    {
                        decorations.push(TextDecoration {
                            x,
                            y,
                            cell_width,
                            cell_height: line_height,
                            color: fg_color,
                            kind: DecorationKind::Underline,
                        });
                    }
                }
            }

            // Check if this cell is part of a search match and add background highlight
            if self.search.active && !self.search.matches.is_empty() {
                let highlight_style = &self.gpu.effect_pipeline.theme().highlight;
                for (match_idx, search_match) in self.search.matches.iter().enumerate() {
                    // Compare against grid_line (search matches use grid-relative coordinates)
                    if search_match.line == grid_line
                        && col >= search_match.start_col
                        && col < search_match.end_col
                    {
                        // Use brighter color for current match, theme color for others
                        let highlight_color = if match_idx == self.search.current_match {
                            highlight_style.current_background.to_array()
                        } else {
                            highlight_style.background.to_array()
                        };
                        decorations.push(TextDecoration {
                            x,
                            y,
                            cell_width,
                            cell_height: line_height,
                            color: highlight_color,
                            kind: DecorationKind::Background,
                        });
                        break; // Only one match highlight per cell
                    }
                }
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
                // Route to appropriate renderer based on semantic zones (OSC 133)
                // - Prompt/Input zones -> grid_renderer (with glow effect)
                // - Output/Unknown zones -> output_grid_renderer (flat, no glow)
                let use_glow = if has_semantic_zones {
                    // Use deterministic zone-based routing
                    let zone = terminal.get_line_zone(grid_line);
                    matches!(zone, SemanticZone::Prompt | SemanticZone::Input)
                } else {
                    // Fallback heuristic: cursor line and line above (for multi-line prompts)
                    viewport_line >= cursor_viewport_line - 1
                        && viewport_line <= cursor_viewport_line
                };

                if use_glow {
                    self.gpu.grid_renderer.push_glyphs(&[glyph], fg_color);
                } else {
                    self.gpu
                        .output_grid_renderer
                        .push_glyphs(&[glyph], fg_color);
                }
            }
        }

        self.gpu.glyph_cache.flush(&shared_gpu.queue);

        Some(TextBufferUpdateResult {
            cursor: CursorInfo {
                x: cursor_x,
                y: cursor_y,
                cell_width,
                cell_height: line_height,
                visible: cursor_visible,
                shape: cursor.shape,
            },
            decorations,
        })
    }

    /// Create a shell for a new tab with a specific working directory
    pub fn create_shell_for_tab_with_cwd(&mut self, tab_id: u64, cwd: Option<std::path::PathBuf>) {
        let size = Size::new(self.cols, self.rows);
        let result = if let Some(dir) = cwd {
            log::info!("Spawning shell in directory: {:?}", dir);
            ShellTerminal::with_cwd(size, dir)
        } else {
            ShellTerminal::new(size)
        };

        match result {
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

    /// Get the current working directory of the active tab's shell
    pub fn active_shell_cwd(&self) -> Option<std::path::PathBuf> {
        let tab_id = self.gpu.tab_bar.active_tab_id()?;
        let shell = self.shells.get(&tab_id)?;
        shell.working_directory()
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
