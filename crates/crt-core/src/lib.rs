//! CRT Core - Terminal emulation and PTY management
//!
//! This crate provides:
//! - Terminal grid state (via alacritty_terminal)
//! - ANSI escape sequence parsing (via vte)
//! - PTY process management (future)

use std::sync::{Arc, Mutex};

use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::term::{self, Config, Term};
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
    size: Size,
}

impl Terminal {
    /// Create a new terminal with the given size
    pub fn new(size: Size) -> Self {
        let config = Config::default();
        let term_size = TermSize::new(size.columns, size.lines);
        let event_proxy = TerminalEventProxy::new();
        let term = Term::new(config, &term_size, event_proxy.clone());

        Self { term, event_proxy, size }
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
        let mut parser: ansi::Processor = ansi::Processor::new();
        parser.advance(&mut self.term, bytes);
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
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
