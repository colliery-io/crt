---
id: vello-powered-backdrop-effects
level: initiative
title: "Vello-Powered Backdrop Effects System"
short_code: "CRT-I-0014"
created_at: 2025-11-27T13:49:27.992400+00:00
updated_at: 2025-11-27T14:10:12.566720+00:00
parent: CRT-V-0001
blocked_by: []
archived: false

tags:
  - "#initiative"
  - "#phase/active"


exit_criteria_met: false
estimated_complexity: L
strategy_id: NULL
initiative_id: vello-powered-backdrop-effects
---

# Vello-Powered Backdrop Effects System Initiative

*This template includes sections for various types of initiatives. Delete sections that don't apply to your specific use case.*

## Context

CRT currently has a single backdrop effect: a perspective grid rendered via WGSL shaders (`background.wgsl`). While visually distinctive, this "one trick pony" approach limits creative expression for theme authors.

The terminal already has Vello integrated for tab bar rendering and terminal overlays (cursor, selection). Vello is a high-performance 2D GPU renderer capable of vector graphics, images, and complex compositing - exactly what's needed for diverse animated backgrounds.

The goal is to create an extensible effects system that allows themes to specify multiple backdrop effects via CSS custom properties. The ultimate benchmark: being able to render something as creative as an animated Nyan Cat with rainbow trail and twinkling stars.

## Goals & Non-Goals

**Goals:**
- Create a trait-based effect system that allows multiple backdrop effects to be composed
- Port existing grid effect from WGSL to Vello (proving the architecture)
- Implement particle-based effects: starfield, rain, floating particles
- Implement matrix-style falling code effect
- Implement custom geometric shapes (rect, circle, star, heart, polygon)
- Support animated sprite rendering (sprite sheets with frame animation)
- Implement reusable motion system (bounce, scroll, float, orbit) for shapes and sprites
- Expose all effects via CSS custom properties in `::backdrop` selector
- Maintain 60fps performance on reasonable hardware
- Enable theme authors to create wildly creative backgrounds

**Non-Goals:**
- Full CSS animation keyframe support (custom properties only)
- 3D effects or WebGL-style shaders (Vello is 2D)
- Sound/audio tied to effects
- User-uploadable effect plugins (built-in effects only for now)

## Use Cases

### Use Case 1: Theme Author Creates Cyberpunk Theme
- **Actor**: Theme author
- **Scenario**: Author wants a blade-runner style theme with rain and neon
  1. Creates CSS theme file
  2. Sets `--rain-enabled: true` with cyan color and slight angle
  3. Sets `--particles-enabled: true` for floating dust/embers
  4. Adds gradient background in orange/pink
- **Expected Outcome**: Rain falls at angle over gradient, particles drift

### Use Case 2: Theme Author Creates Retro Theme with Nyan Cat
- **Actor**: Theme author  
- **Scenario**: Author wants playful retro theme with animated sprite
  1. Creates sprite sheet PNG with animation frames
  2. Sets `--sprite-enabled: true`, `--sprite-url: "nyan.png"`
  3. Configures frame count, FPS, and position
  4. Adds `--starfield-enabled: true` for twinkling background
- **Expected Outcome**: Animated sprite cycles through frames, stars twinkle

### Use Case 3: Theme Author Creates DVD-Style Bouncing Logo
- **Actor**: Theme author
- **Scenario**: Author wants a bouncing shape like classic DVD screensaver
  1. Creates CSS theme with `--shape-enabled: true`
  2. Sets `--shape-type: star` with gold fill and glow
  3. Sets `--shape-motion: bounce` and `--shape-speed: 150`
  4. Optionally adds `--shape-rotation: spin`
- **Expected Outcome**: Star bounces off screen edges, changes direction on impact

### Use Case 4: User Runs Multiple Effects
- **Actor**: End user
- **Scenario**: User loads theme with multiple effects enabled
  1. Theme specifies grid + starfield + particles + bouncing shape
  2. All effects render and composite correctly
  3. Terminal text remains readable over effects
- **Expected Outcome**: Layered effects at 60fps, text legible

## Architecture

### Mental Model
| Want this? | Use this |
|------------|----------|
| Animated grid | GridEffect |
| Starry sky | StarfieldEffect |
| Falling rain | RainEffect |
| Falling code | MatrixEffect |
| Many floating hearts/stars | ParticleEffect (with shape) |
| Single bouncing shape | ShapeEffect |
| Complex animated character | SpriteEffect (sprite sheet PNG) |

**Simple rule:** One shape = ShapeEffect. Many simple shapes = ParticleEffect. Complex/animated = Sprite sheet.

### Overview
A trait-based effect system where each effect type implements `BackdropEffect`. An `EffectsRenderer` manages the collection of effects, updates their animation state each frame, and renders them to a Vello scene which is then composited into the render pipeline.

### Core Trait
```rust
pub trait BackdropEffect: Send + Sync {
    fn effect_type(&self) -> &'static str;
    fn update(&mut self, dt: f32, time: f32);
    fn render(&self, scene: &mut vello::Scene, bounds: Rect);
    fn configure(&mut self, config: &EffectConfig);
}
```

### Component Structure
```
crates/crt-renderer/src/effects/
  mod.rs           - Trait definitions, EffectConfig
  renderer.rs      - EffectsRenderer (manages effects, renders to texture)
  motion.rs        - MotionBehavior enum + physics (bounce, scroll, float, orbit)
  grid.rs          - GridEffect (port from WGSL)
  starfield.rs     - StarfieldEffect (parallax star layers)
  rain.rs          - RainEffect (falling drops)
  matrix.rs        - MatrixEffect (falling code)
  particles.rs     - ParticleEffect (floating dust/embers)
  shape.rs         - ShapeEffect (custom geometric shapes)
  sprite.rs        - SpriteEffect (animated sprite sheets)
```

### Motion System
```rust
pub enum MotionBehavior {
    None,                           // Static position
    Bounce { velocity: Vec2 },      // DVD-logo style, reflects off edges
    Scroll { direction: Vec2 },     // Moves in direction, wraps around
    Float { drift: Vec2 },          // Gentle random wandering
    Orbit { center: Vec2, radius: f32, speed: f32 },
}

impl MotionBehavior {
    pub fn update(&mut self, position: &mut Vec2, bounds: Rect, dt: f32);
}
```

Effects with position (ShapeEffect, SpriteEffect) compose with MotionBehavior for movement.

### Render Pipeline Integration
1. `EffectsRenderer` receives theme config on load/hot-reload
2. Each frame: `update(dt)` advances animation state
3. Each frame: `render()` builds Vello scene, renders to texture
4. Effects texture composited in background pass (before terminal content)

## Detailed Design

### Effect Configurations (CSS Custom Properties)

**GridEffect:**
- `--grid-enabled: true|false`
- `--grid-color: <color>`
- `--grid-spacing: <length>`
- `--grid-line-width: <length>`
- `--grid-perspective: <number>` (0 = flat, 1 = full perspective)
- `--grid-horizon: <percentage>` (where horizon line sits)
- `--grid-animation-speed: <number>`

**StarfieldEffect:**
- `--starfield-enabled: true|false`
- `--starfield-density: <number>` (stars per layer)
- `--starfield-layers: <number>` (parallax depth layers)
- `--starfield-speed: <number>` (movement speed)
- `--starfield-color: <color>`
- `--starfield-twinkle: true|false`

**RainEffect:**
- `--rain-enabled: true|false`
- `--rain-density: <number>` (drops on screen)
- `--rain-speed: <number>`
- `--rain-color: <color>`
- `--rain-angle: <angle>` (0 = vertical, positive = wind right)
- `--rain-length: <length>` (drop trail length)

**MatrixEffect:**
- `--matrix-enabled: true|false`
- `--matrix-density: <number>` (columns)
- `--matrix-speed: <number>`
- `--matrix-color: <color>` (base color, head is brighter)
- `--matrix-charset: <string>` (characters to use)
- `--matrix-font-size: <length>`

**ParticleEffect:** (many simple things floating)
- `--particles-enabled: true|false`
- `--particles-count: <number>`
- `--particles-shape: dot|circle|star|heart|sparkle` (default: dot)
- `--particles-color: <color>`
- `--particles-size: <length>`
- `--particles-speed: <number>`
- `--particles-behavior: float|drift|rise|fall`

**ShapeEffect:** (single geometric shape with motion)
- `--shape-enabled: true|false`
- `--shape-type: rect|circle|ellipse|triangle|star|heart|polygon`
- `--shape-points: <number>` (for star/polygon)
- `--shape-size: <length>`
- `--shape-fill: <color|gradient>`
- `--shape-stroke: <color>`
- `--shape-stroke-width: <length>`
- `--shape-glow: <length> <color>`
- `--shape-rotation: none|spin|wobble`
- `--shape-rotation-speed: <number>` (rotations per second)

**SpriteEffect:**
- `--sprite-enabled: true|false`
- `--sprite-url: <url>` (sprite sheet image)
- `--sprite-frames: <number>` (frames in sheet)
- `--sprite-columns: <number>` (sheet layout)
- `--sprite-fps: <number>` (animation speed)
- `--sprite-scale: <number>`

**Motion System (shared by Shape and Sprite):**
- `--*-motion: none|bounce|scroll|float|orbit`
- `--*-speed: <number>` (pixels per second)
- `--*-direction: <angle>` (initial direction)
- `--*-x: <length|percentage>` (initial/static position)
- `--*-y: <length|percentage>`

### Vello Rendering Techniques

**Vector Shapes:** Grid lines, rain drops, stars use `scene.fill()` and `scene.stroke()` with kurbo shapes (Rect, Line, Circle).

**Glyph Rendering:** Matrix effect uses Vello's text API to render individual characters at positions.

**Image/Sprite Rendering:** `scene.draw_image()` with `peniko::Image`. Sprite frame selection via clip region + transform to show correct frame from sheet.

**Compositing:** Effects render to texture via shared `VelloRenderer`. Texture sampled in background shader pass.

## Alternatives Considered

### Alternative 1: Extend WGSL Shaders
Keep effects in WGSL, add more shader programs for different effects.

**Rejected because:**
- WGSL has no image/texture sampling for sprites
- Each effect needs separate shader + pipeline setup
- Harder to compose multiple effects
- Less flexible for complex animations (matrix text)

### Alternative 2: CPU Rendering to Texture
Render effects on CPU, upload texture each frame.

**Rejected because:**
- Poor performance for complex effects
- Doesn't leverage GPU
- Would need separate image library

### Alternative 3: Use Existing Vello (Chosen)
Leverage Vello already integrated for tab bar/overlays.

**Chosen because:**
- GPU-accelerated 2D rendering
- Supports images, text, vectors, compositing
- Already integrated and battle-tested in codebase
- Scene-based API is clean and composable
- Lazy initialization pattern already exists

## Implementation Plan

### Phase 1: Foundation
- Create `effects/` module with trait definitions
- Implement `EffectsRenderer` scaffolding
- Implement `MotionBehavior` system (bounce, scroll, float, orbit)
- Port grid from WGSL to Vello as `GridEffect`
- Wire into render pipeline, verify parity with existing grid
- Update theme parser for effect properties

### Phase 2: Particle Effects
- Implement `StarfieldEffect` with parallax layers
- Implement `RainEffect` with angle/density config
- Implement `ParticleEffect` for floating dust/embers/hearts
- Add CSS properties to theme parser
- Create demo themes showcasing each effect

### Phase 3: Matrix Code
- Implement `MatrixEffect` with Vello text rendering
- Support custom character sets
- Add head glow and trail fade
- Update matrix.css theme

### Phase 4: Shapes and Motion
- Implement `ShapeEffect` with geometric primitives (rect, circle, star, heart, polygon)
- Support fill, stroke, glow rendering
- Support rotation animations (spin, wobble)
- Integrate motion system for bouncing/scrolling shapes
- Create demo theme with bouncing DVD-style shape

### Phase 5: Sprite Animation
- Implement sprite sheet loading (PNG)
- Implement `SpriteEffect` with frame animation
- Add clip-based frame selection
- Integrate motion system for sprite movement
- Create nyan-cat demo theme (the dream!)

### Phase 6: Polish
- Performance optimization and profiling
- Documentation for theme authors
- Update existing themes with new effects
- Edge case handling and error recovery