//! CRT Core - Terminal emulation and PTY management
//!
//! This crate provides:
//! - Terminal grid state (via alacritty_terminal)
//! - ANSI escape sequence parsing (via vte)
//! - PTY process management (via portable-pty)

pub mod pty;

pub use pty::{Pty, ShellType, SpawnOptions};

// Re-export alacritty_terminal types needed for rendering
pub use alacritty_terminal::event::Event as TerminalEvent;
pub use alacritty_terminal::grid::Scroll;
pub use alacritty_terminal::index::Side;
pub use alacritty_terminal::index::{Column, Line, Point};
pub use alacritty_terminal::selection::{Selection, SelectionRange, SelectionType};
pub use alacritty_terminal::term::TermMode;
pub use alacritty_terminal::term::{
    LineDamageBounds, RenderableContent, RenderableCursor, TermDamage,
    cell::Cell,
    cell::Flags as CellFlags,
    color::{self, Colors},
};
pub use alacritty_terminal::vte::ansi::Color as AnsiColor;
pub use alacritty_terminal::vte::ansi::CursorShape;
pub use alacritty_terminal::vte::ansi::NamedColor;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use alacritty_terminal::event::{Event, EventListener};

/// Semantic zone type from OSC 133 shell integration
///
/// OSC 133 sequences mark boundaries between prompt, input, and output regions.
/// This allows the terminal to apply different rendering (e.g., glow on prompt/input only).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SemanticZone {
    /// No OSC 133 zone information (shell doesn't support it or before first marker)
    #[default]
    Unknown,
    /// Prompt region (between OSC 133;A and OSC 133;B)
    Prompt,
    /// User input region (between OSC 133;B and OSC 133;C)
    Input,
    /// Command output region (between OSC 133;C and next OSC 133;A)
    Output,
}

/// Shell events that can trigger theme effects
///
/// These events are detected from terminal output and can be used
/// to trigger visual effects defined in theme CSS (::on-bell, etc.)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellEvent {
    /// Bell character received (BEL, 0x07)
    Bell,
    /// Command completed successfully (OSC 133;D with exit code 0)
    CommandSuccess,
    /// Command failed (OSC 133;D with non-zero exit code)
    CommandFail(i32),
}
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::term::test::TermSize;
use alacritty_terminal::term::{self, Config as TermConfig, Term};
use alacritty_terminal::vte::ansi;

/// Shared event storage
#[derive(Default)]
struct EventStorage {
    events: Vec<Event>,
}

/// Terminal event handler that collects events for the application
#[derive(Clone)]
pub struct TerminalEventProxy {
    storage: Arc<Mutex<EventStorage>>,
}

impl TerminalEventProxy {
    pub fn new() -> Self {
        Self {
            storage: Arc::new(Mutex::new(EventStorage::default())),
        }
    }

    /// Take all pending events
    pub fn take_events(&self) -> Vec<Event> {
        match self.storage.lock() {
            Ok(mut storage) => storage.events.drain(..).collect(),
            Err(e) => {
                log::warn!("Event storage lock poisoned, returning empty events: {}", e);
                Vec::new()
            }
        }
    }
}

impl Default for TerminalEventProxy {
    fn default() -> Self {
        Self::new()
    }
}

impl EventListener for TerminalEventProxy {
    fn send_event(&self, event: Event) {
        match self.storage.lock() {
            Ok(mut storage) => storage.events.push(event),
            Err(e) => {
                log::warn!("Event storage lock poisoned, dropping event: {}", e);
            }
        }
    }
}

/// Terminal size in characters
#[derive(Debug, Clone, Copy)]
pub struct Size {
    pub columns: usize,
    pub lines: usize,
}

impl Size {
    pub fn new(columns: usize, lines: usize) -> Self {
        Self { columns, lines }
    }
}

/// CRT Terminal wrapper around alacritty_terminal
pub struct Terminal {
    term: Term<TerminalEventProxy>,
    event_proxy: TerminalEventProxy,
    parser: ansi::Processor,
    size: Size,
    /// Semantic zones per line (from OSC 133)
    /// Maps line number (can be negative for scrollback) to zone type
    line_zones: BTreeMap<i32, SemanticZone>,
    /// Current semantic zone state (for marking new content)
    current_zone: SemanticZone,
    /// Pending shell events for theme triggers (bell, command success/fail)
    pending_shell_events: Vec<ShellEvent>,
}

impl Terminal {
    /// Create a new terminal with the given size
    pub fn new(size: Size) -> Self {
        let config = TermConfig::default();
        let term_size = TermSize::new(size.columns, size.lines);
        let event_proxy = TerminalEventProxy::new();
        let term = Term::new(config, &term_size, event_proxy.clone());
        let parser = ansi::Processor::new();

        Self {
            term,
            event_proxy,
            parser,
            size,
            line_zones: BTreeMap::new(),
            current_zone: SemanticZone::Unknown,
            pending_shell_events: Vec::new(),
        }
    }

    /// Get terminal dimensions
    pub fn size(&self) -> Size {
        self.size
    }

    /// Get the number of columns
    pub fn columns(&self) -> usize {
        self.term.columns()
    }

    /// Get the number of visible lines
    pub fn screen_lines(&self) -> usize {
        self.term.screen_lines()
    }

    /// Process input bytes through the terminal parser
    ///
    /// Selection is preserved across output processing to support copy/paste
    /// during active shell output (e.g., during builds, long-running commands).
    pub fn process_input(&mut self, bytes: &[u8]) {
        // Scan for OSC 133 sequences before passing to parser
        self.scan_osc133(bytes);

        // Preserve selection across output processing
        // Alacritty_terminal clears selection when lines are cleared or screen is modified,
        // but we want to keep it for copy/paste convenience
        let saved_selection = self.term.selection.clone();

        // Pass through to terminal parser unchanged
        self.parser.advance(&mut self.term, bytes);

        // Restore selection if it was cleared during processing
        // Only restore if we had a selection and it was cleared
        if saved_selection.is_some() && self.term.selection.is_none() {
            self.term.selection = saved_selection;
        }
    }

    /// Scan input bytes for OSC 133 semantic prompt sequences
    ///
    /// OSC 133 format: `\x1b]133;X\x07` or `\x1b]133;X\x1b\\`
    /// Where X is: A (prompt start), B (command start), C (output start), D (output end)
    /// For D, may include exit code: `\x1b]133;D;exitcode\x07`
    fn scan_osc133(&mut self, bytes: &[u8]) {
        // OSC starts with \x1b] (ESC ])
        let mut i = 0;
        while i < bytes.len() {
            // Look for ESC ]
            if bytes[i] == 0x1b && i + 1 < bytes.len() && bytes[i + 1] == b']' {
                // Check for "133;" pattern
                if i + 6 < bytes.len()
                    && bytes[i + 2] == b'1'
                    && bytes[i + 3] == b'3'
                    && bytes[i + 4] == b'3'
                    && bytes[i + 5] == b';'
                {
                    let cmd = bytes[i + 6];

                    // For D command, try to parse exit code (format: D;exitcode)
                    let exit_code = if cmd == b'D' && i + 8 < bytes.len() && bytes[i + 7] == b';' {
                        // Find terminator and parse exit code
                        let mut end = i + 8;
                        while end < bytes.len()
                            && bytes[end] != 0x07
                            && bytes[end] != 0x1b
                            && bytes[end].is_ascii_digit()
                        {
                            end += 1;
                        }
                        if end > i + 8 {
                            std::str::from_utf8(&bytes[i + 8..end])
                                .ok()
                                .and_then(|s| s.parse::<i32>().ok())
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                    // Check for valid terminator anywhere after command
                    // Scan forward to find BEL or ST
                    let mut term_pos = i + 7;
                    let mut has_terminator = false;
                    while term_pos < bytes.len() && term_pos < i + 20 {
                        // limit search
                        if bytes[term_pos] == 0x07 {
                            has_terminator = true;
                            break;
                        }
                        if bytes[term_pos] == 0x1b
                            && term_pos + 1 < bytes.len()
                            && bytes[term_pos + 1] == b'\\'
                        {
                            has_terminator = true;
                            break;
                        }
                        term_pos += 1;
                    }

                    if has_terminator {
                        self.handle_osc133(cmd, exit_code);
                    }
                }
            }
            i += 1;
        }
    }

    /// Handle an OSC 133 command
    fn handle_osc133(&mut self, cmd: u8, exit_code: Option<i32>) {
        // Get current cursor line from terminal
        let cursor = self.term.renderable_content().cursor;
        let line = cursor.point.line.0;

        match cmd {
            b'A' => {
                // Prompt start
                self.current_zone = SemanticZone::Prompt;
                self.line_zones.insert(line, SemanticZone::Prompt);
                log::debug!("OSC 133;A: Prompt start at line {}", line);
            }
            b'B' => {
                // Command start (end of prompt, user input begins)
                self.current_zone = SemanticZone::Input;
                self.line_zones.insert(line, SemanticZone::Input);
                log::debug!("OSC 133;B: Input start at line {}", line);
            }
            b'C' => {
                // Output start (command executed)
                self.current_zone = SemanticZone::Output;
                self.line_zones.insert(line, SemanticZone::Output);
                log::debug!("OSC 133;C: Output start at line {}", line);
            }
            b'D' => {
                // Output end with exit code - emit command success/fail event
                let code = exit_code.unwrap_or(0);
                log::debug!(
                    "OSC 133;D: Output end at line {}, exit code: {}",
                    line,
                    code
                );
                if code == 0 {
                    self.pending_shell_events.push(ShellEvent::CommandSuccess);
                } else {
                    self.pending_shell_events
                        .push(ShellEvent::CommandFail(code));
                }
            }
            _ => {}
        }
    }

    /// Get semantic zone for a given line
    ///
    /// Returns Unknown if no OSC 133 marker has been seen for this line.
    pub fn get_line_zone(&self, line: i32) -> SemanticZone {
        self.line_zones
            .get(&line)
            .copied()
            .unwrap_or(SemanticZone::Unknown)
    }

    /// Check if any OSC 133 zones have been detected
    ///
    /// Returns true if the shell has sent at least one OSC 133 sequence,
    /// indicating it supports semantic prompts.
    pub fn has_semantic_zones(&self) -> bool {
        !self.line_zones.is_empty()
    }

    /// Get the current semantic zone state
    pub fn current_zone(&self) -> SemanticZone {
        self.current_zone
    }

    /// Get access to renderable content (cells, cursor, etc.)
    pub fn renderable_content(&self) -> term::RenderableContent<'_> {
        self.term.renderable_content()
    }

    /// Get the cursor information
    pub fn cursor(&self) -> term::RenderableCursor {
        self.renderable_content().cursor
    }

    /// Check if cursor should be visible (based on SHOW_CURSOR mode)
    pub fn cursor_mode_visible(&self) -> bool {
        self.term.mode().contains(TermMode::SHOW_CURSOR)
    }

    /// Get terminal mode flags
    pub fn mode(&self) -> TermMode {
        *self.term.mode()
    }

    /// Take pending terminal events
    pub fn take_events(&self) -> Vec<Event> {
        self.event_proxy.take_events()
    }

    /// Take pending shell events (bell, command success/fail)
    ///
    /// These events can trigger theme visual effects.
    pub fn take_shell_events(&mut self) -> Vec<ShellEvent> {
        std::mem::take(&mut self.pending_shell_events)
    }

    /// Resize the terminal
    pub fn resize(&mut self, size: Size) {
        self.size = size;
        let term_size = TermSize::new(size.columns, size.lines);
        self.term.resize(term_size);
    }

    /// Access the underlying Term for advanced operations
    pub fn inner(&self) -> &Term<TerminalEventProxy> {
        &self.term
    }

    /// Mutable access to the underlying Term
    pub fn inner_mut(&mut self) -> &mut Term<TerminalEventProxy> {
        &mut self.term
    }

    /// Get damage information since last reset
    ///
    /// Returns which parts of the terminal have changed and need redrawing.
    /// Call `reset_damage()` after rendering to clear the damage state.
    pub fn damage(&mut self) -> TermDamage<'_> {
        self.term.damage()
    }

    /// Reset damage state after rendering
    ///
    /// Call this after you've rendered the damaged regions to clear the
    /// damage tracking for the next frame.
    pub fn reset_damage(&mut self) {
        self.term.reset_damage();
    }

    /// Check if any damage exists (needs redraw)
    pub fn has_damage(&mut self) -> bool {
        match self.term.damage() {
            TermDamage::Full => true,
            TermDamage::Partial(iter) => iter.count() > 0,
        }
    }

    /// Start a new selection at the given point
    pub fn start_selection(&mut self, point: Point, selection_type: SelectionType) {
        use alacritty_terminal::index::Side;
        use alacritty_terminal::selection::Selection;
        self.term.selection = Some(Selection::new(selection_type, point, Side::Left));
    }

    /// Update the selection end point
    pub fn update_selection(&mut self, point: Point) {
        use alacritty_terminal::index::Side;
        if let Some(selection) = self.term.selection.as_mut() {
            selection.update(point, Side::Right);
        }
    }

    /// Clear the current selection
    pub fn clear_selection(&mut self) {
        self.term.selection = None;
    }

    /// Check if a selection exists
    pub fn has_selection(&self) -> bool {
        self.term.selection.is_some()
    }

    /// Get the selection as text, if any
    pub fn selection_to_string(&self) -> Option<String> {
        self.term.selection_to_string()
    }

    /// Scroll the terminal viewport
    ///
    /// Use `Scroll::Delta(n)` to scroll by n lines (positive = up into history)
    /// Use `Scroll::PageUp`, `Scroll::PageDown`, `Scroll::Top`, `Scroll::Bottom`
    pub fn scroll(&mut self, scroll: alacritty_terminal::grid::Scroll) {
        self.term.scroll_display(scroll);
    }

    /// Get the current display offset (how far scrolled into history)
    /// Returns 0 when at the bottom (live output), >0 when scrolled back
    pub fn display_offset(&self) -> usize {
        self.term.grid().display_offset()
    }

    /// Check if the terminal is scrolled back (not showing live output)
    pub fn is_scrolled_back(&self) -> bool {
        self.display_offset() > 0
    }

    /// Scroll to the bottom (show live output)
    pub fn scroll_to_bottom(&mut self) {
        self.term
            .scroll_display(alacritty_terminal::grid::Scroll::Bottom);
    }

    /// Get total number of lines including history
    pub fn total_lines(&self) -> usize {
        self.term.grid().total_lines()
    }

    /// Get history size (lines above visible area)
    pub fn history_size(&self) -> usize {
        self.term.grid().history_size()
    }

    /// Get all lines as text (history + visible), returns Vec of (line_index, text)
    /// Line indices are relative to the grid: negative = history, 0+ = visible
    pub fn all_lines_text(&self) -> Vec<(i32, String)> {
        let grid = self.term.grid();
        let history_size = grid.history_size() as i32;
        let screen_lines = self.term.screen_lines() as i32;
        let mut lines = Vec::new();

        // History lines (negative indices, from oldest to newest)
        for i in (0..history_size).rev() {
            let line_idx = -(i + 1);
            let row = &grid[alacritty_terminal::index::Line(line_idx)];
            let text: String = row.into_iter().map(|cell| cell.c).collect();
            lines.push((line_idx, text.trim_end().to_string()));
        }

        // Visible lines (0 to screen_lines-1)
        for i in 0..screen_lines {
            let row = &grid[alacritty_terminal::index::Line(i)];
            let text: String = row.into_iter().map(|cell| cell.c).collect();
            lines.push((i, text.trim_end().to_string()));
        }

        lines
    }

    /// Check if bracketed paste mode is enabled
    pub fn bracketed_paste_enabled(&self) -> bool {
        self.term.mode().contains(TermMode::BRACKETED_PASTE)
    }
}

/// A terminal connected to a PTY running a shell
pub struct ShellTerminal {
    terminal: Terminal,
    pty: Pty,
}

impl ShellTerminal {
    /// Create a new shell terminal with the given size
    pub fn new(size: Size) -> anyhow::Result<Self> {
        let terminal = Terminal::new(size);
        let pty = Pty::spawn(None, size.columns as u16, size.lines as u16)?;

        Ok(Self { terminal, pty })
    }

    /// Create a new shell terminal with a specific working directory
    pub fn with_cwd(size: Size, cwd: std::path::PathBuf) -> anyhow::Result<Self> {
        let terminal = Terminal::new(size);
        let pty = Pty::spawn_with_cwd(None, size.columns as u16, size.lines as u16, Some(cwd))?;

        Ok(Self { terminal, pty })
    }

    /// Create a new shell terminal with a specific shell
    pub fn with_shell(size: Size, shell: &str) -> anyhow::Result<Self> {
        let terminal = Terminal::new(size);
        let pty = Pty::spawn(Some(shell), size.columns as u16, size.lines as u16)?;

        Ok(Self { terminal, pty })
    }

    /// Create a new shell terminal with full spawn options
    ///
    /// This enables semantic prompt support (OSC 133) for command success/fail detection.
    pub fn with_options(size: Size, options: SpawnOptions) -> anyhow::Result<Self> {
        let terminal = Terminal::new(size);
        let pty = Pty::spawn_with_options(size.columns as u16, size.lines as u16, options)?;

        Ok(Self { terminal, pty })
    }

    /// Get the current working directory of the shell process
    pub fn working_directory(&self) -> Option<std::path::PathBuf> {
        self.pty.working_directory()
    }

    /// Process any available PTY output through the terminal
    /// Returns true if any output was processed
    pub fn process_pty_output(&mut self) -> bool {
        let output = self.pty.read_available();
        if !output.is_empty() {
            // Log escape sequences for debugging
            if output.len() < 2000 {
                let escaped: String = output
                    .iter()
                    .map(|&b| {
                        if b == 0x1b {
                            "ESC".to_string()
                        } else if b == 0x07 {
                            "BEL".to_string()
                        } else if b < 32 {
                            format!("^{}", (b + 64) as char)
                        } else if b < 127 {
                            (b as char).to_string()
                        } else {
                            format!("\\x{:02x}", b)
                        }
                    })
                    .collect();
                log::debug!("PTY output ({} bytes): {}", output.len(), escaped);
                // Check for black color sequences
                let output_str = String::from_utf8_lossy(&output);
                if output_str.contains("[30m") || output_str.contains("[30;") {
                    log::warn!("PTY output contains BLACK foreground sequence!");
                }
            }
            self.terminal.process_input(&output);
            true
        } else {
            false
        }
    }

    /// Send keyboard input to the PTY
    pub fn send_input(&self, data: &[u8]) {
        self.pty.write(data);
    }

    /// Resize both the terminal and PTY
    pub fn resize(&mut self, size: Size) {
        self.terminal.resize(size);
        self.pty.resize(size.columns as u16, size.lines as u16);
    }

    /// Get access to the terminal for rendering
    pub fn terminal(&self) -> &Terminal {
        &self.terminal
    }

    /// Get mutable access to the terminal
    pub fn terminal_mut(&mut self) -> &mut Terminal {
        &mut self.terminal
    }

    /// Get access to the PTY
    pub fn pty(&self) -> &Pty {
        &self.pty
    }

    /// Take any pending terminal events (title changes, bells, etc.)
    pub fn take_events(&self) -> Vec<Event> {
        self.terminal.take_events()
    }

    /// Check for title change and return it, preserving other events
    /// Returns the title string from the most recent Title event
    pub fn check_title_change(&self) -> Option<String> {
        let (title, _) = self.check_events();
        title
    }

    /// Check for terminal events and return title changes and bell triggers
    /// Returns (Option<title>, bell_triggered)
    ///
    /// Note: For theme-triggerable events, prefer `take_shell_events()` which
    /// provides a unified `ShellEvent` enum including bell, command success/fail.
    pub fn check_events(&self) -> (Option<String>, bool) {
        let events = self.terminal.take_events();
        let mut title = None;
        let mut bell = false;

        if !events.is_empty() {
            log::debug!("Terminal events: {:?}", events);
        }

        for event in events {
            match event {
                Event::Title(t) => title = Some(t),
                Event::Bell => {
                    log::debug!("Bell event received from terminal");
                    bell = true;
                }
                _ => {} // Ignore other events
            }
        }

        (title, bell)
    }

    /// Take shell events for theme triggers (bell, command success/fail)
    ///
    /// This returns events that can trigger visual effects defined in theme CSS
    /// (::on-bell, ::on-command-success, ::on-command-fail).
    ///
    /// Also checks for Bell from alacritty_terminal and includes it as ShellEvent::Bell.
    /// Returns (shell_events, Option<title>) to combine with title checking.
    pub fn take_shell_events(&mut self) -> (Vec<ShellEvent>, Option<String>) {
        let mut shell_events = self.terminal.take_shell_events();

        // Also check terminal events for Bell and title
        let events = self.terminal.take_events();
        let mut title = None;

        for event in events {
            match event {
                Event::Title(t) => title = Some(t),
                Event::Bell => {
                    log::debug!("Bell event converted to ShellEvent");
                    shell_events.push(ShellEvent::Bell);
                }
                _ => {}
            }
        }

        (shell_events, title)
    }

    /// Start a new selection at the given point
    pub fn start_selection(&mut self, point: Point, selection_type: SelectionType) {
        self.terminal.start_selection(point, selection_type);
    }

    /// Update the selection end point
    pub fn update_selection(&mut self, point: Point) {
        self.terminal.update_selection(point);
    }

    /// Clear the current selection
    pub fn clear_selection(&mut self) {
        self.terminal.clear_selection();
    }

    /// Check if a selection exists
    pub fn has_selection(&self) -> bool {
        self.terminal.has_selection()
    }

    /// Get the selection as text, if any
    pub fn selection_to_string(&self) -> Option<String> {
        self.terminal.selection_to_string()
    }

    /// Scroll the terminal viewport
    pub fn scroll(&mut self, scroll: crate::Scroll) {
        self.terminal.scroll(scroll);
    }

    /// Get the current display offset (how far scrolled into history)
    pub fn display_offset(&self) -> usize {
        self.terminal.display_offset()
    }

    /// Check if the terminal is scrolled back (not showing live output)
    pub fn is_scrolled_back(&self) -> bool {
        self.terminal.is_scrolled_back()
    }

    /// Scroll to the bottom (show live output)
    pub fn scroll_to_bottom(&mut self) {
        self.terminal.scroll_to_bottom();
    }

    /// Check if bracketed paste mode is enabled
    pub fn bracketed_paste_enabled(&self) -> bool {
        self.terminal.bracketed_paste_enabled()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn create_terminal() {
        let term = Terminal::new(Size::new(80, 24));
        assert_eq!(term.columns(), 80);
        assert_eq!(term.screen_lines(), 24);
    }

    #[test]
    fn process_simple_text() {
        let mut term = Terminal::new(Size::new(80, 24));
        term.process_input(b"Hello, World!");
        // Text should be in the grid now
    }

    #[test]
    fn bell_event_triggered() {
        use alacritty_terminal::event::Event;

        let mut term = Terminal::new(Size::new(80, 24));

        // Clear any existing events
        term.take_events();

        // Send BEL character (0x07)
        term.process_input(b"\x07");

        // Check for Bell event
        let events = term.take_events();
        let has_bell = events.iter().any(|e| matches!(e, Event::Bell));
        assert!(has_bell, "Expected Bell event, got: {:?}", events);
    }

    #[test]
    fn shell_terminal_integration() {
        let mut shell = ShellTerminal::new(Size::new(80, 24)).expect("Failed to create shell");

        // Give the shell time to start and output prompt
        thread::sleep(Duration::from_millis(100));

        // Process initial output (shell prompt)
        shell.process_pty_output();

        // Send a command
        shell.send_input(b"echo test123\n");

        // Wait for output
        thread::sleep(Duration::from_millis(100));

        // Process the output
        shell.process_pty_output();

        // Terminal should now have content
        // (We're not checking specific content as it depends on shell)
    }

    #[test]
    fn osc133_no_zones_initially() {
        let term = Terminal::new(Size::new(80, 24));
        assert!(!term.has_semantic_zones());
        assert_eq!(term.get_line_zone(0), SemanticZone::Unknown);
        assert_eq!(term.current_zone(), SemanticZone::Unknown);
    }

    #[test]
    fn osc133_prompt_start_with_bel() {
        let mut term = Terminal::new(Size::new(80, 24));

        // OSC 133;A with BEL terminator (prompt start)
        term.process_input(b"\x1b]133;A\x07");

        assert!(term.has_semantic_zones());
        assert_eq!(term.current_zone(), SemanticZone::Prompt);
        assert_eq!(term.get_line_zone(0), SemanticZone::Prompt);
    }

    #[test]
    fn osc133_prompt_start_with_st() {
        let mut term = Terminal::new(Size::new(80, 24));

        // OSC 133;A with ST terminator (ESC \)
        term.process_input(b"\x1b]133;A\x1b\\");

        assert!(term.has_semantic_zones());
        assert_eq!(term.current_zone(), SemanticZone::Prompt);
    }

    #[test]
    fn osc133_full_sequence() {
        let mut term = Terminal::new(Size::new(80, 24));

        // Simulate full shell integration sequence:
        // A = prompt start, B = command start, C = output start

        // Prompt start
        term.process_input(b"\x1b]133;A\x07");
        assert_eq!(term.current_zone(), SemanticZone::Prompt);

        // Some prompt text
        term.process_input(b"$ ");

        // Command start (user input begins)
        term.process_input(b"\x1b]133;B\x07");
        assert_eq!(term.current_zone(), SemanticZone::Input);

        // User types command and hits enter, then output starts
        term.process_input(b"ls -la\n");
        term.process_input(b"\x1b]133;C\x07");
        assert_eq!(term.current_zone(), SemanticZone::Output);

        // Output
        term.process_input(b"file1.txt\nfile2.txt\n");

        // Next prompt
        term.process_input(b"\x1b]133;A\x07");
        assert_eq!(term.current_zone(), SemanticZone::Prompt);
    }

    #[test]
    fn osc133_embedded_in_other_data() {
        let mut term = Terminal::new(Size::new(80, 24));

        // OSC 133 embedded in other text (as it would be from shell)
        term.process_input(b"some text\x1b]133;A\x07more text");

        assert!(term.has_semantic_zones());
        assert_eq!(term.current_zone(), SemanticZone::Prompt);
    }

    #[test]
    fn osc133_unknown_command_ignored() {
        let mut term = Terminal::new(Size::new(80, 24));

        // Unknown OSC 133 command (X) should be ignored
        term.process_input(b"\x1b]133;X\x07");

        // No zones should be set
        assert!(!term.has_semantic_zones());
        assert_eq!(term.current_zone(), SemanticZone::Unknown);
    }

    #[test]
    fn osc133_d_command_success() {
        let mut term = Terminal::new(Size::new(80, 24));

        // OSC 133;D;0 = command completed successfully
        term.process_input(b"\x1b]133;D;0\x07");

        let events = term.take_shell_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0], ShellEvent::CommandSuccess);
    }

    #[test]
    fn osc133_d_command_fail() {
        let mut term = Terminal::new(Size::new(80, 24));

        // OSC 133;D;1 = command failed with exit code 1
        term.process_input(b"\x1b]133;D;1\x07");

        let events = term.take_shell_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0], ShellEvent::CommandFail(1));
    }

    #[test]
    fn osc133_d_command_fail_with_larger_code() {
        let mut term = Terminal::new(Size::new(80, 24));

        // OSC 133;D;127 = command failed with exit code 127 (command not found)
        term.process_input(b"\x1b]133;D;127\x07");

        let events = term.take_shell_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0], ShellEvent::CommandFail(127));
    }

    #[test]
    fn osc133_d_no_exit_code_defaults_success() {
        let mut term = Terminal::new(Size::new(80, 24));

        // OSC 133;D without exit code should default to success (0)
        term.process_input(b"\x1b]133;D\x07");

        let events = term.take_shell_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0], ShellEvent::CommandSuccess);
    }

    #[test]
    fn shell_events_clear_after_take() {
        let mut term = Terminal::new(Size::new(80, 24));

        term.process_input(b"\x1b]133;D;0\x07");
        let events = term.take_shell_events();
        assert_eq!(events.len(), 1);

        // Second take should be empty
        let events = term.take_shell_events();
        assert!(events.is_empty());
    }

    #[test]
    fn multiple_shell_events_accumulated() {
        let mut term = Terminal::new(Size::new(80, 24));

        // Multiple commands
        term.process_input(b"\x1b]133;D;0\x07");
        term.process_input(b"\x1b]133;D;1\x07");

        let events = term.take_shell_events();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0], ShellEvent::CommandSuccess);
        assert_eq!(events[1], ShellEvent::CommandFail(1));
    }
}
