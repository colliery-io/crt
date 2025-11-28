//! Manager modules for application lifecycle
//!
//! This module contains manager types that coordinate higher-level
//! application concerns like window lifecycle management.

mod window_manager;

pub use window_manager::WindowManager;
