# Built-in Themes Reference

CRT ships with 19 built-in themes installed to `~/.config/crt/themes/`. Each theme is a CSS file that can be viewed and modified directly.

To activate a theme, set `name` under `[theme]` in `config.toml`:

```toml
[theme]
name = "synthwave"
```

Themes can also be switched at runtime via right-click > context menu without restarting.

---

## Theme Index

| # | Name | Reactive | Gallery Screenshot |
|---|---|---|---|
| 1 | `synthwave` | No | [synthwave.png](../gallery/synthwave.png) |
| 2 | `dracula` | No | [dracula.png](../gallery/dracula.png) |
| 3 | `tron` | No | [tron.png](../gallery/tron.png) |
| 4 | `vaporwave` | No | [vaporwave.png](../gallery/vaporwave.png) |
| 5 | `matrix` | No | [matrix.png](../gallery/matrix.png) |
| 6 | `robco` | No | [robco.png](../gallery/robco.png) |
| 7 | `robco-reactive` | Yes | — |
| 8 | `alien` | No | [alien.png](../gallery/alien.png) |
| 9 | `wh40k` | No | [wh40k.png](../gallery/wh40k.png) |
| 10 | `solarized` | No | [solarized.png](../gallery/solarized.png) |
| 11 | `minimal` | No | [minimal.png](../gallery/minimal.png) |
| 12 | `starfield` | No | [starfield.png](../gallery/starfield.png) |
| 13 | `rain` | No | [rain.png](../gallery/rain.png) |
| 14 | `particles` | No | [particles.png](../gallery/particles.png) |
| 15 | `shape` | No | [shape.png](../gallery/shape.png) |
| 16 | `stress` | No | [stress.png](../gallery/stress.png) |
| 17 | `nyancat` | No | [nyancat.png](../gallery/nyancat.png) |
| 18 | `nyancat-responsive` | Yes | — |
| 19 | `pokemon` | No | — |

"Reactive" means the theme responds to shell events (command success/failure, bell) by temporarily changing visual properties. Reactive themes require OSC 133 shell integration — see [Shell Integration Requirements](#shell-integration-requirements).

---

## Theme Descriptions

### synthwave

| Property | Value |
|---|---|
| Background | Dark purple (`#0d0021`) |
| Foreground | Cyan (`#00ffff`) |
| Cursor | Magenta |
| Color scheme | Purple-tinted grayscale ANSI palette |
| Effects | Perspective grid (animated, glowing magenta lines), text glow |
| Reactive | No |

80s neon retrowave aesthetic. Animated perspective grid scrolls toward the viewer. The default theme for release builds.

---

### dracula

| Property | Value |
|---|---|
| Background | Dark purple (`#282a36`) |
| Foreground | Light gray (`#f8f8f2`) |
| Cursor | Pink/magenta |
| Color scheme | Dracula standard palette |
| Effects | None |
| Reactive | No |

The classic Dracula color scheme. No background effects; pure color scheme with no GPU overhead beyond basic rendering.

---

### tron

| Property | Value |
|---|---|
| Background | Near-black with blue tint |
| Foreground | Bright cyan |
| Cursor | Cyan |
| Color scheme | Cyan/blue TRON-inspired |
| Effects | Perspective grid (cyan lines) |
| Reactive | No |

TRON Legacy-inspired aesthetic. Animated perspective grid in cyan. Pairs with the blue-dominant ANSI palette.

---

### vaporwave

| Property | Value |
|---|---|
| Background | Dark purple-blue |
| Foreground | Pink/cyan |
| Cursor | Hot pink |
| Color scheme | Vaporwave pink and cyan |
| Effects | Perspective grid (animated, pink/magenta lines) |
| Reactive | No |

Vaporwave aesthetic with pink and cyan color scheme. Animated scrolling perspective grid.

---

### matrix

| Property | Value |
|---|---|
| Background | Black |
| Foreground | Bright green |
| Cursor | Green |
| Color scheme | Green-dominant; all ANSI colors shifted to green variants |
| Effects | Matrix rain (falling green characters), CRT post-processing (scanlines, curvature, vignette) |
| Reactive | No |

The Matrix. Animated falling character rain in the background combined with CRT post-processing effects (scanlines, screen curvature, vignette) on the entire terminal output.

---

### robco

| Property | Value |
|---|---|
| Background | Near-black |
| Foreground | Amber-green phosphor (`#20c20e`) |
| Cursor | Phosphor green |
| Color scheme | Monochrome green phosphor |
| Effects | CRT post-processing (scanlines, curvature, vignette, flicker), text glow |
| Reactive | No |

Fallout Pip-Boy terminal aesthetic. Simulates a green phosphor CRT monitor (Robco Industries MF series). CRT scanlines, screen curvature, vignette, and phosphor flicker are all active.

---

### robco-reactive

| Property | Value |
|---|---|
| Background | Near-black |
| Foreground | Amber-green phosphor |
| Cursor | Phosphor green |
| Color scheme | Monochrome green phosphor |
| Effects | CRT post-processing, Vault Boy sprite |
| Reactive | Yes |

Extends `robco` with a Vault Boy sprite that reacts to shell events: shows a thumbs-up animation on command success and an alert/fail animation on command failure. Requires OSC 133 shell integration (see below).

---

### alien

| Property | Value |
|---|---|
| Background | Near-black |
| Foreground | Amber phosphor |
| Cursor | Amber |
| Color scheme | Amber monochrome |
| Effects | CRT post-processing (scanlines, curvature, heavy vignette), text glow |
| Reactive | No |

Weyland-Yutani MU/TH/UR 6000 computer terminal from *Alien*. Amber phosphor CRT with heavy vignette and scanlines.

---

### wh40k

| Property | Value |
|---|---|
| Background | Dark (`#0a0a0a`) |
| Foreground | Aged parchment/gold |
| Cursor | Gold |
| Color scheme | Imperial gold and grey |
| Effects | Servo skull sprite animation, text glow |
| Reactive | No |

Warhammer 40,000 Imperial cogitator terminal. Includes an animated servo skull sprite. Typography and colors evoke grimdark Imperial aesthetics.

---

### solarized

| Property | Value |
|---|---|
| Background | `#002b36` (solarized dark base) |
| Foreground | `#839496` |
| Cursor | Cyan |
| Color scheme | Full Solarized Dark palette |
| Effects | None |
| Reactive | No |

The Solarized Dark color scheme by Ethan Schoonover. No background effects.

---

### minimal

| Property | Value |
|---|---|
| Background | Pure black (`#000000`) |
| Foreground | Light gray |
| Cursor | White |
| Color scheme | Clean neutral palette |
| Effects | None |
| Reactive | No |

No effects, no animations. Pure black background with a clean neutral color palette. Minimum GPU overhead.

---

### starfield

| Property | Value |
|---|---|
| Background | Black |
| Foreground | White |
| Cursor | White |
| Color scheme | Standard dark |
| Effects | Starfield (twinkling stars, multiple parallax layers) |
| Reactive | No |

Animated twinkling starfield in the background. Stars rendered across multiple parallax layers for depth. Text glow active.

---

### rain

| Property | Value |
|---|---|
| Background | Dark blue-grey |
| Foreground | Light |
| Cursor | Blue |
| Color scheme | Cool neutral |
| Effects | Animated rain drops |
| Reactive | No |

Animated rain drops falling in the background. Drop density, speed, and angle are configurable in the CSS.

---

### particles

| Property | Value |
|---|---|
| Background | Dark |
| Foreground | White/pink |
| Cursor | Pink |
| Color scheme | Warm |
| Effects | Floating heart particle system |
| Reactive | No |

Heart-shaped particles float upward in the background. Uses the particle system with `--particles-shape: heart` and `--particles-behavior: rise`.

---

### shape

| Property | Value |
|---|---|
| Background | Dark |
| Foreground | White |
| Cursor | White |
| Color scheme | Neutral |
| Effects | Animated spinning polygon (geometric shape) |
| Reactive | No |

A single animated geometric shape (polygon) bounces and rotates in the background. Demonstrates the shape effect system.

---

### stress

| Property | Value |
|---|---|
| Background | Dark |
| Foreground | White |
| Cursor | White |
| Color scheme | Neutral |
| Effects | All effects simultaneously (grid, starfield, rain, particles, matrix, shape, CRT post-processing) |
| Reactive | No |

All visual effects enabled at once. Intended for GPU performance testing and benchmarking, not normal use. Use this theme with `benchmark_gpu` to establish worst-case frame times.

---

### nyancat

| Property | Value |
|---|---|
| Background | Black |
| Foreground | White |
| Cursor | Rainbow/white |
| Color scheme | Standard dark |
| Effects | Animated Nyan Cat sprite, starfield |
| Reactive | No |

Nyan Cat sprite sheet animation plays in the corner. Background starfield active. The default theme for debug builds.

---

### nyancat-responsive

| Property | Value |
|---|---|
| Background | Black |
| Foreground | White |
| Cursor | White |
| Color scheme | Standard dark |
| Effects | Animated Nyan Cat sprite, starfield |
| Reactive | Yes |

Extends `nyancat` with reactive sprite swaps: happy Nyan Cat on command success, sad Nyan Cat on command failure, alert Nyan Cat on bell. Requires OSC 133 shell integration.

---

### pokemon

| Property | Value |
|---|---|
| Background | Dark |
| Foreground | White |
| Cursor | Yellow |
| Color scheme | Warm with yellow accent |
| Effects | Animated Pikachu sprite (run cycle) |
| Reactive | No |

Animated Pikachu run-cycle sprite. Uses the sprite animation system with a multi-frame sprite sheet.

---

## Shell Integration Requirements

Reactive themes (`robco-reactive`, `nyancat-responsive`) require OSC 133 semantic prompt markers to detect command exit codes. See [How to Set Up Reactive Themes](../how-to/set-up-reactive-themes.md) for setup instructions.

---

## Theme Categories

| Category | Themes |
|----------|--------|
| Minimal | minimal, solarized, dracula |
| Retro/CRT | robco, robco-reactive, alien, matrix, wh40k |
| Neon/Aesthetic | synthwave, tron, vaporwave |
| Effects Showcase | starfield, rain, particles, shape |
| Animated/Fun | nyancat, nyancat-responsive, pokemon |
| Performance Testing | stress |
