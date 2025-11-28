//! GPU Rendering Benchmark Tool
//!
//! Run with: cargo run --release --bin benchmark
//!
//! This opens a real window, runs rendering scenarios, and reports performance data.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use crt_core::{Size, Terminal};

/// Frame timing statistics
#[derive(Debug, Default)]
struct FrameStats {
    frame_times: VecDeque<Duration>,
    max_samples: usize,
}

impl FrameStats {
    fn new(max_samples: usize) -> Self {
        Self {
            frame_times: VecDeque::with_capacity(max_samples),
            max_samples,
        }
    }

    fn record(&mut self, duration: Duration) {
        if self.frame_times.len() >= self.max_samples {
            self.frame_times.pop_front();
        }
        self.frame_times.push_back(duration);
    }

    fn avg_ms(&self) -> f64 {
        if self.frame_times.is_empty() {
            return 0.0;
        }
        let sum: Duration = self.frame_times.iter().sum();
        sum.as_secs_f64() * 1000.0 / self.frame_times.len() as f64
    }

    fn min_ms(&self) -> f64 {
        self.frame_times
            .iter()
            .min()
            .map_or(0.0, |d| d.as_secs_f64() * 1000.0)
    }

    fn max_ms(&self) -> f64 {
        self.frame_times
            .iter()
            .max()
            .map_or(0.0, |d| d.as_secs_f64() * 1000.0)
    }

    fn p99_ms(&self) -> f64 {
        if self.frame_times.is_empty() {
            return 0.0;
        }
        let mut sorted: Vec<_> = self.frame_times.iter().collect();
        sorted.sort();
        let idx = (sorted.len() as f64 * 0.99) as usize;
        sorted
            .get(idx.min(sorted.len() - 1))
            .map_or(0.0, |d| d.as_secs_f64() * 1000.0)
    }

    fn fps(&self) -> f64 {
        let avg = self.avg_ms();
        if avg > 0.0 { 1000.0 / avg } else { 0.0 }
    }
}

/// Memory statistics
#[derive(Debug, Clone)]
struct MemoryStats {
    rss_bytes: u64,
    timestamp: Instant,
}

impl MemoryStats {
    #[cfg(target_os = "macos")]
    fn capture() -> Option<Self> {
        use std::process::Command;

        let pid = std::process::id();
        let output = Command::new("ps")
            .args(["-o", "rss=", "-p", &pid.to_string()])
            .output()
            .ok()?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let rss_kb: u64 = stdout.trim().parse().ok()?;

        Some(Self {
            rss_bytes: rss_kb * 1024,
            timestamp: Instant::now(),
        })
    }

    #[cfg(not(target_os = "macos"))]
    fn capture() -> Option<Self> {
        // Linux: read from /proc/self/status
        use std::fs;
        let status = fs::read_to_string("/proc/self/status").ok()?;
        for line in status.lines() {
            if line.starts_with("VmRSS:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    let rss_kb: u64 = parts[1].parse().ok()?;
                    return Some(Self {
                        rss_bytes: rss_kb * 1024,
                        timestamp: Instant::now(),
                    });
                }
            }
        }
        None
    }

    fn rss_mb(&self) -> f64 {
        self.rss_bytes as f64 / (1024.0 * 1024.0)
    }
}

/// Benchmark scenario
#[derive(Debug, Clone)]
struct Scenario {
    name: &'static str,
    description: &'static str,
    duration_secs: u64,
}

/// Benchmark result for a single scenario
#[derive(Debug)]
struct ScenarioResult {
    scenario: Scenario,
    frame_stats: FrameStats,
    memory_start: Option<MemoryStats>,
    memory_end: Option<MemoryStats>,
    total_frames: u64,
}

impl ScenarioResult {
    fn print_report(&self) {
        println!("\n{}", "=".repeat(60));
        println!("Scenario: {}", self.scenario.name);
        println!("{}", self.scenario.description);
        println!("{}", "-".repeat(60));

        println!("Frame Timing:");
        println!("  Total Frames: {}", self.total_frames);
        println!(
            "  Avg Frame:    {:.2} ms ({:.1} FPS)",
            self.frame_stats.avg_ms(),
            self.frame_stats.fps()
        );
        println!("  Min Frame:    {:.2} ms", self.frame_stats.min_ms());
        println!("  Max Frame:    {:.2} ms", self.frame_stats.max_ms());
        println!("  P99 Frame:    {:.2} ms", self.frame_stats.p99_ms());

        if let (Some(start), Some(end)) = (&self.memory_start, &self.memory_end) {
            println!("\nMemory:");
            println!("  Start RSS:    {:.2} MB", start.rss_mb());
            println!("  End RSS:      {:.2} MB", end.rss_mb());
            let delta = end.rss_bytes as i64 - start.rss_bytes as i64;
            let delta_mb = delta as f64 / (1024.0 * 1024.0);
            println!("  Delta:        {:+.2} MB", delta_mb);
        }

        // Performance assessment
        let avg = self.frame_stats.avg_ms();
        let status = if avg < 8.33 {
            "EXCELLENT (>120 FPS)"
        } else if avg < 16.67 {
            "GOOD (60+ FPS)"
        } else if avg < 33.33 {
            "OK (30+ FPS)"
        } else {
            "POOR (<30 FPS)"
        };
        println!("\nAssessment: {}", status);
    }
}

/// Generate content for different scenarios
fn generate_scenario_content(scenario_name: &str, frame: u64) -> Vec<u8> {
    match scenario_name {
        "static_text" => {
            // Just return empty - content is pre-loaded
            vec![]
        }
        "scrolling_output" => {
            // Continuous output like `yes` or log streaming
            format!(
                "[{:08}] Log line with timestamp and some data: {:064x}\n",
                frame, frame
            )
            .into_bytes()
        }
        "rapid_updates" => {
            // Rapid cursor movement and text updates (like htop)
            let mut buf = Vec::new();
            // Move cursor around and update values
            for row in 0..24 {
                buf.extend_from_slice(format!("\x1b[{};1H", row + 1).as_bytes());
                buf.extend_from_slice(
                    format!(
                        "CPU{:02}: {:5.1}% | ",
                        row,
                        (frame + row as u64) as f64 % 100.0
                    )
                    .as_bytes(),
                );
            }
            buf
        }
        "color_stress" => {
            // Lots of color changes (like colored ls output)
            let mut buf = Vec::new();
            for i in 0..80 {
                let color = (frame as u8).wrapping_add(i as u8);
                buf.extend_from_slice(format!("\x1b[38;5;{}m#", color).as_bytes());
            }
            buf.extend_from_slice(b"\x1b[0m\n");
            buf
        }
        "unicode_heavy" => {
            // CJK and emoji (wider glyphs, more cache pressure)
            let chars = ['中', '文', '字', '符', '日', '本', '語', 'あ', 'い', 'う'];
            let mut buf = String::new();
            for i in 0..40 {
                buf.push(chars[(frame as usize + i) % chars.len()]);
            }
            buf.push('\n');
            buf.into_bytes()
        }
        "mixed_content" => {
            // Realistic terminal usage: commands, output, colors
            let templates = [
                "\x1b[32m$\x1b[0m ls -la\n",
                "\x1b[34mdrwxr-xr-x\x1b[0m  5 user  staff   160 Nov 28 10:00 \x1b[1;34msrc\x1b[0m\n",
                "\x1b[31merror\x1b[0m: compilation failed\n",
                "\x1b[33mwarning\x1b[0m: unused variable\n",
                "   --> src/main.rs:42:9\n",
            ];
            templates[(frame as usize) % templates.len()]
                .as_bytes()
                .to_vec()
        }
        _ => vec![],
    }
}

/// Run a single benchmark scenario
fn run_scenario(scenario: Scenario) -> ScenarioResult {
    let mut terminal = Terminal::new(Size::new(120, 40));
    let mut frame_stats = FrameStats::new(1000);
    let memory_start = MemoryStats::capture();
    let start_time = Instant::now();
    let duration = Duration::from_secs(scenario.duration_secs);
    let mut total_frames = 0u64;

    // Pre-load content for static scenario
    if scenario.name == "static_text" {
        let content = "A".repeat(120).repeat(40);
        terminal.process_input(content.as_bytes());
    }

    println!(
        "Running scenario: {} ({} seconds)...",
        scenario.name, scenario.duration_secs
    );

    while start_time.elapsed() < duration {
        let frame_start = Instant::now();

        // Generate and process content
        let content = generate_scenario_content(scenario.name, total_frames);
        if !content.is_empty() {
            terminal.process_input(&content);
        }

        // Simulate render: get renderable content (this is what the GPU renderer would consume)
        let _content = terminal.renderable_content();

        // Get damage info
        let _damage = terminal.damage();
        terminal.reset_damage();

        let frame_time = frame_start.elapsed();
        frame_stats.record(frame_time);
        total_frames += 1;

        // Small sleep to simulate ~60 FPS target (don't spin at 100%)
        if frame_time < Duration::from_micros(1000) {
            std::thread::sleep(Duration::from_micros(500));
        }
    }

    let memory_end = MemoryStats::capture();

    ScenarioResult {
        scenario,
        frame_stats,
        memory_start,
        memory_end,
        total_frames,
    }
}

fn main() {
    println!("CRT Terminal GPU Benchmark Tool");
    println!("================================\n");
    println!("This benchmark measures terminal emulation performance.");
    println!("Note: This measures CPU-side preparation, not actual GPU rendering.\n");
    println!("For full GPU benchmarking, use: cargo run --release --bin benchmark-gpu\n");

    let scenarios = vec![
        Scenario {
            name: "static_text",
            description: "Static content, minimal updates (baseline)",
            duration_secs: 3,
        },
        Scenario {
            name: "scrolling_output",
            description: "Continuous scrolling output (like log streaming)",
            duration_secs: 5,
        },
        Scenario {
            name: "rapid_updates",
            description: "Rapid in-place updates (like htop/top)",
            duration_secs: 5,
        },
        Scenario {
            name: "color_stress",
            description: "Heavy color attribute changes",
            duration_secs: 5,
        },
        Scenario {
            name: "unicode_heavy",
            description: "CJK and wide characters (glyph cache pressure)",
            duration_secs: 5,
        },
        Scenario {
            name: "mixed_content",
            description: "Realistic mixed terminal usage",
            duration_secs: 5,
        },
    ];

    let mut results = Vec::new();

    for scenario in scenarios {
        results.push(run_scenario(scenario));
    }

    // Print summary report
    println!("\n\n");
    println!("{}", "=".repeat(60));
    println!("BENCHMARK RESULTS SUMMARY");
    println!("{}", "=".repeat(60));

    for result in &results {
        result.print_report();
    }

    // Overall summary
    println!("\n\n");
    println!("{}", "=".repeat(60));
    println!("OVERALL SUMMARY");
    println!("{}", "=".repeat(60));

    let total_frames: u64 = results.iter().map(|r| r.total_frames).sum();
    let avg_fps: f64 =
        results.iter().map(|r| r.frame_stats.fps()).sum::<f64>() / results.len() as f64;
    let worst_p99: f64 = results
        .iter()
        .map(|r| r.frame_stats.p99_ms())
        .fold(0.0, f64::max);

    println!("Total Frames Rendered: {}", total_frames);
    println!("Average FPS (across scenarios): {:.1}", avg_fps);
    println!("Worst P99 Frame Time: {:.2} ms", worst_p99);

    if let Some(mem) = MemoryStats::capture() {
        println!("Final Memory (RSS): {:.2} MB", mem.rss_mb());
    }

    println!("\nBenchmark complete!");
}
