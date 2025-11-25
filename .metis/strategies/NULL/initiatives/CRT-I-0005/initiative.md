---
id: crt-terminal-implementation
level: initiative
title: "CRT Terminal Implementation"
short_code: "CRT-I-0005"
created_at: 2025-11-25T03:02:23.958402+00:00
updated_at: 2025-11-25T03:03:46.668006+00:00
parent: 
blocked_by: []
archived: false

tags:
  - "#initiative"
  - "#phase/active"


exit_criteria_met: false
estimated_complexity: XL
strategy_id: NULL
initiative_id: crt-terminal-implementation
---

# CRT Terminal Implementation Initiative

*This template includes sections for various types of initiatives. Delete sections that don't apply to your specific use case.*

## Context

Building CRT - a GPU-accelerated terminal emulator with CSS-driven theming. Research initiative CRT-I-0001 validated the CSS-to-shader pipeline approach. ADR CRT-A-0001 documents the hybrid architecture decision (shader generation + uniforms).

Prototypes completed:
- `prototype_a.rs` - Static shader + uniforms mapping
- `prototype_b.rs` - Dynamic shader generation  
- `text_glow.rs` - CSS text-shadow style effects
- `synthwave.rs` - Full theme composition (gradient + grid + glow)
- `font_rendering.rs` - Real font rendering with ligatures via glyphon

## Goals & Non-Goals

**Goals:**
- Build a working terminal emulator with shell interaction
- GPU-accelerated rendering with visual effects (glow, gradients, patterns)
- CSS-like theming system for customization
- Support macOS and Linux platforms

**Non-Goals:**
- Windows support (initially)
- Plugin/extension system
- Multiplexing (tmux-style)

## Architecture

### Workspace Structure
```
crt/
├── Cargo.toml          # Workspace root
├── crates/
│   ├── crt-core/       # Terminal state, grid, PTY integration
│   ├── crt-renderer/   # GPU rendering (text + effects)
│   ├── crt-theme/      # CSS parsing, theme engine
│   └── crt-app/        # Window management, event loop, binary
├── shaders/            # WGSL shaders (existing)
└── examples/           # Prototypes (existing)
```

### Crate Responsibilities

**crt-core**: Terminal emulation using `alacritty_terminal` + `vte` crates
- Shell process lifecycle (spawn, read, write)
- Terminal grid state (cells, attributes, cursor)
- Scrollback buffer management

**crt-renderer**: GPU rendering using `wgpu` + `glyphon`
- Text atlas management (glyph caching)
- Effect pipeline (glow, gradients, grid patterns)
- Render pass composition

**crt-theme**: CSS parsing using `cssparser`
- Parse CSS-like theme files
- Map CSS properties to shader uniforms
- Theme hot-reloading

**crt-app**: Application shell using `winit`
- Window creation and management
- Event loop (keyboard, mouse, resize)
- Configuration loading

## Detailed Design

### Key Dependencies
```toml
# Core terminal
vte = "0.13"
alacritty_terminal = "0.24"

# Rendering
wgpu = "25"
glyphon = "0.9"
bytemuck = { version = "1.14", features = ["derive"] }

# Windowing
winit = "0.30"

# Theme parsing
cssparser = "0.34"
```

### Render Pipeline
1. Update theme uniforms from Theme struct
2. Background pass (gradient + pattern)
3. Glow pass (pre-render text for blur)
4. Text pass (glyphon rendering)

## Alternatives Considered

See ADR CRT-A-0001 for full analysis. Summary:
- Pure uniform approach: rejected (limited flexibility)
- Full shader generation: rejected (complexity)
- Chosen: Hybrid (shader gen for effects + uniforms for params)

## Implementation Plan

### Phase 1: Foundation
- Set up workspace structure
- Integrate alacritty_terminal for grid/scrollback
- Integrate vte for escape sequence parsing
- Set up PTY for shell spawning
- Basic text rendering from prototype

**Exit criteria**: Can spawn shell, display output, accept input

### Phase 2: Effects Pipeline
- Port synthwave shader to modular effect system
- Implement theme uniform binding
- Add background gradient support
- Add text glow/shadow support

**Exit criteria**: Themes can enable/disable effects at runtime

### Phase 3: Theme Engine
- Define CSS-like syntax subset
- Implement parser for theme files
- Map CSS properties to renderer uniforms
- Add hot-reload support

**Exit criteria**: Load .css theme file and apply to terminal

### Phase 4: Polish
- Performance optimization
- Platform-specific polish
- Configuration system
- Font fallback handling

**Exit criteria**: Daily-driver ready