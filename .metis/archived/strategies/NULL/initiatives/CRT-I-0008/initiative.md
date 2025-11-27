---
id: terminal-rendering-pipeline-and
level: initiative
title: "Terminal Rendering Pipeline and ANSI Color Support"
short_code: "CRT-I-0008"
created_at: 2025-11-26T17:12:33.450480+00:00
updated_at: 2025-11-26T21:24:54.130574+00:00
parent: CRT-V-0001
blocked_by: []
archived: true

tags:
  - "#initiative"
  - "#phase/completed"


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

### Overview (Vello-First)

With vello as the primary shape renderer, the architecture simplifies:

- **Vello**: All UI shapes (cursor, selection, tab bar, cell backgrounds)
- **Glyph pipeline**: Terminal text, tab titles (ANSI-colored)
- **Glow shader**: Applied based on CSS `text-shadow` property

```
Rendering Layers
├── Background shader    - gradient + grid animation
├── Vello scene         - cursor, selection, tab bar shapes
├── Glyph pipeline      - terminal text, tab titles
└── Glow shader         - applied when text-shadow present
```

### Component Rendering

| Component      | Renderer | Glow Effect                    |
|----------------|----------|--------------------------------|
| Background     | Shader   | No                             |
| Terminal text  | Glyphs   | No (crisp for readability)     |
| Cursor         | Vello    | Yes (via CSS text-shadow)      |
| Selection      | Vello    | No                             |
| Tab bar        | Vello    | No                             |
| Tab titles     | Glyphs   | Yes (via CSS text-shadow)      |

### Render Order

1. Background shader (gradient + grid)
2. Vello scene → texture (cursor, selection, cell backgrounds)
3. Terminal text direct to screen (raw ANSI colors)
4. Composite vello texture (cursor gets glow if text-shadow)
5. Tab bar vello shapes (already implemented)
6. Tab titles with glow

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

### Phase 2: Split Terminal Text from Effects (Vello-First)
- Create `TerminalVelloRenderer` for cursor/selection shapes
- Route terminal text directly to screen (bypass glow)
- Cursor shape rendered via vello
- Glow applied to cursor when CSS `text-shadow` present

### Phase 3: Cursor Styling
- Parse `.cursor` CSS with color, shape, text-shadow
- Implement block/underline/bar cursor shapes via vello
- Render cursor through vello + optional glow shader

### Phase 4: Selection Support
- Parse `.selection` CSS with background color
- Hook into alacritty_terminal selection state
- Render selection rectangles via vello (no glow)