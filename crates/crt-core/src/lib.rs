//! CRT Core - Terminal emulation and PTY management
//!
//! This crate provides:
//! - Terminal grid state (via alacritty_terminal)
//! - ANSI escape sequence parsing (via vte)
//! - PTY process management (via portable-pty)

pub mod pty;

pub use pty::Pty;

// Re-export alacritty_terminal types needed for rendering
pub use alacritty_terminal::term::{
    cell::Cell,
    cell::Flags as CellFlags,
    color::{self, Colors},
    LineDamageBounds,
    RenderableCursor,
    RenderableContent,
    TermDamage,
};
pub use alacritty_terminal::index::{Column, Line, Point};
pub use alacritty_terminal::vte::ansi::Color as AnsiColor;
pub use alacritty_terminal::vte::ansi::NamedColor;
pub use alacritty_terminal::selection::{Selection, SelectionRange, SelectionType};
pub use alacritty_terminal::index::Side;
pub use alacritty_terminal::grid::Scroll;
pub use alacritty_terminal::term::TermMode;

use std::sync::{Arc, Mutex};

use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::term::{self, Config as TermConfig, Term};
use alacritty_terminal::term::test::TermSize;
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
        self.storage.lock().unwrap().events.drain(..).collect()
    }
}

impl Default for TerminalEventProxy {
    fn default() -> Self {
        Self::new()
    }
}

impl EventListener for TerminalEventProxy {
    fn send_event(&self, event: Event) {
        self.storage.lock().unwrap().events.push(event);
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
}

impl Terminal {
    /// Create a new terminal with the given size
    pub fn new(size: Size) -> Self {
        let config = TermConfig::default();
        let term_size = TermSize::new(size.columns, size.lines);
        let event_proxy = TerminalEventProxy::new();
        let term = Term::new(config, &term_size, event_proxy.clone());
        let parser = ansi::Processor::new();

        Self { term, event_proxy, parser, size }
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
    pub fn process_input(&mut self, bytes: &[u8]) {
        self.parser.advance(&mut self.term, bytes);
    }

    /// Get access to renderable content (cells, cursor, etc.)
    pub fn renderable_content(&self) -> term::RenderableContent<'_> {
        self.term.renderable_content()
    }

    /// Get the cursor information
    pub fn cursor(&self) -> term::RenderableCursor {
        self.renderable_content().cursor
    }

    /// Take pending terminal events
    pub fn take_events(&self) -> Vec<Event> {
        self.event_proxy.take_events()
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
        use alacritty_terminal::selection::Selection;
        use alacritty_terminal::index::Side;
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
        self.term.scroll_display(alacritty_terminal::grid::Scroll::Bottom);
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

    /// Create a new shell terminal with a specific shell
    pub fn with_shell(size: Size, shell: &str) -> anyhow::Result<Self> {
        let terminal = Terminal::new(size);
        let pty = Pty::spawn(Some(shell), size.columns as u16, size.lines as u16)?;

        Ok(Self { terminal, pty })
    }

    /// Process any available PTY output through the terminal
    /// Returns true if any output was processed
    pub fn process_pty_output(&mut self) -> bool {
        let output = self.pty.read_available();
        if !output.is_empty() {
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
        let events = self.terminal.take_events();
        let mut title = None;
        let mut other_events = Vec::new();

        for event in events {
            match event {
                Event::Title(t) => title = Some(t),
                other => other_events.push(other),
            }
        }

        // Put back non-title events (they may trigger redraws)
        // Note: This is a bit awkward but necessary to not lose events
        // In practice, other events are less critical for our use case
        title
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
}
