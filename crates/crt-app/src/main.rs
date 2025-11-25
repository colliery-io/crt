//! CRT - GPU-accelerated terminal with CSS theming

fn main() {
    env_logger::init();

    log::info!("CRT Terminal starting...");

    crt_core::init();
    crt_theme::init();
    crt_renderer::init();

    log::info!("CRT Terminal initialized");
}
