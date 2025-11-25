//! CRT Core - Terminal emulation and PTY management
//!
//! This crate provides:
//! - Terminal grid state (via alacritty_terminal)
//! - ANSI escape sequence parsing (via vte)
//! - PTY process management

pub fn init() {
    log::info!("crt-core initialized");
}
