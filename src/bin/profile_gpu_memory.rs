//! GPU memory profiling tool.
//!
//! Exercises the full rendering pipeline via headless GPU rendering and
//! reports GPU-side resource usage (glyph atlas, instance buffers, textures).
//!
//! Unlike `profile_memory` (DHAT-based, CPU heap only), this tool tracks
//! GPU allocations that are invisible to system allocators: textures, vertex
//! buffers, glyph atlas, staging buffers, and pipeline state.
//!
//! # Usage
//!
//! ```sh
//! cargo run --release --bin profile_gpu_memory
//! ```

use std::io::Write;
use std::time::Instant;

use crt_renderer::{
    CrtPipeline, GlyphCache, GlyphStyle, GridRenderer, RectRenderer,
};

fn main() {
    env_logger::init();

    println!("CRT GPU Memory Profiler");
    println!("=======================\n");

    // ── Create headless GPU context ──────────────────────────────────
    let width: u32 = 1920;
    let height: u32 = 1080;

    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let adapter = pollster::block_on(async {
        instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
    })
    .expect("No GPU adapter found");

    println!(
        "GPU: {} ({:?})",
        adapter.get_info().name,
        adapter.get_info().backend,
    );

    let (device, queue): (wgpu::Device, wgpu::Queue) = pollster::block_on(async {
        adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
    })
    .expect("Failed to create device");

    let format = wgpu::TextureFormat::Rgba8UnormSrgb;

    // ── Track baseline GPU allocations ───────────────────────────────
    println!("\n--- Baseline GPU Allocations ---\n");

    let mut gpu_bytes: u64 = 0;

    // Render target texture
    let render_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Render Target"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let render_view = render_texture.create_view(&Default::default());
    let tex_bytes = (width as u64) * (height as u64) * 4;
    gpu_bytes += tex_bytes;
    println!(
        "  Render target ({width}x{height} RGBA8):  {:>8.2} MB",
        tex_bytes as f64 / 1024.0 / 1024.0
    );

    // Staging buffer (for readback)
    let padded_row = ((width * 4 + 255) / 256) * 256;
    let staging_bytes = (padded_row as u64) * (height as u64);
    let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Staging Buffer"),
        size: staging_bytes,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    gpu_bytes += staging_bytes;
    println!(
        "  Staging buffer (readback):              {:>8.2} MB",
        staging_bytes as f64 / 1024.0 / 1024.0
    );

    // ── Load font and create glyph cache ─────────────────────────────
    let font_data = load_system_font();
    let mut glyph_cache = GlyphCache::new(&device, &font_data, 14.0)
        .expect("Failed to create glyph cache");

    let (glyph_count, utilization, atlas_w, atlas_h) = glyph_cache.atlas_stats();
    let atlas_bytes = (atlas_w as u64) * (atlas_h as u64); // R8 = 1 byte/pixel
    gpu_bytes += atlas_bytes;
    println!(
        "  Glyph atlas ({atlas_w}x{atlas_h} R8):         {:>8.2} MB  ({glyph_count} glyphs, {:.1}% utilized)",
        atlas_bytes as f64 / 1024.0 / 1024.0,
        utilization * 100.0,
    );

    // ── Create renderers ─────────────────────────────────────────────
    let mut grid_renderer = GridRenderer::new(&device, format);
    grid_renderer.set_glyph_cache(&device, &glyph_cache);
    grid_renderer.update_screen_size(&queue, width as f32, height as f32);

    let mut rect_renderer = RectRenderer::new(&device, format);
    rect_renderer.update_screen_size(&queue, width as f32, height as f32);

    // Instance buffers
    let grid_instance_buffer = GridRenderer::create_instance_buffer(&device);
    let grid_buf_bytes: u64 = 32 * 1024 * 48; // 1.5 MB
    gpu_bytes += grid_buf_bytes;
    println!(
        "  Grid instance buffer (32K×48B):         {:>8.2} MB",
        grid_buf_bytes as f64 / 1024.0 / 1024.0
    );

    let rect_instance_buffer = RectRenderer::create_instance_buffer(&device);
    let rect_buf_bytes: u64 = 16 * 1024 * 32; // 512 KB
    gpu_bytes += rect_buf_bytes;
    println!(
        "  Rect instance buffer (16K×32B):         {:>8.2} MB",
        rect_buf_bytes as f64 / 1024.0 / 1024.0
    );

    // CRT pipeline (optional post-processing)
    let crt_pipeline = CrtPipeline::new(&device, format);

    // CRT intermediate texture (same size as render target)
    let crt_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("CRT Intermediate"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let _crt_view = crt_texture.create_view(&Default::default());
    gpu_bytes += tex_bytes;
    println!(
        "  CRT intermediate texture:               {:>8.2} MB",
        tex_bytes as f64 / 1024.0 / 1024.0
    );

    // Pipeline uniform buffers (small)
    let uniform_bytes: u64 = 256 * 3; // grid, rect, CRT uniforms
    gpu_bytes += uniform_bytes;
    println!(
        "  Pipeline uniform buffers:               {:>8.2} MB",
        uniform_bytes as f64 / 1024.0 / 1024.0
    );

    println!(
        "\n  TOTAL baseline GPU memory:              {:>8.2} MB",
        gpu_bytes as f64 / 1024.0 / 1024.0
    );

    // ── Phase 1: ASCII text rendering (populates glyph atlas) ────────
    println!("\n--- Phase 1: ASCII Pre-cache ---\n");

    glyph_cache.precache_ascii();
    glyph_cache.flush(&queue);

    let (glyph_count, utilization, _, _) = glyph_cache.atlas_stats();
    println!(
        "  After ASCII precache: {glyph_count} glyphs, {:.1}% atlas utilized",
        utilization * 100.0,
    );

    // ── Phase 2: Extended character rendering ────────────────────────
    println!("\n--- Phase 2: Extended Characters ---\n");

    let extended_chars: Vec<char> = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz\
        0123456789!@#$%^&*()_+-=[]{}|;':\",./<>?`~\
        ÀÁÂÃÄÅÆÇÈÉÊËÌÍÎÏÐÑÒÓÔÕÖØÙÚÛÜÝÞßàáâãäåæçèéêëìíîïðñòóôõöøùúûüýþÿ\
        αβγδεζηθικλμνξοπρστυφχψω\
        ←↑→↓↔↕▲▼◆●■□▪▫\
        ─│┌┐└┘├┤┬┴┼═║╔╗╚╝╠╣╦╩╬\
        中文字符日本語テスト한국어"
        .chars()
        .collect();

    for &ch in &extended_chars {
        glyph_cache.get_or_insert_styled(ch, GlyphStyle::default());
    }
    // Bold variants
    for &ch in &extended_chars[..52] {
        glyph_cache.get_or_insert_styled(
            ch,
            GlyphStyle {
                bold: true,
                ..Default::default()
            },
        );
    }
    // Italic variants
    for &ch in &extended_chars[..52] {
        glyph_cache.get_or_insert_styled(
            ch,
            GlyphStyle {
                italic: true,
                ..Default::default()
            },
        );
    }
    glyph_cache.flush(&queue);

    let (glyph_count, utilization, atlas_w, atlas_h) = glyph_cache.atlas_stats();
    let atlas_bytes_now = (atlas_w as u64) * (atlas_h as u64);
    println!(
        "  After extended chars: {glyph_count} glyphs, {:.1}% atlas utilized",
        utilization * 100.0,
    );
    println!(
        "  Atlas size: {atlas_w}x{atlas_h} R8 = {:.2} MB",
        atlas_bytes_now as f64 / 1024.0 / 1024.0,
    );

    // ── Phase 3: Sustained rendering (simulates long-running session) ──
    println!("\n--- Phase 3: Sustained Rendering (1000 frames) ---\n");

    let cell_width = glyph_cache.cell_width();
    let line_height = glyph_cache.line_height();
    let cols = (width as f32 / cell_width) as usize;
    let rows = (height as f32 / line_height) as usize;

    let start = Instant::now();
    let mut total_glyphs_rendered: u64 = 0;

    for frame in 0..1000 {
        // Build frame content — simulates typical terminal output
        grid_renderer.clear();
        rect_renderer.clear();

        // Render full grid of characters
        for row in 0..rows.min(40) {
            for col in 0..cols.min(120) {
                let ch_idx = (frame + row * cols + col) % extended_chars.len();
                let ch = extended_chars[ch_idx];
                let x = col as f32 * cell_width;
                let y = row as f32 * line_height;
                if let Some(glyph) = glyph_cache.position_char(ch, x, y) {
                    let color = [
                        ((frame + col) % 256) as f32 / 255.0,
                        ((frame + row) % 256) as f32 / 255.0,
                        0.8,
                        1.0,
                    ];
                    grid_renderer.push_glyphs(&[glyph], color);
                    total_glyphs_rendered += 1;
                }
            }
        }

        // Add some cell backgrounds
        for row in 0..rows.min(40) {
            if row % 3 == 0 {
                let x = 0.0;
                let y = row as f32 * line_height;
                rect_renderer.push_rect(
                    x,
                    y,
                    cols.min(120) as f32 * cell_width,
                    line_height,
                    [0.15, 0.15, 0.2, 0.5],
                );
            }
        }

        // Submit GPU work
        glyph_cache.flush(&queue);

        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Frame Encoder"),
            });

        // Clear pass
        {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Clear"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &render_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.05,
                            g: 0.05,
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

        // Rect pass
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Rects"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &render_view,
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
            rect_renderer.render(&queue, &mut pass, &rect_instance_buffer);
        }

        // Grid (text) pass
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Grid"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &render_view,
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
            grid_renderer.render(&queue, &mut pass, &grid_instance_buffer);
        }

        queue.submit(std::iter::once(encoder.finish()));

        // Print progress
        if frame % 200 == 199 {
            let elapsed = start.elapsed();
            let fps = (frame + 1) as f64 / elapsed.as_secs_f64();
            print!("  Frame {}: {:.0} fps... ", frame + 1, fps);
            std::io::stdout().flush().ok();

            let (gc, util, _, _) = glyph_cache.atlas_stats();
            println!("atlas: {gc} glyphs, {:.1}% used", util * 100.0);
        }
    }

    let elapsed = start.elapsed();
    println!(
        "\n  Rendered 1000 frames in {:.1}s ({:.0} fps)",
        elapsed.as_secs_f64(),
        1000.0 / elapsed.as_secs_f64(),
    );
    println!("  Total glyphs submitted: {total_glyphs_rendered}");

    // ── Final GPU memory summary ─────────────────────────────────────
    println!("\n--- Final GPU Memory Summary ---\n");

    let (glyph_count, utilization, atlas_w, atlas_h) = glyph_cache.atlas_stats();
    let final_atlas_bytes = (atlas_w as u64) * (atlas_h as u64);

    // Recalculate total with actual atlas size
    let final_gpu_bytes = tex_bytes          // render target
        + staging_bytes                       // staging
        + final_atlas_bytes                   // glyph atlas
        + grid_buf_bytes                      // grid instance
        + rect_buf_bytes                      // rect instance
        + tex_bytes                           // CRT intermediate
        + uniform_bytes;                      // uniforms

    println!("  Resource                                  Size");
    println!("  ──────────────────────────────────────  ────────");
    println!(
        "  Render target ({width}x{height} RGBA8)     {:>6.2} MB",
        tex_bytes as f64 / 1024.0 / 1024.0,
    );
    println!(
        "  CRT intermediate ({width}x{height} RGBA8)  {:>6.2} MB",
        tex_bytes as f64 / 1024.0 / 1024.0,
    );
    println!(
        "  Staging buffer (readback)               {:>6.2} MB",
        staging_bytes as f64 / 1024.0 / 1024.0,
    );
    println!(
        "  Glyph atlas ({atlas_w}x{atlas_h} R8)          {:>6.2} MB  ({glyph_count} glyphs, {:.1}%)",
        final_atlas_bytes as f64 / 1024.0 / 1024.0,
        utilization * 100.0,
    );
    println!(
        "  Grid instance buffer                    {:>6.2} MB",
        grid_buf_bytes as f64 / 1024.0 / 1024.0,
    );
    println!(
        "  Rect instance buffer                    {:>6.2} MB",
        rect_buf_bytes as f64 / 1024.0 / 1024.0,
    );
    println!(
        "  Uniform buffers                         {:>6.2} MB",
        uniform_bytes as f64 / 1024.0 / 1024.0,
    );
    println!("  ──────────────────────────────────────  ────────");
    println!(
        "  TOTAL tracked GPU memory                {:>6.2} MB",
        final_gpu_bytes as f64 / 1024.0 / 1024.0,
    );

    println!("\n  Note: Actual VRAM usage is higher due to:");
    println!("    - GPU driver overhead and page alignment");
    println!("    - wgpu/Metal/Vulkan internal state objects");
    println!("    - Shader pipeline caches (~1-2 MB per pipeline)");
    println!("    - Command buffer allocation pools");
    println!("    - Vello renderer caches (if CSS effects enabled)");

    // RSS measurement
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        let pid = std::process::id();
        if let Ok(output) = Command::new("ps")
            .args(["-o", "rss=", "-p", &pid.to_string()])
            .output()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Ok(rss_kb) = stdout.trim().parse::<u64>() {
                println!(
                    "\n  Process RSS (includes GPU mapping): {:.2} MB",
                    rss_kb as f64 / 1024.0,
                );
            }
        }
    }

    // Clean up
    drop(crt_pipeline);
    drop(staging_buffer);
    drop(render_texture);
    drop(crt_texture);
    println!("\nDone.");
}

/// Load a monospace font from system fonts.
fn load_system_font() -> Vec<u8> {
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
                    println!("Font: {family}");
                    return font_data;
                }
            }
        }
    }

    panic!("No suitable monospace font found");
}
