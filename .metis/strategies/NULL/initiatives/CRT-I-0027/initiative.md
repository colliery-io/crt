---
id: new-window-cwd-inheritance
level: initiative
title: "New Window CWD Inheritance"
short_code: "CRT-I-0027"
created_at: 2025-12-31T14:28:09.071025+00:00
updated_at: 2025-12-31T14:36:42.857857+00:00
parent: CRT-V-0001
blocked_by: []
archived: false

tags:
  - "#initiative"
  - "#phase/completed"


exit_criteria_met: false
estimated_complexity: S
strategy_id: NULL
initiative_id: new-window-cwd-inheritance
---

# New Window CWD Inheritance Initiative

## Context

Currently, new **tabs** in CRT inherit the working directory from the active tab - this works well. However, new **windows** always start in the default directory specified in `config.toml` (or home directory if not configured).

This creates friction when working in a project directory: opening a new window via Cmd+N drops you in `~` instead of the project directory you're working in. Users expect the new window to behave like a new tab - starting where they currently are.

### Existing Implementation

**New Tab (already works):**
```rust
// src/main.rs:549-566
let cwd = state.active_shell_cwd();  // Gets CWD from active tab's PTY
let spawn_options = SpawnOptions {
    shell: shell_program,
    cwd,  // Passed to new shell
    ...
};
```

**New Window (needs fix):**
```rust
// Currently uses config default, not active window's CWD
let cwd = self.config.shell.working_directory.clone();
```

## Goals & Non-Goals

**Goals:**
- New windows start in the active window's current working directory
- If no window is active/focused, fall back to config default
- Consistent behavior between new tabs and new windows

**Non-Goals:**
- Persisting CWD across app restarts
- Complex CWD history/picker UI
- Per-profile CWD settings

## Detailed Design

### Implementation Approach

The fix is straightforward - we need to:

1. **Track the focused window** - Know which window is currently active
2. **Query its CWD** - Use existing `active_shell_cwd()` method
3. **Pass to new window** - Use that CWD in `SpawnOptions`

### Code Changes

**1. Ensure focused window tracking exists:**

The app likely already tracks which window is focused via winit's `WindowEvent::Focused`. Verify this exists in `App` state.

```rust
// In App struct (may already exist)
pub focused_window: Option<WindowId>,
```

**2. Modify new window creation:**

```rust
// src/main.rs - handle_new_window() or equivalent
fn create_new_window(&mut self) {
    // Get CWD from focused window if available
    let cwd = self.focused_window
        .and_then(|id| self.windows.get(&id))
        .and_then(|state| state.active_shell_cwd())
        .or_else(|| self.config.shell.working_directory.clone());
    
    let spawn_options = SpawnOptions {
        shell: self.config.shell.program.clone(),
        cwd,
        semantic_prompts: self.config.shell.semantic_prompts,
        shell_assets_dir: self.shell_assets_dir.clone(),
    };
    
    // ... create window with spawn_options
}
```

### Edge Cases

| Scenario | Behavior |
|----------|----------|
| No windows open | Use config default or `$HOME` |
| Focused window has no tabs | Use config default (shouldn't happen) |
| Shell CWD query fails | Fall back to config default |
| First launch | Use config default |

### Interaction with Tab Detachment (CRT-I-0026)

When a tab is detached to a new window:
- The new window should inherit the **detached tab's** CWD, not query the source window
- This is already handled since the ShellTerminal (with its PTY and CWD) transfers directly

## Alternatives Considered

1. **Always use config default** - Current behavior, rejected as inconvenient
2. **Prompt user for directory** - Overkill for this common operation
3. **Remember last-used directory globally** - Adds state complexity, less predictable

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

## Acceptance Criteria

- [x] New window (Cmd+N) opens in the focused window's active tab CWD
- [x] Falls back to config default when no window is focused
- [x] Falls back to config default when CWD query fails
- [x] First app launch still uses config default (no focused window exists)

## Implementation Checklist

1. [x] Find where new windows are created (likely `main.rs` near `UserEvent::NewWindow`)
2. [x] Check if focused window is already tracked in `App` state
3. [x] If not tracked, add `focused_window: Option<WindowId>` and handle `WindowEvent::Focused`
4. [x] Modify new window creation to query `focused_window.active_shell_cwd()`
5. [x] Add fallback chain: focused CWD → config default → None
6. [x] Manual test: `cd /tmp && Cmd+N` → new window should be in `/tmp`

## Files to Modify

- `src/main.rs` - New window creation logic, possibly focus tracking
- `src/window.rs` - May need to expose `active_shell_cwd()` if not already public