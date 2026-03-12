//! Render state and terminal render data preparation.
//!
//! Contains render-related state, cell collection, and the pure
//! `prepare_render_cells()` function for renderer-agnostic terminal data.

use crt_core::{AnsiColor, CellFlags, SemanticZone};
use crt_theme::AnsiPalette;

use super::interaction::SearchMatch;
use super::types::ansi_color_to_rgba;

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

/// Collected cell data for single-pass processing
/// Avoids multiple terminal.renderable_content() calls
#[derive(Clone, Copy)]
pub(crate) struct CollectedCell {
    pub(crate) col: usize,
    pub(crate) grid_line: i32,
    pub(crate) c: char,
    pub(crate) flags: CellFlags,
    pub(crate) fg: AnsiColor,
    pub(crate) bg: AnsiColor,
}

/// A cell prepared for rendering, with computed colors and routing.
/// This is renderer-agnostic — no GPU types involved.
#[derive(Clone)]
pub struct PreparedCell {
    /// The character to render
    pub character: char,
    /// X pixel position
    pub x: f32,
    /// Y pixel position
    pub y: f32,
    /// Computed foreground color (after INVERSE, DIM)
    pub fg_color: [f32; 4],
    /// Whether this cell is bold
    pub bold: bool,
    /// Whether this cell is italic
    pub italic: bool,
    /// Whether this cell should use the glow renderer (prompt/input zone)
    pub use_glow: bool,
}

/// Renderer-agnostic terminal render data, prepared from terminal state.
/// Can be consumed by any renderer (GPU or mock) for testing.
#[allow(dead_code)]
pub struct TerminalRenderData {
    /// Cells ready for rendering (characters with positions and colors)
    pub cells: Vec<PreparedCell>,
    /// Cursor info (position, shape, visibility)
    pub cursor: CursorInfo,
    /// Text decorations (backgrounds, underlines, strikethroughs)
    pub decorations: Vec<TextDecoration>,
}

/// Layout parameters for terminal rendering
pub struct RenderLayout {
    pub offset_x: f32,
    pub offset_y: f32,
    pub padding: f32,
    pub cell_width: f32,
    pub line_height: f32,
}

/// Context for preparing terminal render data (read-only references)
pub struct RenderContext<'a> {
    pub layout: RenderLayout,
    pub display_offset: i32,
    pub cursor_viewport_line: i32,
    pub palette: &'a AnsiPalette,
    pub default_fg: [f32; 4],
    pub default_bg: [f32; 4],
    pub hovered_url_index: Option<usize>,
    pub detected_urls: &'a [crate::input::DetectedUrl],
    pub search_active: bool,
    pub search_matches: &'a [SearchMatch],
    pub current_match: usize,
    pub highlight_style: Option<&'a crt_theme::HighlightStyle>,
    pub has_semantic_zones: bool,
    /// Function to determine semantic zone for a grid line
    pub get_line_zone: Box<dyn Fn(i32) -> SemanticZone + 'a>,
}

/// Prepare terminal render data from collected cells.
///
/// Pure function — takes collected cell data and render context, returns
/// renderer-agnostic data suitable for any renderer (GPU or mock).
pub fn prepare_render_cells(
    collected_cells: &[CollectedCell],
    ctx: &RenderContext<'_>,
) -> (Vec<PreparedCell>, Vec<TextDecoration>) {
    let mut prepared = Vec::with_capacity(collected_cells.len());
    let mut decorations = Vec::new();

    for cell in collected_cells {
        let col = cell.col;
        let grid_line = cell.grid_line;
        let viewport_line = grid_line + ctx.display_offset;

        let x = ctx.layout.offset_x + ctx.layout.padding + (col as f32 * ctx.layout.cell_width);
        let y = ctx.layout.offset_y
            + ctx.layout.padding
            + (viewport_line as f32 * ctx.layout.line_height);

        let flags = cell.flags;

        // Handle INVERSE flag - swap foreground and background colors
        let (fg_ansi, bg_ansi) = if flags.contains(CellFlags::INVERSE) {
            (cell.bg, cell.fg)
        } else {
            (cell.fg, cell.bg)
        };

        // Get foreground color
        let mut fg_color =
            ansi_color_to_rgba(fg_ansi, ctx.palette, ctx.default_fg, ctx.default_bg);

        // Apply DIM flag by reducing alpha
        if flags.contains(CellFlags::DIM) {
            fg_color[3] *= 0.5;
        }

        // Get background color and add decoration if non-default
        let is_spacer = flags
            .intersects(CellFlags::WIDE_CHAR_SPACER | CellFlags::LEADING_WIDE_CHAR_SPACER);
        let is_hidden = flags.contains(CellFlags::HIDDEN);

        if !is_spacer && !is_hidden {
            let bg_color =
                ansi_color_to_rgba(bg_ansi, ctx.palette, ctx.default_fg, ctx.default_bg);
            if bg_color != ctx.default_bg {
                decorations.push(TextDecoration {
                    x,
                    y,
                    cell_width: ctx.layout.cell_width,
                    cell_height: ctx.layout.line_height,
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
                cell_width: ctx.layout.cell_width,
                cell_height: ctx.layout.line_height,
                color: fg_color,
                kind: DecorationKind::Underline,
            });
        }

        // Collect strikethrough decorations
        if flags.contains(CellFlags::STRIKEOUT) {
            decorations.push(TextDecoration {
                x,
                y,
                cell_width: ctx.layout.cell_width,
                cell_height: ctx.layout.line_height,
                color: fg_color,
                kind: DecorationKind::Strikethrough,
            });
        }

        // Check if this cell is part of the hovered URL
        let in_hovered_url = if let Some(hovered_idx) = ctx.hovered_url_index
            && let Some(url) = ctx.detected_urls.get(hovered_idx)
        {
            let vp_line = viewport_line as usize;
            vp_line >= url.line
                && vp_line <= url.end_line
                && {
                    if url.line == url.end_line {
                        col >= url.start_col && col < url.end_col
                    } else if vp_line == url.line {
                        col >= url.start_col
                    } else if vp_line == url.end_line {
                        col < url.end_col
                    } else {
                        true
                    }
                }
        } else {
            false
        };
        if in_hovered_url {
            decorations.push(TextDecoration {
                x,
                y,
                cell_width: ctx.layout.cell_width,
                cell_height: ctx.layout.line_height,
                color: fg_color,
                kind: DecorationKind::Underline,
            });
        }

        // Check if this cell is part of a search match
        if ctx.search_active && !ctx.search_matches.is_empty() {
            if let Some(highlight_style) = ctx.highlight_style {
                for (match_idx, search_match) in ctx.search_matches.iter().enumerate() {
                    if search_match.line == grid_line
                        && col >= search_match.start_col
                        && col < search_match.end_col
                    {
                        let highlight_color = if match_idx == ctx.current_match {
                            highlight_style.current_background.to_array()
                        } else {
                            highlight_style.background.to_array()
                        };
                        decorations.push(TextDecoration {
                            x,
                            y,
                            cell_width: ctx.layout.cell_width,
                            cell_height: ctx.layout.line_height,
                            color: highlight_color,
                            kind: DecorationKind::Background,
                        });
                        break;
                    }
                }
            }
        }

        let c = cell.c;
        if c == ' ' {
            continue;
        }

        // Determine glow routing
        let use_glow = if ctx.has_semantic_zones {
            let zone = (ctx.get_line_zone)(grid_line);
            matches!(zone, SemanticZone::Prompt | SemanticZone::Input)
        } else {
            viewport_line >= ctx.cursor_viewport_line - 1
                && viewport_line <= ctx.cursor_viewport_line
        };

        prepared.push(PreparedCell {
            character: c,
            x,
            y,
            fg_color,
            bold: flags.contains(CellFlags::BOLD),
            italic: flags.contains(CellFlags::ITALIC),
            use_glow,
        });
    }

    (prepared, decorations)
}

/// Cached rendering state that persists across frames
#[derive(Default)]
pub struct CachedRenderState {
    /// Cached decorations from last content update
    pub decorations: Vec<TextDecoration>,
    /// Cached cursor info
    pub cursor: Option<CursorInfo>,
    /// Reusable line text buffer for URL detection (cleared and reused each update)
    pub(crate) line_texts: std::collections::BTreeMap<i32, String>,
    /// Reusable cell collection buffer (cleared and reused each update)
    pub(crate) collected_cells: Vec<CollectedCell>,
    /// Per-line cached prepared cells from previous frame (for partial damage reuse)
    pub(crate) line_cells: std::collections::HashMap<i32, Vec<PreparedCell>>,
    /// Per-line cached decorations from previous frame
    pub(crate) line_decorations: std::collections::HashMap<i32, Vec<TextDecoration>>,
}

/// Render state (dirty tracking, frame count, visibility)
///
/// Groups state related to rendering decisions and caching.
#[derive(Default)]
pub struct RenderState {
    /// Whether the window needs redrawing
    pub dirty: bool,
    /// Frame counter for periodic operations
    pub frame_count: u32,
    /// Window is occluded (hidden, minimized, or fully covered)
    pub occluded: bool,
    /// Window has keyboard focus (effects animate only when focused)
    pub focused: bool,
    /// Cached decorations from last content update
    pub cached: CachedRenderState,
    /// Paste operation just occurred - normalize INVERSE flags on next render
    pub paste_pending: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crt_core::NamedColor;

    #[test]
    fn test_prepare_render_cells_smoke() {
        // Create some collected cells
        let cells = vec![
            CollectedCell {
                col: 0,
                grid_line: 0,
                c: 'H',
                flags: CellFlags::empty(),
                fg: AnsiColor::Named(NamedColor::Foreground),
                bg: AnsiColor::Named(NamedColor::Background),
            },
            CollectedCell {
                col: 1,
                grid_line: 0,
                c: 'i',
                flags: CellFlags::BOLD,
                fg: AnsiColor::Named(NamedColor::Green),
                bg: AnsiColor::Named(NamedColor::Background),
            },
            CollectedCell {
                col: 2,
                grid_line: 0,
                c: ' ', // Space — should be skipped in prepared cells
                flags: CellFlags::empty(),
                fg: AnsiColor::Named(NamedColor::Foreground),
                bg: AnsiColor::Named(NamedColor::Background),
            },
            CollectedCell {
                col: 3,
                grid_line: 0,
                c: '!',
                flags: CellFlags::UNDERLINE | CellFlags::DIM,
                fg: AnsiColor::Named(NamedColor::Foreground),
                bg: AnsiColor::Named(NamedColor::Background),
            },
        ];

        let palette = AnsiPalette::default();
        let default_fg = [1.0, 1.0, 1.0, 1.0];
        let default_bg = [0.0, 0.0, 0.0, 1.0];

        let ctx = RenderContext {
            layout: RenderLayout {
                offset_x: 0.0,
                offset_y: 0.0,
                padding: 10.0,
                cell_width: 8.0,
                line_height: 16.0,
            },
            display_offset: 0,
            cursor_viewport_line: 0,
            palette: &palette,
            default_fg,
            default_bg,
            hovered_url_index: None,
            detected_urls: &[],
            search_active: false,
            search_matches: &[],
            current_match: 0,
            highlight_style: None,
            has_semantic_zones: false,
            get_line_zone: Box::new(|_| SemanticZone::Unknown),
        };

        let (prepared, decorations) = prepare_render_cells(&cells, &ctx);

        // Space should be skipped
        assert_eq!(prepared.len(), 3);
        assert_eq!(prepared[0].character, 'H');
        assert_eq!(prepared[1].character, 'i');
        assert!(prepared[1].bold);
        assert_eq!(prepared[2].character, '!');

        // DIM cell should have reduced alpha
        assert!(prepared[2].fg_color[3] < 1.0);

        // Underline decoration for '!' cell
        let underlines: Vec<_> = decorations
            .iter()
            .filter(|d| d.kind == DecorationKind::Underline)
            .collect();
        assert_eq!(underlines.len(), 1);

        // Pixel positions should be correct
        assert_eq!(prepared[0].x, 10.0); // padding + 0 * cell_width
        assert_eq!(prepared[0].y, 10.0); // padding + 0 * line_height
        assert_eq!(prepared[1].x, 18.0); // padding + 1 * cell_width
    }
}
