# Theme CSS Properties Reference

Complete reference of all CSS properties supported by CRT Terminal themes.

## Selectors

| Selector | Description |
|----------|-------------|
| `:terminal` | Main terminal styling (typography, colors, background) |
| `:terminal::selection` | Text selection appearance |
| `:terminal::highlight` | Search match highlighting |
| `:terminal::backdrop` | Background effects (grid, particles, CRT, etc.) |
| `:terminal::palette` | ANSI color palette (colors 0-255) |
| `:terminal::tab-bar` | Tab bar container |
| `:tab` | Individual tab styling |
| `:tab.active` | Active tab styling |
| `:terminal::tab-close` | Tab close button |

---

## :terminal Properties

### Typography

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| `font-family` | string list | system default | Comma-separated font names |
| `font-size` | number | 14 | Font size in points |
| `line-height` | number | 1.3 | Line height multiplier |
| `--font-bold` | string | auto | Bold font family name |
| `--font-italic` | string | auto | Italic font family name |
| `--font-bold-italic` | string | auto | Bold italic font family name |

### Colors

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| `color` | color | #c8c8c8 | Foreground text color |
| `background` | color/gradient | #1a1a1a | Background color or gradient |
| `cursor-color` | color | #ffffff | Cursor color |
| `text-shadow` | shadow | none | Text glow effect |

### Background Image

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| `background-image` | url | none | Path to background image |
| `background-size` | keyword/value | cover | `cover`, `contain`, `auto`, px, or % |
| `background-position` | keyword | center | `center`, `top`, `bottom`, `left`, `right`, or combinations |
| `background-repeat` | keyword | no-repeat | `no-repeat`, `repeat`, `repeat-x`, `repeat-y` |
| `--background-opacity` | number | 1.0 | Image opacity (0.0-1.0) |

---

## :terminal::selection Properties

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| `background` | color | theme-based | Selection background color |
| `color` | color | theme-based | Selection text color |

---

## :terminal::highlight Properties

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| `background` | color | yellow | Search match background |
| `color` | color | black | Search match text color |
| `--current-background` | color | orange | Current match background |

---

## :terminal::palette Properties

ANSI colors using `--color-N` where N is 0-255.

### Base 16 Colors (0-15)

| Property | Color Name |
|----------|------------|
| `--color-0` | Black |
| `--color-1` | Red |
| `--color-2` | Green |
| `--color-3` | Yellow |
| `--color-4` | Blue |
| `--color-5` | Magenta |
| `--color-6` | Cyan |
| `--color-7` | White |
| `--color-8` | Bright Black |
| `--color-9` | Bright Red |
| `--color-10` | Bright Green |
| `--color-11` | Bright Yellow |
| `--color-12` | Bright Blue |
| `--color-13` | Bright Magenta |
| `--color-14` | Bright Cyan |
| `--color-15` | Bright White |

### Alternative Named Syntax

Can also use `--ansi-{name}` in `:terminal`:

```css
:terminal {
    --ansi-red: #ff5555;
    --ansi-bright-red: #ff6e6e;
}
```

---

## :terminal::backdrop Properties

### Grid Effect

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| `--grid-enabled` | bool | false | Enable grid effect |
| `--grid-color` | color | magenta | Grid line color |
| `--grid-spacing` | number | 8.0 | Space between grid lines |
| `--grid-line-width` | number | 0.02 | Grid line thickness |
| `--grid-perspective` | number | 2.0 | Perspective depth |
| `--grid-horizon` | number | 0.35 | Horizon line position (0-1) |
| `--grid-animation-speed` | number | 0.5 | Scroll animation speed |
| `--grid-glow-radius` | number | 0.0 | Glow blur radius |
| `--grid-glow-intensity` | number | 0.0 | Glow brightness |
| `--grid-curved` | bool | false | Enable curved grid lines |
| `--grid-vanishing-spread` | number | 0.3 | Vanishing point spread |

### Starfield Effect

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| `--starfield-enabled` | bool | false | Enable starfield |
| `--starfield-color` | color | white | Star color |
| `--starfield-density` | number | 100 | Number of stars |
| `--starfield-layers` | number | 3 | Parallax layers |
| `--starfield-speed` | number | 0.3 | Movement speed |
| `--starfield-direction` | keyword | down | `up`, `down`, `left`, `right` |
| `--starfield-twinkle` | bool | false | Enable twinkling |
| `--starfield-twinkle-speed` | number | 2.0 | Twinkle frequency |
| `--starfield-min-size` | number | 1.0 | Minimum star size |
| `--starfield-max-size` | number | 3.0 | Maximum star size |
| `--starfield-glow-radius` | number | 0.0 | Star glow radius |
| `--starfield-glow-intensity` | number | 0.0 | Star glow brightness |

### Rain Effect

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| `--rain-enabled` | bool | false | Enable rain |
| `--rain-color` | color | blue | Raindrop color |
| `--rain-density` | number | 150 | Number of drops |
| `--rain-speed` | number | 1.0 | Fall speed |
| `--rain-angle` | number | 0.0 | Rain angle (degrees) |
| `--rain-length` | number | 20.0 | Raindrop length |
| `--rain-thickness` | number | 1.5 | Raindrop width |
| `--rain-glow-radius` | number | 0.0 | Drop glow radius |
| `--rain-glow-intensity` | number | 0.0 | Drop glow brightness |

### Particle Effect

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| `--particles-enabled` | bool | false | Enable particles |
| `--particles-color` | color | white | Particle color |
| `--particles-count` | number | 50 | Number of particles |
| `--particles-shape` | keyword | circle | `circle`, `square`, `triangle` |
| `--particles-behavior` | keyword | float | `float`, `fall`, `rise`, `swirl` |
| `--particles-size` | number | 4.0 | Particle size |
| `--particles-speed` | number | 0.5 | Movement speed |
| `--particles-glow-radius` | number | 0.0 | Glow radius |
| `--particles-glow-intensity` | number | 0.0 | Glow brightness |

### Matrix Effect

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| `--matrix-enabled` | bool | false | Enable matrix rain |
| `--matrix-color` | color | green | Character color |
| `--matrix-density` | number | 1.0 | Column density |
| `--matrix-speed` | number | 8.0 | Fall speed |
| `--matrix-font-size` | number | 14.0 | Character size |
| `--matrix-charset` | string | A-Z0-9 | Characters to display |

### Shape Effect

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| `--shape-enabled` | bool | false | Enable shape |
| `--shape-type` | keyword | circle | `circle`, `square`, `triangle`, `hexagon`, `star`, `polygon` |
| `--shape-size` | number | 100.0 | Shape size |
| `--shape-fill` | color | none | Fill color |
| `--shape-stroke` | color | none | Stroke color |
| `--shape-stroke-width` | number | 2.0 | Stroke thickness |
| `--shape-glow-radius` | number | 0.0 | Glow radius |
| `--shape-glow-color` | color | stroke | Glow color |
| `--shape-rotation` | keyword | none | `none`, `clockwise`, `counterclockwise` |
| `--shape-rotation-speed` | number | 1.0 | Rotation speed |
| `--shape-motion` | keyword | none | `none`, `float`, `bounce`, `orbit` |
| `--shape-motion-speed` | number | 1.0 | Motion speed |
| `--shape-polygon-sides` | number | 6 | Sides for polygon type |

### Sprite Animation

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| `--sprite-enabled` | bool | false | Enable sprite |
| `--sprite-path` | string | none | Path to sprite sheet |
| `--sprite-frame-width` | number | 64 | Frame width in pixels |
| `--sprite-frame-height` | number | 64 | Frame height in pixels |
| `--sprite-columns` | number | 1 | Columns in sprite sheet |
| `--sprite-rows` | number | 1 | Rows in sprite sheet |
| `--sprite-frame-count` | number | auto | Total frames |
| `--sprite-fps` | number | 12.0 | Animation speed |
| `--sprite-scale` | number | 1.0 | Display scale |
| `--sprite-opacity` | number | 1.0 | Sprite opacity |
| `--sprite-position` | keyword | center | Position on screen |
| `--sprite-motion` | keyword | none | `none`, `float`, `bounce`, `pace` |
| `--sprite-motion-speed` | number | 1.0 | Motion speed |

### CRT Post-Processing

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| `--crt-enabled` | bool | false | Enable CRT effect |
| `--crt-scanline-intensity` | number | 0.15 | Scanline darkness (0.0-1.0) |
| `--crt-scanline-frequency` | number | 2.0 | Scanline density |
| `--crt-curvature` | number | 0.02 | Screen curvature (0.0-0.1) |
| `--crt-vignette` | number | 0.3 | Edge darkening (0.0-1.0) |
| `--crt-chromatic-aberration` | number | 0.0 | RGB color separation |
| `--crt-bloom` | number | 0.0 | Bright area glow |
| `--crt-flicker` | number | 0.0 | Brightness variation |

---

## Tab Bar Properties

### :terminal::tab-bar

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| `background` | color | theme-based | Bar background |
| `border-color` | color | theme-based | Bottom border |
| `height` | number | 36 | Bar height in pixels |
| `padding` | number | 4 | Internal padding |

### :tab

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| `background` | color | theme-based | Tab background |
| `color` | color | theme-based | Tab text color |
| `border-radius` | number | 4 | Corner radius |
| `min-width` | number | 80 | Minimum tab width |
| `max-width` | number | 200 | Maximum tab width |
| `text-shadow` | shadow | none | Text glow |

### :tab.active

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| `background` | color | theme-based | Active tab background |
| `color` | color | theme-based | Active tab text |
| `accent-color` | color | theme-based | Accent indicator |
| `text-shadow` | shadow | none | Text glow |

### :terminal::tab-close

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| `color` | color | theme-based | Close button color |
| `--hover-background` | color | red | Hover background |
| `--hover-color` | color | white | Hover text color |

---

## Color Formats

Supported color formats:

- Hex: `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`
- RGB: `rgb(255, 128, 0)`
- RGBA: `rgba(255, 128, 0, 0.5)`
- Named: `red`, `blue`, `transparent`, etc.

## Gradient Format

```css
background: linear-gradient(to bottom, #color1, #color2);
```
