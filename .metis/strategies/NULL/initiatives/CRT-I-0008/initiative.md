---
id: terminal-rendering-pipeline-and
level: initiative
title: "Terminal Rendering Pipeline and ANSI Color Support"
short_code: "CRT-I-0008"
created_at: 2025-11-26T17:12:33.450480+00:00
updated_at: 2025-11-26T17:51:03.720485+00:00
parent: CRT-V-0001
blocked_by: []
archived: false

tags:
  - "#initiative"
  - "#phase/active"


exit_criteria_met: false
estimated_complexity: L
strategy_id: NULL
initiative_id: terminal-rendering-pipeline-and
---

# Terminal Rendering Pipeline and ANSI Color Support Initiative

*This template includes sections for various types of initiatives. Delete sections that don't apply to your specific use case.*

## Context

Currently, all terminal text goes through the same glow/composite rendering pass. This creates a uniform CRT aesthetic but doesn't distinguish between:
- UI elements (cursor, tab titles) that should have effects
- Terminal output (command results, ANSI-colored text) that should be crisp and readable

Additionally, ANSI colors are not yet implemented - all terminal text renders in a single hardcoded color.

## Goals & Non-Goals

**Goals:**
- Implement ANSI 16-color palette with CSS theme overrides
- Create pluggable render pipeline architecture for composable effects
- Render terminal text with raw colors (no glow) for readability
- Render cursor and UI elements through effect passes (with glow)
- Support cursor styling (color, shape: block/underline/bar)
- Support selection highlighting with themed colors

**Non-Goals:**
- 256-color or 24-bit true color support (future initiative)
- Cursor blinking (can be added later)
- New effect types (scanlines, CRT curvature) - architecture enables this but not in scope



## Architecture

### Overview

Introduce a pluggable render pipeline where components are built from composable passes:

```
RenderPass trait
├── RawColorPass     - renders glyphs/quads with solid colors
├── GlowPass         - applies blur/glow effect to a texture
├── BackgroundPass   - gradient + grid animation
└── CompositePass    - blends multiple layers together
```

### Component Pipelines

Each renderable component declares which passes it uses:

| Component      | Pipeline                              |
|----------------|---------------------------------------|
| Background     | BackgroundPass → screen               |
| Terminal text  | RawColorPass → screen (no effects)    |
| Cursor         | RawColorPass → GlowPass → screen      |
| Selection      | RawColorPass → screen (no effects)    |
| Tab bar        | RawColorPass → screen                 |
| Tab titles     | RawColorPass → GlowPass → screen      |

### Render Order

1. BackgroundPass (gradient + grid)
2. Terminal text direct to screen (raw ANSI colors)
3. Selection overlay (raw color)
4. Cursor through GlowPass
5. Tab bar quads (raw color)
6. Tab titles through GlowPass

### CSS Color Variables

All in the same theme CSS file:

```css
/* ANSI 16-color palette - raw colors, no effects */
--ansi-black: #1a1a2e;
--ansi-red: #ff5555;
--ansi-green: #50fa7b;
--ansi-yellow: #f1fa8c;
--ansi-blue: #6272a4;
--ansi-magenta: #ff79c6;
--ansi-cyan: #8be9fd;
--ansi-white: #f8f8f2;
--ansi-bright-black: #44475a;
--ansi-bright-red: #ff6e6e;
--ansi-bright-green: #69ff94;
--ansi-bright-yellow: #ffffa5;
--ansi-bright-blue: #d6acff;
--ansi-bright-magenta: #ff92df;
--ansi-bright-cyan: #a4ffff;
--ansi-bright-white: #ffffff;

/* Cursor - goes through effects */
--cursor-color: #f1fa8c;
--cursor-shape: block; /* block | underline | bar */

/* Selection - raw color overlay */
--selection-background: rgba(255, 255, 255, 0.3);
```

## Detailed Design

### Phase 1: ANSI Color Palette in Theme

**crt-theme changes:**
- Parse `--ansi-*` CSS variables into `AnsiPalette` struct (16 colors)
- Parse `--cursor-color`, `--cursor-shape`, `--selection-background`
- Add to `Theme` struct alongside existing effect properties

**crt-renderer changes:**
- Accept `AnsiPalette` in grid renderer
- Map alacritty_terminal cell colors to palette colors

### Phase 2: Pluggable Render Pipeline

**New trait in crt-renderer:**
```rust
pub trait RenderPass {
    fn prepare(&mut self, queue: &wgpu::Queue);
    fn render<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>);
}
```

**Refactor existing code:**
- Extract `BackgroundPass` from `BackgroundPipeline`
- Extract `GlowPass` from composite shader logic
- Create `RawColorPass` wrapper for direct-to-screen rendering

### Phase 3: Split Terminal and Effect Rendering

**Render pipeline restructure:**
- Terminal text: bypass glow, render direct to screen with ANSI colors
- Cursor: render to small offscreen texture, apply glow, composite
- Keep tab titles through existing glow path

### Phase 4: Cursor and Selection

**Cursor rendering:**
- Support block (full cell), underline (bottom 2px), bar (left 2px)
- Render through GlowPass for effect treatment
- Color from `--cursor-color`

**Selection rendering:**
- Track selection state from alacritty_terminal
- Render selection rectangles as raw color overlay
- Color from `--selection-background`



## Alternatives Considered

### 1. Single unified glow for all text
**Rejected:** Makes terminal output harder to read; doesn't distinguish UI from content.

### 2. Separate glyph caches per render treatment
**Considered but simplified:** We already have separate caches (terminal vs tab). The key change is which render pass they go through, not adding more caches.

### 3. Shader-based per-character effect toggling
**Rejected:** Would require passing per-glyph metadata to shader, adding complexity. Simpler to route entire components through different pipelines.

### 4. Post-process mask for glow regions
**Rejected:** Would require rendering a mask texture and sampling it in composite shader. More GPU overhead than routing components to different passes.

## Implementation Plan

### Phase 1: ANSI Palette in CSS (MVP foundation)
- Add `--ansi-*` variable parsing to crt-theme
- Add `AnsiPalette` struct to Theme
- Wire palette through to window.rs text rendering
- Update hardcoded `[0.9, 0.9, 0.9, 1.0]` to use palette

### Phase 2: Pluggable Render Pipeline Architecture
- Define `RenderPass` trait
- Refactor BackgroundPipeline → BackgroundPass
- Extract GlowPass from composite logic
- Create pipeline builder pattern

### Phase 3: Split Terminal Text from Effects
- Route terminal text directly to screen (bypass glow)
- Keep cursor rendering through glow pipeline
- Verify tab titles still render with glow

### Phase 4: Cursor Styling
- Parse `--cursor-color` and `--cursor-shape`
- Implement block/underline/bar cursor shapes
- Render cursor through effect pipeline

### Phase 5: Selection Support
- Parse `--selection-background`
- Hook into alacritty_terminal selection state
- Render selection as raw color overlay