---
id: tab-detachment-to-new-window
level: initiative
title: "Tab Detachment to New Window"
short_code: "CRT-I-0026"
created_at: 2025-12-31T14:28:08.984646+00:00
updated_at: 2025-12-31T14:28:08.984646+00:00
parent: CRT-V-0001
blocked_by: []
archived: false

tags:
  - "#initiative"
  - "#phase/discovery"


exit_criteria_met: false
estimated_complexity: L
strategy_id: NULL
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
- Drag a tab outside the window bounds to detach it into a new window
- The shell/PTY session continues uninterrupted during detachment
- New window inherits the tab's current working directory and shell state
- Visual feedback during drag (ghost tab, drop indicator)

**Non-Goals:**
- Tab reordering within a window (separate initiative, simpler)
- Merging tabs from different windows (reverse operation - future work)
- Dragging tabs between existing windows (complex drop targeting)
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

### Detachment Flow
```
1. Mouse down on tab → Start potential drag
2. Mouse move → Track drag state, show ghost if threshold exceeded
3. Mouse exits window bounds → Detach mode activated
4. Mouse up outside window → Execute detachment:
   a. Remove tab from source TabBar
   b. Remove ShellTerminal from source shells HashMap
   c. Create new window
   d. Add tab to new TabBar
   e. Add ShellTerminal to new shells HashMap
   f. Focus new window at mouse position
```

## Detailed Design

### New Types

```rust
// src/input/drag.rs (new)
pub struct TabDragState {
    pub tab_id: TabId,
    pub source_window_id: WindowId,
    pub start_pos: PhysicalPosition<f64>,
    pub current_pos: PhysicalPosition<f64>,
    pub is_detaching: bool,  // true when cursor outside window
}

// In WindowState or App
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

### Phase 1: Drag State Infrastructure
- Add `TabDragState` struct
- Modify mouse handling to track potential drags
- Add drag threshold (prevent accidental drags)

### Phase 2: Window Bounds Detection  
- Track when cursor exits window during drag
- Set `is_detaching` flag appropriately
- Handle multi-monitor scenarios

### Phase 3: Tab/Shell Extraction
- Add `TabBarState::remove_tab()` method
- Implement shell extraction from source window
- Handle "last tab" case (close source window)

### Phase 4: New Window Creation
- Create window at cursor position
- Initialize with extracted tab and shell
- Focus new window

### Phase 5: Visual Feedback
- Ghost tab rendering during drag
- Detachment indicator when outside window
- Smooth animation/transition

### Phase 6: Edge Cases & Polish
- Cancel drag with Escape key
- Handle drag to screen edge
- Multi-monitor support
- Test PTY stability during transfer