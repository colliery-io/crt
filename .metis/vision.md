---
id: crt-crt-rust-terminal
level: vision
title: "CRT - CRT Rust Terminal"
short_code: "CRT-V-0001"
created_at: 2025-11-25T00:28:56.171659+00:00
updated_at: 2025-12-29T14:25:43.681264+00:00
archived: false

tags:
  - "#vision"
  - "#phase/published"


exit_criteria_met: false
strategy_id: NULL
initiative_id: NULL
---

# CRT - CRT’s a Ridiculous Terminal

*A GPU-accelerated terminal emulator with CSS theming and rich visual effects.*

## Purpose

CRT exists to prove that terminal emulators don't have to choose between performance and visual richness. Hyper.js demonstrated the appeal of CSS-themed terminals but shackled itself to Electron's overhead. Alacritty proved GPU-accelerated terminals could be blazingly fast but offered minimal customization. CRT combines both: native Rust performance with CSS-driven theming for glowing text, animated backgrounds, and effects that make spending 8+ hours a day in a terminal feel *good*.

## Product Overview

CRT is a cross-platform terminal emulator targeting developers and power users who want both speed and aesthetics. It renders via wgpu (WebGPU/Vulkan/Metal/DX12), parses a CSS subset for theming, and supports modern font features including ligatures.

**Target Audience:** Developers who appreciate visual polish, former Hyper.js users frustrated by performance, Alacritty/Kitty users who want more customization.

**Key Differentiator:** First native GPU-accelerated terminal with true CSS theming - no Electron, no compromise.

## Current State

The terminal emulator landscape in 2025:

- **Hyper.js**: CSS theming via Electron. Beautiful themes, poor performance (\~200MB RAM), effectively unmaintained.
- **Alacritty**: GPU-accelerated, minimal features, no ligatures, YAML/TOML config only.
- **WezTerm**: Feature-rich, Lua config, ligature support, but complex and heavier.
- **Kitty**: GPU-accelerated, custom config format, good performance, no CSS.
- **Rio**: wgpu-based, MIT-licensed, closest to our vision architecturally. Has blur effects and even a CRT shader. But no CSS theming - uses TOML config. Currently mid-rewrite for 0.3.0. Primary reference implementation to study.

No terminal combines native GPU performance with CSS-based styling.

## Future State

CRT delivers:

- Sub-3ms frame rendering matching Alacritty's performance ceiling
- CSS files for all visual configuration (colors, fonts, effects, backgrounds)
- Text glow, shadows, and outline effects via SDF rendering
- Animated/procedural backgrounds and backdrop blur
- Full ligature support for programming fonts (Fira Code, JetBrains Mono, etc.)
- Cross-platform: Linux, macOS, Windows (best-effort)

Users write CSS like:

```css
:root {
  --foreground: #c0caf5;
  --background: rgba(26, 27, 38, 0.9);
  --cursor-color: #7aa2f7;
}

.text {
  text-shadow: 0 0 8px var(--cursor-color);
}

.background {
  backdrop-filter: blur(12px);
}
```

## Major Features

- **CSS Theme Engine**: Parse CSS subset via cssparser, map properties to shader uniforms and configuration. Support custom properties, gradients, shadows, blur.

- **wgpu Renderer**: Cross-platform GPU abstraction targeting Vulkan (Linux), Metal (macOS), DX12 (Windows). WGSL shaders compiled via Naga.

- **SDF Text Effects**: Signed Distance Field rendering for efficient glow, outline, and shadow effects without multi-pass overhead.

- **Ligature Support**: Full text shaping via cosmic-text/rustybuzz. HarfBuzz-compatible font feature support.

- **Dual Kawase Blur**: Efficient backdrop blur for transparent terminals, falling back to platform APIs where available.

- **Hot Reload**: Watch CSS files and apply changes without restart.

## Success Criteria

- Render performance within 50% of Alacritty on equivalent hardware
- CSS theme compatibility with common Hyper.js theme patterns (colors, basic effects)
- Ligatures render correctly for top 5 programming fonts
- Usable as daily driver terminal on Linux and macOS
- Community-contributed themes exist

## Principles

1. **Performance is non-negotiable**: Effects that tank frame rate get cut or made optional. Target 60fps minimum on integrated graphics.

2. **CSS as the styling language**: No inventing new config formats. CSS is known, toolable, and has existing themes to adapt.

3. **Design before implementation**: Interfaces and architecture established before coding. Modular crate structure from day one.

4. **Pragmatic scope**: Ship a great terminal first. Multiplexing, tabs, splits can come later or never.

5. **Learn from existing work**: Study Alacritty, WezTerm, and especially Rio's Sugarloaf renderer. Don't reinvent what's solved - but build from scratch where CSS-first architecture demands it.

## Architectural Approach

**Study Rio, Build CRT.**

Rio terminal is the closest existing project to our vision - it's MIT-licensed, wgpu-based, and already has some visual effects (blur, a CRT shader). However, we're building from scratch rather than forking for these reasons:

1. **Rio 0.3.0 architectural rewrite in progress** - main branch is unstable, forking now means chasing a moving target or being stuck on 0.2.x

2. **CSS-first architecture** - Rio's Sugarloaf renderer and TOML config weren't designed around CSS semantics. Grafting CSS parsing onto their system would fight the grain.

3. **Tight coupling** - Sugarloaf is deeply integrated with Rio's specific needs. Extracting and adapting it may be harder than building a focused renderer.

4. **Learning value** - understanding every line of the renderer matters for a project centered on visual effects.

**What we'll study from Rio:**

- Sugarloaf's wgpu pipeline architecture and shader organization
- Their Redux-style state machine for dirty tracking
- Platform-specific blur implementations
- How they integrated Alacritty's VTE parser

**What we'll reuse directly (battle-tested crates):**

- `vte` - Alacritty's VT parser
- `wgpu` - GPU abstraction
- `cosmic-text` / `rustybuzz` - text shaping with ligatures
- `cssparser` - Mozilla's CSS tokenizer
- `winit` - window/event handling
- `portable-pty` - cross-platform PTY

**Crate structure (design-first):**

- `crt` - application shell, CLI, main loop
- `crt-term` - terminal state, grid, scrollback
- `crt-render` - wgpu renderer, shaders, glyph atlas
- `crt-css` - CSS parsing, theme compilation to uniforms
- `crt-font` - font loading, shaping, SDF generation

## Constraints

- **No Electron/webview**: Native rendering only. The whole point is avoiding web runtime overhead.

- **CSS subset, not full spec**: Support properties that map to GPU operations. No layout engine, no flexbox, no grid.

- **wgpu minimum requirements**: Users need Vulkan 1.0+ / Metal / DX12 capable hardware. No ancient GPU fallback.

- **Single window initially**: Tabs and splits are scope creep for v1. Use tmux/zellij if needed.

- **Linux/macOS primary**: Windows support is best-effort via wgpu's DX12 backend.