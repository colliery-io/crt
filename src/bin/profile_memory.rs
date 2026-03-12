//! Memory profiling via DHAT (heap allocation profiler).
//!
//! Exercises the terminal hot paths and produces a DHAT allocation profile.
//!
//! # Usage
//!
//! ```sh
//! cargo run --release --features dhat-heap --bin profile_memory
//! ```
//!
//! This produces `dhat-heap.json` in the current directory.
//! Open it with the DHAT viewer: https://nnethercote.github.io/dh_view/dh_view.html

#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

use std::time::Instant;

use crt_core::{Size, Terminal};

/// Simulate realistic terminal workload:
/// - Scrolling output (log-like)
/// - Rapid in-place updates (htop-like)
/// - Color-heavy output
/// - Unicode / wide characters
fn main() {
    #[cfg(feature = "dhat-heap")]
    let _profiler = dhat::Profiler::new_heap();

    #[cfg(not(feature = "dhat-heap"))]
    {
        eprintln!("ERROR: Run with --features dhat-heap to enable heap profiling.");
        eprintln!("  cargo run --release --features dhat-heap --bin profile_memory");
        std::process::exit(1);
    }

    println!("Memory profiling: exercising terminal hot paths...\n");

    let mut terminal = Terminal::new(Size::new(120, 40));

    // Phase 1: Scrolling output (5000 lines)
    print!("Phase 1: Scrolling output (5000 lines)... ");
    let start = Instant::now();
    for i in 0..5000 {
        let line = format!(
            "[{:08}] INFO  server::handler - Request processed in {}ms, status=200, path=/api/v1/data\r\n",
            i,
            i % 500
        );
        terminal.process_input(line.as_bytes());
    }
    println!("{:.1}ms", start.elapsed().as_secs_f64() * 1000.0);

    // Read content (simulates render path)
    let _ = terminal.renderable_content();
    let _ = terminal.all_lines_text();

    // Phase 2: Rapid in-place updates (2000 frames of htop-like content)
    print!("Phase 2: Rapid in-place updates (2000 frames)... ");
    let start = Instant::now();
    for frame in 0..2000 {
        let mut buf = Vec::with_capacity(2048);
        for row in 0..24 {
            buf.extend_from_slice(format!("\x1b[{};1H", row + 1).as_bytes());
            buf.extend_from_slice(
                format!(
                    "\x1b[32mCPU{:02}\x1b[0m: \x1b[1;33m{:5.1}%\x1b[0m | \x1b[34mMEM\x1b[0m: {:4} MB",
                    row,
                    (frame + row as u64) as f64 % 100.0,
                    1024 + (frame % 512)
                )
                .as_bytes(),
            );
        }
        terminal.process_input(&buf);

        // Simulate render every 16ms worth of frames
        if frame % 3 == 0 {
            let _ = terminal.renderable_content();
            terminal.reset_damage();
        }
    }
    println!("{:.1}ms", start.elapsed().as_secs_f64() * 1000.0);

    // Phase 3: Color stress (256-color cycling)
    print!("Phase 3: Color stress (1000 lines, 256 colors)... ");
    let start = Instant::now();
    for i in 0..1000 {
        let mut buf = Vec::with_capacity(2048);
        for c in 0..80 {
            let color = ((i + c) % 256) as u8;
            buf.extend_from_slice(format!("\x1b[38;5;{}m#", color).as_bytes());
        }
        buf.extend_from_slice(b"\x1b[0m\r\n");
        terminal.process_input(&buf);
    }
    println!("{:.1}ms", start.elapsed().as_secs_f64() * 1000.0);

    // Phase 4: Unicode and wide characters
    print!("Phase 4: Unicode/CJK (500 lines)... ");
    let start = Instant::now();
    let cjk_chars = ['中', '文', '字', '符', '日', '本', '語', 'テ', 'ス', 'ト'];
    for i in 0..500 {
        let mut line = String::with_capacity(120);
        for j in 0..40 {
            line.push(cjk_chars[(i + j) % cjk_chars.len()]);
        }
        line.push_str("\r\n");
        terminal.process_input(line.as_bytes());
    }
    println!("{:.1}ms", start.elapsed().as_secs_f64() * 1000.0);

    // Phase 5: Damage tracking cycle
    print!("Phase 5: Damage tracking (1000 cycles)... ");
    let start = Instant::now();
    for _ in 0..1000 {
        terminal.process_input(b"x");
        let _ = terminal.has_damage();
        let _ = terminal.damaged_line_set();
        terminal.reset_damage();
    }
    println!("{:.1}ms", start.elapsed().as_secs_f64() * 1000.0);

    // Final content extraction (exercises all_lines_text allocation)
    print!("Phase 6: Content extraction... ");
    let start = Instant::now();
    for _ in 0..100 {
        let _ = terminal.all_lines_text();
    }
    println!("{:.1}ms", start.elapsed().as_secs_f64() * 1000.0);

    println!("\nProfiling complete.");
    println!("DHAT profile written to dhat-heap.json");
    println!("View at: https://nnethercote.github.io/dh_view/dh_view.html");

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
                println!("Final RSS: {:.2} MB", rss_kb as f64 / 1024.0);
            }
        }
    }
}
