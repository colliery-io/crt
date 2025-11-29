# CRT Terminal Theming Guide

CRT Terminal uses CSS-like syntax for theming. Themes are stored in `~/.config/crt/themes/` and selected via `config.toml`.

## Quick Start

1. Create a theme file: `~/.config/crt/themes/mytheme.css`
2. Set it in config: `name = "mytheme"` under `[theme]`
3. The terminal will hot-reload when you save changes

## Theme Structure

```css
/* Main terminal styling */
:terminal {
    /* Typography, colors, background */
}

/* Text selection */
:terminal::selection {
    background: #color;
    color: #color;
}

/* Search highlight */
:terminal::highlight {
    background: #color;
    color: #color;
}

/* Backdrop effects */
:terminal::backdrop {
    /* Grid, starfield, particles, CRT effects */
}

/* ANSI color palette */
:terminal::palette {
    --color-0: #000000;  /* black */
    /* ... colors 0-15, plus extended 16-255 */
}

/* Tab bar styling */
:terminal::tab-bar { }
:tab { }
:tab.active { }
:terminal::tab-close { }
```

## Terminal Properties

### Typography

```css
:terminal {
    font-family: "MesloLGS NF", "Fira Code", monospace;
    font-size: 14;
    line-height: 1.5;

    /* Font variants (optional) */
    --font-bold: "MesloLGS NF Bold";
    --font-italic: "MesloLGS NF Italic";
    --font-bold-italic: "MesloLGS NF Bold Italic";
}
```

### Colors

```css
:terminal {
    /* Foreground text color */
    color: #c8c8c8;

    /* Background - solid or gradient */
    background: #1a1a1a;
    background: linear-gradient(to bottom, #1a0a2e, #16213e);

    /* Text glow effect */
    text-shadow: 0 0 8px rgba(255, 176, 0, 0.6);
}

/* Cursor color and glow */
:terminal::cursor {
    background: #ffffff;
    text-shadow: 0 0 8px rgba(255, 255, 255, 0.6);  /* optional glow */
}
```

### Background Image

```css
:terminal {
    background-image: url("images/background.jpg");
    background-size: cover;      /* cover, contain, auto, 100px, 50% */
    background-position: center; /* center, top, bottom, left, right */
    background-repeat: no-repeat; /* no-repeat, repeat, repeat-x, repeat-y */
    --background-opacity: 0.8;   /* 0.0 - 1.0 */
}
```

## ANSI Palette

### Named Colors (Preferred)

```css
:terminal {
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
}
```

### Numeric Colors (Extended Palette)

```css
:terminal::palette {
    --color-0: #000000;   /* black */
    --color-1: #ff0000;   /* red */
    /* ... through --color-15 for bright white */

    /* Extended 256-color palette (16-255) */
    --color-226: #ffff00;
    --color-178: #d7af00;
}
```

## Backdrop Effects

Effects are rendered behind the terminal content.

### Grid Effect

```css
:terminal::backdrop {
    --grid-enabled: true;
    --grid-color: rgba(255, 0, 255, 0.3);
    --grid-spacing: 8.0;
    --grid-line-width: 0.02;
    --grid-perspective: 2.0;
    --grid-horizon: 0.35;
    --grid-animation-speed: 0.5;
    --grid-glow-radius: 8.0;
    --grid-glow-intensity: 0.5;
    --grid-curved: true;
}
```

### Starfield Effect

```css
:terminal::backdrop {
    --starfield-enabled: true;
    --starfield-color: #ffffff;
    --starfield-density: 100;
    --starfield-layers: 3;
    --starfield-speed: 0.3;
    --starfield-direction: down;  /* up, down, left, right */
    --starfield-twinkle: true;
    --starfield-twinkle-speed: 2.0;
    --starfield-min-size: 1.0;
    --starfield-max-size: 3.0;
}
```

### Rain Effect

```css
:terminal::backdrop {
    --rain-enabled: true;
    --rain-color: rgba(100, 150, 255, 0.6);
    --rain-density: 150;
    --rain-speed: 1.0;
    --rain-angle: 0.0;
    --rain-length: 20.0;
    --rain-thickness: 1.5;
}
```

### Particle Effect

```css
:terminal::backdrop {
    --particles-enabled: true;
    --particles-color: #ff00ff;
    --particles-count: 50;
    --particles-shape: circle;  /* dot, circle, star, heart, sparkle */
    --particles-behavior: float; /* float, drift, rise, fall */
    --particles-size: 4.0;
    --particles-speed: 0.5;
}
```

### Matrix Effect

```css
:terminal::backdrop {
    --matrix-enabled: true;
    --matrix-color: #00ff00;
    --matrix-density: 1.0;
    --matrix-speed: 8.0;
    --matrix-font-size: 14.0;
    --matrix-charset: "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
}
```

### Shape Effect

```css
:terminal::backdrop {
    --shape-enabled: true;
    --shape-type: polygon;  /* circle, rect, ellipse, triangle, star, heart, polygon */
    --shape-size: 100.0;
    --shape-fill: rgba(255, 0, 255, 0.2);
    --shape-stroke: #ff00ff;
    --shape-stroke-width: 2.0;
    --shape-glow-radius: 20.0;
    --shape-glow-color: #ff00ff;
    --shape-rotation: spin;  /* none, spin, wobble */
    --shape-rotation-speed: 1.0;
    --shape-motion: float;  /* none, bounce, scroll, float, orbit */
    --shape-motion-speed: 1.0;
    --shape-polygon-sides: 6;  /* for polygon type */
}
```

### Sprite Animation

```css
:terminal::backdrop {
    --sprite-enabled: true;
    --sprite-path: "images/sprite.png";
    --sprite-frame-width: 64;
    --sprite-frame-height: 64;
    --sprite-columns: 8;
    --sprite-rows: 4;
    --sprite-frame-count: 32;
    --sprite-fps: 12.0;
    --sprite-scale: 1.0;
    --sprite-opacity: 1.0;
    --sprite-position: bottom-right;  /* center, corners, edges */
    --sprite-motion: none;  /* none, bounce, scroll, float, orbit */
    --sprite-motion-speed: 1.0;
}
```

See [How to Create a Sprite Sheet from a GIF](how-to/create-sprite-from-gif.md) for creating sprite sheets from animated GIFs.

### CRT Post-Processing Effect

Applies authentic CRT monitor effects as a post-processing pass.

```css
:terminal::backdrop {
    --crt-enabled: true;

    /* Scanlines - horizontal dark bands */
    --crt-scanline-intensity: 0.08;  /* 0.0-1.0, higher = darker lines */
    --crt-scanline-frequency: 1.0;   /* lines per pixel height */

    /* Screen curvature - barrel distortion */
    --crt-curvature: 0.015;          /* 0.0 = flat, 0.1 = very curved */

    /* Vignette - edge darkening */
    --crt-vignette: 0.2;             /* 0.0-1.0 */

    /* Chromatic aberration - RGB color fringing */
    --crt-chromatic-aberration: 0.2; /* 0.0 = none, higher = more */

    /* Bloom - glow for bright areas */
    --crt-bloom: 0.0;                /* 0.0-1.0 */

    /* Flicker - subtle brightness variation */
    --crt-flicker: 0.01;             /* 0.0-1.0, keep very low */
}
```

**Recommended starting values for subtle effect:**
- scanline-intensity: 0.05-0.1
- curvature: 0.01-0.02
- vignette: 0.15-0.25
- chromatic-aberration: 0.1-0.3
- flicker: 0.005-0.02

## Tab Bar Styling

```css
:terminal::tab-bar {
    background: #1a1a2e;
    border-color: #44475a;
    height: 36px;
    padding: 4px;
}

:tab {
    background: #282a36;
    color: #6272a4;
    border-radius: 4px;
    min-width: 80px;
    max-width: 200px;
}

:tab.active {
    background: #44475a;
    color: #f8f8f2;
    accent-color: #bd93f9;
    text-shadow: 0 0 8px rgba(189, 147, 249, 0.6);
}

:terminal::tab-close {
    color: #6272a4;
    --hover-background: #ff5555;
    --hover-color: #f8f8f2;
}
```

## Complete Example: Retro CRT Theme

```css
/* Retro amber CRT terminal */
:terminal {
    font-family: "MesloLGS NF", monospace;
    font-size: 14;
    line-height: 1.5;

    color: #ffb000;
    background: linear-gradient(to bottom, #0f0a00, #050300);
    background-image: url("images/scanlines.jpg");
    background-size: cover;
    --background-opacity: 0.7;

    text-shadow: 0 0 8px rgba(255, 176, 0, 0.6);
}

:terminal::cursor {
    background: #ffb000;
    text-shadow: 0 0 12px rgba(255, 176, 0, 0.8);
}

:terminal::selection {
    background: #ffb000;
    color: #0a0800;
}

:terminal::backdrop {
    --crt-enabled: true;
    --crt-scanline-intensity: 0.08;
    --crt-curvature: 0.015;
    --crt-vignette: 0.2;
    --crt-chromatic-aberration: 0.2;
    --crt-flicker: 0.01;
}

:terminal::palette {
    --color-0: #0a0800;
    --color-1: #ff3300;
    --color-2: #ffb000;
    --color-7: #ffb000;
}
```

## Tips

1. **Hot Reload**: Save your theme file and changes apply immediately
2. **Colors**: Use hex (#rrggbb), rgb(), or rgba() for transparency
3. **Paths**: Image paths are relative to the theme file location
4. **Performance**: Multiple backdrop effects can impact performance
5. **CRT Effect**: Works best with amber/green monochrome color schemes
