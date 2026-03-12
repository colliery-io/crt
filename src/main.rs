//! CRT Terminal with Effects
//!
//! Two-pass rendering:
//! 1. Render text to offscreen texture using swash-based glyph cache
//! 2. Composite text with effects (gradient, grid, glow) to screen

mod app;
mod config;
mod font;
mod gpu;
mod input;
mod menu;
pub mod profiling;
mod render;
mod theme_registry;
mod watcher;
mod window;

use winit::event_loop::{ControlFlow, EventLoop};

fn main() {
    // Enable debug logging when profiling is enabled
    let profiling_enabled = std::env::var("CRT_PROFILE").is_ok();
    let default_filter = if profiling_enabled {
        "warn,crt=debug,crt_renderer=debug,crt_theme=debug,crt_core=debug"
    } else {
        "warn,crt=info"
    };

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(default_filter))
        .init();

    if profiling_enabled {
        log::info!("CRT Terminal starting (profiling mode - debug logging enabled)");
    } else {
        log::info!("CRT Terminal starting");
    }

    // Initialize profiling (enabled via CRT_PROFILE=1)
    profiling::init();

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);
    event_loop.run_app(&mut app::App::new()).unwrap();

    // Flush profiling data on exit
    profiling::shutdown();
}
