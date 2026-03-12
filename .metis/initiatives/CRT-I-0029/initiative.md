---
id: test-coverage-sprint
level: initiative
title: "Test Coverage Sprint"
short_code: "CRT-I-0029"
created_at: 2026-03-11T14:33:11.169668+00:00
updated_at: 2026-03-11T21:10:52.382485+00:00
parent: CRT-V-0001
blocked_by: []
archived: false

tags:
  - "#initiative"
  - "#phase/completed"


exit_criteria_met: false
estimated_complexity: L
initiative_id: test-coverage-sprint
---

# Test Coverage Sprint Initiative

## Context

CRT currently has 285 tests across 26 files, but coverage is heavily skewed. Terminal emulation (56 tests), theme parsing (93 tests), and configuration (31 tests) are well-covered. However, major subsystems have zero or near-zero test coverage:

| Subsystem | Files | Tests | Status |
|-----------|-------|-------|--------|
| Terminal emulation | 6 | 56 | Good |
| Theme parsing | 2 | 93 | Good |
| Configuration | 1 | 31 | Good |
| Input handling | 3 | 8 | Poor |
| GPU/Rendering pipeline | 8 | ~2 | Critical gap |
| Tab bar | 4 | 3 | Critical gap |
| Effects system | 8 | 28 | Scattered, unit-only |
| Window/Main/Menu | 5 | 0 | None |
| Font/Glyph cache | 2 | 0 | None |
| Theme registry | 1 | 0 | None |
| File watcher | 1 | 0 | None |

The existing test infrastructure is well-designed but underutilized. `MockRenderer` (`crates/crt-renderer/src/mock.rs`, 497 lines) has 15+ assertion helpers but is only used in its own self-tests. `TerminalTestHarness` (`tests/common/mod.rs`) supports expect-style testing but only covers basic terminal scenarios.

No coverage tracking exists in CI. No property-based testing. Shell tests are flaky due to `sleep()`-based timing (5-second timeouts).

**Depends on:** CRT-I-0028 (Testability Refactoring) — the render pipeline, input handlers, and PTY must be decomposed before they can be meaningfully unit-tested.

## Goals & Non-Goals

**Goals:**
- Add 150+ new tests targeting the critical coverage gaps listed above
- Leverage MockRenderer for component-level UI verification (tab bar rendering, context menu, selection, effects)
- Add keyboard and mouse input unit tests using the pure decision functions from CRT-I-0028
- Add integration tests for the rendering pipeline using MockRenderer as the rendering backend
- Integrate coverage tracking into CI (target: track and report, not gate on percentage)
- Stabilize shell tests by replacing `sleep()` with event-based synchronization

**Non-Goals:**
- Visual/pixel-level regression testing (that's CRT-I-0032)
- Refactoring code for testability (that's CRT-I-0028)
- Performance benchmarking integration (that's CRT-I-0031)
- 100% coverage — focus on high-value tests that catch real regressions

## Detailed Design

### Test Area 1: Tab Bar System (20-30 tests)

**Files to test:** `crates/crt-renderer/src/tab_bar/{state.rs, layout.rs, mod.rs}`

Currently only 3 tests exist (in `state.rs`). Critical gaps:

- **State management:** Tab creation, deletion, reordering, renaming, activity indicators, selection
- **Layout calculations:** Tab width computation, overflow handling (too many tabs), hit testing (which tab was clicked at coordinate X,Y), bounds calculation at different window widths
- **Rendering via MockRenderer:** Verify correct `TabRenderInfo` structs are passed — active tab highlighting, title truncation, close button positioning

Tab state and layout are pure data structures with no GPU dependency — these are the highest-ROI tests to add.

### Test Area 2: Input Handling (30-50 tests)

**Files to test:** `src/input/{keyboard.rs, mouse.rs, mod.rs, key_encoder.rs}`

Currently 8 tests (7 in `key_encoder.rs`, 1 trivial in `mod.rs`). After CRT-I-0028 extracts pure decision functions:

- **Keyboard action determination:** All modifier combinations (Cmd, Ctrl, Alt, Shift), special keys (Tab, Enter, Escape, arrows), tab switching shortcuts (Cmd+1-9), zoom (Cmd+/Cmd-), copy/paste, search toggle
- **Mouse interaction:** Click-to-position mapping, drag selection, double-click word selection, triple-click line selection, scroll wheel, URL hover detection, right-click context menu
- **Key encoding:** Extended key encoding tests for terminal applications (vim, tmux, SSH)
- **URL detection:** More edge cases — URLs with parentheses, query strings, fragments, adjacent punctuation

### Test Area 3: Rendering Pipeline Integration (25-40 tests)

**Files to test:** `src/render/{mod.rs, context_menu.rs, dialogs.rs, selection.rs, overlays.rs}`

Zero tests currently. After CRT-I-0028 extracts pure functions from `render_frame()`:

- **PTY update processing:** Verify content change detection, shell event extraction, title updates
- **Override computation:** Given shell events + theme config, verify correct overrides are computed
- **Effect patching:** Verify patch application logic for starfield, particles, matrix, etc.
- **Context menu:** State transitions (show, hover, select, dismiss), coordinate positioning, item rendering via MockRenderer
- **Search dialog:** Input buffering, query building, match highlighting positions
- **Selection rendering:** Viewport coordinate conversion, multi-line selection rect generation, selection across scrollback boundary

### Test Area 4: Effects System (15-25 tests)

**Files to test:** `crates/crt-renderer/src/effects/{mod.rs, renderer.rs, matrix.rs}`

28 scattered unit tests exist but no integration tests. Gaps:

- **Effect lifecycle:** Enable, configure, update, disable transitions
- **Multi-effect composition:** Multiple effects active simultaneously
- **Event-triggered effects:** Bell triggers flash, command success/fail triggers override
- **Matrix rain effect:** Zero tests currently (`effects/matrix.rs`)
- **Effect renderer orchestration:** `EffectsRenderer` managing multiple `BackdropEffect` instances

### Test Area 5: Configuration Edge Cases (10-15 tests)

**Files to test:** `src/config.rs`, `src/theme_registry.rs`

31 config tests exist but gaps remain:

- **Theme registry:** Theme loading from disk, caching, switching, invalid theme handling, hot reload
- **Keybinding validation:** Conflict detection, invalid key names, modifier parsing edge cases
- **Config hot-reload:** File change detection, partial config updates, error recovery

### Test Area 6: Shell Test Stabilization

**File:** `tests/shell_tests.rs`

Replace timing-based synchronization with event-driven waits:
- Use PTY output events or terminal damage tracking instead of `sleep()`
- Reduce default timeouts from 5 seconds to 1-2 seconds
- Add `#[ignore]` attribute to genuinely slow tests with CI annotation

### Infrastructure Additions

- **Coverage tracking:** Add `cargo-llvm-cov` to CI workflow, generate reports, track trends
- **Test organization:** Add `tests/component_tests.rs` for MockRenderer-based tests, `tests/input_tests.rs` for input handling
- **Property-based testing:** Add `proptest` for theme parser fuzzing (color parsing, CSS property parsing)

## Alternatives Considered

**Gate CI on coverage percentage:** Rejected — coverage gates incentivize low-value tests to hit numbers. Instead, track and report coverage to inform decisions, but focus test writing on high-value areas identified by the analysis.

**Snapshot testing for all renderer output:** Rejected for this initiative — MockRenderer assertion helpers are sufficient for structural verification. Pixel-level comparison is deferred to CRT-I-0032.

**Rewrite shell tests with a mock PTY:** Partially adopted — CRT-I-0028 provides MockPty. Some shell tests should remain as true integration tests against real shells to catch environment-specific issues.

## Implementation Plan

**Phase 1 (can start before CRT-I-0028 completes):**
- Tab bar state and layout tests (no refactoring dependency)
- URL detection edge case tests (pure functions)
- Config edge case and theme registry tests
- Shell test stabilization
- CI coverage tracking setup

**Phase 2 (after CRT-I-0028 Phase 2 — PtyBackend):**
- MockPty-based terminal integration tests
- PTY update processing tests

**Phase 3 (after CRT-I-0028 Phase 3 — Input split):**
- Keyboard action determination tests
- Mouse interaction tests

**Phase 4 (after CRT-I-0028 Phase 1 — Render decomposition):**
- Rendering pipeline integration tests
- Override and effect patch computation tests
- Context menu, dialog, and selection tests

**Target: 150+ new tests bringing total from 285 to 435+**