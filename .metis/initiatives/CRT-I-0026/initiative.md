---
id: tab-detachment-to-new-window
level: initiative
title: "Tab Detachment to New Window"
short_code: "CRT-I-0026"
created_at: 2025-12-31T14:28:08.984646+00:00
updated_at: 2026-03-26T16:20:41.720779+00:00
parent: CRT-V-0001
blocked_by: []
archived: false

tags:
  - "#initiative"
  - "#phase/active"


exit_criteria_met: false
estimated_complexity: L
initiative_id: tab-detachment-to-new-window
---

# Tab Detachment to New Window Initiative

## Context

Currently, tabs in CRT are fixed to their parent window. Users cannot reorganize tabs across windows or detach a tab to create a new window. This limits workflow flexibility - for example, when you want to move a long-running process to its own window or compare two terminal sessions side-by-side.

The current tab implementation (`crt-renderer/src/tab_bar/`) handles:
- Tab creation/deletion within a window
- Tab selection and switching
- Tab title management
- Basic click interactions (select, close button)

However, there's no drag-and-drop infrastructure for tabs.

## Goals & Non-Goals

**Goals:**
- **Tab reordering** — drag within the tab bar to rearrange tab order
- **Tab detachment** — drag a tab outside the window bounds to create a new window
- **Tab merging** — drag a tab onto an existing window's tab bar to move it there
- The shell/PTY session continues uninterrupted during all drag operations
- New window inherits the tab's current working directory and shell state
- Visual feedback during drag (ghost tab, drop indicator, insertion caret for reorder/merge)
- Unified drag state machine shared across all three interactions

**Non-Goals:**
- Drag to dock/taskbar (OS-specific, out of scope)

## Use Cases

### Use Case 1: Detach Long-Running Process
- **Actor**: Developer running a build/test process
- **Scenario**: 
  1. User has multiple tabs, one running a long build
  2. User wants to monitor build while working in other tabs
  3. User drags build tab outside window
  4. New window appears with just the build tab
- **Expected Outcome**: Build continues uninterrupted in new window

### Use Case 2: Side-by-Side Comparison
- **Actor**: User comparing logs or outputs
- **Scenario**:
  1. User has two tabs with different log files
  2. User drags one tab out to create second window
  3. User arranges windows side-by-side
- **Expected Outcome**: Both terminals visible simultaneously

## Architecture

### Current Tab/Shell Relationship
```
WindowState
├── gpu: WindowGpuState
│   └── tab_bar: TabBar (owns Tab structs - UI only)
└── shells: HashMap<TabId, ShellTerminal>  (owns PTY connections)
```

### Key Insight
The `ShellTerminal` (PTY handle) is independent of the window. We can:
1. Remove it from source window's `shells` HashMap
2. Add it to destination window's `shells` HashMap
3. PTY file descriptors remain valid across this move

### Unified Drag State Machine
```
1. Mouse down on tab → Start potential drag (record start position)
2. Mouse move beyond threshold → Activate drag (show ghost tab at cursor)
3. Determine drop target based on cursor position:
   a. Over same window's tab bar → REORDER mode (show insertion caret)
   b. Over different window's tab bar → MERGE mode (show insertion caret on target)
   c. Outside all window bounds → DETACH mode (show detach indicator)
4. Mouse up → Execute action based on mode:
   - REORDER: Move tab to new index in same TabBar
   - MERGE: Extract tab+shell from source, insert into target window
   - DETACH: Extract tab+shell from source, create new window at cursor
5. Escape key at any point → Cancel drag, restore original state
```

### Drop Target Resolution
The app must track all window positions/sizes to resolve which window (if any)
the cursor is over. On macOS, `winit` provides `outer_position()` and
`inner_size()` per window. The drop target resolver checks windows in
front-to-back z-order and further narrows to "is cursor over the tab bar region"
to distinguish merge vs. detach when cursor is over a window but not its tab bar.

## Detailed Design

### New Types

```rust
// src/input/drag.rs (new)

/// What will happen when the user releases the mouse
#[derive(Debug, Clone, PartialEq)]
pub enum DragDropTarget {
    /// Reorder within the source window's tab bar
    Reorder { insert_index: usize },
    /// Merge into a different window's tab bar
    Merge { target_window_id: WindowId, insert_index: usize },
    /// Detach into a brand new window
    Detach,
    /// Cursor hasn't moved past threshold yet (or is in ambiguous zone)
    Pending,
}

pub struct TabDragState {
    pub tab_id: TabId,
    pub source_window_id: WindowId,
    pub start_pos: PhysicalPosition<f64>,
    pub current_pos: PhysicalPosition<f64>,
    pub drop_target: DragDropTarget,
    pub drag_active: bool,  // false until threshold exceeded
}

// Lives on App, not WindowState — needs cross-window visibility
pub drag_state: Option<TabDragState>,
```

### Mouse Event Handling Changes

**`src/input/mouse.rs` modifications:**

```rust
fn handle_mouse_pressed(/* ... */) {
    if let Some((tab_id, is_close)) = tab_bar.hit_test(x, y) {
        if !is_close {
            // Start potential drag instead of immediate select
            self.drag_state = Some(TabDragState::new(tab_id, window_id, pos));
        }
    }
}

fn handle_mouse_moved(/* ... */) {
    if let Some(ref mut drag) = self.drag_state {
        drag.current_pos = pos;
        
        // Check if cursor left window bounds
        let window_rect = /* get window rect */;
        drag.is_detaching = !window_rect.contains(pos);
        
        // Request redraw for ghost tab rendering
        window.request_redraw();
    }
}

fn handle_mouse_released(/* ... */) {
    if let Some(drag) = self.drag_state.take() {
        if drag.is_detaching {
            self.detach_tab(drag.tab_id, drag.source_window_id, drag.current_pos);
        } else {
            // Normal click - select tab
            self.select_tab(drag.tab_id);
        }
    }
}
```

### Tab Detachment Implementation

```rust
// src/main.rs or dedicated module
fn detach_tab(
    &mut self,
    tab_id: TabId,
    source_window_id: WindowId,
    screen_pos: PhysicalPosition<f64>,
) {
    let source = self.windows.get_mut(&source_window_id).unwrap();
    
    // 1. Extract tab data
    let tab = source.gpu.tab_bar.remove_tab(tab_id);
    let shell = source.shells.remove(&tab_id).unwrap();
    
    // 2. If source window now empty, close it
    if source.gpu.tab_bar.tabs().is_empty() {
        self.close_window(source_window_id);
    }
    
    // 3. Create new window at cursor position
    let new_window_id = self.create_window_at(screen_pos);
    let new_window = self.windows.get_mut(&new_window_id).unwrap();
    
    // 4. Add tab and shell to new window
    new_window.gpu.tab_bar.add_existing_tab(tab);
    new_window.shells.insert(tab_id, shell);
    new_window.gpu.tab_bar.select_tab(tab_id);
}
```

### Visual Feedback

**Ghost Tab Rendering:**
- When `drag_state.is_some()`, render a semi-transparent copy of tab at cursor
- Use existing tab rendering with alpha modification
- Consider OS-level drag image if available (platform-specific)

**Drop Indicator:**
- When cursor outside window, show visual cue (glow, outline) indicating detachment will occur
- Could tint ghost tab or add icon

### TabBarState Extensions

```rust
impl TabBarState {
    // New method to remove and return tab data
    pub fn remove_tab(&mut self, id: TabId) -> Option<Tab> {
        let idx = self.tabs.iter().position(|t| t.id == id)?;
        Some(self.tabs.remove(idx))
    }
    
    // New method to add existing tab (preserves id)
    pub fn add_existing_tab(&mut self, tab: Tab) {
        self.tabs.push(tab);
    }
}
```

## Alternatives Considered

1. **Native OS Drag-and-Drop** - Rejected: Would require serializing PTY state, overly complex
2. **Keyboard shortcut only** - Rejected: Less intuitive, doesn't match user expectations from browsers/IDEs
3. **Context menu "Move to New Window"** - Consider as complement, not replacement for drag

## Implementation Plan

### Phase 1: Drag State Infrastructure & Drop Target Resolution
- Add `TabDragState`, `DragDropTarget` types in `src/input/drag.rs`
- Modify mouse handling to track potential drags with threshold
- Build drop target resolver: given cursor screen position + all window rects, return `DragDropTarget`
- Drag state lives on `App` (not per-window) for cross-window visibility

### Phase 2: Tab Reordering (simplest case first)
- Implement `TabBarState::move_tab(from_index, to_index)`
- Render insertion caret between tabs during reorder drag
- Wire up mouse release → reorder execution
- No cross-window concerns yet — validates the drag infra end-to-end

### Phase 3: Tab/Shell Extraction & Detachment
- Add `TabBarState::remove_tab()` and `add_existing_tab()` methods
- Implement shell extraction from source window's `shells` HashMap
- Last-tab guard: if source window has only one tab, drag is not initiated (no detach/merge allowed)
- Create new window at cursor position with extracted tab+shell
- Verify PTY stability during transfer

### Phase 4: Tab Merging (cross-window drop)
- Extend drop target resolver to hit-test other windows' tab bar regions
- Reuse extraction logic from Phase 3, but insert into existing window
- Render insertion caret on the target window's tab bar during hover
- Handle edge case: source and target are the same window (→ reorder)

### Phase 5: Visual Feedback & Polish
- Ghost tab rendering at cursor during active drag
- Distinct visual cues per mode (reorder caret, merge caret, detach glow)
- Smooth animation/transition on drop
- Cancel drag with Escape key

### Phase 6: Edge Cases
- Multi-monitor support (cursor leaves all screens)
- Drag to screen edge
- Window z-order accuracy for drop target resolution
- Test suite for drag state machine transitions

## Testing Policy

**After completing every task**, run the full test suite before marking it done:
```
cargo test --workspace
```
All existing tests must pass. Any new functionality must include unit tests. No task is complete until the full suite is green.