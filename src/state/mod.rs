//! Application state modules
//!
//! This module contains pure state management types that are independent of
//! GPU, window, and terminal implementations. This allows for easy unit testing.

mod selection;
mod tab_state;

pub use selection::{selection_to_ranges, LineRange, SelectionMode, SelectionState};
pub use tab_state::{TabId, TabInfo, TabState};
