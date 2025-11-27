---
id: css-and-2d-rendering-foundation
level: initiative
title: "CSS and 2D Rendering Foundation"
short_code: "CRT-I-0009"
created_at: 2025-11-26T18:14:23.389421+00:00
updated_at: 2025-11-26T21:31:28.846153+00:00
parent: CRT-V-0001
blocked_by: []
archived: true

tags:
  - "#initiative"
  - "#phase/completed"


exit_criteria_met: false
estimated_complexity: L
strategy_id: NULL
initiative_id: css-and-2d-rendering-foundation
---

# CSS and 2D Rendering Foundation Initiative

## Context

Currently CRT uses:
- Custom CSS property parser in `crt-theme` (regex/string-based)
- Hand-rolled wgpu shaders for all rendering (background, glow, text, tab bar)

This works but limits future extensibility:
- Adding CSS features (animations, calc(), color functions) requires parser updates
- Each new visual effect needs a custom shader
- Theming is limited to what we explicitly support

## Goals & Non-Goals

**Goals:**
- Integrate `lightningcss` for proper CSS parsing (future-proofs CSS support)
- Integrate `vello` for general 2D rendering (gradients, shapes, paths)
- Establish architecture where CSS drives rendering
- Maintain current visual quality and performance

**Non-Goals:**
- Full CSS layout engine (we don't need box model)
- Replace ALL shaders (perspective grid + blur stay custom for now)
- vello for text rendering (keep cosmic-text + GridRenderer)

## Architecture

### Overview

```
┌─────────────────────────────────────────────────┐
│  Theme CSS File                                 │
│  - Custom properties (--ansi-*, --gradient-*)   │
│  - @keyframes for animations                    │
│  - Standard CSS colors, gradients               │
└────────────────────┬────────────────────────────┘
                     │ parse
                     ▼
┌─────────────────────────────────────────────────┐
│  lightningcss                                   │
│  - Tokenize and parse CSS                       │
│  - Extract custom properties                    │
│  - Parse @keyframes, calc(), color functions    │
└────────────────────┬────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────┐
│  Theme Runtime                                  │
│  - Computed values per frame (animations)       │
│  - Color palette, gradients, effect params      │
└────────────────────┬────────────────────────────┘
                     │
        ┌────────────┴────────────┐
        ▼                         ▼
┌───────────────────┐   ┌─────────────────────────┐
│  vello (GPU)      │   │  Custom Shaders         │
│  - 2D shapes      │   │  - Synthwave grid       │
│  - Gradients      │   │    (perspective)        │
│  - Tab bar chrome │   │  - Blur/glow            │
│  - Future: blur   │   │    (until vello adds)   │
└───────────────────┘   └─────────────────────────┘
        │                         │
        └────────────┬────────────┘
                     ▼
┌─────────────────────────────────────────────────┐
│  wgpu Compositor                                │
│  - Combine vello output + shader output + text  │
└─────────────────────────────────────────────────┘
```

### Library Responsibilities

| Library | Handles | Does NOT Handle |
|---------|---------|-----------------|
| lightningcss | CSS parsing, @keyframes, calc(), colors | Rendering |
| vello | 2D shapes, gradients, paths, (future: blur) | 3D transforms, text |
| Custom shaders | Perspective grid, blur/glow | General 2D |
| cosmic-text | Text shaping, glyph atlas | - |

### vello Current Limitations

Per [vello issue #476](https://github.com/linebender/vello/issues/476):
- No blur/filter effects yet (PR #784 in progress)
- No 3D transforms (2D renderer only)

We keep custom shaders for these until vello adds support.

## Detailed Design

### Phase 1: lightningcss Integration

Replace `crt-theme` CSS parsing with lightningcss:

```rust
use lightningcss::stylesheet::{StyleSheet, ParserOptions};
use lightningcss::properties::custom::CustomProperty;

fn parse_theme(css: &str) -> Theme {
    let stylesheet = StyleSheet::parse(css, ParserOptions::default())?;
    
    // Extract custom properties
    for rule in stylesheet.rules {
        // Handle --ansi-*, --gradient-*, etc.
    }
}
```

Benefits:
- Proper CSS tokenization (handles comments, escapes, units)
- Built-in support for `calc()`, `rgba()`, `hsl()`
- @keyframes parsing for future animations

### Phase 2: vello Integration

Add vello as rendering backend for 2D primitives:

```rust
use vello::{Scene, RenderContext};
use vello::peniko::{Color, Fill, Gradient};

fn render_tab_bar(scene: &mut Scene, theme: &Theme) {
    // Draw tab backgrounds with gradients
    scene.fill(
        Fill::NonZero,
        Affine::IDENTITY,
        &tab_gradient,
        None,
        &tab_rect,
    );
}
```

Initial scope:
- Tab bar background rendering
- Simple gradient backgrounds
- Solid color rectangles

### Phase 3: Compositor Integration

Combine render outputs:

1. vello renders to texture
2. Custom shaders render to texture (grid, glow)
3. Compositor blends layers onto final surface
4. Text rendered on top

## Alternatives Considered

### 1. Keep custom CSS parser
**Rejected:** Each new CSS feature requires parser work. lightningcss is maintained by Parcel team and handles edge cases.

### 2. Use vello for everything including text
**Rejected:** We have working cosmic-text + GridRenderer. vello text is different approach, unnecessary churn.

### 3. Wait for vello blur before integrating
**Rejected:** Can integrate now for what it does support, keep custom blur shader, replace later.

### 4. Use femtovg instead of vello
**Considered:** femtovg is simpler but less performant. vello's compute-shader approach better for our GPU-heavy rendering.

## Implementation Plan

### Phase 1: lightningcss Integration
- Add lightningcss dependency
- Create new CSS parser module
- Migrate existing theme properties
- Add support for @keyframes parsing (prep for animations)

### Phase 2: vello Setup
- Add vello dependency
- Create vello render context alongside wgpu
- Implement basic shape rendering
- Render tab bar via vello (proof of concept)

### Phase 3: Compositor
- Implement texture-based compositing
- vello output -> texture -> compositor
- Custom shader output -> texture -> compositor
- Verify visual parity with current rendering

### Phase 4: Expand vello Usage
- Move more 2D rendering to vello
- Prepare for future blur support
- Document shader vs vello decision criteria