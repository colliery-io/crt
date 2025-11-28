//! Common test utilities and harness
//!
//! Provides reusable utilities for functional testing including:
//! - Test environment setup (temp directories, configs)
//! - Terminal test helpers
//! - Assertion utilities

use std::path::PathBuf;
use std::time::{Duration, Instant};

use crt_core::{ShellTerminal, Size, Terminal};
use tempfile::TempDir;

/// Test environment with isolated config directory
pub struct TestEnvironment {
    /// Temporary directory for test config
    pub temp_dir: TempDir,
    /// Path to the config directory
    pub config_dir: PathBuf,
}

impl TestEnvironment {
    /// Create a new isolated test environment
    pub fn new() -> Self {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let config_dir = temp_dir.path().to_path_buf();

        // Create themes subdirectory
        std::fs::create_dir_all(config_dir.join("themes"))
            .expect("Failed to create themes directory");

        Self {
            temp_dir,
            config_dir,
        }
    }

    /// Write a test config file
    pub fn write_config(&self, content: &str) {
        let config_path = self.config_dir.join("config.toml");
        std::fs::write(&config_path, content).expect("Failed to write test config");
    }

    /// Write a test theme file
    pub fn write_theme(&self, name: &str, content: &str) {
        let theme_path = self.config_dir.join("themes").join(format!("{}.css", name));
        std::fs::write(&theme_path, content).expect("Failed to write test theme");
    }

    /// Get config directory path for setting CRT_CONFIG_DIR
    pub fn config_dir_str(&self) -> &str {
        self.config_dir
            .to_str()
            .expect("Path should be valid UTF-8")
    }
}

impl Default for TestEnvironment {
    fn default() -> Self {
        Self::new()
    }
}

/// Test harness for terminal testing
pub struct TerminalTestHarness {
    terminal: Terminal,
}

impl TerminalTestHarness {
    /// Create a new terminal test harness
    pub fn new(cols: usize, lines: usize) -> Self {
        let terminal = Terminal::new(Size::new(cols, lines));
        Self { terminal }
    }

    /// Create with default size (80x24)
    pub fn default_size() -> Self {
        Self::new(80, 24)
    }

    /// Get the underlying terminal
    pub fn terminal(&self) -> &Terminal {
        &self.terminal
    }

    /// Get mutable terminal access
    pub fn terminal_mut(&mut self) -> &mut Terminal {
        &mut self.terminal
    }

    /// Process input bytes through terminal
    pub fn input(&mut self, bytes: &[u8]) {
        self.terminal.process_input(bytes);
    }

    /// Process a string as input
    pub fn input_str(&mut self, s: &str) {
        self.input(s.as_bytes());
    }

    /// Get all visible lines as strings
    pub fn visible_lines(&self) -> Vec<String> {
        let content = self.terminal.renderable_content();
        let screen_lines = self.terminal.screen_lines();

        let mut lines = vec![String::new(); screen_lines];

        for cell in content.display_iter {
            let row = cell.point.line.0 as usize;
            let col = cell.point.column.0;

            if row < screen_lines {
                // Ensure line is long enough
                while lines[row].len() < col {
                    lines[row].push(' ');
                }
                lines[row].push(cell.cell.c);
            }
        }

        // Trim trailing spaces from each line
        lines.iter().map(|l| l.trim_end().to_string()).collect()
    }

    /// Get visible content as a single string (lines joined by newlines)
    pub fn visible_text(&self) -> String {
        self.visible_lines().join("\n")
    }

    /// Get cursor position (row, col)
    pub fn cursor_position(&self) -> (i32, usize) {
        let cursor = self.terminal.cursor();
        (cursor.point.line.0, cursor.point.column.0)
    }

    /// Assert cursor is at specific position
    pub fn assert_cursor_at(&self, row: i32, col: usize) {
        let (r, c) = self.cursor_position();
        assert_eq!(
            (r, c),
            (row, col),
            "Expected cursor at ({}, {}), got ({}, {})",
            row,
            col,
            r,
            c
        );
    }

    /// Assert line contains specific text
    pub fn assert_line_contains(&self, line_num: usize, expected: &str) {
        let lines = self.visible_lines();
        assert!(
            line_num < lines.len(),
            "Line {} out of range (total: {})",
            line_num,
            lines.len()
        );
        assert!(
            lines[line_num].contains(expected),
            "Line {} doesn't contain '{}'. Actual: '{}'",
            line_num,
            expected,
            lines[line_num]
        );
    }

    /// Assert line equals specific text
    pub fn assert_line_eq(&self, line_num: usize, expected: &str) {
        let lines = self.visible_lines();
        assert!(
            line_num < lines.len(),
            "Line {} out of range (total: {})",
            line_num,
            lines.len()
        );
        assert_eq!(
            lines[line_num], expected,
            "Line {} mismatch. Expected: '{}', Actual: '{}'",
            line_num, expected, lines[line_num]
        );
    }
}

/// Test harness for shell integration testing
pub struct ShellTestHarness {
    shell: ShellTerminal,
    /// Default timeout for waiting on output
    pub timeout: Duration,
}

impl ShellTestHarness {
    /// Create a new shell test harness
    pub fn new(cols: usize, lines: usize) -> anyhow::Result<Self> {
        let shell = ShellTerminal::new(Size::new(cols, lines))?;
        Ok(Self {
            shell,
            timeout: Duration::from_secs(5),
        })
    }

    /// Create with a specific shell
    pub fn with_shell(cols: usize, lines: usize, shell_path: &str) -> anyhow::Result<Self> {
        let shell = ShellTerminal::with_shell(Size::new(cols, lines), shell_path)?;
        Ok(Self {
            shell,
            timeout: Duration::from_secs(5),
        })
    }

    /// Create with default size (80x24)
    pub fn default_size() -> anyhow::Result<Self> {
        Self::new(80, 24)
    }

    /// Get the underlying shell terminal
    pub fn shell(&self) -> &ShellTerminal {
        &self.shell
    }

    /// Get mutable shell terminal access
    pub fn shell_mut(&mut self) -> &mut ShellTerminal {
        &mut self.shell
    }

    /// Send input to the shell
    pub fn send(&self, input: &str) {
        self.shell.send_input(input.as_bytes());
    }

    /// Send input bytes to the shell
    pub fn send_bytes(&self, input: &[u8]) {
        self.shell.send_input(input);
    }

    /// Send a command and wait for it to complete (waits for shell prompt)
    pub fn execute(&mut self, command: &str) -> anyhow::Result<String> {
        // Send command
        self.send(&format!("{}\n", command));

        // Wait for output to stabilize
        self.wait_for_stable_output(Duration::from_millis(200))
    }

    /// Process pending PTY output and return true if any was processed
    pub fn process_output(&mut self) -> bool {
        self.shell.process_pty_output()
    }

    /// Wait for output to stabilize (no new output for given duration)
    pub fn wait_for_stable_output(
        &mut self,
        stability_duration: Duration,
    ) -> anyhow::Result<String> {
        let start = Instant::now();
        let mut last_output_time = Instant::now();
        let mut accumulated = String::new();

        loop {
            if self.process_output() {
                last_output_time = Instant::now();
            }

            // Check for timeout
            if start.elapsed() > self.timeout {
                anyhow::bail!("Timeout waiting for stable output");
            }

            // Check if output has been stable long enough
            if last_output_time.elapsed() > stability_duration {
                break;
            }

            std::thread::sleep(Duration::from_millis(10));
        }

        // Get final content
        let lines = self.shell.terminal().all_lines_text();
        for (_, line) in lines {
            accumulated.push_str(&line);
            accumulated.push('\n');
        }

        Ok(accumulated)
    }

    /// Wait for specific text to appear in terminal output
    pub fn wait_for_text(&mut self, expected: &str) -> anyhow::Result<()> {
        let start = Instant::now();

        loop {
            self.process_output();

            let lines = self.shell.terminal().all_lines_text();
            let content: String = lines
                .iter()
                .map(|(_, l)| l.as_str())
                .collect::<Vec<_>>()
                .join("\n");

            if content.contains(expected) {
                return Ok(());
            }

            if start.elapsed() > self.timeout {
                anyhow::bail!(
                    "Timeout waiting for text '{}'. Current content: {}",
                    expected,
                    content
                );
            }

            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// Get visible lines as strings
    pub fn visible_lines(&self) -> Vec<String> {
        let terminal = self.shell.terminal();
        let content = terminal.renderable_content();
        let screen_lines = terminal.screen_lines();

        let mut lines = vec![String::new(); screen_lines];

        for cell in content.display_iter {
            let row = cell.point.line.0 as usize;
            let col = cell.point.column.0;

            if row < screen_lines {
                while lines[row].len() < col {
                    lines[row].push(' ');
                }
                lines[row].push(cell.cell.c);
            }
        }

        lines.iter().map(|l| l.trim_end().to_string()).collect()
    }

    /// Get all terminal content (including history)
    pub fn all_content(&self) -> String {
        self.shell
            .terminal()
            .all_lines_text()
            .iter()
            .map(|(_, l)| l.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// Memory statistics for performance testing
#[derive(Debug, Clone, Default)]
pub struct MemoryStats {
    /// Resident set size in bytes
    pub rss: Option<u64>,
    /// Virtual memory size in bytes
    pub vsize: Option<u64>,
}

impl MemoryStats {
    /// Get current memory stats for this process (macOS)
    #[cfg(target_os = "macos")]
    pub fn current() -> Self {
        use std::process::Command;

        let pid = std::process::id();

        // Use ps to get memory info
        let output = Command::new("ps")
            .args(["-o", "rss=,vsize=", "-p", &pid.to_string()])
            .output()
            .ok();

        if let Some(output) = output {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let parts: Vec<&str> = stdout.split_whitespace().collect();

            if parts.len() >= 2 {
                let rss = parts[0].parse::<u64>().ok().map(|kb| kb * 1024);
                let vsize = parts[1].parse::<u64>().ok().map(|kb| kb * 1024);
                return Self { rss, vsize };
            }
        }

        Self::default()
    }

    /// Get current memory stats (fallback for non-macOS)
    #[cfg(not(target_os = "macos"))]
    pub fn current() -> Self {
        // On Linux, could read from /proc/self/status
        Self::default()
    }

    /// Format RSS in human-readable form
    pub fn rss_human(&self) -> String {
        self.rss
            .map_or("N/A".to_string(), |bytes| format_bytes(bytes))
    }

    /// Format virtual size in human-readable form
    pub fn vsize_human(&self) -> String {
        self.vsize
            .map_or("N/A".to_string(), |bytes| format_bytes(bytes))
    }
}

/// Format bytes in human-readable form
fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Simple performance timer
pub struct Timer {
    start: Instant,
    name: String,
}

impl Timer {
    /// Start a new timer with a name
    pub fn new(name: &str) -> Self {
        Self {
            start: Instant::now(),
            name: name.to_string(),
        }
    }

    /// Get elapsed time
    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }

    /// Stop and report elapsed time
    pub fn stop(self) -> Duration {
        let elapsed = self.elapsed();
        println!("[TIMER] {}: {:?}", self.name, elapsed);
        elapsed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_environment_creation() {
        let env = TestEnvironment::new();
        assert!(env.config_dir.exists());
        assert!(env.config_dir.join("themes").exists());
    }

    #[test]
    fn test_environment_write_config() {
        let env = TestEnvironment::new();
        env.write_config("[window]\ncolumns = 100\n");

        let config_path = env.config_dir.join("config.toml");
        assert!(config_path.exists());

        let content = std::fs::read_to_string(config_path).unwrap();
        assert!(content.contains("columns = 100"));
    }

    #[test]
    fn test_terminal_harness_basic() {
        let mut harness = TerminalTestHarness::default_size();
        harness.input_str("Hello");

        let (row, col) = harness.cursor_position();
        assert_eq!(row, 0);
        assert_eq!(col, 5);
    }

    #[test]
    fn test_terminal_harness_visible_lines() {
        let mut harness = TerminalTestHarness::default_size();
        harness.input_str("Line 1\nLine 2\nLine 3");

        let lines = harness.visible_lines();
        assert!(lines[0].contains("Line 1"));
        assert!(lines[1].contains("Line 2"));
        assert!(lines[2].contains("Line 3"));
    }

    #[test]
    fn test_memory_stats() {
        let stats = MemoryStats::current();
        // On macOS, we should get some memory info
        #[cfg(target_os = "macos")]
        {
            assert!(stats.rss.is_some(), "Should get RSS on macOS");
        }
    }

    #[test]
    fn test_timer() {
        let timer = Timer::new("test");
        std::thread::sleep(Duration::from_millis(10));
        let elapsed = timer.elapsed();
        assert!(elapsed >= Duration::from_millis(10));
    }
}
