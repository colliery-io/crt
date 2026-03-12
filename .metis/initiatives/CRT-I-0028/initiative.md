---
id: testability-refactoring
level: initiative
title: "Testability Refactoring"
short_code: "CRT-I-0028"
created_at: 2026-03-11T14:33:11.131016+00:00
updated_at: 2026-03-11T20:28:51.162131+00:00
parent: CRT-V-0001
blocked_by: []
archived: false

tags:
  - "#initiative"
  - "#phase/completed"


exit_criteria_met: false
estimated_complexity: L
initiative_id: testability-refactoring
---

# Testability Refactoring Initiative

## Context

Several core subsystems in CRT are currently untestable in isolation due to tight coupling between concerns. The most critical example is `render_frame()` in `src/render/mod.rs` — a 1,150-line monolith that mixes PTY output processing, state updates, theme override application, and GPU rendering in a single function. Input handlers (`src/input/keyboard.rs`, `src/input/mouse.rs`) mutate `WindowState` directly while also returning action enums, making it impossible to test decision logic without constructing the full window state including GPU resources. The PTY subsystem (`crates/crt-core/src/pty.rs`) has no trait abstraction, so terminal tests must spawn real shell processes.

These coupling issues are the primary blocker for all other testing initiatives (CRT-I-0029, CRT-I-0032). This initiative must be completed first to unlock meaningful test coverage improvements.

**Current testability scores by subsystem:**
- Configuration: 9/10 (pure functions, already well-tested)
- Renderer traits/MockRenderer: 9/10 (excellent abstraction)
- Terminal core: 7/10 (needs PTY abstraction)
- Input handling: 4/10 (mixed pure + side effects)
- Theme overrides: 3/10 (complex state, GPU-dependent)
- Rendering pipeline: 2/10 (monolithic, GPU-coupled)
- Window management: 2/10 (winit-coupled)
- PTY/Shell: 1/10 (external process, no mock)

## Goals & Non-Goals

**Goals:**
- Extract pure, testable functions from `render_frame()` so PTY processing, state updates, and override logic can be verified independently of GPU rendering
- Create a `PtyBackend` trait so terminal tests can inject deterministic mock I/O instead of spawning real shells
- Split input handlers into pure decision functions and side-effectful application functions
- Reduce the minimum `WindowState` surface needed to test any single concern

**Non-Goals:**
- Writing the actual tests (that's CRT-I-0029)
- Abstracting wgpu/Vello behind traits (diminishing returns for GPU code)
- Changing user-facing behavior — all refactorings must be behavior-preserving
- Performance optimization (that's CRT-I-0031)

## Architecture

### Overview

The refactoring follows a "seam extraction" pattern: identify the boundaries between concerns in coupled code, extract the pure logic into standalone functions/traits, and leave the orchestration as a thin shell that calls them. No new crates or major architectural changes — just better separation within existing modules.

### Key Architectural Changes

**1. Render pipeline decomposition** (`src/render/mod.rs`)

Current: `render_frame(state, shared)` does everything.

Proposed: Split into phases that can be called and tested independently:
```
process_pty_updates(shells, tab_id) -> PtyUpdateResult
compute_override_patches(ui_state, theme) -> Vec<EffectPatch>
prepare_render_data(terminal, theme) -> RenderData
execute_gpu_render(render_data, shared) // only this needs GPU
```

**2. PTY abstraction** (`crates/crt-core/src/pty.rs`)

Create `PtyBackend` trait with `write()`, `try_read()`, `resize()` methods. Current `Pty` implements it. New `MockPty` allows injecting scripted output sequences for testing.

**3. Input handler split** (`src/input/keyboard.rs`, `src/input/mouse.rs`)

Current: `handle_keyboard_input(&mut WindowState, key, text, mods) -> KeyboardAction` (mutates state AND returns action).

Proposed:
```
determine_action(key, text, mods, context) -> KeyboardAction  // pure
apply_action(&mut WindowState, action)                        // side-effectful
```

Where `context` is a small read-only struct extracted from WindowState (active tab, terminal mode, etc.).

## Detailed Design

### Phase 1: Render Pipeline Decomposition

**Target file:** `src/render/mod.rs:33-1181`

Extract the following pure functions:

1. **`process_pty_updates()`** (lines 82-98): Process PTY output for active tab, return whether content changed and any shell events. No GPU dependency.

2. **`compute_shell_event_overrides()`** (lines 118-145): Given shell events and theme config, compute which overrides to activate. Pure data transformation.

3. **`compute_effect_patches()`** (lines 156-229): Given override state and current effects, compute which effects need patching. Currently interleaved with `effects_renderer` mutations — extract the decision logic.

4. **`prepare_terminal_render_data()`** (lines ~300-800): Collect cell content, cursor info, selection ranges from terminal state into renderer-agnostic data structures. This is where the MockRenderer traits (`TextRenderer`, `UiRenderer`) already define the right interface — we just need to separate data preparation from GPU submission.

The remaining `render_frame()` becomes a thin orchestrator: call each phase, then pass results to GPU renderers.

### Phase 2: PtyBackend Trait

**Target file:** `crates/crt-core/src/pty.rs`

```rust
pub trait PtyBackend: Send {
    fn write(&self, data: &[u8]);
    fn try_read(&self) -> Option<Vec<u8>>;
    fn read_available(&self) -> Vec<u8>;
    fn resize(&self, cols: u16, rows: u16);
    fn shutdown(&self);
    fn process_id(&self) -> Option<u32>;
    fn working_directory(&self) -> Option<PathBuf>;
}
```

`Pty` implements this trait. `ShellTerminal` becomes generic over `P: PtyBackend`. A `MockPty` in `#[cfg(test)]` allows:
- Pre-loaded output sequences (feed exact bytes)
- Captured input verification (assert what was sent to shell)
- Deterministic timing (no sleep-based waits)

This also consolidates the duplicated spawn logic between `spawn_with_options()` (lines 52-204) and `spawn_with_cwd()` (lines 207-311) — ~160 lines of near-identical thread spawning code.

### Phase 3: Input Handler Refactoring

**Target files:** `src/input/keyboard.rs`, `src/input/mouse.rs`

Create a lightweight read-only context struct:
```rust
pub struct InputContext {
    pub active_tab: TabId,
    pub tab_count: usize,
    pub terminal_mode: TermMode,
    pub has_selection: bool,
    pub search_active: bool,
    pub context_menu_visible: bool,
    pub bracketed_paste: bool,
}
```

Extract `determine_keyboard_action(key, text, mods, &InputContext) -> KeyboardAction` as a pure function. The existing `handle_keyboard_input()` becomes:
```rust
pub fn handle_keyboard_input(state: &mut WindowState, ...) -> KeyboardAction {
    let ctx = InputContext::from(state);
    let action = determine_keyboard_action(key, text, mods, &ctx);
    apply_keyboard_action(state, &action);
    action
}
```

Same pattern for mouse input.

## Alternatives Considered

**Full trait abstraction for GPU:** Wrapping wgpu behind traits (GpuFactory, MockDevice, etc.) was considered but rejected. The effort is high, the abstractions leak (GPU APIs are inherently stateful), and the MockRenderer trait system already provides the right testing seam for rendering verification. GPU code is best tested via integration tests (CRT-I-0032).

**Rewrite render pipeline with ECS:** An entity-component-system approach would solve testability but is a massive architectural change disproportionate to the problem. Seam extraction achieves the same testability with minimal disruption.

**Test via shell scripts:** Running the full terminal and scripting interactions was considered but is too slow, flaky, and doesn't catch unit-level regressions.

## Implementation Plan

**Phase 1: Render decomposition** — Extract pure functions from `render_frame()`. Each extraction is a self-contained PR that can be reviewed independently. Existing behavior must not change (no functional diff).

**Phase 2: PtyBackend trait** — Introduce trait, make `ShellTerminal` generic, add `MockPty`. Consolidate duplicated spawn code. Existing shell tests should pass unchanged.

**Phase 3: Input handler split** — Extract `InputContext` and pure decision functions. Verify via existing key_encoder tests + new unit tests for action determination.

**Sequencing:** Phase 1 can begin immediately. Phase 2 is independent and can run in parallel. Phase 3 depends on neither. All three can be decomposed into tasks once this initiative reaches the decompose phase.

**Dependencies:** This initiative blocks CRT-I-0029 (Test Coverage Sprint) and partially blocks CRT-I-0032 (Visual Regression Testing).