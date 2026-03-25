# Build Your First Theme

CRT Terminal themes are plain CSS files. This tutorial walks you through building a complete theme from scratch, adding one layer at a time so you can see each change take effect as you go. By the end you will have a polished dark theme with a custom color palette, styled cursor and selection, an animated starfield backdrop, and glowing text.

The theme in this tutorial is called **"aurora"** — a deep northern-night aesthetic with teal and violet accents.

---

## Prerequisites

- CRT Terminal installed and running. If you haven't done that yet, see [Getting Started with CRT Terminal](./getting-started.md).
- A text editor open alongside CRT (in a split or second window) so you can see changes live.

---

## Step 1: Create the Theme File and Wire It Up

Create the CSS file:

```bash
mkdir -p ~/.config/crt/themes
touch ~/.config/crt/themes/aurora.css
```

Tell CRT to use it. Open `~/.config/crt/config.toml` in your editor and set:

```toml
[theme]
name = "aurora"
```

Save `config.toml`. CRT will now load `aurora.css` every time you save it. Since the file is currently empty, the terminal may show default system colors — that is fine. Everything you add from here on takes effect immediately on save.

---

## Step 2: Set Base Colors and Typography

The `:terminal` selector controls the fundamental look: text color, background, and font. This is the foundation everything else builds on.

Add the following to `~/.config/crt/themes/aurora.css`:

```css
:terminal {
    font-family: "JetBrains Mono", "Fira Code", "SF Mono", "Menlo", monospace;
    font-size: 14;
    line-height: 1.5;

    /* Off-white text on deep midnight blue */
    color: #cdd6f4;
    background: linear-gradient(to bottom, #0d0f1a, #070910);
}
```

Save the file. You should see a deep near-black background with light blue-tinted text.

**What each property does:**

- `font-family` — Ordered list of fonts; CRT uses the first one installed on your system.
- `font-size` — Size in points.
- `line-height` — Line spacing multiplier. `1.5` gives comfortable breathing room.
- `color` — Foreground text color.
- `background` — A `linear-gradient` here creates a subtle two-tone background from top to bottom. You can also use a solid hex color like `#0d0f1a`.

---

## Step 3: Style the Cursor

The cursor is the most prominent interactive element in any terminal. Give it a color that stands out from the text.

Append to your CSS file:

```css
:terminal::cursor {
    background: #89dceb;
    text-shadow: 0 0 12px rgba(137, 220, 235, 0.7);
}
```

Save. Your cursor should now be a bright teal, with a soft glow around it.

The `text-shadow` on the cursor creates a neon halo effect. The `rgba` value here uses the same teal color at 70% opacity for the glow.

---

## Step 4: Style Text Selection and Search Highlights

Define how selected text looks, and how search matches are highlighted:

```css
:terminal::selection {
    background: #313244;
    color: #cdd6f4;
}

:terminal::highlight {
    background: #89dceb;
    color: #0d0f1a;
}
```

Save. Try selecting some text to verify the selection color. If you open search with `Cmd+F` and type a matching word, you will see the highlight color.

- `::selection` — Used when you click-drag to select text. A dark background with light text ensures readability.
- `::highlight` — Used for search result matches. A bright color on a dark foreground makes results pop.

---

## Step 5: Add the ANSI Color Palette

Programs in the terminal use 16 standard ANSI colors (color-0 through color-15) for syntax highlighting, `ls` output, prompts, and more. Without defining them, CRT uses its built-in defaults, which may clash with your theme.

The first 8 are "normal" colors; the second 8 are "bright" variants:

```css
:terminal::palette {
    /* Normal colors */
    --color-0: #1e1e2e;   /* black - darkest background shade */
    --color-1: #f38ba8;   /* red */
    --color-2: #a6e3a1;   /* green */
    --color-3: #f9e2af;   /* yellow */
    --color-4: #89b4fa;   /* blue */
    --color-5: #cba6f7;   /* magenta */
    --color-6: #89dceb;   /* cyan */
    --color-7: #cdd6f4;   /* white */

    /* Bright variants */
    --color-8: #313244;   /* bright black (used for comments, dim text) */
    --color-9: #f38ba8;   /* bright red */
    --color-10: #a6e3a1;  /* bright green */
    --color-11: #f9e2af;  /* bright yellow */
    --color-12: #89b4fa;  /* bright blue */
    --color-13: #cba6f7;  /* bright magenta */
    --color-14: #89dceb;  /* bright cyan */
    --color-15: #ffffff;  /* bright white */
}
```

Save. Open a directory listing with `ls` or run a colorized command. You should see the palette taking effect in how file names and output are colored.

**Tips for choosing palette colors:**

- Colors 0 and 8 are used for backgrounds and dim text. Keep color-0 close to your background value.
- Colors 1–6 carry semantic meaning (red = error, green = success, etc.) — ensure they are readable against your background.
- Colors 9–14 are typically brighter or more saturated versions of 1–6.

---

## Step 6: Style the Tab Bar

If you use multiple tabs, the tab bar is highly visible. Style it to match your theme.

```css
:terminal::tab-bar {
    background: #0d0f1a;
    border-color: #313244;
    height: 36px;
    padding: 4px;
}

:tab {
    background: #1e1e2e;
    color: #6c7086;
    border-radius: 4px;
    min-width: 80px;
    max-width: 200px;
}

:tab.active {
    background: #313244;
    color: #89dceb;
    accent-color: #89dceb;
    text-shadow: 0 0 8px rgba(137, 220, 235, 0.6);
}

:terminal::tab-close {
    color: #6c7086;
    --hover-background: #f38ba8;
    --hover-color: #1e1e2e;
}
```

Save. Open a second tab with `Cmd+T`. You will see the inactive tab uses a subdued text color, and the active tab uses the bright teal with a glow, making it immediately clear which tab is focused.

**Property guide:**

- `:terminal::tab-bar` controls the bar container — background, separator border, height.
- `:tab` styles all tabs in their resting state.
- `:tab.active` styles the currently selected tab. Use `accent-color` for the active tab indicator color.
- `text-shadow` on `:tab.active` creates the active glow effect.
- `--hover-background` and `--hover-color` on `::tab-close` control what the X button looks like when you hover over it.

---

## Step 7: Add a Starfield Backdrop

The backdrop is drawn behind the terminal content. CRT supports several animated backdrop effects; the starfield is a great starting point — subtle but atmospheric.

Add a `::backdrop` block to your CSS:

```css
:terminal::backdrop {
    --starfield-enabled: true;
    --starfield-color: rgba(255, 255, 255, 0.85);
    --starfield-density: 150;
    --starfield-layers: 3;
    --starfield-speed: 0.25;
    --starfield-direction: up;
    --starfield-glow-radius: 2;
    --starfield-glow-intensity: 0.4;
    --starfield-twinkle: true;
    --starfield-twinkle-speed: 1.5;
    --starfield-min-size: 0.5;
    --starfield-max-size: 1.8;
}
```

Save. Tiny stars will now drift slowly upward behind your text content.

**Tuning the starfield:**

| Property | What it controls | Suggested range |
|----------|-----------------|-----------------|
| `--starfield-density` | Number of stars on screen | 50–300 |
| `--starfield-layers` | Depth layers (parallax) | 1–6 |
| `--starfield-speed` | Movement speed | 0.1 (slow crawl) to 1.0 (fast scroll) |
| `--starfield-direction` | `up`, `down`, `left`, `right` | — |
| `--starfield-twinkle` | Random brightness variation | `true` or `false` |
| `--starfield-min-size` / `--starfield-max-size` | Star size range in pixels | 0.3–3.0 |

Keep `--starfield-speed` low (0.1–0.3) for a "drifting through space" feel. Higher speeds feel more like flying.

**Want a different backdrop effect instead?** The same `::backdrop` block supports `--rain-enabled`, `--particles-enabled`, `--matrix-enabled`, `--grid-enabled`, and `--shape-enabled`. See [Theme CSS Properties Reference](../reference/theme-css-properties.md) for the full list of properties for each effect.

---

## Step 8: Add Text Glow

Text glow gives your theme a neon or bioluminescent look. It is applied via the `text-shadow` property on `:terminal`.

Update the `:terminal` block you created in Step 2 to add `text-shadow`:

```css
:terminal {
    font-family: "JetBrains Mono", "Fira Code", "SF Mono", "Menlo", monospace;
    font-size: 14;
    line-height: 1.5;

    color: #cdd6f4;
    background: linear-gradient(to bottom, #0d0f1a, #070910);

    /* Subtle blue-teal glow */
    text-shadow: 0 0 8px rgba(137, 220, 235, 0.3);
}
```

Save. The text should now have a faint halo that catches the eye without obscuring readability.

**Glow intensity guide:**

The `rgba` alpha value controls how strong the glow is:

- `0.1–0.2` — Almost invisible, adds a hint of warmth
- `0.3–0.4` — Soft, atmospheric (good default)
- `0.5–0.7` — Noticeable neon effect
- `0.8–1.0` — Aggressive glow; use for retro or cyberpunk themes

The blur radius (the third number in `0 0 8px`) controls the spread:

- `4px` — Tight halo
- `8–12px` — Standard soft glow
- `16–24px` — Wide bloom effect

---

## Step 9: Add CRT Post-Processing (Optional)

For a retro CRT monitor aesthetic, enable the post-processing pass. Add these properties inside your `::backdrop` block:

```css
:terminal::backdrop {
    /* ... existing starfield properties ... */

    --crt-enabled: true;
    --crt-scanline-intensity: 0.04;
    --crt-curvature: 0.01;
    --crt-vignette: 0.15;
    --crt-chromatic-aberration: 0.03;
    --crt-flicker: 0.0;
}
```

Save. You will see subtle horizontal scanlines across the screen, slight barrel distortion at the edges, and a darkened vignette around the corners.

These are conservative values — each effect is barely perceptible, but combined they add up to a convincing CRT feel without interfering with readability. Increase `--crt-scanline-intensity` and `--crt-curvature` for a stronger retro look.

To disable CRT effects, set `--crt-enabled: false` or remove the block entirely.

---

## Step 10: Review the Complete Theme

Here is the full `aurora.css` as it stands after all the steps above:

```css
:terminal {
    font-family: "JetBrains Mono", "Fira Code", "SF Mono", "Menlo", monospace;
    font-size: 14;
    line-height: 1.5;

    color: #cdd6f4;
    background: linear-gradient(to bottom, #0d0f1a, #070910);

    text-shadow: 0 0 8px rgba(137, 220, 235, 0.3);
}

:terminal::cursor {
    background: #89dceb;
    text-shadow: 0 0 12px rgba(137, 220, 235, 0.7);
}

:terminal::selection {
    background: #313244;
    color: #cdd6f4;
}

:terminal::highlight {
    background: #89dceb;
    color: #0d0f1a;
}

:terminal::palette {
    --color-0: #1e1e2e;
    --color-1: #f38ba8;
    --color-2: #a6e3a1;
    --color-3: #f9e2af;
    --color-4: #89b4fa;
    --color-5: #cba6f7;
    --color-6: #89dceb;
    --color-7: #cdd6f4;

    --color-8: #313244;
    --color-9: #f38ba8;
    --color-10: #a6e3a1;
    --color-11: #f9e2af;
    --color-12: #89b4fa;
    --color-13: #cba6f7;
    --color-14: #89dceb;
    --color-15: #ffffff;
}

:terminal::tab-bar {
    background: #0d0f1a;
    border-color: #313244;
    height: 36px;
    padding: 4px;
}

:tab {
    background: #1e1e2e;
    color: #6c7086;
    border-radius: 4px;
    min-width: 80px;
    max-width: 200px;
}

:tab.active {
    background: #313244;
    color: #89dceb;
    accent-color: #89dceb;
    text-shadow: 0 0 8px rgba(137, 220, 235, 0.6);
}

:terminal::tab-close {
    color: #6c7086;
    --hover-background: #f38ba8;
    --hover-color: #1e1e2e;
}

:terminal::backdrop {
    --starfield-enabled: true;
    --starfield-color: rgba(255, 255, 255, 0.85);
    --starfield-density: 150;
    --starfield-layers: 3;
    --starfield-speed: 0.25;
    --starfield-direction: up;
    --starfield-glow-radius: 2;
    --starfield-glow-intensity: 0.4;
    --starfield-twinkle: true;
    --starfield-twinkle-speed: 1.5;
    --starfield-min-size: 0.5;
    --starfield-max-size: 1.8;

    --crt-enabled: true;
    --crt-scanline-intensity: 0.04;
    --crt-curvature: 0.01;
    --crt-vignette: 0.15;
    --crt-chromatic-aberration: 0.03;
    --crt-flicker: 0.0;
}
```

---

## Troubleshooting

**Theme not loading after saving config.toml?**
- Check that the `name` value in `[theme]` exactly matches the filename without `.css`.
- Verify the file is saved to `~/.config/crt/themes/aurora.css`.

**Hot reload not working?**
- Make sure CRT is actively watching the file. Confirm you saved the correct file path.
- If you moved or renamed the file, update `config.toml` to match.

**Colors look wrong for specific programs?**
- Some programs use 256-color or truecolor; others only use the base 16. Check that your palette defines all 16 colors (color-0 through color-15).
- If `ls` colors look wrong, your shell's `LS_COLORS` setting may override the palette for file types.

**Glow is too strong?**
- Lower the alpha in `text-shadow`. Start at `0.2` and work up.
- The cursor `text-shadow` and the text `text-shadow` stack — reduce both if the screen feels overlit.

---

## Next Steps

Your aurora theme is complete. Here are directions to go deeper:

- **Add animated sprites** (characters, mascots, effects) — [Add Animated Sprites to Your Theme](./add-animated-sprites.md)
- **Reactive events** (change appearance on command success/fail) — see the [Theme CSS Properties Reference](../reference/theme-css-properties.md), Event Selectors section
- **Background images** — add a `background-image: url("images/bg.jpg")` to `:terminal` and place the file in `~/.config/crt/themes/images/`
- **Full property reference** — [Theme CSS Properties Reference](../reference/theme-css-properties.md)
- **More backdrop effects** — replace or combine `--starfield-enabled` with `--rain-enabled`, `--particles-enabled`, `--matrix-enabled`, or `--grid-enabled`
