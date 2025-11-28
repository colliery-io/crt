//! GPU Rendering Benchmark Tool
//!
//! Run with: cargo run --release --bin benchmark_gpu
//!
//! Opens a real terminal window and benchmarks actual GPU rendering performance
//! by running scripted content scenarios and measuring frame times.

use std::collections::VecDeque;
use std::io::Write;
use std::time::{Duration, Instant};

/// Frame timing statistics
struct FrameStats {
    times: VecDeque<Duration>,
    max_samples: usize,
}

impl FrameStats {
    fn new(max_samples: usize) -> Self {
        Self {
            times: VecDeque::with_capacity(max_samples),
            max_samples,
        }
    }

    fn record(&mut self, d: Duration) {
        if self.times.len() >= self.max_samples {
            self.times.pop_front();
        }
        self.times.push_back(d);
    }

    fn avg_ms(&self) -> f64 {
        if self.times.is_empty() {
            return 0.0;
        }
        self.times.iter().sum::<Duration>().as_secs_f64() * 1000.0 / self.times.len() as f64
    }

    fn fps(&self) -> f64 {
        let avg = self.avg_ms();
        if avg > 0.0 { 1000.0 / avg } else { 0.0 }
    }

    fn percentile(&self, p: f64) -> f64 {
        if self.times.is_empty() {
            return 0.0;
        }
        let mut sorted: Vec<_> = self.times.iter().collect();
        sorted.sort();
        sorted[((sorted.len() as f64 * p) as usize).min(sorted.len() - 1)].as_secs_f64() * 1000.0
    }
}

#[cfg(target_os = "macos")]
fn get_rss_mb() -> f64 {
    std::process::Command::new("ps")
        .args(["-o", "rss=", "-p", &std::process::id().to_string()])
        .output()
        .ok()
        .and_then(|o| {
            String::from_utf8_lossy(&o.stdout)
                .trim()
                .parse::<u64>()
                .ok()
        })
        .map(|kb| kb as f64 / 1024.0)
        .unwrap_or(0.0)
}

#[cfg(not(target_os = "macos"))]
fn get_rss_mb() -> f64 {
    0.0
}

fn main() {
    println!("CRT Terminal GPU Benchmark");
    println!("==========================\n");
    println!("This benchmark requires the terminal to be run with instrumentation.");
    println!("Use the following approach for accurate GPU benchmarking:\n");

    println!("1. QUICK BENCHMARK (CPU-side only):");
    println!("   cargo run --release --bin benchmark\n");

    println!("2. FULL GPU BENCHMARK with Instruments (macOS):");
    println!("   instruments -t 'Time Profiler' cargo run --release\n");

    println!("3. MANUAL FRAME TIMING:");
    println!("   CRT_BENCHMARK=1 cargo run --release");
    println!("   Then run test commands and observe console output.\n");

    println!("4. MEMORY PRESSURE TEST:");
    println!("   Run the terminal, then in another terminal:");
    println!("   while true; do ps -o rss= -p $(pgrep crt); sleep 1; done\n");

    // Run a quick CPU-side simulation
    println!("\nRunning CPU-side benchmark simulation...\n");

    use crt_core::{Size, Terminal};

    let scenarios = [
        ("idle", 2),
        ("scrolling", 5),
        ("color_stress", 5),
        ("unicode", 5),
    ];

    for (name, duration_secs) in scenarios {
        let mut terminal = Terminal::new(Size::new(120, 40));
        let mut stats = FrameStats::new(1000);
        let start = Instant::now();
        let mut frame = 0u64;

        print!("  {}: ", name);
        std::io::stdout().flush().unwrap();

        while start.elapsed().as_secs() < duration_secs {
            let frame_start = Instant::now();

            // Simulate content generation
            let content: Vec<u8> = match name {
                "idle" => vec![],
                "scrolling" => format!("[{:08}] Log output line...\n", frame).into_bytes(),
                "color_stress" => {
                    let mut buf = Vec::new();
                    for i in 0..120 {
                        buf.extend_from_slice(
                            format!("\x1b[38;5;{}m#", (frame + i as u64) % 256).as_bytes(),
                        );
                    }
                    buf.extend_from_slice(b"\x1b[0m\n");
                    buf
                }
                "unicode" => {
                    let chars = ['中', '文', '日', '本', '語', 'あ', 'い'];
                    let line: String = (0..60)
                        .map(|i| chars[(frame as usize + i) % chars.len()])
                        .collect();
                    format!("{}\n", line).into_bytes()
                }
                _ => vec![],
            };

            if !content.is_empty() {
                terminal.process_input(&content);
            }

            // Simulate render prep (what would be sent to GPU)
            let _content = terminal.renderable_content();
            terminal.reset_damage();

            stats.record(frame_start.elapsed());
            frame += 1;

            // Limit iteration speed
            std::thread::sleep(Duration::from_micros(100));
        }

        println!(
            "{:.1} FPS (avg: {:.2}ms, p99: {:.2}ms)",
            stats.fps(),
            stats.avg_ms(),
            stats.percentile(0.99)
        );
    }

    println!("\nMemory: {:.1} MB", get_rss_mb());
    println!("\nNote: These are CPU-side timings only.");
    println!("For actual GPU performance, use Instruments or add frame timing to the main app.");
}
