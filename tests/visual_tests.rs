//! Visual regression tests for the rendering pipeline.
//!
//! These tests render terminal content via the headless GPU renderer and
//! compare output against golden reference images.
//!
//! # Usage
//!
//! ```sh
//! # Run visual tests
//! cargo test --test visual_tests
//!
//! # Update golden files (after verifying output is correct)
//! UPDATE_GOLDEN=1 cargo test --test visual_tests
//! ```

use crt_renderer::golden;
use crt_renderer::headless::HeadlessRenderer;
use crt_renderer::{
    BackgroundPipeline, CrtPipeline, CrtUniforms, GlyphCache, GlyphStyle, GridRenderer,
    RectRenderer,
};
use crt_theme::CrtEffect;

/// Project root for resolving golden/failure paths.
fn project_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Load a monospace font for testing. Tries common system fonts.
fn load_test_font() -> Vec<u8> {
    use fontdb::{Database, Family, Query, Style, Weight};

    let mut db = Database::new();
    db.load_system_fonts();

    let families = [
        "MesloLGS NF",
        "Menlo",
        "Monaco",
        "Consolas",
        "DejaVu Sans Mono",
        "Liberation Mono",
        "Courier New",
    ];

    for family in &families {
        let query = Query {
            families: &[Family::Name(family)],
            weight: Weight::NORMAL,
            style: Style::Normal,
            ..Default::default()
        };
        if let Some(face_id) = db.query(&query) {
            if let Some(face) = db.face(face_id) {
                let data = match &face.source {
                    fontdb::Source::File(path) => std::fs::read(path).ok(),
                    fontdb::Source::Binary(data) => Some(data.as_ref().as_ref().to_vec()),
                    fontdb::Source::SharedFile(_path, data) => {
                        Some(data.as_ref().as_ref().to_vec())
                    }
                };
                if let Some(font_data) = data {
                    eprintln!("Visual tests using font: {family}");
                    return font_data;
                }
            }
        }
    }

    panic!("No suitable monospace font found for visual tests");
}

/// Test rendering context: headless renderer + glyph cache + renderers.
struct VisualTestContext {
    headless: HeadlessRenderer,
    glyph_cache: GlyphCache,
    grid_renderer: GridRenderer,
    rect_renderer: RectRenderer,
    grid_instance_buffer: wgpu::Buffer,
    rect_instance_buffer: wgpu::Buffer,
}

impl VisualTestContext {
    fn new(width: u32, height: u32) -> Option<Self> {
        let headless = HeadlessRenderer::new(width, height)
            .or_else(|_| HeadlessRenderer::with_options(width, height, false))
            .ok()?;

        let device = headless.device();
        let format = headless.format();
        let font_data = load_test_font();

        let mut glyph_cache = GlyphCache::new(device, &font_data, 14.0)
            .expect("Failed to create glyph cache");
        glyph_cache.precache_ascii();
        glyph_cache.flush(headless.queue());

        let mut grid_renderer = GridRenderer::new(device, format);
        grid_renderer.set_glyph_cache(device, &glyph_cache);
        grid_renderer.update_screen_size(headless.queue(), width as f32, height as f32);

        let mut rect_renderer = RectRenderer::new(device, format);
        rect_renderer.update_screen_size(headless.queue(), width as f32, height as f32);

        let grid_instance_buffer = GridRenderer::create_instance_buffer(device);
        let rect_instance_buffer = RectRenderer::create_instance_buffer(device);

        Some(Self {
            headless,
            glyph_cache,
            grid_renderer,
            rect_renderer,
            grid_instance_buffer,
            rect_instance_buffer,
        })
    }

    /// Render text at a grid position (col, row) with the given color.
    fn draw_text(&mut self, text: &str, col: usize, row: usize, color: [f32; 4]) {
        self.draw_text_styled(text, col, row, color, GlyphStyle::default());
    }

    /// Render styled text at a grid position.
    fn draw_text_styled(
        &mut self,
        text: &str,
        col: usize,
        row: usize,
        color: [f32; 4],
        style: GlyphStyle,
    ) {
        let cell_width = self.glyph_cache.cell_width();
        let line_height = self.glyph_cache.line_height();

        for (i, ch) in text.chars().enumerate() {
            if ch == ' ' {
                continue;
            }
            let x = (col + i) as f32 * cell_width;
            let y = row as f32 * line_height;
            if let Some(glyph) = self.glyph_cache.position_char_styled(ch, x, y, style) {
                self.grid_renderer.push_glyphs(&[glyph], color);
            }
        }
    }

    /// Draw a filled rectangle at pixel coordinates.
    fn draw_rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: [f32; 4]) {
        self.rect_renderer.push_rect(x, y, w, h, color);
    }

    /// Draw a cell background at grid position.
    fn draw_cell_bg(&mut self, col: usize, row: usize, width: usize, color: [f32; 4]) {
        let cell_width = self.glyph_cache.cell_width();
        let line_height = self.glyph_cache.line_height();
        let x = col as f32 * cell_width;
        let y = row as f32 * line_height;
        self.rect_renderer
            .push_rect(x, y, width as f32 * cell_width, line_height, color);
    }

    /// Execute render passes and capture as PNG.
    fn render_and_capture(&mut self) -> Vec<u8> {
        self.glyph_cache.flush(self.headless.queue());

        let mut encoder = self
            .headless
            .device()
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Visual Test Encoder"),
            });

        // Pass 1: Clear to dark background
        {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Clear Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: self.headless.texture_view(),
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.1,
                            b: 0.12,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
        }

        // Pass 2: Render cell backgrounds
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Rect Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: self.headless.texture_view(),
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            self.rect_renderer.render(
                self.headless.queue(),
                &mut pass,
                &self.rect_instance_buffer,
            );
        }

        // Pass 3: Render text glyphs
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Grid Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: self.headless.texture_view(),
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            self.grid_renderer.render(
                self.headless.queue(),
                &mut pass,
                &self.grid_instance_buffer,
            );
        }

        self.headless.queue().submit(std::iter::once(encoder.finish()));
        self.headless.capture_png().expect("PNG capture failed")
    }

    fn cell_width(&self) -> f32 {
        self.glyph_cache.cell_width()
    }

    fn line_height(&self) -> f32 {
        self.glyph_cache.line_height()
    }
}

/// Assert that a rendered PNG matches its golden file.
fn assert_golden(actual_png: &[u8], test_name: &str) {
    golden::assert_visual_match(actual_png, test_name, &project_root(), None);
}

fn assert_golden_with_tolerance(actual_png: &[u8], test_name: &str, tolerance: f64) {
    golden::assert_visual_match(actual_png, test_name, &project_root(), Some(tolerance));
}

// ============================================================
// Text rendering tests
// ============================================================

/// Basic ASCII text rendering - white text on dark background.
#[test]
fn visual_text_basic_ascii() {
    let mut ctx = match VisualTestContext::new(320, 200) {
        Some(c) => c,
        None => {
            eprintln!("Skipping: no GPU adapter");
            return;
        }
    };

    let white = [1.0, 1.0, 1.0, 1.0];
    ctx.draw_text("Hello, World!", 0, 0, white);
    ctx.draw_text("The quick brown fox jumps", 0, 1, white);
    ctx.draw_text("over the lazy dog.", 0, 2, white);
    ctx.draw_text("0123456789 !@#$%^&*()", 0, 4, white);

    let png = ctx.render_and_capture();
    assert_golden(&png, "text_basic_ascii");
}

/// Bold and italic text styles.
#[test]
fn visual_text_styles() {
    let mut ctx = match VisualTestContext::new(320, 200) {
        Some(c) => c,
        None => {
            eprintln!("Skipping: no GPU adapter");
            return;
        }
    };

    let white = [1.0, 1.0, 1.0, 1.0];
    ctx.draw_text("Regular text", 0, 0, white);
    ctx.draw_text_styled(
        "Bold text",
        0,
        1,
        white,
        GlyphStyle::new(true, false),
    );
    ctx.draw_text_styled(
        "Italic text",
        0,
        2,
        white,
        GlyphStyle::new(false, true),
    );
    ctx.draw_text_styled(
        "Bold Italic",
        0,
        3,
        white,
        GlyphStyle::new(true, true),
    );

    let png = ctx.render_and_capture();
    assert_golden(&png, "text_styles");
}

/// ANSI 16-color text rendering.
#[test]
fn visual_text_ansi_colors() {
    let mut ctx = match VisualTestContext::new(320, 200) {
        Some(c) => c,
        None => {
            eprintln!("Skipping: no GPU adapter");
            return;
        }
    };

    // Standard ANSI colors (approximate sRGB linear values)
    let colors: &[([f32; 4], &str)] = &[
        ([0.0, 0.0, 0.0, 1.0], "Black"),
        ([0.8, 0.0, 0.0, 1.0], "Red"),
        ([0.0, 0.8, 0.0, 1.0], "Green"),
        ([0.8, 0.8, 0.0, 1.0], "Yellow"),
        ([0.0, 0.0, 0.8, 1.0], "Blue"),
        ([0.8, 0.0, 0.8, 1.0], "Magenta"),
        ([0.0, 0.8, 0.8, 1.0], "Cyan"),
        ([0.9, 0.9, 0.9, 1.0], "White"),
    ];

    for (i, (color, label)) in colors.iter().enumerate() {
        ctx.draw_text(label, 0, i, *color);
    }

    let png = ctx.render_and_capture();
    assert_golden(&png, "text_ansi_colors");
}

/// Unicode text rendering: box-drawing, CJK, emoji.
#[test]
fn visual_text_unicode() {
    let mut ctx = match VisualTestContext::new(400, 200) {
        Some(c) => c,
        None => {
            eprintln!("Skipping: no GPU adapter");
            return;
        }
    };

    let white = [1.0, 1.0, 1.0, 1.0];
    let green = [0.0, 1.0, 0.0, 1.0];

    // Box-drawing characters
    ctx.draw_text("┌──────────┐", 0, 0, green);
    ctx.draw_text("│  Box Art │", 0, 1, green);
    ctx.draw_text("└──────────┘", 0, 2, green);

    // Accented characters
    ctx.draw_text("Héllo Wörld Ñoño", 0, 4, white);

    // Arrows and symbols
    ctx.draw_text("← → ↑ ↓ ■ □ ● ○", 0, 6, white);

    let png = ctx.render_and_capture();
    // Higher tolerance for Unicode since glyph availability varies
    assert_golden_with_tolerance(&png, "text_unicode", 2.0);
}

// ============================================================
// Cursor rendering tests
// ============================================================

/// Block cursor rendering.
#[test]
fn visual_cursor_block() {
    let mut ctx = match VisualTestContext::new(320, 120) {
        Some(c) => c,
        None => {
            eprintln!("Skipping: no GPU adapter");
            return;
        }
    };

    let white = [1.0, 1.0, 1.0, 1.0];
    let cursor_green = [0.0, 1.0, 0.25, 0.8];

    ctx.draw_text("$ echo hello", 0, 0, white);

    // Block cursor at position (12, 0) - after the text
    let cw = ctx.cell_width();
    let lh = ctx.line_height();
    ctx.draw_rect(12.0 * cw, 0.0, cw, lh, cursor_green);

    let png = ctx.render_and_capture();
    assert_golden(&png, "cursor_block");
}

/// Beam (bar) cursor rendering.
#[test]
fn visual_cursor_beam() {
    let mut ctx = match VisualTestContext::new(320, 120) {
        Some(c) => c,
        None => {
            eprintln!("Skipping: no GPU adapter");
            return;
        }
    };

    let white = [1.0, 1.0, 1.0, 1.0];
    let cursor_green = [0.0, 1.0, 0.25, 1.0];

    ctx.draw_text("$ echo hello", 0, 0, white);

    // Beam cursor: thin vertical bar at column 12
    let cw = ctx.cell_width();
    let lh = ctx.line_height();
    ctx.draw_rect(12.0 * cw, 0.0, 2.0, lh, cursor_green);

    let png = ctx.render_and_capture();
    assert_golden(&png, "cursor_beam");
}

/// Underline cursor rendering.
#[test]
fn visual_cursor_underline() {
    let mut ctx = match VisualTestContext::new(320, 120) {
        Some(c) => c,
        None => {
            eprintln!("Skipping: no GPU adapter");
            return;
        }
    };

    let white = [1.0, 1.0, 1.0, 1.0];
    let cursor_green = [0.0, 1.0, 0.25, 1.0];

    ctx.draw_text("$ echo hello", 0, 0, white);

    // Underline cursor at column 12
    let cw = ctx.cell_width();
    let lh = ctx.line_height();
    ctx.draw_rect(12.0 * cw, lh - 2.0, cw, 2.0, cursor_green);

    let png = ctx.render_and_capture();
    assert_golden(&png, "cursor_underline");
}

// ============================================================
// Selection rendering tests
// ============================================================

/// Single-line selection highlight.
#[test]
fn visual_selection_single_line() {
    let mut ctx = match VisualTestContext::new(320, 120) {
        Some(c) => c,
        None => {
            eprintln!("Skipping: no GPU adapter");
            return;
        }
    };

    let white = [1.0, 1.0, 1.0, 1.0];
    let sel_bg = [0.0, 1.0, 0.25, 0.3]; // Semi-transparent green

    ctx.draw_text("Select some text here", 0, 0, white);

    // Selection background on "some text" (columns 7-15)
    ctx.draw_cell_bg(7, 0, 9, sel_bg);

    let png = ctx.render_and_capture();
    assert_golden(&png, "selection_single_line");
}

/// Multi-line selection highlight.
#[test]
fn visual_selection_multi_line() {
    let mut ctx = match VisualTestContext::new(320, 160) {
        Some(c) => c,
        None => {
            eprintln!("Skipping: no GPU adapter");
            return;
        }
    };

    let white = [1.0, 1.0, 1.0, 1.0];
    let sel_bg = [0.0, 1.0, 0.25, 0.3];

    ctx.draw_text("First line of text", 0, 0, white);
    ctx.draw_text("Second line of text", 0, 1, white);
    ctx.draw_text("Third line of text", 0, 2, white);

    // Selection from col 6 on line 0 through col 10 on line 2
    ctx.draw_cell_bg(6, 0, 12, sel_bg); // Rest of line 0
    ctx.draw_cell_bg(0, 1, 19, sel_bg); // All of line 1
    ctx.draw_cell_bg(0, 2, 10, sel_bg); // Start of line 2

    let png = ctx.render_and_capture();
    assert_golden(&png, "selection_multi_line");
}

// ============================================================
// Tab bar rendering tests
// ============================================================

/// Single tab display.
#[test]
fn visual_tab_single() {
    let mut ctx = match VisualTestContext::new(400, 80) {
        Some(c) => c,
        None => {
            eprintln!("Skipping: no GPU adapter");
            return;
        }
    };

    let tab_bg = [0.15, 0.15, 0.2, 1.0];
    let active_bg = [0.2, 0.2, 0.3, 1.0];
    let white = [1.0, 1.0, 1.0, 1.0];
    let accent = [0.0, 1.0, 0.25, 1.0];

    // Tab bar background
    let lh = ctx.line_height();
    ctx.draw_rect(0.0, 0.0, 400.0, lh + 4.0, tab_bg);

    // Active tab background
    ctx.draw_rect(0.0, 0.0, 120.0, lh + 4.0, active_bg);

    // Active indicator (green underline)
    ctx.draw_rect(0.0, lh + 2.0, 120.0, 2.0, accent);

    // Tab title
    ctx.draw_text("~/projects/crt", 1, 0, white);

    let png = ctx.render_and_capture();
    assert_golden(&png, "tab_single");
}

/// Multiple tabs with active indicator.
#[test]
fn visual_tab_multiple() {
    let mut ctx = match VisualTestContext::new(500, 80) {
        Some(c) => c,
        None => {
            eprintln!("Skipping: no GPU adapter");
            return;
        }
    };

    let tab_bg = [0.15, 0.15, 0.2, 1.0];
    let active_bg = [0.2, 0.2, 0.3, 1.0];
    let white = [1.0, 1.0, 1.0, 1.0];
    let dim = [0.6, 0.6, 0.6, 1.0];
    let accent = [0.0, 1.0, 0.25, 1.0];

    let lh = ctx.line_height();
    let cw = ctx.cell_width();
    let tab_width = 15.0 * cw; // ~15 chars per tab

    // Tab bar background
    ctx.draw_rect(0.0, 0.0, 500.0, lh + 4.0, tab_bg);

    // Tab 1 (inactive)
    ctx.draw_text("~/projects", 1, 0, dim);

    // Tab 2 (active)
    ctx.draw_rect(tab_width, 0.0, tab_width, lh + 4.0, active_bg);
    ctx.draw_rect(tab_width, lh + 2.0, tab_width, 2.0, accent);
    ctx.draw_text("~/documents", 16, 0, white);

    // Tab 3 (inactive)
    ctx.draw_text("~/downloads", 31, 0, dim);

    let png = ctx.render_and_capture();
    assert_golden(&png, "tab_multiple");
}

/// Long tab title truncation.
#[test]
fn visual_tab_truncation() {
    let mut ctx = match VisualTestContext::new(320, 80) {
        Some(c) => c,
        None => {
            eprintln!("Skipping: no GPU adapter");
            return;
        }
    };

    let tab_bg = [0.15, 0.15, 0.2, 1.0];
    let active_bg = [0.2, 0.2, 0.3, 1.0];
    let white = [1.0, 1.0, 1.0, 1.0];
    let accent = [0.0, 1.0, 0.25, 1.0];

    let lh = ctx.line_height();

    // Tab bar background
    ctx.draw_rect(0.0, 0.0, 320.0, lh + 4.0, tab_bg);
    ctx.draw_rect(0.0, 0.0, 200.0, lh + 4.0, active_bg);
    ctx.draw_rect(0.0, lh + 2.0, 200.0, 2.0, accent);

    // Long title (truncated visually)
    ctx.draw_text("~/very/long/path/to/s...", 1, 0, white);

    let png = ctx.render_and_capture();
    assert_golden(&png, "tab_truncation");
}

// ============================================================
// CRT effect tests
// ============================================================

/// Helper: Render text content to a source texture, then apply CRT post-processing
/// to the headless render target, and capture as PNG.
fn render_with_crt_effect(width: u32, height: u32, crt_uniforms: CrtUniforms) -> Option<Vec<u8>> {
    let headless = HeadlessRenderer::new(width, height)
        .or_else(|_| HeadlessRenderer::with_options(width, height, false))
        .ok()?;

    let device = headless.device();
    let queue = headless.queue();
    let format = headless.format();
    let font_data = load_test_font();

    // Set up text rendering
    let mut glyph_cache = GlyphCache::new(device, &font_data, 14.0).ok()?;
    glyph_cache.precache_ascii();
    glyph_cache.flush(queue);

    let mut grid_renderer = GridRenderer::new(device, format);
    grid_renderer.set_glyph_cache(device, &glyph_cache);
    grid_renderer.update_screen_size(queue, width as f32, height as f32);

    let grid_instance_buffer = GridRenderer::create_instance_buffer(device);

    // Create intermediate texture for text content (CRT reads from this)
    let source_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("CRT Source Texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let source_view = source_texture.create_view(&wgpu::TextureViewDescriptor::default());

    // Render text content
    let white = [1.0, 1.0, 1.0, 1.0];
    let green = [0.0, 1.0, 0.25, 1.0];
    let cell_width = glyph_cache.cell_width();
    let line_height = glyph_cache.line_height();

    let text_lines = [
        ("$ ls -la", green),
        ("total 42", white),
        ("drwxr-xr-x  5 user staff  160 Mar 11 19:00 .", white),
        ("-rw-r--r--  1 user staff 1234 Mar 11 18:30 Cargo.toml", white),
        ("-rw-r--r--  1 user staff 5678 Mar 11 18:30 README.md", white),
    ];

    for (row, (text, color)) in text_lines.iter().enumerate() {
        for (i, ch) in text.chars().enumerate() {
            if ch == ' ' {
                continue;
            }
            let x = i as f32 * cell_width;
            let y = row as f32 * line_height;
            if let Some(glyph) = glyph_cache.position_char(ch, x, y) {
                grid_renderer.push_glyphs(&[glyph], *color);
            }
        }
    }
    glyph_cache.flush(queue);

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("CRT Effect Test Encoder"),
    });

    // Pass 1: Render text to source texture
    {
        let _clear = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Clear Source"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &source_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.04,
                        g: 0.04,
                        b: 0.08,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
    }
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Text Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &source_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        grid_renderer.render(queue, &mut pass, &grid_instance_buffer);
    }

    // Pass 2: Apply CRT post-processing to headless render target
    let mut crt_pipeline = CrtPipeline::new(device, format);
    // Enable with specific parameters
    crt_pipeline.set_effect(Some(CrtEffect {
        enabled: true,
        scanline_intensity: crt_uniforms.scanline_intensity,
        scanline_frequency: crt_uniforms.scanline_frequency,
        curvature: crt_uniforms.curvature,
        vignette: crt_uniforms.vignette,
        chromatic_aberration: crt_uniforms.chromatic_aberration,
        bloom: crt_uniforms.bloom,
        flicker: crt_uniforms.flicker,
    }));
    // Update uniforms (time will be ~0ms since pipeline was just created)
    crt_pipeline.update_uniforms(queue, width as f32, height as f32);
    let bind_group = crt_pipeline.create_bind_group(device, &source_view);
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("CRT Effect Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: headless.texture_view(),
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        crt_pipeline.render(&mut pass, &bind_group);
    }

    queue.submit(std::iter::once(encoder.finish()));
    headless.capture_png().ok()
}

/// CRT scanline effect at different intensities.
#[test]
fn visual_crt_scanlines() {
    let uniforms = CrtUniforms {
        screen_size: [320.0, 240.0],
        time: 0.0,
        scanline_intensity: 0.5,
        scanline_frequency: 2.0,
        curvature: 0.0,
        vignette: 0.0,
        chromatic_aberration: 0.0,
        bloom: 0.0,
        flicker: 0.0,
        reference_height: 1080.0,
        _pad: [0.0; 5],
    };
    let png = match render_with_crt_effect(320, 240, uniforms) {
        Some(p) => p,
        None => {
            eprintln!("Skipping: no GPU adapter");
            return;
        }
    };
    assert_golden_with_tolerance(&png, "crt_scanlines", 1.0);
}

/// CRT screen curvature effect.
#[test]
fn visual_crt_curvature() {
    let uniforms = CrtUniforms {
        screen_size: [320.0, 240.0],
        time: 0.0,
        scanline_intensity: 0.0,
        scanline_frequency: 2.0,
        curvature: 0.05,
        vignette: 0.0,
        chromatic_aberration: 0.0,
        bloom: 0.0,
        flicker: 0.0,
        reference_height: 1080.0,
        _pad: [0.0; 5],
    };
    let png = match render_with_crt_effect(320, 240, uniforms) {
        Some(p) => p,
        None => {
            eprintln!("Skipping: no GPU adapter");
            return;
        }
    };
    assert_golden_with_tolerance(&png, "crt_curvature", 1.0);
}

/// CRT vignette effect.
#[test]
fn visual_crt_vignette() {
    let uniforms = CrtUniforms {
        screen_size: [320.0, 240.0],
        time: 0.0,
        scanline_intensity: 0.0,
        scanline_frequency: 2.0,
        curvature: 0.0,
        vignette: 0.5,
        chromatic_aberration: 0.0,
        bloom: 0.0,
        flicker: 0.0,
        reference_height: 1080.0,
        _pad: [0.0; 5],
    };
    let png = match render_with_crt_effect(320, 240, uniforms) {
        Some(p) => p,
        None => {
            eprintln!("Skipping: no GPU adapter");
            return;
        }
    };
    assert_golden_with_tolerance(&png, "crt_vignette", 1.0);
}

/// CRT glow/bloom on text.
#[test]
fn visual_crt_glow() {
    let uniforms = CrtUniforms {
        screen_size: [320.0, 240.0],
        time: 0.0,
        scanline_intensity: 0.0,
        scanline_frequency: 2.0,
        curvature: 0.0,
        vignette: 0.0,
        chromatic_aberration: 0.0,
        bloom: 0.6,
        flicker: 0.0,
        reference_height: 1080.0,
        _pad: [0.0; 5],
    };
    let png = match render_with_crt_effect(320, 240, uniforms) {
        Some(p) => p,
        None => {
            eprintln!("Skipping: no GPU adapter");
            return;
        }
    };
    assert_golden_with_tolerance(&png, "crt_glow", 1.0);
}

// ============================================================
// Theme rendering tests
// ============================================================

/// Default theme: dark background with standard terminal colors.
#[test]
fn visual_theme_default() {
    let mut ctx = match VisualTestContext::new(320, 200) {
        Some(c) => c,
        None => {
            eprintln!("Skipping: no GPU adapter");
            return;
        }
    };

    // Default theme colors
    let fg = [0.878, 0.878, 0.878, 1.0]; // #e0e0e0
    let green = [0.0, 1.0, 0.255, 1.0]; // #00ff41
    let red = [1.0, 0.0, 0.333, 1.0]; // #ff0055
    let blue = [0.0, 0.749, 1.0, 1.0]; // #00bfff
    let yellow = [0.941, 0.902, 0.549, 1.0]; // #f0e68c

    ctx.draw_text("$ git status", 0, 0, green);
    ctx.draw_text("On branch main", 0, 1, fg);
    ctx.draw_text("Changes not staged:", 0, 2, fg);
    ctx.draw_text("  modified:  src/main.rs", 0, 3, red);
    ctx.draw_text("  modified:  src/lib.rs", 0, 4, red);
    ctx.draw_text("Untracked files:", 0, 6, fg);
    ctx.draw_text("  tests/new_test.rs", 0, 7, blue);
    ctx.draw_text("2 files changed", 0, 9, yellow);

    let png = ctx.render_and_capture();
    assert_golden(&png, "theme_default");
}

/// Custom theme with overridden colors.
#[test]
fn visual_theme_custom_colors() {
    let mut ctx = match VisualTestContext::new(320, 200) {
        Some(c) => c,
        None => {
            eprintln!("Skipping: no GPU adapter");
            return;
        }
    };

    // Custom warm theme colors
    let fg = [1.0, 0.96, 0.9, 1.0]; // Warm white
    let orange = [1.0, 0.6, 0.2, 1.0]; // Orange prompt
    let cyan = [0.4, 0.9, 0.9, 1.0]; // Teal
    let pink = [1.0, 0.4, 0.6, 1.0]; // Pink
    let dark_bg = [0.12, 0.1, 0.14, 1.0]; // Dark purple-ish background

    // Dark background
    ctx.draw_rect(0.0, 0.0, 320.0, 200.0, dark_bg);
    ctx.draw_text("~/projects $ ls", 0, 0, orange);
    ctx.draw_text("Cargo.toml  README.md", 0, 1, fg);
    ctx.draw_text("src/        tests/", 0, 2, cyan);
    ctx.draw_text("Error: file not found", 0, 4, pink);

    let png = ctx.render_and_capture();
    assert_golden(&png, "theme_custom_colors");
}

/// Background gradient rendering test.
#[test]
fn visual_theme_background_gradient() {
    let headless = match HeadlessRenderer::new(320, 240)
        .or_else(|_| HeadlessRenderer::with_options(320, 240, false))
    {
        Ok(r) => r,
        Err(_) => {
            eprintln!("Skipping: no GPU adapter");
            return;
        }
    };

    let device = headless.device();
    let queue = headless.queue();
    let format = headless.format();

    // Create and render the background pipeline (gradient + animated grid)
    let bg_pipeline = BackgroundPipeline::new(device, format);
    bg_pipeline.update_uniforms(queue, 320.0, 240.0);

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Background Gradient Test"),
    });

    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Background Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: headless.texture_view(),
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        bg_pipeline.render(&mut pass);
    }

    queue.submit(std::iter::once(encoder.finish()));
    let png = headless.capture_png().expect("PNG capture failed");

    // Background gradient is time-dependent, use higher tolerance
    assert_golden_with_tolerance(&png, "theme_background_gradient", 2.0);
}
