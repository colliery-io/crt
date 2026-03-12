---
id: readability-and-module
level: initiative
title: "Readability and Module Decomposition"
short_code: "CRT-I-0030"
created_at: 2026-03-11T14:33:11.206071+00:00
updated_at: 2026-03-11T21:14:51.477034+00:00
parent: CRT-V-0001
blocked_by: []
archived: false

tags:
  - "#initiative"
  - "#phase/decompose"


exit_criteria_met: false
estimated_complexity: M
initiative_id: readability-and-module
---

# Readability and Module Decomposition Initiative

## Context

Three files in the codebase have grown beyond maintainable size, creating friction for development, code review, and onboarding:

| File | Lines | Types/Functions | Issue |
|------|-------|----------------|-------|
| `src/window.rs` | 1,620 | 23+ public types | Kitchen sink for all window-related state |
| `src/main.rs` | 1,714 | App struct + event handler | Config, themes, menus, and event loop in one file |
| `src/render/mod.rs` | 1,311 | render_frame() monolith | 1,150-line function mixing all rendering phases |

Additionally, there are specific code duplication and type safety issues:

- **PTY spawn duplication:** `spawn_with_options()` and `spawn_with_cwd()` share ~160 lines of near-identical thread spawning code (`crates/crt-core/src/pty.rs:52-311`)
- **Theme switching duplication:** Identical 24-line theme switch sequences appear in `main.rs` at lines ~825 and ~1458
- **Override getter repetition:** 7 identical-pattern getter methods in `window.rs:385-446`
- **Tab menu repetition:** 9 individual match arms for SelectTab1-9 in `main.rs:776-784`
- **String-typed effect names:** Effect IDs like `"starfield"` used as magic strings instead of an enum
- **Magic numbers:** `dt = 1.0 / 60.0` in render loop instead of a named constant

This initiative is independent of the testability refactoring (CRT-I-0028) and can run in parallel, though the render_frame decomposition in CRT-I-0028 will naturally address the render/mod.rs monolith. This initiative focuses on the structural/organizational issues that CRT-I-0028 does not cover.

## Goals & Non-Goals

**Goals:**
- Break `window.rs` into a `window/` module directory with focused sub-modules
- Split `main.rs` into entry point + application handler + initialization modules
- Consolidate all identified code duplication (PTY spawn, theme switching, override getters, tab menu)
- Replace string-typed effect identifiers with an `EffectId` enum
- Replace magic numbers with named constants
- Ensure zero functional changes — all refactoring is behavior-preserving

**Non-Goals:**
- Decomposing `render_frame()` into testable phases (that's CRT-I-0028)
- Adding documentation to untouched code
- Changing public API signatures beyond what's needed for module moves
- Performance improvements (that's CRT-I-0031)

## Detailed Design

### 1. Window Module Decomposition

**Current:** `src/window.rs` (1,620 lines, 23+ types)

**Proposed structure:**
```
src/window/
  mod.rs          — WindowState struct and core lifecycle (create, resize, close)
  state.rs        — RenderState, CursorInfo, TextDecoration
  ui.rs           — UiState, ToastType, CopyIndicator, ZoomIndicator, BellState
  interaction.rs  — InteractionState, SelectionState, UrlDetection
  overrides.rs    — OverrideState, ActiveOverride, getter methods
  types.rs        — TabId, small enums, type aliases
```

Each sub-module re-exports its public types through `mod.rs` so existing `use crate::window::*` imports continue to work. This is a pure file reorganization — no logic changes.

### 2. Main Module Decomposition

**Current:** `src/main.rs` (1,714 lines)

**Proposed structure:**
```
src/main.rs           — Entry point only (fn main, event loop setup)
src/app/
  mod.rs              — App struct definition
  handler.rs          — ApplicationHandler impl (window_event, about_to_wait, etc.)
  initialization.rs   — Theme loading, config setup, GPU init
  menu_actions.rs     — handle_menu_action() and related helpers
```

The `App` struct stays in `app/mod.rs`. The massive `ApplicationHandler` impl moves to `handler.rs`. Menu action handling (195 lines, 30+ match arms) moves to `menu_actions.rs`.

### 3. PTY Spawn Consolidation

**Current:** Two functions with ~80% code overlap in `crates/crt-core/src/pty.rs`

**Fix:** Extract shared thread-spawning logic into a private helper:
```rust
fn spawn_pty_threads(
    pair: PtyPair,
    child: Box<dyn Child>,
) -> (Sender<PtyInput>, Receiver<Vec<u8>>, Box<dyn Child>)
```

Both `spawn_with_options()` and `spawn_with_cwd()` call this helper after their specific setup (environment variables, init scripts). Eliminates ~130 lines of duplication.

### 4. Theme Switching Helper

**Current:** 24-line theme switch sequence duplicated in menu action handler and context menu click handler.

**Fix:** Extract `fn apply_theme_switch(&mut self, theme_name: &str)` on the App struct. Both call sites become one-liners.

### 5. Override Getter Macro/Generic

**Current:** 7 methods in `OverrideState` with identical pattern:
```rust
pub fn get_foreground(&self) -> Option<Color> {
    self.active.iter().filter(|o| o.is_active())
        .filter_map(|o| o.properties.foreground).next_back()
}
```

**Fix:** Either a macro:
```rust
macro_rules! override_getter {
    ($name:ident, $field:ident, $ty:ty) => {
        pub fn $name(&self) -> Option<$ty> {
            self.active.iter().filter(|o| o.is_active())
                .filter_map(|o| o.properties.$field).next_back()
        }
    };
}
override_getter!(get_foreground, foreground, Color);
// ...
```

Or a generic helper method that takes a field accessor closure. Reduces 56 lines to ~15.

### 6. Tab Menu Dispatch

**Current:** 9 match arms `SelectTab1 => select_tab_index(0)` through `SelectTab9 => select_tab_index(8)`.

**Fix:** Add a method to `MenuAction`:
```rust
impl MenuAction {
    fn tab_index(&self) -> Option<usize> { /* match SelectTab1..9 to 0..8 */ }
}
```
Then: `if let Some(idx) = action.tab_index() { self.select_tab_index(idx); }`

### 7. EffectId Enum

**Current:** `HashSet<String>` for tracking patched effects, string comparisons like `is_patched("starfield")`.

**Fix:**
```rust
pub enum EffectId { Starfield, Particles, Matrix, Rain, Grid, Shape, Sprite }
```
Replace `HashSet<String>` with `HashSet<EffectId>`. Provides compile-time validation that effect names are valid.

### 8. Named Constants

Replace magic numbers:
- `1.0 / 60.0` → `const ASSUMED_DT: f32 = 1.0 / 60.0;`
- Any other numeric literals found during refactoring

## Alternatives Considered

**Leave large files as-is, use IDE navigation:** Rejected — file size affects code review quality, merge conflict frequency, and cognitive load. Multiple developers editing `window.rs` simultaneously will create conflicts.

**Move to a workspace crate per concern:** Rejected — too heavy for what's essentially file organization. Module-level splitting within existing crates is sufficient.

**Automated refactoring tools:** Rust-analyzer's "move to module" works for simple cases but can't handle the override getter pattern or PTY consolidation. Manual refactoring with careful testing is more reliable.

## Implementation Plan

All tasks are independent and can be executed in any order:

1. **Window module split** — Highest impact, most files affected. Do first to establish the pattern.
2. **PTY spawn consolidation** — Self-contained in one file, low risk.
3. **Theme switching helper** — Quick win, touches main.rs.
4. **Main module split** — Can be done after theme switching helper to avoid double-moving code.
5. **Override getters, tab dispatch, EffectId enum, constants** — Small mechanical changes, batch together.

Each task should be a separate commit/PR for easy review and revert if needed. All existing tests must pass unchanged after each step.