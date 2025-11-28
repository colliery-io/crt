//! Manager modules for application lifecycle
//!
//! This module contains manager types that coordinate higher-level
//! application concerns like window lifecycle and configuration management.

mod config_manager;
mod window_manager;

pub use config_manager::{ConfigChange, ConfigManager};
pub use window_manager::WindowManager;
