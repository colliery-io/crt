//! CRT - GPU-accelerated terminal with CSS theming

use crt_core::{Terminal, Size};

fn main() {
    env_logger::init();

    log::info!("CRT Terminal starting...");

    // Create a terminal with standard dimensions
    let terminal = Terminal::new(Size::new(80, 24));
    log::info!("Terminal created: {}x{}", terminal.columns(), terminal.screen_lines());

    log::info!("CRT Terminal initialized");
}
