//! Application state modules
//!
//! This module contains pure state management types that are independent of
//! GPU, window, and terminal implementations. This allows for easy unit testing.

mod selection;
mod tab_state;
mod ui_state;

pub use selection::{LineRange, SelectionMode, SelectionState, selection_to_ranges};
pub use tab_state::{TabId, TabInfo, TabState};
pub use ui_state::{BellState, ContextMenuState, SearchMatch, SearchState, UiState};
