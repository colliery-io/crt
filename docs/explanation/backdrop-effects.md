# Backdrop Effects System

CRT's backdrop effects are animated layers that render behind the terminal text. A retro perspective grid, a drifting starfield, falling rain, matrix-style cascading characters, floating particles, geometric shapes, and sprite sheet animations are all implemented as backdrop effects. This document explains how the effect system is structured, how effects are configured and updated, and what happens under the hood when multiple effects run simultaneously.

## The BackdropEffect Trait

Each effect implements the `BackdropEffect` trait:

```rust
pub trait BackdropEffect: Send + Sync {
    fn effect_type(&self) -> &'static str;
    fn update(&mut self, dt: f32, time: f32);
    fn render(&self, scene: &mut Scene, bounds: Rect);
    fn configure(&mut self, config: &EffectConfig);
    fn is_enabled(&self) -> bool;

    // Optional GPU resource hooks
    fn prepare_gpu_resources(&mut self, device, queue, renderer) { }
    fn needs_gpu_resources(&self) -> bool { false }
    fn cleanup_gpu_resources(&mut self, renderer) { }
}
```

The split between `update` and `render` reflects a clean separation of concerns:

- `update()` advances time-dependent state (particle positions, animation frame counters, scroll offsets). It receives `dt` (delta time since the last frame in seconds) and `time` (total elapsed seconds since the effect was initialized). This state mutation must happen once per frame.
- `render()` draws the current state into a vello `Scene`. It is a read-only operation — it should not mutate the effect's state. `render()` is called only for enabled effects.

Effects are `Send + Sync` because the `EffectsRenderer` holds them in a `Vec<Box<dyn BackdropEffect>>`, and the effects renderer itself needs to be sendable between threads (though in practice all rendering happens on the main thread).

`effect_type()` returns a string identifier like `"grid"`, `"starfield"`, or `"sprite"`. This identifier is used to prefix CSS properties during configuration dispatch.

## EffectsRenderer

`EffectsRenderer` manages the collection of active effects, drives their update-render cycle, and composites the results onto the frame.

### The Effect Collection

Effects are stored as `Vec<Box<dyn BackdropEffect>>`. The set of available effects is fixed at startup: `GridEffect`, `StarfieldEffect`, `RainEffect`, `ParticleEffect`, `MatrixEffect`, `ShapeEffect`, and `SpriteEffect` are all added to the collection during window initialization. Whether an effect is *active* is controlled by its `is_enabled()` return value, which is set from its CSS configuration.

This means all effect objects exist in memory at all times, regardless of whether they are enabled. The cost is minimal (each effect struct is a few hundred bytes), and the benefit is that toggling an effect on and off does not require allocation or configuration re-parsing — just an `enabled` flag flip.

### Configuration Dispatch

When a theme is loaded or hot-reloaded, `EffectsRenderer::configure()` is called with the full `EffectConfig` map from the theme. For each effect, it builds a filtered config containing only the properties prefixed with that effect's type name:

```
For GridEffect (type = "grid"):
    "grid-enabled" → "enabled"
    "grid-color"   → "color"
    "grid-spacing" → "spacing"
    ...
```

The prefix is stripped, and the resulting per-effect config is passed to `effect.configure()`. This prefix-based dispatch means the CSS properties namespace is flat (all under `::backdrop`) but the effect system sees only its own properties.

Each effect's `configure()` method reads from the `EffectConfig` using typed accessors (`get_bool`, `get_f32`, `get_usize`, etc.) and updates its state. Missing keys leave the existing state unchanged — configure is additive. This means effects that have been running have their configurations updated in place rather than being reconstructed.

### The Render Cycle

Each frame where any effects are enabled:

1. `EffectsRenderer::update(dt)` is called. This increments the shared elapsed time and calls `effect.update(dt, time)` for each enabled effect.

2. The vello `Scene` is reset to empty.

3. Each enabled effect's `render()` method is called, writing paths, shapes, and fills into the scene. Effects execute in the order they were added to the collection. Later effects paint over earlier ones.

4. The scene is submitted to the vello `Renderer` via `render_to_texture()`, which uses wgpu internally to rasterize the scene into the effects intermediate texture (`Rgba8Unorm` format).

5. A subsequent wgpu render pass blits the intermediate texture onto the main render target using alpha blending — the `SharedEffectsBlitPipeline` is a simple textured-quad pipeline with `src_alpha` blending.

The effects rendering step happens *before* the main wgpu command encoder is created for the frame. This is required because the vello renderer submits its own wgpu commands independently. Running vello inside the same command encoder is not supported. The result is that effects render as a separate GPU submission, and the main encoder reads from the completed effects texture.

## Rendering with Vello

Each effect draws into a shared vello `Scene` using vello's path-based API:

- `Line`, `Rect`, `BezPath`, `Circle` from the `kurbo` geometry library
- `Stroke` (outlined shapes) and filled shapes via `peniko::Fill`
- `Brush` (solid color, gradient) for fill and stroke colors
- `Affine` transforms for rotation and positioning

For example, the grid effect draws a series of `Line` segments using perspective projection: horizontal lines are drawn with decreasing spacing toward the horizon, vertical lines converge toward a vanishing point. The grid uses `BezPath` for curved vertical lines when the `curved` option is enabled.

The starfield renders circles of varying sizes with opacity modulated by a twinkle function based on `time` and a per-star phase offset.

The rain effect renders short diagonal `Line` segments with alpha that depends on position along the line (brighter at the top, fading at the bottom).

The matrix effect renders cascading columns of characters using vello's text rendering, with characters at the leading edge brighter than trailing characters.

### Why Vello for Effects?

Vello is appropriate for effects because:

1. Effects need **arbitrary 2D paths** — curves, polygons, compositing. Writing WGSL shaders for each effect type would be verbose and inflexible.
2. Effects render at **full frame rate** but their geometry changes each frame. Vello's scene model (build a scene description, render it) fits this pattern.
3. The **composition semantics** (alpha blending, layering) are already correct in vello without needing custom blend modes in WGSL.

The main cost is the vello `Renderer`'s internal GPU memory growth. Vello uses a GPU texture atlas for glyph rendering and path coverage data. Because the atlas does not shrink between frames, long-running effects cause atlas accumulation. This is managed by periodically destroying and recreating the vello renderer (see the rendering pipeline documentation).

## The Sprite Effect and SpriteRenderer

Sprites are handled by two separate systems with different trade-offs.

### SpriteEffect (Vello)

`SpriteEffect` in the backdrop effects system loads a sprite sheet PNG, decodes the frames using vello's `ImageData` type, and renders the current frame's sub-rectangle using an `Affine` transform to crop to the appropriate atlas region.

Vello's sprite approach uses `ImageBrush` to fill a rectangle with the image data. The brush transform is computed each frame to select the correct sub-rectangle for the current animation frame.

The limitation is that vello's atlas for images grows similarly to its path atlas. Animated sprites drive continuous atlas updates. For the common case of a looping sprite, this causes the same atlas memory growth problem.

### SpriteAnimationState (Raw wgpu)

`SpriteAnimationState` (managed by the `SpriteRenderer`) bypasses vello entirely. It loads the sprite sheet as a raw wgpu `Texture` with the full sprite sheet as one texture, then uses a custom WGSL shader to sample the correct UV region for the current animation frame. The UV coordinates are updated each frame by writing to a small uniform buffer.

This approach has constant GPU memory cost — the sprite sheet texture is uploaded once and stays in VRAM, and no vello atlas accumulation occurs. The trade-off is that it requires more code: a custom pipeline, UV arithmetic, and manual texture uploads.

Which system is used depends on how the sprite is configured in the theme:

- Sprites configured via the `::backdrop` pseudo-selector and `--sprite-*` custom properties use the `SpriteEffect` (vello path).
- Sprites configured via the `:terminal::sprite` selector use `SpriteAnimationState` (raw wgpu path).

The raw wgpu path is recommended for sprites that need to run continuously (like an animated character) because of the memory stability advantage.

### Animation State

Both systems advance animation using the same pattern: accumulate elapsed time and compute the current frame index when elapsed time exceeds the frame duration:

```
current_frame = floor(elapsed_time * fps) % total_frames
```

When the frame changes, the GPU-side state (UV coordinates or a texture array index) is updated.

### Motion Behaviors

Both sprite systems support several motion behaviors: `None` (static position), `Bounce` (reflects off screen edges), `Scroll` (moves steadily in one direction and wraps), `Float` (gentle sinusoidal drift), and `Orbit` (circular path). The motion is computed in `update()` using the elapsed time and velocity, producing a position that is passed to `render()`.

## Multiple Effects Composing

All backdrop effects write into the same vello `Scene` object before it is rendered. The scene accumulates drawing commands in order. Because vello renders with painter's algorithm semantics, effects added later paint over effects added earlier.

The render order within the effects layer is the order effects were added to `EffectsRenderer::effects` at initialization time. By convention this is: grid, starfield, rain, particles, matrix, shape, sprite. A perspective grid would typically render first (so it appears behind stars), with particles on top (so they float in front of the grid).

Because all effects share a single scene and a single render pass to texture, there is no per-effect texture allocation or per-effect compositing step. The cost of running five effects is the sum of their scene-building time plus one vello render pass — not five separate render passes.

The blit pass that composites the effects texture onto the main render target uses `BlendState::ALPHA_BLENDING`. Effect pixels with alpha 0 are transparent (the gradient background shows through). Effect pixels with alpha 1 fully replace the background.

## The CRT Post-Processing Effect

CRT post-processing is fundamentally different from backdrop effects and is not part of the `BackdropEffect` system. It is implemented as a separate `CrtPipeline` that processes the *entire frame* — text, background, effects, everything — through a final WGSL shader.

Backdrop effects live *behind* the text. CRT post-processing transforms *the entire rendered image*. This placement means CRT scanlines and curvature apply to the text glyphs themselves, making the terminal look like it is being displayed on a physical CRT monitor rather than just having a decorative background.

When CRT is enabled, the rendering pipeline routes everything through an intermediate `crt_texture` instead of rendering directly to the window surface. The `CrtPipeline` then reads `crt_texture` and writes its post-processed output to the surface.

The CRT shader parameters include:

- `scanline_intensity` / `scanline_frequency`: Dark horizontal bands simulating electron beam scan lines
- `curvature`: Barrel/pincushion distortion simulating a curved CRT screen
- `vignette`: Corner darkening simulating phosphor falloff at the edges of a CRT tube
- `chromatic_aberration`: Red/blue channel displacement simulating imperfect electron gun convergence
- `bloom`: Local glow around bright areas
- `flicker`: Subtle per-frame brightness variation simulating AC power interference

All parameters are continuous floats configurable from the theme. A value of 0.0 disables that particular sub-effect. The CRT pipeline is only enabled when at least `enabled: true` is set in the theme's `::crt` section — when disabled, no intermediate texture is created and no CRT pass executes.

## Performance Implications

### Stacking Effects

Each enabled backdrop effect adds to the vello scene construction time proportional to the number of geometric primitives it generates. Typical per-effect scene construction costs:

- **Grid**: Low (a fixed number of line segments)
- **Starfield**: Low-to-medium (scales with star count / density)
- **Rain**: Medium (scales with density)
- **Particles**: Medium-to-high (scales with particle count)
- **Matrix**: High (rasterizes text characters per column)
- **Shape**: Very low (single shape)
- **Sprite**: Low (single image blit)

The single vello render-to-texture call after scene construction is the dominant GPU cost. That cost scales with screen resolution and scene complexity. All enabled effects share this one render call.

For typical desktop resolutions (1080p to 4K) with two or three effects enabled, the effects render pass takes 1–3 ms on a modern GPU. This is within the 16.67 ms frame budget at 60 fps with room to spare.

### Atlas Memory Growth

The vello renderer's internal atlas accumulates memory during continuous effect rendering. The 300-frame reset cycle (described in the rendering pipeline document) bounds this accumulation to roughly the atlas state built up over 5 seconds of effects at 60 fps, which in practice is well under 100 MB.

If effects are disabled (no enabled `BackdropEffect`), the vello renderer is never initialized and the periodic reset is skipped. The lazy initialization design means themes without effects pay no memory cost for the vello renderer infrastructure.

### CRT Effect Cost

The CRT post-processing adds one full-screen texture sample and several ALU operations per pixel. At 4K (3840×2160), this is approximately 8.3 million pixel shader invocations per frame. On modern GPUs this costs 0.5–2 ms. The intermediate texture required for CRT post-processing costs approximately 32 MB at 4K (4 bytes × 3840 × 2160) and is drawn from the `TexturePool`.

CRT post-processing is optional. Themes that do not enable it pay none of this cost.
