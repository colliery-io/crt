//! CRT - GPU-accelerated terminal with CSS theming
//!
//! This is the root crate containing examples/prototypes.
//! The actual implementation lives in:
//! - `crt-core` - Terminal emulation and PTY
//! - `crt-renderer` - GPU rendering
//! - `crt-theme` - CSS theming
//! - `crt-app` - Application shell
//!
//! Run prototypes:
//! ```sh
//! cargo run --example prototype_a
//! cargo run --example synthwave
//! cargo run --example font_rendering
//! ```

// Input command types (side-effect-free representations of user actions)
#[path = "input/commands.rs"]
mod commands;
pub use commands::{Command, SelectionMode};
