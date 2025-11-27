//! Shader module - WGSL shaders for GPU rendering
//!
//! Shaders are stored as external .wgsl files and included at compile time.
//! This enables better IDE support (syntax highlighting, validation) while
//! keeping the binary self-contained.

/// Built-in shaders included at compile time
pub mod builtin {
    /// Background shader - renders gradient + animated perspective grid
    pub const BACKGROUND: &str = include_str!("background.wgsl");

    /// Background image shader - renders textured background with sizing/positioning
    pub const BACKGROUND_IMAGE: &str = include_str!("background_image.wgsl");

    /// Composite shader - applies glow blur to text texture (25-sample Gaussian)
    pub const COMPOSITE: &str = include_str!("composite.wgsl");

    /// Grid shader - GPU-accelerated text glyph rendering using instanced quads
    pub const GRID: &str = include_str!("grid.wgsl");

    /// Rect shader - solid color rectangle rendering using instanced quads
    pub const RECT: &str = include_str!("rect.wgsl");
}
