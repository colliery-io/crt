---
id: visual-regression-testing
level: initiative
title: "Visual Regression Testing Infrastructure"
short_code: "CRT-I-0032"
created_at: 2026-03-11T14:33:13.336962+00:00
updated_at: 2026-03-11T21:23:12.611685+00:00
parent: CRT-V-0001
blocked_by: []
archived: false

tags:
  - "#initiative"
  - "#phase/decompose"


exit_criteria_met: false
estimated_complexity: L
initiative_id: visual-regression-testing
---

# Visual Regression Testing Infrastructure Initiative

## Context

CRT has zero visual/UI rendering tests. All 285 existing tests verify terminal _state_ (cursor position, text content, semantic zones) but not _appearance_ (colors, glyph rendering, cursor shape, selection highlighting, tab bar layout, effects). A bug that renders the wrong color, misaligns the cursor, or breaks the CRT post-processing pipeline would pass all current tests.

The codebase has strong foundations for visual testing:

- **Renderer traits** (`crates/crt-renderer/src/traits.rs`): `TextRenderer`, `UiRenderer`, `BackdropRenderer` define clean abstractions
- **MockRenderer** (`crates/crt-renderer/src/mock.rs`): Records all render calls with assertion helpers — already proves the trait system works
- **wgpu headless support:** wgpu v26 can create devices without a window surface, enabling offscreen rendering
- **Vello CPU rendering:** Vello can render scenes to CPU buffers for verification
- **Offscreen textures:** The existing `CompositePipeline` and `CrtPipeline` already render to offscreen textures before final presentation

The gap is infrastructure: no headless rendering harness, no frame capture mechanism, no golden file comparison, no CI integration for GPU tests.

**Depends on:** Partially depends on CRT-I-0028 (Testability Refactoring) for render pipeline decomposition. The headless renderer can be built independently, but testing individual render phases requires the decomposition.

## Goals & Non-Goals

**Goals:**
- Build a `HeadlessRenderer` that renders terminal state to an offscreen texture and captures the result as PNG bytes
- Implement golden file comparison with configurable perceptual diff tolerance
- Create visual regression tests for critical rendering paths: text rendering, cursor shapes, selection highlighting, tab bar, CRT post-processing effects
- Run visual tests in CI on both macOS and Linux (GitHub Actions runners have GPU support via software rendering)
- Provide a workflow for updating golden files when intentional visual changes are made

**Non-Goals:**
- Full end-to-end testing with real windowing (headless X11/Wayland compositor) — too much infrastructure cost
- Testing every possible terminal state visually — focus on critical rendering paths
- Pixel-perfect cross-platform matching — allow per-platform golden files where needed
- Real-time visual debugging tools (e.g., live frame viewer)

## Architecture

### Overview

The visual testing architecture has three layers:

```
┌─────────────────────────────────────────────┐
│  Visual Regression Tests                     │
│  (tests/visual_tests.rs)                     │
│  - Set up terminal state                     │
│  - Call HeadlessRenderer                      │
│  - Compare captured frame to golden file     │
└──────────────────┬──────────────────────────┘
                   │
┌──────────────────▼──────────────────────────┐
│  HeadlessRenderer                             │
│  (crates/crt-renderer/src/headless.rs)       │
│  - Creates wgpu device without window        │
│  - Renders to offscreen texture              │
│  - Reads back pixels via staging buffer      │
│  - Returns PNG bytes                          │
└──────────────────┬──────────────────────────┘
                   │
┌──────────────────▼──────────────────────────┐
│  Golden File Comparison                       │
│  (tests/visual/compare.rs)                   │
│  - Pixel-level diff                           │
│  - Perceptual diff (optional)                │
│  - Diff image generation for failures        │
│  - Platform-aware golden file selection       │
└──────────────────────────────────────────────┘
```

### Key Design Decisions

**Offscreen wgpu over Vello CPU rendering:** While Vello can render to CPU, using wgpu offscreen rendering tests the actual GPU pipeline (shaders, blending, texture sampling) rather than a parallel path. This catches GPU-specific bugs.

**Per-platform golden files:** Font rendering differs between macOS (Core Text) and Linux (FreeType). Golden files are stored as `tests/visual/golden/{test_name}.{platform}.png`. Platform detection is automatic.

**Perceptual diff with tolerance:** Exact pixel matching is brittle (anti-aliasing, subpixel rendering). Use a configurable tolerance (default: 0.5% pixel difference allowed). Failures generate a diff image highlighting changed pixels.

## Detailed Design

### 1. HeadlessRenderer

**Location:** `crates/crt-renderer/src/headless.rs`

```rust
pub struct HeadlessRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    render_texture: wgpu::Texture,
    staging_buffer: wgpu::Buffer,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
}

impl HeadlessRenderer {
    /// Create a headless renderer with given dimensions
    pub fn new(width: u32, height: u32) -> Result<Self, HeadlessError>;
    
    /// Get the render texture view for passing to rendering pipelines
    pub fn texture_view(&self) -> &wgpu::TextureView;
    
    /// Capture the current frame as RGBA bytes
    pub fn capture_frame(&self) -> Vec<u8>;
    
    /// Capture and encode as PNG
    pub fn capture_png(&self) -> Vec<u8>;
    
    /// Access device/queue for pipeline setup
    pub fn device(&self) -> &wgpu::Device;
    pub fn queue(&self) -> &wgpu::Queue;
}
```

Internally uses `wgpu::Instance::request_adapter()` with `power_preference: LowPower` and `force_fallback_adapter: true` for CI compatibility (software rendering). The render texture has `COPY_SRC` usage for readback via staging buffer.

### 2. Golden File Comparison

**Location:** `tests/visual/compare.rs`

```rust
pub struct ComparisonResult {
    pub matched: bool,
    pub diff_percentage: f64,     // 0.0 to 100.0
    pub diff_pixels: usize,
    pub total_pixels: usize,
    pub diff_image: Option<Vec<u8>>,  // PNG of highlighted differences
}

pub fn compare_with_golden(
    actual: &[u8],          // PNG bytes
    golden_path: &Path,     // Path to golden file
    tolerance: f64,         // Max allowed diff percentage (e.g., 0.5)
) -> ComparisonResult;

pub fn update_golden(actual: &[u8], golden_path: &Path);
```

When a test fails:
1. The actual image is saved to `tests/visual/failures/{test_name}.actual.png`
2. A diff image is saved to `tests/visual/failures/{test_name}.diff.png`
3. The assertion message includes the diff percentage and paths

Golden files are checked into git. An environment variable `UPDATE_GOLDEN=1` causes tests to overwrite golden files instead of comparing (for intentional visual changes).

### 3. Visual Test Cases

**Location:** `tests/visual_tests.rs`

**Text Rendering Tests:**
- Basic ASCII text (verify glyph positioning, spacing)
- Bold/italic text styles
- ANSI 16-color foreground/background
- 256-color and truecolor
- Unicode characters (CJK, emoji, box-drawing)

**Cursor Tests:**
- Block cursor (default)
- Bar cursor
- Underline cursor
- Blinking cursor (at specific animation frame)
- Cursor color override from theme

**Selection Tests:**
- Single-line selection highlight
- Multi-line selection
- Selection color from theme

**Tab Bar Tests:**
- Single tab
- Multiple tabs with active indicator
- Tab with long title (truncation)
- Tab overflow (scrolling indicator)

**CRT Effect Tests:**
- Scanline overlay at different intensities
- Screen curvature
- Vignette effect
- Glow/bloom on text

**Theme Tests:**
- Default theme rendering
- Custom theme with overridden colors
- Background gradient rendering

### 4. CI Integration

**GitHub Actions configuration:**

```yaml
visual-tests:
  runs-on: ${{ matrix.os }}
  strategy:
    matrix:
      os: [macos-latest, ubuntu-latest]
  steps:
    - uses: actions/checkout@v4
    - name: Install deps (Linux)
      if: runner.os == 'Linux'
      run: sudo apt-get install -y mesa-utils libegl1-mesa-dev
    - name: Run visual tests
      run: cargo test --test visual_tests
      env:
        WGPU_BACKEND: vulkan  # or gl on Linux
    - name: Upload failure artifacts
      if: failure()
      uses: actions/upload-artifact@v4
      with:
        name: visual-test-failures-${{ matrix.os }}
        path: tests/visual/failures/
```

Failed visual tests upload diff images as artifacts for inspection.

## Alternatives Considered

**Vello CPU-only rendering:** Renders scenes to CPU buffer without GPU. Pros: no GPU required in CI, deterministic output. Cons: doesn't test the actual GPU pipeline (shaders, blending), which is where visual bugs live. Could be used as a lighter-weight complement.

**Screenshot comparison with headless display server (Xvfb):** Captures actual window screenshots. Pros: tests the full stack including windowing. Cons: requires display server setup, very slow, flaky due to window manager differences. Too much infrastructure for the value.

**DOM-like structural testing only (via MockRenderer):** Already available and covered by CRT-I-0029. Catches structural issues (wrong cells rendered, missing cursor) but not visual issues (wrong color, misaligned pixels, broken shaders).

**Video recording and frame comparison:** Overkill for a terminal emulator. Screenshot comparison at key states is sufficient.

## Implementation Plan

**Phase 1: HeadlessRenderer infrastructure**
- Implement `HeadlessRenderer` with offscreen wgpu rendering
- Implement frame capture and PNG encoding
- Verify it works on both macOS and Linux with software rendering
- Add `image` crate (already a dependency) for PNG encoding/comparison

**Phase 2: Golden file comparison framework**
- Implement pixel-level and perceptual diff comparison
- Implement diff image generation
- Set up golden file directory structure with platform awareness
- Add `UPDATE_GOLDEN=1` workflow

**Phase 3: Initial visual test suite**
- Text rendering tests (ASCII, colors, styles)
- Cursor tests (shapes, colors)
- Selection tests
- Tab bar tests
- Generate initial golden files for macOS and Linux

**Phase 4: CRT effects and CI integration**
- CRT post-processing tests (scanlines, curvature, glow)
- Theme rendering tests
- GitHub Actions workflow for visual tests
- Failure artifact uploading

**Dependencies:** Phase 1-2 can proceed independently. Phase 3-4 benefit from CRT-I-0028's render decomposition but can work around it by using the full rendering pipeline.