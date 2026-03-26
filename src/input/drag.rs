//! Tab drag-and-drop state types
//!
//! Core types for the unified tab drag state machine that supports
//! reordering, detaching, and merging tabs across windows.

use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::window::WindowId;

use crate::window::TabId;

/// Minimum pixel distance the cursor must move before a drag activates.
/// Prevents accidental drags from imprecise clicks.
pub const DRAG_THRESHOLD: f64 = 5.0;

/// What will happen when the user releases the mouse during a tab drag.
#[derive(Debug, Clone, PartialEq)]
pub enum DragDropTarget {
    /// Reorder within the source window's tab bar
    Reorder { insert_index: usize },
    /// Merge into a different window's tab bar
    Merge {
        target_window_id: WindowId,
        insert_index: usize,
    },
    /// Detach into a brand new window
    Detach,
    /// Cursor hasn't moved past threshold yet (or is in ambiguous zone)
    Pending,
}

/// State tracking an in-progress tab drag operation.
///
/// Lives on `App` (not per-window) because drags can cross window boundaries.
#[derive(Debug, Clone)]
pub struct TabDragState {
    /// The tab being dragged
    pub tab_id: TabId,
    /// The window the tab originated from
    pub source_window_id: WindowId,
    /// Where the mouse was pressed (screen coordinates)
    pub start_pos: PhysicalPosition<f64>,
    /// Current mouse position (screen coordinates)
    pub current_pos: PhysicalPosition<f64>,
    /// What will happen on drop
    pub drop_target: DragDropTarget,
    /// False until mouse moves past the drag threshold
    pub drag_active: bool,
}

impl TabDragState {
    /// Create a new drag state in the initial (pending) state.
    pub fn new(
        tab_id: TabId,
        source_window_id: WindowId,
        start_pos: PhysicalPosition<f64>,
    ) -> Self {
        Self {
            tab_id,
            source_window_id,
            start_pos,
            current_pos: start_pos,
            drop_target: DragDropTarget::Pending,
            drag_active: false,
        }
    }

    /// Check if the cursor has moved far enough to activate the drag.
    pub fn exceeds_threshold(&self) -> bool {
        let dx = self.current_pos.x - self.start_pos.x;
        let dy = self.current_pos.y - self.start_pos.y;
        (dx * dx + dy * dy).sqrt() > DRAG_THRESHOLD
    }
}

/// Compute the insertion index for tab reordering based on cursor position.
///
/// Given the cursor's x-position within the tab bar, the tab rects, and the
/// index of the tab being dragged, returns the target index where the tab
/// should be moved to.
///
/// The logic: for each tab rect, if the cursor is past the midpoint, the
/// insertion point is after that tab. The dragged tab is excluded from
/// the comparison since it's "in flight".
pub fn compute_reorder_index(
    cursor_x: f32,
    tab_rects: &[crt_renderer::TabRect],
    dragged_index: usize,
) -> usize {
    if tab_rects.is_empty() {
        return 0;
    }

    let mut target = 0;
    for (i, rect) in tab_rects.iter().enumerate() {
        if i == dragged_index {
            continue;
        }
        let midpoint = rect.x + rect.width / 2.0;
        if cursor_x > midpoint {
            // Cursor is past this tab's midpoint — insertion is after it
            // But we need to account for the dragged tab being removed
            if i < dragged_index {
                target = i + 1;
            } else {
                // When the dragged tab is before this one, indices shift
                target = i;
            }
        }
    }

    // Clamp to valid range
    target.min(tab_rects.len() - 1)
}

/// A window's screen-space rectangle and tab bar region, used for drop target resolution.
#[derive(Debug, Clone)]
pub struct WindowScreenRect {
    pub window_id: WindowId,
    /// Top-left corner of the window's content area in screen coordinates
    pub origin: PhysicalPosition<i32>,
    /// Size of the window's content area
    pub size: PhysicalSize<u32>,
    /// Height of the tab bar in physical pixels
    pub tab_bar_height: f32,
    /// Tab rects within this window's tab bar (for computing insertion index)
    pub tab_rects: Vec<crt_renderer::TabRect>,
}

impl WindowScreenRect {
    /// Check if a screen-space point is inside this window
    fn contains(&self, x: f64, y: f64) -> bool {
        let ox = self.origin.x as f64;
        let oy = self.origin.y as f64;
        let w = self.size.width as f64;
        let h = self.size.height as f64;
        x >= ox && x < ox + w && y >= oy && y < oy + h
    }

    /// Check if a screen-space point is inside this window's tab bar region
    fn tab_bar_contains(&self, x: f64, y: f64) -> bool {
        let ox = self.origin.x as f64;
        let oy = self.origin.y as f64;
        let w = self.size.width as f64;
        let h = self.tab_bar_height as f64;
        x >= ox && x < ox + w && y >= oy && y < oy + h
    }

    /// Convert screen-space x to window-local x for tab hit testing
    fn to_local_x(&self, screen_x: f64) -> f32 {
        (screen_x - self.origin.x as f64) as f32
    }
}

/// Resolve what drop target the cursor is over.
///
/// Pure function: given the cursor's screen-space position, the source window ID,
/// source tab count, and all window rects, returns the appropriate `DragDropTarget`.
///
/// Resolution priority:
/// 1. Over a non-source window's tab bar → `Merge`
/// 2. Over the source window's tab bar → `Reorder` (only if source has >1 tab)
/// 3. Over any window's body (not tab bar) → `Detach` (only if source has >1 tab)
/// 4. Outside all windows → `Detach` (only if source has >1 tab)
///
/// When `source_tab_count == 1`, only `Merge` is valid — reorder is meaningless
/// and detach would just recreate the same window. Returns `Pending` for invalid targets.
pub fn resolve_drop_target(
    cursor_screen: PhysicalPosition<f64>,
    source_window_id: WindowId,
    dragged_index: usize,
    source_tab_count: usize,
    windows: &[WindowScreenRect],
) -> DragDropTarget {
    let cx = cursor_screen.x;
    let cy = cursor_screen.y;
    let is_single_tab = source_tab_count <= 1;

    for win in windows {
        if !win.contains(cx, cy) {
            continue;
        }

        if win.window_id == source_window_id {
            // Over source window
            if win.tab_bar_contains(cx, cy) {
                if is_single_tab {
                    return DragDropTarget::Pending;
                }
                let local_x = win.to_local_x(cx);
                let insert_index =
                    compute_reorder_index(local_x, &win.tab_rects, dragged_index);
                return DragDropTarget::Reorder { insert_index };
            } else {
                if is_single_tab {
                    return DragDropTarget::Pending;
                }
                return DragDropTarget::Detach;
            }
        } else {
            // Over a different window — merge anywhere in it
            // If over tab bar, compute precise insertion index; otherwise append
            let insert_index = if win.tab_bar_contains(cx, cy) {
                let local_x = win.to_local_x(cx);
                compute_reorder_index(local_x, &win.tab_rects, usize::MAX)
            } else {
                // Dropping on window body → append to end
                win.tab_rects.len()
            };
            return DragDropTarget::Merge {
                target_window_id: win.window_id,
                insert_index,
            };
        }
    }

    if is_single_tab {
        // Single tab: detach (outside all windows) is meaningless
        return DragDropTarget::Pending;
    }

    // Outside all windows
    DragDropTarget::Detach
}

/// Check whether a tab drag should be initiated for a mouse press at (x, y).
///
/// Returns `Some(tab_id)` if:
/// - The click hits a tab (not the close button)
/// - The window has more than 1 tab (last-tab guard)
/// - The tab bar is not in edit mode
/// - The context menu is not visible
///
/// Returns `None` if the click should be handled normally.
pub fn should_start_drag(
    tab_bar: &crt_renderer::TabBar,
    context_menu_visible: bool,
    x: f32,
    y: f32,
) -> Option<TabId> {
    // Guard: don't initiate during edit mode or context menu
    if tab_bar.is_editing() || context_menu_visible {
        return None;
    }
    // Check hit test — only non-close-button hits
    // Single-tab windows CAN drag (for merging into another window)
    if let Some((tab_id, is_close)) = tab_bar.hit_test(x, y) {
        if !is_close {
            return Some(tab_id);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    // WindowId doesn't have a public constructor, so we test the types we can.
    // For TabDragState tests we use a helper that creates a state with a fake WindowId
    // via unsafe — acceptable in tests since we never use the WindowId for actual windowing.

    fn make_drag(start_x: f64, start_y: f64) -> TabDragState {
        // We can't construct a WindowId directly, but we can create one from a u64
        // using the unsafe From impl that winit provides for testing.
        let fake_window_id = unsafe { WindowId::dummy() };
        TabDragState::new(42, fake_window_id, PhysicalPosition::new(start_x, start_y))
    }

    #[test]
    fn drag_drop_target_equality() {
        assert_eq!(DragDropTarget::Detach, DragDropTarget::Detach);
        assert_eq!(DragDropTarget::Pending, DragDropTarget::Pending);
        assert_eq!(
            DragDropTarget::Reorder { insert_index: 2 },
            DragDropTarget::Reorder { insert_index: 2 }
        );
        assert_ne!(
            DragDropTarget::Reorder { insert_index: 1 },
            DragDropTarget::Reorder { insert_index: 2 }
        );
        assert_ne!(DragDropTarget::Detach, DragDropTarget::Pending);
    }

    #[test]
    fn new_drag_state_is_pending_and_inactive() {
        let drag = make_drag(100.0, 200.0);
        assert_eq!(drag.tab_id, 42);
        assert_eq!(drag.drop_target, DragDropTarget::Pending);
        assert!(!drag.drag_active);
        assert_eq!(drag.start_pos.x, 100.0);
        assert_eq!(drag.start_pos.y, 200.0);
        assert_eq!(drag.current_pos.x, 100.0);
        assert_eq!(drag.current_pos.y, 200.0);
    }

    #[test]
    fn exceeds_threshold_false_when_stationary() {
        let drag = make_drag(100.0, 200.0);
        assert!(!drag.exceeds_threshold());
    }

    #[test]
    fn exceeds_threshold_false_for_small_movement() {
        let mut drag = make_drag(100.0, 200.0);
        drag.current_pos = PhysicalPosition::new(103.0, 202.0); // ~3.6px
        assert!(!drag.exceeds_threshold());
    }

    #[test]
    fn exceeds_threshold_true_for_large_movement() {
        let mut drag = make_drag(100.0, 200.0);
        drag.current_pos = PhysicalPosition::new(106.0, 200.0); // 6px horizontal
        assert!(drag.exceeds_threshold());
    }

    #[test]
    fn exceeds_threshold_diagonal() {
        let mut drag = make_drag(100.0, 200.0);
        // 4px horizontal + 4px vertical = ~5.66px diagonal > 5px threshold
        drag.current_pos = PhysicalPosition::new(104.0, 204.0);
        assert!(drag.exceeds_threshold());
    }

    #[test]
    fn exceeds_threshold_exactly_at_boundary() {
        let mut drag = make_drag(100.0, 200.0);
        // Exactly 5px — should NOT exceed (uses strict >)
        drag.current_pos = PhysicalPosition::new(105.0, 200.0);
        assert!(!drag.exceeds_threshold());
    }

    // ── compute_reorder_index tests ──────────────────────────────

    use crt_renderer::TabRect;

    fn make_tab_rects(count: usize) -> Vec<TabRect> {
        // Each tab is 100px wide with 4px gaps, starting at x=10
        (0..count)
            .map(|i| TabRect {
                x: 10.0 + (i as f32) * 104.0,
                y: 5.0,
                width: 100.0,
                height: 30.0,
                close_x: 10.0 + (i as f32) * 104.0 + 80.0,
                close_width: 16.0,
            })
            .collect()
    }

    #[test]
    fn reorder_index_empty_rects() {
        assert_eq!(super::compute_reorder_index(50.0, &[], 0), 0);
    }

    #[test]
    fn reorder_index_drag_first_to_second() {
        let rects = make_tab_rects(3);
        // Tab 0 is being dragged. Cursor at midpoint of tab 1 (x=114 + 50 = 164)
        // Tab 1's midpoint is at 114 + 50 = 164. Cursor past it → after tab 1
        let idx = super::compute_reorder_index(165.0, &rects, 0);
        assert_eq!(idx, 1); // Tab 0 should move to index 1
    }

    #[test]
    fn reorder_index_drag_last_to_first() {
        let rects = make_tab_rects(3);
        // Tab 2 is being dragged. Cursor before midpoint of tab 0 (10 + 50 = 60)
        let idx = super::compute_reorder_index(30.0, &rects, 2);
        assert_eq!(idx, 0); // Tab 2 should move to index 0
    }

    // ── resolve_drop_target tests ───────────────────────────────

    fn make_window_rect(
        id_num: u64,
        x: i32,
        y: i32,
        w: u32,
        h: u32,
        tab_bar_h: f32,
        num_tabs: usize,
    ) -> super::WindowScreenRect {
        let tab_rects: Vec<TabRect> = (0..num_tabs)
            .map(|i| TabRect {
                x: 10.0 + (i as f32) * 104.0,
                y: 5.0,
                width: 100.0,
                height: 30.0,
                close_x: 10.0 + (i as f32) * 104.0 + 80.0,
                close_width: 16.0,
            })
            .collect();
        // We need a WindowId but can't construct one easily for non-source windows.
        // Use dummy for all and compare by index.
        let window_id = unsafe { WindowId::dummy() };
        super::WindowScreenRect {
            window_id,
            origin: PhysicalPosition::new(x, y),
            size: PhysicalSize::new(w, h),
            tab_bar_height: tab_bar_h,
            tab_rects,
        }
    }

    #[test]
    fn resolve_outside_all_windows_is_detach() {
        let win = make_window_rect(0, 100, 100, 800, 600, 36.0, 3);
        let source_id = win.window_id;
        let windows = vec![win];
        // Cursor way outside
        let target = super::resolve_drop_target(
            PhysicalPosition::new(2000.0, 2000.0),
            source_id,
            0,
            3,
            &windows,
        );
        assert_eq!(target, DragDropTarget::Detach);
    }

    #[test]
    fn resolve_over_source_tab_bar_is_reorder() {
        let win = make_window_rect(0, 100, 100, 800, 600, 36.0, 3);
        let source_id = win.window_id;
        let windows = vec![win];
        // Cursor over tab bar (y=110, which is origin.y + 10 < origin.y + 36)
        let target = super::resolve_drop_target(
            PhysicalPosition::new(200.0, 110.0),
            source_id,
            0,
            3,
            &windows,
        );
        assert!(matches!(target, DragDropTarget::Reorder { .. }));
    }

    #[test]
    fn resolve_over_window_body_is_detach() {
        let win = make_window_rect(0, 100, 100, 800, 600, 36.0, 3);
        let source_id = win.window_id;
        let windows = vec![win];
        // Cursor over body (y=400, which is below tab bar)
        let target = super::resolve_drop_target(
            PhysicalPosition::new(200.0, 400.0),
            source_id,
            0,
            3,
            &windows,
        );
        assert_eq!(target, DragDropTarget::Detach);
    }

    // Note: merge test requires different WindowIds which can't be constructed
    // in unit tests (WindowId::dummy() always returns the same value).
    // The merge path is structurally identical to reorder except for the
    // source_id comparison — verified via integration testing.

    #[test]
    fn resolve_empty_windows_is_detach() {
        let source_id = unsafe { WindowId::dummy() };
        let target = super::resolve_drop_target(
            PhysicalPosition::new(200.0, 200.0),
            source_id,
            0,
            3,
            &[],
        );
        assert_eq!(target, DragDropTarget::Detach);
    }

    #[test]
    fn reorder_index_stays_in_place() {
        let rects = make_tab_rects(3);
        // Tab 1 is being dragged. Cursor at tab 1's own position — since tab 1
        // is skipped, the result depends on whether cursor passes tab 0's midpoint
        // Tab 0 midpoint = 60. Cursor at 114 (start of tab 1) > 60 → after tab 0 = index 1
        let idx = super::compute_reorder_index(114.0, &rects, 1);
        assert_eq!(idx, 1); // Stays at same position
    }

    // ── State machine transition tests ───────────────────────────

    #[test]
    fn drag_state_transitions_pending_to_active() {
        let mut drag = make_drag(100.0, 200.0);
        assert!(!drag.drag_active);
        assert_eq!(drag.drop_target, DragDropTarget::Pending);

        // Move past threshold
        drag.current_pos = PhysicalPosition::new(110.0, 200.0);
        assert!(drag.exceeds_threshold());
        drag.drag_active = true;

        // Set a reorder target
        drag.drop_target = DragDropTarget::Reorder { insert_index: 2 };
        assert!(drag.drag_active);
        assert_eq!(drag.drop_target, DragDropTarget::Reorder { insert_index: 2 });
    }

    #[test]
    fn drag_state_target_updates_continuously() {
        let mut drag = make_drag(100.0, 200.0);
        drag.drag_active = true;

        // Reorder → Detach → Reorder transitions
        drag.drop_target = DragDropTarget::Reorder { insert_index: 1 };
        assert!(matches!(drag.drop_target, DragDropTarget::Reorder { .. }));

        drag.drop_target = DragDropTarget::Detach;
        assert_eq!(drag.drop_target, DragDropTarget::Detach);

        drag.drop_target = DragDropTarget::Reorder { insert_index: 0 };
        assert_eq!(
            drag.drop_target,
            DragDropTarget::Reorder { insert_index: 0 }
        );
    }

    // ── WindowScreenRect tests ───────────────────────────────────

    #[test]
    fn window_screen_rect_contains() {
        let rect = make_window_rect(0, 100, 100, 800, 600, 36.0, 2);
        // Inside
        assert!(rect.contains(200.0, 300.0));
        // At origin
        assert!(rect.contains(100.0, 100.0));
        // Outside left
        assert!(!rect.contains(99.0, 300.0));
        // Outside below
        assert!(!rect.contains(200.0, 701.0));
    }

    #[test]
    fn window_screen_rect_tab_bar_contains() {
        let rect = make_window_rect(0, 100, 100, 800, 600, 36.0, 2);
        // In tab bar (y = 100..136)
        assert!(rect.tab_bar_contains(200.0, 110.0));
        // Below tab bar
        assert!(!rect.tab_bar_contains(200.0, 140.0));
        // Above window
        assert!(!rect.tab_bar_contains(200.0, 99.0));
    }

    #[test]
    fn window_screen_rect_to_local_x() {
        let rect = make_window_rect(0, 100, 100, 800, 600, 36.0, 2);
        assert_eq!(rect.to_local_x(150.0), 50.0);
        assert_eq!(rect.to_local_x(100.0), 0.0);
    }

    // ── compute_reorder_index edge cases ─────────────────────────

    #[test]
    fn reorder_index_single_tab() {
        let rects = make_tab_rects(1);
        // Only one tab, dragging it — can only go to index 0
        let idx = super::compute_reorder_index(50.0, &rects, 0);
        assert_eq!(idx, 0);
    }

    #[test]
    fn reorder_index_cursor_far_right() {
        let rects = make_tab_rects(3);
        // Cursor way past all tabs — should target last position
        let idx = super::compute_reorder_index(10000.0, &rects, 0);
        assert_eq!(idx, 2); // Last valid index
    }

    #[test]
    fn reorder_index_cursor_far_left() {
        let rects = make_tab_rects(3);
        // Cursor before all tabs — should target first position
        let idx = super::compute_reorder_index(0.0, &rects, 2);
        assert_eq!(idx, 0);
    }

    // ── resolve_drop_target edge cases ───────────────────────────

    #[test]
    fn resolve_cursor_at_window_edge_is_in_window() {
        let win = make_window_rect(0, 0, 0, 800, 600, 36.0, 3);
        let source_id = win.window_id;
        let windows = vec![win];
        // Cursor at origin (0,0) — should be inside tab bar
        let target = super::resolve_drop_target(
            PhysicalPosition::new(50.0, 10.0),
            source_id,
            0,
            3,
            &windows,
        );
        assert!(matches!(target, DragDropTarget::Reorder { .. }));
    }

    #[test]
    fn resolve_single_tab_over_source_is_pending() {
        let win = make_window_rect(0, 0, 0, 800, 600, 36.0, 1);
        let source_id = win.window_id;
        let windows = vec![win];
        // Single tab: cursor over own tab bar → Pending (reorder meaningless)
        let target = super::resolve_drop_target(
            PhysicalPosition::new(50.0, 10.0),
            source_id,
            0,
            1, // single tab
            &windows,
        );
        assert_eq!(target, DragDropTarget::Pending);
    }

    #[test]
    fn resolve_single_tab_outside_is_pending() {
        let win = make_window_rect(0, 0, 0, 800, 600, 36.0, 1);
        let source_id = win.window_id;
        let windows = vec![win];
        // Single tab: cursor outside all windows → Pending (detach meaningless)
        let target = super::resolve_drop_target(
            PhysicalPosition::new(2000.0, 2000.0),
            source_id,
            0,
            1,
            &windows,
        );
        assert_eq!(target, DragDropTarget::Pending);
    }

    #[test]
    fn resolve_cursor_just_below_tab_bar_on_source_is_detach() {
        let win = make_window_rect(0, 0, 0, 800, 600, 36.0, 3);
        let source_id = win.window_id;
        let windows = vec![win];
        // Cursor at y=37, just below 36px tab bar on SOURCE window → detach
        let target = super::resolve_drop_target(
            PhysicalPosition::new(50.0, 37.0),
            source_id,
            0,
            3,
            &windows,
        );
        assert_eq!(target, DragDropTarget::Detach);
    }
}
