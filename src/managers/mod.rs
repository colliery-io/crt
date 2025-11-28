//! Manager modules for application lifecycle
//!
//! This module contains manager types that coordinate higher-level
//! application concerns like window lifecycle, configuration, and event handling.

mod config_manager;
mod event_handler;
mod window_manager;

pub use config_manager::{ConfigChange, ConfigManager};
pub use event_handler::{EventHandler, KeyboardResult, ModifierState, MouseResult};
pub use window_manager::WindowManager;
