# Theming System

CRT themes are written in a CSS-like syntax. A theme is a `.css` file that lives in `~/.config/crt/themes/`. Loading a theme changes the gradient background, text glow color, ANSI palette, backdrop effects, the CRT post-processing parameters, and more — all by editing text in a plain file. This document explains how that works end-to-end, from parsing to pixels.

## Why CSS Syntax?

The choice to use CSS syntax rather than TOML, YAML, or a custom format is primarily about familiarity. Terminal themes are authored by a technical audience who already knows CSS. The property-value model maps naturally to configuration data, pseudo-selectors provide a clean namespace for different terminal regions, and custom properties (the `--property: value` syntax) allow extension without polluting the CSS keyword namespace.

CSS also brings useful semantics for free: the cascade (later declarations win), the ability to group related properties under the same selector, and a human-readable color format shared with every web tool.

### Limitations Compared to Real CSS

CRT does not implement a full CSS engine. The syntax is CSS but the semantics are deliberately limited:

- **Selectors**: Only CRT-specific pseudo-selectors are recognized (`:terminal`, `::selection`, `::cursor`, `::backdrop`, etc.). No element selectors, class selectors, or combinators.
- **Gradients**: Only `linear-gradient(to bottom, ...)` is supported. Gradients must be top-to-bottom.
- **Cascade**: Later declarations win within the same file. There is no import system, inheritance, or specificity calculation.
- **calc()**: Not supported. Numeric values must be literals.
- **@media queries**: Not supported.
- **Variables**: CSS custom properties (`--x: y`) are used for effect configuration, but they are not CSS variables in the `var(--x)` sense — they are simply effect configuration keys read by effect parsers.

These limitations are intentional. A full CSS engine would add significant complexity for little benefit. CRT themes configure GPU shader parameters, not DOM layout.

## The Parser

The parser (`crt-theme/src/parser.rs`) uses **lightningcss** to tokenize and structure the CSS input. lightningcss handles the lexical work: it finds rule blocks, identifies property names and values, resolves CSS color syntax (hex, `rgb()`, `rgba()`, named colors, `hsl()`), and gives the parser a structured AST to walk.

The parser's job is to walk that AST and populate a `Theme` struct. For each CSS rule it finds, it looks at the selector to determine which part of the `Theme` to update, then reads each property to set the appropriate field.

The `Theme::from_css()` and `Theme::from_css_with_base()` functions are the public entry points. The `with_base` variant takes a directory path that becomes the `base_dir` for resolving relative paths in the theme file — image paths and sprite sheet paths are resolved relative to the directory containing the theme file.

### Color Parsing

Colors are parsed through `parse_color_string()`, which handles:

- Hex: `#rrggbb` and `#rrggbbaa`
- CSS functions: `rgb(r, g, b)` and `rgba(r, g, b, a)` with both 0–255 and 0.0–1.0 ranges
- Named colors: A hand-curated list of ~100 CSS named colors

All colors are stored internally as `Color { r, g, b, a: f32 }` in the 0.0–1.0 range. This matches the format expected by wgpu shader uniforms and requires no conversion at render time.

The parser intentionally falls back gracefully: an unrecognized color value produces a warning rather than a parse error. This means a theme with one bad color value still loads with all other properties intact.

### Custom Properties for Effects

Backdrop effect configuration is passed through CSS custom properties on the `::backdrop` selector:

```css
:terminal::backdrop {
    --grid-enabled: true;
    --grid-color: rgba(255, 0, 255, 0.3);
    --grid-spacing: 8;
    --starfield-enabled: false;
}
```

These properties are collected into a `HashMap<String, String>` during parsing — the raw property name (without `--`) maps to its raw string value. This map is stored in the `Theme` struct's relevant effect fields and later handed to each effect's `configure()` method.

Effect configuration is intentionally kept as raw strings until the effect itself processes them. This avoids the parser needing to know the semantics of every possible effect property, and allows new effect properties to be added without changing the parser.

## The Theme Struct

`Theme` is a large plain-data struct. Each field corresponds to an aspect of the visual presentation:

```
Theme
├── typography (font family, size, line height, variants)
├── gradient (LinearGradient: top and bottom colors)
├── text_shadow (TextShadow: color, radius, intensity)
├── palette (AnsiPalette: 16 base colors + extended 256-color overrides)
├── selection (SelectionStyle)
├── cursor (CursorStyle: shape, color)
├── background_image (BackgroundImage: path, size, position, repeat, opacity)
├── grid (Option<GridEffect>)
├── starfield (Option<StarfieldEffect>)
├── rain (Option<RainEffect>)
├── particles (Option<ParticleEffect>)
├── matrix (Option<MatrixEffect>)
├── shape (Option<ShapeEffect>)
├── sprite (Option<SpriteEffect>)
├── crt (Option<CrtEffect>)
├── tabs (TabTheme)
├── ui (UiStyle: context menu, search bar, focus indicators)
├── on_bell (Option<EventOverride>)
├── on_command_success (Option<EventOverride>)
├── on_command_fail (Option<EventOverride>)
├── on_focus (Option<EventOverride>)
└── on_blur (Option<EventOverride>)
```

The effect fields are `Option`-wrapped because a theme may not configure every effect. A `None` value means the effect is not configured by this theme; the renderer treats `None` the same as `enabled: false`. This distinguishes "not mentioned in the theme" from "explicitly disabled".

### ThemeUniforms

The renderer does not read `Theme` directly during the per-frame render loop. Instead, `Theme::to_uniforms()` converts the theme into `ThemeUniforms`, a `repr(C)` struct that can be uploaded to the GPU as a uniform buffer:

```rust
ThemeUniforms {
    gradient_top: [f32; 4],
    gradient_bottom: [f32; 4],
    glow_color: [f32; 4],
    glow_radius: f32,
    glow_intensity: f32,
    screen_width: f32,
    screen_height: f32,
    time: f32,
    // ... padding for 16-byte alignment
}
```

`to_uniforms()` takes current `width`, `height`, and `time` as arguments because these are frame-varying values that must be embedded in the same buffer as the theme-static values. The result is a single uniform buffer upload per frame that drives both the background gradient shader and the composite glow shader.

## The CSS Cascade and Mergeable Trait

Within a single theme file, the cascade rule is simple: later property declarations win. This is implemented through the `Mergeable` trait:

```rust
pub trait Mergeable {
    fn merge(&mut self, other: Self);
}
```

Patch structs (internal structs with all-`Option` fields) implement `Mergeable` via a macro that checks each field: if `other.field` is `Some`, it overwrites `self.field`. This lets the parser process declarations in order, accumulating a final merged result where the last value for any property wins.

A practical consequence: themes can use multiple CSS rules for the same selector, with later rules overriding earlier ones. This allows a theme to define a base palette and then override specific colors lower in the file.

The `Mergeable` pattern is also used for event overrides (see below): an override patch is merged onto the base theme at runtime, and when the override expires, the base values are restored.

## Hot Reload

Theme hot reload works through a file watcher. The `ConfigWatcher` uses the `notify` crate to watch the `~/.config/crt/themes/` directory for file-system events. On macOS, this uses FSEvents (via the kernel); on Linux, inotify. The watcher is polled from the `about_to_wait` event loop callback, which runs after all pending input events are processed.

When a theme file change is detected:

1. `App::reload_theme()` calls `ThemeRegistry::reload_all()`
2. The registry re-reads each `.css` file in the themes directory and re-parses it
3. Each open window receives the freshly parsed theme for its current selection
4. Effects are reconfigured from the new theme data
5. Content hashes are zeroed so all tabs re-render on the next frame
6. CRT pipeline, background image, and sprite state are rebuilt if relevant

The reload is synchronous on the main thread and takes a few milliseconds at most. During the reload, the previous frame remains visible; there is no blank frame or flash. If the CSS file contains a parse error, the old theme remains in use and a toast notification appears explaining the error.

This workflow — edit the `.css` file in your editor, see the result immediately — is the intended way to develop themes.

## Theme Registry

`ThemeRegistry` provides runtime theme switching. At startup it scans the themes directory, parses every `.css` file it finds, and caches the parsed `Theme` structs in a `HashMap<String, Theme>` keyed by the filename stem (the part before `.css`).

The registry provides `list_themes()` (sorted alphabetically), `get_theme(name)`, and `get_default_theme()`. The context menu (right-click in the terminal) queries the registry's theme list to populate its "Switch Theme" submenu. The native macOS menu bar has a corresponding submenu built at startup.

Switching themes at runtime calls `apply_theme_to_window()` for the target window, which:
- Updates the window's stored theme and theme name
- Reconfigures backdrop effects from the new theme
- Recreates the sprite animation state if a sprite is configured
- Updates CRT pipeline settings and creates/destroys the CRT intermediate texture as needed
- Loads any new background image
- Zeros content hashes to force a full re-render

## Event Overrides

Event overrides are the mechanism by which the terminal responds visually to shell events. A theme can define what happens when a bell is received, when a command succeeds, or when a command fails:

```css
:terminal::on-bell {
    --cursor-color: #ff5555;
    --glow-intensity: 1.0;
    --duration: 500;
}

:terminal::on-command-fail {
    --cursor-color: #ff0000;
    --starfield-color: #ff4444;
    --duration: 3000;
}
```

These selectors are parsed into `EventOverride` structs, which are `Option` fields on the `Theme`. An `EventOverride` contains the override properties and a `duration_ms`.

At runtime, when a shell event arrives (via the OSC 133 mechanism or a BEL character), the matching `EventOverride` is applied to the `OverrideState`. The override state holds the active overrides with their expiry times. On each frame:

1. `overrides.update()` expires any overrides whose duration has elapsed
2. `compute_effect_patches()` computes which effect configurations need to change based on current overrides
3. The renderer applies patches to the effects renderer

When an override expires, the effect configuration is restored to the base theme values. This two-step model (compute then apply) keeps the override logic as a pure function, making it testable without GPU state.

The `ToEffectConfig` trait converts effect patch structs (which have the same structure as effect config structs but with `Option` fields) into `EffectConfig` key-value maps that the effects system can consume. This is the bridge between the typed theme structs and the stringly-typed effect configuration system.

## Path Resolution

Paths in theme files (for `background-image` and sprite sheet `--sprite-path`) are resolved relative to the theme file's directory. This is stored as `base_dir: Option<PathBuf>` in `BackgroundImage` and `SpriteEffect`.

When `Theme::from_css_with_base()` is called with a base directory, the parser stores that directory in any path-bearing struct it creates. When the path is actually needed (at image load time), `resolved_path()` joins the stored `base_dir` with the relative path.

This allows a theme at `~/.config/crt/themes/synthwave/` to reference `./wallpaper.png` or `./pikachu.png` without encoding absolute paths. The theme directory is self-contained and portable.

Absolute paths bypass resolution entirely — if the path starts with `/`, it is used as-is. This is useful for referencing system images or shared assets outside the theme directory.
