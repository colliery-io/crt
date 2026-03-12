//! Window state management
//!
//! Per-window state including shells, GPU resources, and interaction state.

mod interaction;
mod overrides;
mod render;
mod types;
mod ui;

// Re-export all public types for backward compatibility
pub use interaction::{ContextMenu, ContextMenuItem, InteractionState, SearchMatch, SearchState};
pub use overrides::{ActiveOverride, OverrideEventType, OverrideState};
pub use render::{
    CachedRenderState, CursorInfo, DecorationKind, PreparedCell, RenderContext, RenderLayout,
    RenderState, TerminalRenderData, TextBufferUpdateResult, TextDecoration, prepare_render_cells,
};
pub use types::{EffectId, TabId};
pub use ui::{
    BellState, CopyIndicator, Toast, ToastType, UiState, WindowRenameState, ZoomIndicator,
};

use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::hash::Hasher;
use std::sync::Arc;

use crt_core::{CellFlags, ShellTerminal, Size, SpawnOptions};
use crt_renderer::GlyphStyle;
use crt_theme::Theme;
use winit::window::Window;

use crate::gpu::{SharedGpuState, WindowGpuState};
use crate::input::{detect_urls_in_line, merge_wrapped_urls};

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
    // Rendering state (dirty, frame_count, occluded, focused, cached)
    pub render: RenderState,
    // Interaction state (cursor, mouse, selection, URLs)
    pub interaction: InteractionState,
    // UI overlay state (search, bell, context menu)
    pub ui: UiState,
    // Custom window title (None = use default "CRT Terminal")
    pub custom_title: Option<String>,
    // Per-window theme
    pub theme: Theme,
    pub theme_name: String,
}

impl WindowState {
    /// Set the theme for this window, updating all GPU resources
    pub fn set_theme(&mut self, name: &str, theme: Theme) {
        self.theme_name = name.to_string();
        self.theme = theme.clone();

        // Update effect pipeline with new theme
        self.gpu.effect_pipeline.set_theme(theme.clone());

        // Update tab bar theme
        self.gpu.tab_bar.set_theme(theme.tabs);

        // Update cursor colors
        self.gpu.terminal_vello.set_cursor_color([
            theme.cursor_color.r,
            theme.cursor_color.g,
            theme.cursor_color.b,
            theme.cursor_color.a,
        ]);
        self.gpu
            .terminal_vello
            .set_cursor_glow(theme.cursor_glow.map(|g| {
                (
                    [g.color.r, g.color.g, g.color.b, g.color.a],
                    g.radius,
                    g.intensity,
                )
            }));

        // Mark window as needing redraw
        self.render.dirty = true;
    }

    /// Update text buffer for this window's active shell
    ///
    /// Returns cursor position and decorations if content changed, None otherwise
    pub fn update_text_buffer(
        &mut self,
        shared_gpu: &SharedGpuState,
    ) -> Option<TextBufferUpdateResult> {
        let active_tab_id = self.gpu.tab_bar.active_tab_id();

        // Get damage info via mutable reference before taking immutable terminal ref
        let damaged_lines = active_tab_id.and_then(|id| {
            self.shells
                .get_mut(&id)
                .map(|shell| shell.terminal_mut().damaged_line_set())
        });

        let shell = active_tab_id.and_then(|id| self.shells.get(&id));

        shell?;
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
        log::debug!(
            "update_text_buffer: prev_hash={}, content_hash={}, will_render={}",
            prev_hash,
            content_hash,
            prev_hash == 0 || content_hash != prev_hash
        );
        if content_hash == prev_hash && prev_hash != 0 {
            return None; // No changes
        }
        self.content_hashes.insert(tab_id, content_hash);

        // Get content offset (excluding tab bar)
        let (offset_x, offset_y) = self.gpu.tab_bar.content_offset();

        // Re-read content since we consumed it above for hashing
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

        // Single pass: collect cells AND build line text for URL detection
        // This avoids a second terminal.renderable_content() call
        // Reuse cached collections to avoid per-update allocations
        // Clear line_texts values (keeping keys to reuse String allocations)
        for s in self.render.cached.line_texts.values_mut() {
            s.clear();
        }
        self.render.cached.collected_cells.clear();

        let mut inverse_count = 0;
        let mut total_cells = 0;
        let mut line1_cells = 0;
        for cell in content.display_iter {
            let viewport_line = cell.point.line.0 + display_offset;
            self.render
                .cached
                .line_texts
                .entry(viewport_line)
                .or_default()
                .push(cell.c);

            // Track cells on line 1 for debugging paste issue
            if cell.point.line.0 == 1 {
                line1_cells += 1;
                if line1_cells <= 20 {
                    log::debug!(
                        "Line1 cell: col={}, char='{}', inverse={}, fg={:?}, bg={:?}",
                        cell.point.column.0,
                        cell.c,
                        cell.flags.contains(CellFlags::INVERSE),
                        cell.fg,
                        cell.bg
                    );
                }
            }

            // Track INVERSE cells for debugging
            if cell.flags.contains(CellFlags::INVERSE) {
                inverse_count += 1;
                if inverse_count <= 5 {
                    log::debug!(
                        "INVERSE cell: line={}, col={}, char='{}', fg={:?}, bg={:?}",
                        cell.point.line.0,
                        cell.point.column.0,
                        cell.c,
                        cell.fg,
                        cell.bg
                    );
                }
            }
            total_cells += 1;

            // Collect cell data for rendering pass
            self.render
                .cached
                .collected_cells
                .push(render::CollectedCell {
                    col: cell.point.column.0,
                    grid_line: cell.point.line.0,
                    c: cell.c,
                    flags: cell.flags,
                    fg: cell.fg,
                    bg: cell.bg,
                });
        }
        if inverse_count > 0 {
            log::info!(
                "Collected {} cells, {} with INVERSE flag",
                total_cells,
                inverse_count
            );
        }

        // Normalize INVERSE flag on paste to fix visual boundary issue
        // zsh enables INVERSE mid-line for paste highlighting, creating an ugly
        // discontinuity at the cursor position. Clear INVERSE from all cells on
        // lines with mixed INVERSE states during paste operations only.
        if self.render.paste_pending {
            // Count INVERSE vs non-INVERSE cells per line (skip whitespace)
            let mut line_stats: HashMap<i32, (usize, usize)> = HashMap::new();
            let mut total_inverse = 0usize;
            for cell in &self.render.cached.collected_cells {
                if cell.c == ' ' || cell.c == '\0' {
                    continue;
                }
                let entry = line_stats.entry(cell.grid_line).or_insert((0, 0));
                if cell.flags.contains(CellFlags::INVERSE) {
                    entry.0 += 1;
                    total_inverse += 1;
                } else {
                    entry.1 += 1;
                }
            }

            // Find lines with mixed INVERSE states
            let mixed_lines: HashSet<i32> = line_stats
                .iter()
                .filter(|(_, (inv, non_inv))| *inv > 0 && *non_inv > 0)
                .map(|(line, _)| *line)
                .collect();

            if !mixed_lines.is_empty() {
                // Found mixed lines - normalize by adding INVERSE to all cells
                // This keeps the reverse highlight look that zsh paste provides
                log::info!(
                    "Paste: adding INVERSE to all cells on {} lines with mixed states",
                    mixed_lines.len()
                );
                for cell in &mut self.render.cached.collected_cells {
                    if mixed_lines.contains(&cell.grid_line) {
                        cell.flags.insert(CellFlags::INVERSE);
                    }
                }
                self.render.paste_pending = false;
            } else if total_inverse == 0 {
                // No INVERSE cells yet - PTY hasn't responded, keep waiting
                log::debug!("Paste: waiting for PTY response (no INVERSE cells yet)");
            } else {
                // INVERSE exists but no mixed lines - nothing to normalize, clear flag
                self.render.paste_pending = false;
            }
        }

        // Detect URLs before rendering so we can underline them with text color
        self.interaction.detected_urls.clear();
        for (viewport_line, line_text) in &self.render.cached.line_texts {
            let urls = detect_urls_in_line(line_text, *viewport_line as usize);
            self.interaction.detected_urls.extend(urls);
        }
        // Merge URLs that wrap across multiple lines
        merge_wrapped_urls(
            &mut self.interaction.detected_urls,
            &self.render.cached.line_texts,
            self.cols,
        );

        // Flatten the Option<Option<Vec>> from the early damage query
        let damaged_lines = damaged_lines.flatten();

        // Prepare render data using pure function (no GPU calls)
        let has_semantic_zones = terminal.has_semantic_zones();
        let theme = self.gpu.effect_pipeline.theme();
        let ctx = RenderContext {
            layout: RenderLayout {
                offset_x,
                offset_y,
                padding,
                cell_width,
                line_height,
            },
            display_offset,
            cursor_viewport_line,
            palette: &theme.palette,
            default_fg: theme.foreground.to_array(),
            default_bg: theme.background.bottom.to_array(),
            hovered_url_index: self.interaction.hovered_url_index,
            detected_urls: &self.interaction.detected_urls,
            search_active: self.ui.search.active,
            search_matches: &self.ui.search.matches,
            current_match: self.ui.search.current_match,
            highlight_style: if self.ui.search.active {
                Some(&theme.highlight)
            } else {
                None
            },
            has_semantic_zones,
            get_line_zone: Box::new(|grid_line| terminal.get_line_zone(grid_line)),
        };

        // Damage-aware rendering: only prepare cells for changed lines,
        // reuse cached data for undamaged lines.
        let mut all_decorations = Vec::new();
        let is_partial = damaged_lines.is_some();

        if let Some(ref damaged) = damaged_lines
            && !damaged.is_empty()
            && !self.render.cached.line_cells.is_empty()
        {
            // Partial damage — only re-prepare damaged lines
            let damaged_set: HashSet<i32> = damaged
                .iter()
                .map(|&line| line as i32 + display_offset)
                .collect();

            // Group collected cells by viewport line
            let mut cells_by_line: HashMap<i32, Vec<render::CollectedCell>> = HashMap::new();
            for cell in &self.render.cached.collected_cells {
                let viewport_line = cell.grid_line + display_offset;
                cells_by_line
                    .entry(viewport_line)
                    .or_default()
                    .push(cell.clone());
            }

            // Collect all viewport lines that have content
            let all_lines: HashSet<i32> = cells_by_line.keys().copied().collect();

            for &vp_line in &all_lines {
                if damaged_set.contains(&vp_line) {
                    // Damaged line — re-prepare
                    if let Some(line_cells) = cells_by_line.get(&vp_line) {
                        let (prepared, decos) = prepare_render_cells(line_cells, &ctx);
                        self.render
                            .cached
                            .line_cells
                            .insert(vp_line, prepared.clone());
                        self.render
                            .cached
                            .line_decorations
                            .insert(vp_line, decos.clone());
                        all_decorations.extend(decos);
                        for cell in &prepared {
                            let style = GlyphStyle::new(cell.bold, cell.italic);
                            if let Some(glyph) = self
                                .gpu
                                .glyph_cache
                                .position_char_styled(cell.character, cell.x, cell.y, style)
                            {
                                if cell.use_glow {
                                    self.gpu
                                        .grid_renderer
                                        .push_glyphs(&[glyph], cell.fg_color);
                                } else {
                                    self.gpu
                                        .output_grid_renderer
                                        .push_glyphs(&[glyph], cell.fg_color);
                                }
                            }
                        }
                    }
                } else {
                    // Undamaged line — replay cached data
                    if let Some(cached_cells) = self.render.cached.line_cells.get(&vp_line) {
                        for cell in cached_cells {
                            let style = GlyphStyle::new(cell.bold, cell.italic);
                            if let Some(glyph) = self
                                .gpu
                                .glyph_cache
                                .position_char_styled(cell.character, cell.x, cell.y, style)
                            {
                                if cell.use_glow {
                                    self.gpu
                                        .grid_renderer
                                        .push_glyphs(&[glyph], cell.fg_color);
                                } else {
                                    self.gpu
                                        .output_grid_renderer
                                        .push_glyphs(&[glyph], cell.fg_color);
                                }
                            }
                        }
                    }
                    if let Some(cached_decos) = self.render.cached.line_decorations.get(&vp_line) {
                        all_decorations.extend(cached_decos.iter().cloned());
                    }
                }
            }
        } else {
            // Full damage or first render — prepare everything
            let (prepared_cells, decorations) =
                prepare_render_cells(&self.render.cached.collected_cells, &ctx);

            // Cache per-line results for future partial damage reuse
            self.render.cached.line_cells.clear();
            self.render.cached.line_decorations.clear();
            for cell in &prepared_cells {
                let vp_line = ((cell.y - offset_y - padding) / line_height).round() as i32;
                self.render
                    .cached
                    .line_cells
                    .entry(vp_line)
                    .or_default()
                    .push(cell.clone());
            }
            for deco in &decorations {
                let vp_line = ((deco.y - offset_y - padding) / line_height).round() as i32;
                self.render
                    .cached
                    .line_decorations
                    .entry(vp_line)
                    .or_default()
                    .push(deco.clone());
            }

            // Push to GPU
            for cell in &prepared_cells {
                let style = GlyphStyle::new(cell.bold, cell.italic);
                if let Some(glyph) = self
                    .gpu
                    .glyph_cache
                    .position_char_styled(cell.character, cell.x, cell.y, style)
                {
                    if cell.use_glow {
                        self.gpu.grid_renderer.push_glyphs(&[glyph], cell.fg_color);
                    } else {
                        self.gpu
                            .output_grid_renderer
                            .push_glyphs(&[glyph], cell.fg_color);
                    }
                }
            }

            all_decorations = decorations;
        }

        if is_partial {
            log::debug!(
                "Partial damage render: {} damaged lines",
                damaged_lines.as_ref().map_or(0, |v| v.len())
            );
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
            decorations: all_decorations,
        })
    }

    /// Create a shell for a new tab with spawn options
    pub fn create_shell_for_tab(&mut self, tab_id: u64, options: SpawnOptions) {
        let size = Size::new(self.cols, self.rows);
        log::info!(
            "Spawning shell for tab {} with semantic_prompts={}",
            tab_id,
            options.semantic_prompts
        );
        let result = ShellTerminal::with_options(size, options);

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
            self.render.dirty = true;
        }
    }
}
