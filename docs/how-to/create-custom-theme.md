# How to Create a Custom Theme

This guide shows how to create and apply a custom theme for CRT Terminal.

## Prerequisites

- CRT Terminal installed
- A text editor

## Steps

### 1. Create the theme file

Create a new CSS file in the themes directory:

```bash
touch ~/.config/crt/themes/mytheme.css
```

### 2. Add basic styling

Start with the essential properties:

```css
:terminal {
    /* Colors */
    color: #e0e0e0;
    background: #1a1a1a;

    /* Typography */
    font-family: "JetBrains Mono", monospace;
    font-size: 14;
    line-height: 1.4;
}

:terminal::cursor {
    background: #ffffff;
}

:terminal::selection {
    background: #3d5a80;
    color: #ffffff;
}
```

### 3. Set the ANSI palette

Define the 16 base colors:

```css
:terminal::palette {
    --color-0: #1a1a1a;   /* black */
    --color-1: #ff6b6b;   /* red */
    --color-2: #69db7c;   /* green */
    --color-3: #ffd43b;   /* yellow */
    --color-4: #4dabf7;   /* blue */
    --color-5: #da77f2;   /* magenta */
    --color-6: #38d9a9;   /* cyan */
    --color-7: #e0e0e0;   /* white */
    --color-8: #4a4a4a;   /* bright black */
    --color-9: #ff8787;   /* bright red */
    --color-10: #8ce99a;  /* bright green */
    --color-11: #ffe066;  /* bright yellow */
    --color-12: #74c0fc;  /* bright blue */
    --color-13: #e599f7;  /* bright magenta */
    --color-14: #63e6be;  /* bright cyan */
    --color-15: #ffffff;  /* bright white */
}
```

### 4. Style the tab bar

```css
:terminal::tab-bar {
    background: #141414;
    border-color: #333333;
}

:tab {
    background: #1a1a1a;
    color: #888888;
}

:tab.active {
    background: #2a2a2a;
    color: #ffffff;
}
```

### 5. Apply the theme

Edit your config file:

```bash
nano ~/.config/crt/config.toml
```

Set the theme name:

```toml
[theme]
name = "mytheme"
```

Save and the theme applies immediately (hot reload).

## Adding Effects

### Text Glow

Add a subtle glow to text:

```css
:terminal {
    text-shadow: 0 0 8px rgba(224, 224, 224, 0.3);
}
```

### Background Image

```css
:terminal {
    background-image: url("images/bg.jpg");
    background-size: cover;
    --background-opacity: 0.3;
}
```

Place the image in `~/.config/crt/themes/images/`.

### CRT Effect

For a retro monitor look:

```css
:terminal::backdrop {
    --crt-enabled: true;
    --crt-scanline-intensity: 0.08;
    --crt-curvature: 0.015;
    --crt-vignette: 0.2;
}
```

## Troubleshooting

**Theme not loading?**
- Check the file is in `~/.config/crt/themes/`
- Verify the filename matches the config (without `.css`)
- Check for CSS syntax errors in the terminal log

**Colors look wrong?**
- Ensure hex colors have 6 digits: `#rrggbb`
- Check you're using the correct property names

**Effects not showing?**
- Effects need `--{effect}-enabled: true`
- Some effects require additional properties

## Next Steps

- See the [CSS Properties Reference](../reference/theme-css-properties.md) for all available options
- Browse the built-in themes in `assets/themes/` for examples
