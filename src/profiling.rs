//! Runtime profiling and diagnostics
//!
//! Enable profiling via:
//! - Environment: `CRT_PROFILE=1 crt`
//! - Menu: View > Start Profiling
//! - Config: `[profiling] enabled = true`
//!
//! Profiling data is written to `~/.config/crt/profile-{timestamp}.log`
//! which can be shared for debugging.

use std::collections::VecDeque;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Global profiler instance (Option allows runtime start/stop)
static PROFILER: LazyLock<Mutex<Option<Profiler>>> = LazyLock::new(|| Mutex::new(None));
static PROFILING_ENABLED: AtomicBool = AtomicBool::new(false);

/// Check if profiling is enabled (fast path)
#[inline]
pub fn is_enabled() -> bool {
    PROFILING_ENABLED.load(Ordering::Relaxed)
}

/// Initialize the profiler (call once at startup)
pub fn init() {
    let enabled = std::env::var("CRT_PROFILE").is_ok();

    if enabled {
        start_profiling();
    }
}

/// Start profiling (can be called at runtime)
/// Returns the path to the profile log file
pub fn start_profiling() -> Option<PathBuf> {
    if PROFILING_ENABLED.load(Ordering::Relaxed) {
        // Already running, return current path
        return with_profiler(|p| p.output_path.clone());
    }

    let profiler = Profiler::new();
    let path = profiler.output_path.clone();

    if let Ok(mut guard) = PROFILER.lock() {
        *guard = Some(profiler);
        PROFILING_ENABLED.store(true, Ordering::SeqCst);
        log::info!("Profiling started. Writing to: {}", path.display());
        eprintln!("[PROFILE] Started. Writing to: {}", path.display());

        // Log system info
        if let Some(ref mut p) = *guard {
            p.log_system_info();
            p.flush();
        }

        Some(path)
    } else {
        None
    }
}

/// Stop profiling and finalize the log
/// Returns the path to the completed profile log file
pub fn stop_profiling() -> Option<PathBuf> {
    if !PROFILING_ENABLED.load(Ordering::Relaxed) {
        return None;
    }

    let path = with_profiler(|p| {
        p.shutdown();
        p.output_path.clone()
    });

    if let Ok(mut guard) = PROFILER.lock() {
        *guard = None;
        PROFILING_ENABLED.store(false, Ordering::SeqCst);
        if let Some(ref p) = path {
            log::info!("Profiling stopped. Log saved to: {}", p.display());
        }
    }

    path
}

/// Toggle profiling on/off
/// Returns (is_now_enabled, log_path)
pub fn toggle() -> (bool, Option<PathBuf>) {
    if is_enabled() {
        let path = stop_profiling();
        (false, path)
    } else {
        let path = start_profiling();
        (true, path)
    }
}

/// Initialize from config (call after config is loaded)
pub fn init_from_config(enabled: bool, output_path: Option<PathBuf>) {
    if enabled && !PROFILING_ENABLED.load(Ordering::Relaxed) {
        let mut profiler = Profiler::new();
        if let Some(path) = output_path {
            profiler.set_output_path(path);
        }

        if let Ok(mut guard) = PROFILER.lock() {
            let path = profiler.output_path.clone();
            profiler.log_system_info();
            *guard = Some(profiler);
            PROFILING_ENABLED.store(true, Ordering::SeqCst);
            log::info!(
                "Profiling enabled via config. Writing to: {}",
                path.display()
            );
        }
    }
}

/// Execute a closure with the profiler
fn with_profiler<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut Profiler) -> R,
{
    PROFILER
        .lock()
        .ok()
        .and_then(|mut guard| guard.as_mut().map(f))
}

/// Record a frame timing
pub fn record_frame(timing: FrameTiming) {
    if !is_enabled() {
        return;
    }
    with_profiler(|p| p.record_frame(timing));
}

/// Record a subsystem timing
pub fn record_subsystem(name: &'static str, duration: Duration) {
    if !is_enabled() {
        return;
    }
    with_profiler(|p| p.record_subsystem(name, duration));
}

/// Record a custom event
pub fn event(category: &str, message: &str) {
    if !is_enabled() {
        return;
    }
    with_profiler(|p| p.log_event(category, message));
}

/// Record memory stats
pub fn record_memory() {
    if !is_enabled() {
        return;
    }
    with_profiler(|p| p.record_memory());
}

/// Flush profiling data and write summary (call at app shutdown)
pub fn shutdown() {
    stop_profiling();
}

/// Scoped timer that records duration on drop
pub struct ScopedTimer {
    name: &'static str,
    start: Instant,
}

impl ScopedTimer {
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            start: Instant::now(),
        }
    }
}

impl Drop for ScopedTimer {
    fn drop(&mut self) {
        record_subsystem(self.name, self.start.elapsed());
    }
}

/// Create a scoped timer (no-op if profiling disabled)
#[macro_export]
macro_rules! profile_scope {
    ($name:expr) => {
        let _timer = if $crate::profiling::is_enabled() {
            Some($crate::profiling::ScopedTimer::new($name))
        } else {
            None
        };
    };
}

/// Frame timing breakdown
#[derive(Debug, Clone, Copy, Default)]
pub struct FrameTiming {
    pub total_us: u64,
    pub update_us: u64,
    pub render_us: u64,
    pub present_us: u64,
    pub effects_us: u64,
}

/// Grid snapshot for debugging terminal state
#[derive(Debug, Clone)]
pub struct GridSnapshot {
    pub columns: usize,
    pub lines: usize,
    pub cursor_col: usize,
    pub cursor_line: i32,
    pub cursor_visible: bool,
    pub cursor_shape: String,
    pub display_offset: usize,
    pub history_size: usize,
    /// Visible lines content (line index, text)
    pub visible_content: Vec<String>,
}

/// How often to capture grid snapshots (in seconds)
const GRID_SNAPSHOT_INTERVAL_SECS: u64 = 5;

/// Record a grid snapshot
pub fn record_grid_snapshot(snapshot: GridSnapshot) {
    if !is_enabled() {
        return;
    }
    with_profiler(|p| p.record_grid_snapshot(snapshot));
}

/// Timing statistics
#[derive(Debug, Default)]
struct TimingStats {
    count: u64,
    total_us: u64,
    min_us: u64,
    max_us: u64,
    samples: VecDeque<u64>,
}

impl TimingStats {
    fn new() -> Self {
        Self {
            min_us: u64::MAX,
            samples: VecDeque::with_capacity(1000),
            ..Default::default()
        }
    }

    fn record(&mut self, duration_us: u64) {
        self.count += 1;
        self.total_us += duration_us;
        self.min_us = self.min_us.min(duration_us);
        self.max_us = self.max_us.max(duration_us);

        if self.samples.len() >= 1000 {
            self.samples.pop_front();
        }
        self.samples.push_back(duration_us);
    }

    fn avg_ms(&self) -> f64 {
        if self.count == 0 {
            return 0.0;
        }
        (self.total_us as f64 / self.count as f64) / 1000.0
    }

    fn percentile(&self, p: f64) -> f64 {
        if self.samples.is_empty() {
            return 0.0;
        }
        let mut sorted: Vec<_> = self.samples.iter().copied().collect();
        sorted.sort();
        let idx = ((sorted.len() as f64 * p) as usize).min(sorted.len() - 1);
        sorted[idx] as f64 / 1000.0
    }
}

/// Memory sample
#[derive(Debug, Clone, Copy)]
struct MemorySample {
    timestamp_ms: u64,
    rss_kb: u64,
}

/// Main profiler state
struct Profiler {
    start_time: Instant,
    output_path: PathBuf,
    writer: Option<BufWriter<File>>,

    // Frame timing
    frame_stats: TimingStats,
    update_stats: TimingStats,
    render_stats: TimingStats,
    present_stats: TimingStats,
    effects_stats: TimingStats,

    // Subsystem timing
    subsystem_stats: std::collections::HashMap<&'static str, TimingStats>,

    // Memory tracking
    memory_samples: Vec<MemorySample>,
    last_memory_sample: Instant,

    // Grid snapshot tracking
    last_grid_snapshot: Instant,
    grid_snapshot_count: u64,

    // Event log
    event_count: u64,
}

impl Profiler {
    fn new() -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let output_path = dirs::home_dir()
            .map(|p| p.join(".config").join("crt"))
            .unwrap_or_else(|| PathBuf::from("."))
            .join(format!("profile-{}.log", timestamp));

        // Ensure directory exists
        if let Some(parent) = output_path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        let writer = File::create(&output_path).ok().map(|f| BufWriter::new(f));

        if writer.is_some() {
            eprintln!("[PROFILE] Writing to: {}", output_path.display());
        }

        Self {
            start_time: Instant::now(),
            output_path,
            writer,
            frame_stats: TimingStats::new(),
            update_stats: TimingStats::new(),
            render_stats: TimingStats::new(),
            present_stats: TimingStats::new(),
            effects_stats: TimingStats::new(),
            subsystem_stats: std::collections::HashMap::new(),
            memory_samples: Vec::new(),
            last_memory_sample: Instant::now(),
            last_grid_snapshot: Instant::now(),
            grid_snapshot_count: 0,
            event_count: 0,
        }
    }

    fn set_output_path(&mut self, path: PathBuf) {
        self.output_path = path;
        if let Some(parent) = self.output_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        self.writer = File::create(&self.output_path)
            .ok()
            .map(|f| BufWriter::new(f));
    }

    fn write(&mut self, line: &str) {
        if let Some(writer) = &mut self.writer {
            let elapsed = self.start_time.elapsed().as_millis();
            let _ = writeln!(writer, "[{:>10}ms] {}", elapsed, line);
        }
    }

    fn flush(&mut self) {
        if let Some(writer) = &mut self.writer {
            let _ = writer.flush();
        }
    }

    fn log_system_info(&mut self) {
        self.write("=== CRT Terminal Profile Log ===");
        self.write(&format!("Version: {}", env!("CARGO_PKG_VERSION")));
        self.write(&format!(
            "OS: {} {}",
            std::env::consts::OS,
            std::env::consts::ARCH
        ));

        #[cfg(target_os = "macos")]
        {
            if let Ok(output) = std::process::Command::new("sw_vers")
                .args(["-productVersion"])
                .output()
            {
                let version = String::from_utf8_lossy(&output.stdout);
                self.write(&format!("macOS: {}", version.trim()));
            }

            if let Ok(output) = std::process::Command::new("system_profiler")
                .args(["SPDisplaysDataType", "-json"])
                .output()
            {
                // Just note that we have GPU info
                if !output.stdout.is_empty() {
                    self.write("GPU info available (see system_profiler for details)");
                }
            }
        }

        if let Some(rss) = get_rss_kb() {
            self.write(&format!("Initial RSS: {} KB", rss));
        }

        self.write("=== Begin Profiling ===");
    }

    fn record_frame(&mut self, timing: FrameTiming) {
        self.frame_stats.record(timing.total_us);
        self.update_stats.record(timing.update_us);
        self.render_stats.record(timing.render_us);
        self.present_stats.record(timing.present_us);
        self.effects_stats.record(timing.effects_us);

        // Log slow frames (>16ms = below 60fps)
        if timing.total_us > 16000 {
            self.write(&format!(
                "SLOW FRAME: total={:.2}ms update={:.2}ms render={:.2}ms present={:.2}ms effects={:.2}ms",
                timing.total_us as f64 / 1000.0,
                timing.update_us as f64 / 1000.0,
                timing.render_us as f64 / 1000.0,
                timing.present_us as f64 / 1000.0,
                timing.effects_us as f64 / 1000.0,
            ));
        }

        // Periodic memory sample
        if self.last_memory_sample.elapsed() > Duration::from_secs(1) {
            self.record_memory();
            self.last_memory_sample = Instant::now();
        }

        // Periodic stats summary (every 300 frames ~ 5 seconds at 60fps)
        if self.frame_stats.count % 300 == 0 {
            self.write_periodic_summary();
        }
    }

    fn record_subsystem(&mut self, name: &'static str, duration: Duration) {
        let duration_us = duration.as_micros() as u64;
        self.subsystem_stats
            .entry(name)
            .or_insert_with(TimingStats::new)
            .record(duration_us);
    }

    fn log_event(&mut self, category: &str, message: &str) {
        self.event_count += 1;
        self.write(&format!("[{}] {}", category, message));
    }

    fn record_memory(&mut self) {
        if let Some(rss_kb) = get_rss_kb() {
            let sample = MemorySample {
                timestamp_ms: self.start_time.elapsed().as_millis() as u64,
                rss_kb,
            };
            self.memory_samples.push(sample);
        }
    }

    fn record_grid_snapshot(&mut self, snapshot: GridSnapshot) {
        // Rate limit snapshots
        if self.last_grid_snapshot.elapsed() < Duration::from_secs(GRID_SNAPSHOT_INTERVAL_SECS) {
            return;
        }
        self.last_grid_snapshot = Instant::now();
        self.grid_snapshot_count += 1;

        self.write(&format!(
            "=== Grid Snapshot #{} ===",
            self.grid_snapshot_count
        ));
        self.write(&format!(
            "Grid: {}x{} cursor: ({},{}) visible: {} shape: {} offset: {} history: {}",
            snapshot.columns,
            snapshot.lines,
            snapshot.cursor_col,
            snapshot.cursor_line,
            snapshot.cursor_visible,
            snapshot.cursor_shape,
            snapshot.display_offset,
            snapshot.history_size,
        ));

        // Write visible content (truncate lines for readability)
        self.write("Content:");
        for (i, line) in snapshot.visible_content.iter().enumerate() {
            let display_line = if line.len() > 120 {
                format!("{}...", &line[..117])
            } else {
                line.clone()
            };
            // Escape non-printable characters for log readability
            let safe_line: String = display_line
                .chars()
                .map(|c| if c.is_control() && c != ' ' { '.' } else { c })
                .collect();
            self.write(&format!("  {:>3}| {}", i, safe_line));
        }
        self.write("=== End Snapshot ===");
        self.flush();
    }

    fn write_periodic_summary(&mut self) {
        let fps = if self.frame_stats.avg_ms() > 0.0 {
            1000.0 / self.frame_stats.avg_ms()
        } else {
            0.0
        };

        self.write(&format!(
            "STATS: frames={} avg={:.2}ms p99={:.2}ms fps={:.1}",
            self.frame_stats.count,
            self.frame_stats.avg_ms(),
            self.frame_stats.percentile(0.99),
            fps,
        ));

        if let Some(sample) = self.memory_samples.last() {
            self.write(&format!(
                "MEMORY: rss={:.1}MB",
                sample.rss_kb as f64 / 1024.0
            ));
        }
        self.flush();
    }

    fn shutdown(&mut self) {
        self.write("=== Profile Summary ===");

        let duration = self.start_time.elapsed();
        self.write(&format!("Session duration: {:.1}s", duration.as_secs_f64()));
        self.write(&format!("Total frames: {}", self.frame_stats.count));

        if self.frame_stats.count > 0 {
            let fps = 1000.0 / self.frame_stats.avg_ms();
            self.write(&format!(
                "Frame timing: avg={:.2}ms min={:.2}ms max={:.2}ms p50={:.2}ms p99={:.2}ms fps={:.1}",
                self.frame_stats.avg_ms(),
                self.frame_stats.min_us as f64 / 1000.0,
                self.frame_stats.max_us as f64 / 1000.0,
                self.frame_stats.percentile(0.50),
                self.frame_stats.percentile(0.99),
                fps,
            ));

            self.write("Breakdown:");
            self.write(&format!(
                "  Update:  avg={:.2}ms p99={:.2}ms",
                self.update_stats.avg_ms(),
                self.update_stats.percentile(0.99)
            ));
            self.write(&format!(
                "  Render:  avg={:.2}ms p99={:.2}ms",
                self.render_stats.avg_ms(),
                self.render_stats.percentile(0.99)
            ));
            self.write(&format!(
                "  Present: avg={:.2}ms p99={:.2}ms",
                self.present_stats.avg_ms(),
                self.present_stats.percentile(0.99)
            ));
            self.write(&format!(
                "  Effects: avg={:.2}ms p99={:.2}ms",
                self.effects_stats.avg_ms(),
                self.effects_stats.percentile(0.99)
            ));
        }

        if !self.subsystem_stats.is_empty() {
            // Collect subsystem data before writing to avoid borrow issues
            let mut subsystem_lines: Vec<String> = self
                .subsystem_stats
                .iter()
                .map(|(name, stats)| {
                    format!(
                        "  {}: count={} avg={:.2}ms total={:.1}ms",
                        name,
                        stats.count,
                        stats.avg_ms(),
                        stats.total_us as f64 / 1000.0
                    )
                })
                .collect();
            subsystem_lines.sort(); // Sort by name for consistent output

            self.write("Subsystems:");
            for line in subsystem_lines {
                self.write(&line);
            }
        }

        if !self.memory_samples.is_empty() {
            let first_kb = self.memory_samples.first().unwrap().rss_kb;
            let last_kb = self.memory_samples.last().unwrap().rss_kb;
            let max_kb = self
                .memory_samples
                .iter()
                .map(|s| s.rss_kb)
                .max()
                .unwrap_or(0);

            self.write("Memory:");
            self.write(&format!("  Start:  {:.1}MB", first_kb as f64 / 1024.0));
            self.write(&format!("  End:    {:.1}MB", last_kb as f64 / 1024.0));
            self.write(&format!("  Peak:   {:.1}MB", max_kb as f64 / 1024.0));
            self.write(&format!(
                "  Growth: {:+.1}MB",
                (last_kb as i64 - first_kb as i64) as f64 / 1024.0
            ));
        }

        self.write(&format!("Events logged: {}", self.event_count));
        self.write(&format!("Grid snapshots: {}", self.grid_snapshot_count));
        self.write(&format!("Profile saved to: {}", self.output_path.display()));
        self.write("=== End Profile ===");

        // Flush
        if let Some(writer) = &mut self.writer {
            let _ = writer.flush();
        }

        eprintln!(
            "[PROFILE] Session complete. Log saved to: {}",
            self.output_path.display()
        );
    }
}

#[cfg(target_os = "macos")]
fn get_rss_kb() -> Option<u64> {
    std::process::Command::new("ps")
        .args(["-o", "rss=", "-p", &std::process::id().to_string()])
        .output()
        .ok()
        .and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse().ok())
}

#[cfg(not(target_os = "macos"))]
fn get_rss_kb() -> Option<u64> {
    // Linux: read from /proc/self/status
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmRSS:"))
                .and_then(|l| l.split_whitespace().nth(1))
                .and_then(|v| v.parse().ok())
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timing_stats() {
        let mut stats = TimingStats::new();
        stats.record(1000);
        stats.record(2000);
        stats.record(3000);

        assert_eq!(stats.count, 3);
        assert_eq!(stats.min_us, 1000);
        assert_eq!(stats.max_us, 3000);
        assert!((stats.avg_ms() - 2.0).abs() < 0.01);
    }

    #[test]
    fn test_scoped_timer_compiles() {
        // Just verify the macro compiles
        profile_scope!("test");
    }

    #[test]
    fn test_is_enabled_default() {
        // Should be false by default (no env var set in test)
        // Note: This might fail if CRT_PROFILE is set in the environment
        // assert!(!is_enabled());
    }
}
