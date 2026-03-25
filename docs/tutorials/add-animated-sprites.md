# Add Animated Sprites to Your Theme

CRT Terminal can display animated sprite characters in the backdrop of your terminal — running mascots, idle animations, looping pixel art. This tutorial shows you how to take any animated GIF and turn it into a working sprite, configure it in a theme CSS file, and then wire up reactive events so the sprite responds to what happens in your shell (command success, failure, the terminal bell).

By the end of this tutorial you will have:

- A sprite sheet PNG created from a GIF of your choice
- A theme that displays the sprite in the corner of your terminal
- Reactive behavior that changes the sprite when commands succeed or fail

---

## Prerequisites

- CRT Terminal installed and running. See [Getting Started with CRT Terminal](./getting-started.md) if you haven't done that yet.
- An animated GIF you want to use. For this tutorial we will use a running cycle animation called `runcycle.gif`. Any animated GIF will work.
- **ffmpeg** — for extracting frames from the GIF
- **ImageMagick** — for background removal and sprite sheet assembly

Install the tools on macOS:

```bash
brew install ffmpeg imagemagick
```

Install on Ubuntu/Debian:

```bash
sudo apt-get install ffmpeg imagemagick
```

Install on Fedora/RHEL:

```bash
sudo dnf install ffmpeg ImageMagick
```

Verify both tools are working:

```bash
ffmpeg -version | head -1
convert --version | head -1
```

---

## Step 1: Inspect Your GIF

Before extracting frames, gather information about your source GIF. Run:

```bash
identify -verbose runcycle.gif | grep -E "Geometry|Delay|Image:"
```

Example output:

```
Image: runcycle.gif[0]
  Geometry: 128x72+0+0
Image: runcycle.gif[1]
  Geometry: 128x72+0+0
...
  Delay: 10x100
```

Take note of:

- **Geometry** — The frame dimensions. In this example each frame is 128×72 pixels.
- **Delay** — The frame timing in `centiseconds/100`. A delay of `10x100` means 10/100 of a second per frame, which is 10 fps.
- **Number of frames** — Count the `Image:` lines, or simply count files after extraction.

If `identify` gives you a large block of output, count the frames quickly:

```bash
identify runcycle.gif | wc -l
```

Each line corresponds to one frame.

---

## Step 2: Create a Working Directory

Keep the intermediate files tidy:

```bash
mkdir -p ~/sprite-work/runcycle
cd ~/sprite-work/runcycle
cp /path/to/runcycle.gif .
```

Replace `/path/to/runcycle.gif` with the actual path to your GIF file.

---

## Step 3: Extract Frames

Use ffmpeg to split the GIF into individual PNG frames:

```bash
ffmpeg -i runcycle.gif -vsync 0 frame_%04d.png
```

This creates files named `frame_0001.png`, `frame_0002.png`, and so on.

Check how many frames were extracted:

```bash
ls frame_*.png | wc -l
```

For a typical run cycle animation you might have 6–12 frames. In this tutorial we assume 8 frames, which we will arrange in a single row (8 columns, 1 row).

**If your GIF has a large number of frames** (30+), consider extracting every other frame to keep the sprite sheet small:

```bash
ffmpeg -i runcycle.gif -vf "select=not(mod(n\,2))" -vsync 0 frame_%04d.png
```

This selects every second frame, halving the frame count while maintaining smooth enough motion.

---

## Step 4: Remove the Background

Most GIFs have a solid background color that needs to be made transparent before the sprite can be placed over the terminal.

**Identify the background color.** Look at the first frame:

```bash
identify -verbose frame_0001.png | grep -i "background\|colors"
```

Or open `frame_0001.png` in any image viewer and sample a corner pixel.

**Common background colors and their removal commands:**

White background:

```bash
for f in frame_*.png; do
    convert "$f" -fuzz 10% -transparent white "${f%.png}_t.png"
done
```

Black background:

```bash
for f in frame_*.png; do
    convert "$f" -fuzz 10% -transparent black "${f%.png}_t.png"
done
```

Specific hex color (e.g., `#f0f0f0`):

```bash
for f in frame_*.png; do
    convert "$f" -fuzz 10% -transparent "#f0f0f0" "${f%.png}_t.png"
done
```

The `-fuzz 10%` flag allows for slight color variations — JPEG-style artifacts, anti-aliasing, or slight hue shifts near the edges of the character. Increase it if there are remaining background pixels; decrease it if you are accidentally removing parts of the character.

Verify the result by checking one frame:

```bash
identify -verbose frame_0001_t.png | grep "Alpha"
```

You should see `Alpha: 8-bit` indicating transparency data is present.

**Optional: preview a frame.** On macOS:

```bash
open frame_0001_t.png
```

The checkerboard pattern in image viewers indicates transparency.

---

## Step 5: Decide on Sprite Sheet Layout

CRT Terminal reads sprite sheets as a grid of equally-sized frames. You need to decide how to arrange your frames.

For a small number of frames, a single horizontal row is simplest:

```
[frame1][frame2][frame3][frame4][frame5][frame6][frame7][frame8]
```

This layout uses `--sprite-columns: 8` and `--sprite-rows: 1`.

For larger frame counts, a grid keeps the image dimensions manageable:

| Frame count | Recommended layout | CSS values |
|-------------|-------------------|------------|
| 4–10 | Single row | `columns: N, rows: 1` |
| 12 | 4×3 grid | `columns: 4, rows: 3` |
| 16 | 4×4 grid | `columns: 4, rows: 4` |
| 24 | 6×4 grid | `columns: 6, rows: 4` |
| 25 | 5×5 grid | `columns: 5, rows: 5` |

For this tutorial we have 8 frames, so we will use a single row.

---

## Step 6: Build the Sprite Sheet

Assemble all the transparent frames into one image using ImageMagick's `montage` command.

**Single horizontal row (8 frames, 1 row):**

```bash
montage frame_*_t.png -tile 8x1 -geometry +0+0 -background none sprite-sheet.png
```

**Grid layout example (12 frames, 4×3):**

```bash
montage frame_*_t.png -tile 4x3 -geometry +0+0 -background none sprite-sheet.png
```

Parameters:
- `-tile 8x1` — Columns × rows. Use `x1` for a single row, `1x` for a single column.
- `-geometry +0+0` — Zero padding between frames. Frames must be packed tightly.
- `-background none` — Preserve transparency.

Check that the output dimensions are correct:

```bash
identify sprite-sheet.png
```

Expected for 8 frames of 128×72 pixels in a single row:

```
sprite-sheet.png PNG 1024x72 ...
```

The width should be `frame_width × columns` and height `frame_height × rows`.

**If the dimensions look wrong**, the frames may not all be the same size. Check:

```bash
identify frame_*.png | awk '{print $3}' | sort | uniq
```

All frames must be the same dimensions. If they differ, normalize them:

```bash
for f in frame_*_t.png; do
    convert "$f" -gravity center -extent 128x72 "${f%.png}_norm.png"
done
montage frame_*_norm.png -tile 8x1 -geometry +0+0 -background none sprite-sheet.png
```

Replace `128x72` with your actual frame dimensions.

---

## Step 7: Install the Sprite Sheet

CRT themes look for image files relative to the themes directory at `~/.config/crt/themes/`. Create a subdirectory for your sprite assets:

```bash
mkdir -p ~/.config/crt/themes/images
cp sprite-sheet.png ~/.config/crt/themes/images/runcycle.png
```

The path `"images/runcycle.png"` in your CSS will resolve to this location.

---

## Step 8: Create the Theme and Configure the Sprite

Create a new theme file:

```bash
touch ~/.config/crt/themes/myscout.css
```

Set it as the active theme in `~/.config/crt/config.toml`:

```toml
[theme]
name = "myscout"
```

Open `~/.config/crt/themes/myscout.css` and add a base terminal style plus the sprite configuration:

```css
:terminal {
    font-family: "JetBrains Mono", "Fira Code", "Menlo", monospace;
    font-size: 14;
    line-height: 1.5;
    color: #e0e0e0;
    background: #1a1a2e;
}

:terminal::cursor {
    background: #e94560;
}

:terminal::selection {
    background: #16213e;
    color: #e0e0e0;
}

:terminal::backdrop {
    --sprite-enabled: true;
    --sprite-path: "images/runcycle.png";
    --sprite-frame-width: 128;
    --sprite-frame-height: 72;
    --sprite-columns: 8;
    --sprite-rows: 1;
    --sprite-frame-count: 8;
    --sprite-fps: 10;
    --sprite-scale: 2.0;
    --sprite-opacity: 0.9;
    --sprite-position: bottom-right;
    --sprite-motion: none;
}
```

Save the file. The sprite should appear in the bottom-right corner of your terminal, animating at 10 fps.

**Adjust the values to match your sprite:**

- `--sprite-frame-width` and `--sprite-frame-height` — Must match the pixel dimensions of a single frame in your sprite sheet.
- `--sprite-columns` and `--sprite-rows` — Must match the grid layout you used in Step 6.
- `--sprite-frame-count` — Total number of animation frames.
- `--sprite-fps` — Frames per second. Match the original GIF speed (from Step 1).
- `--sprite-scale` — Display size multiplier. `1.0` is native pixel size; `2.0` doubles it.
- `--sprite-opacity` — 0.0 (invisible) to 1.0 (fully opaque).
- `--sprite-position` — Where on screen to anchor the sprite.

**Available positions:**

`center`, `top-left`, `top-center`, `top-right`, `center-left`, `center-right`, `bottom-left`, `bottom-center`, `bottom-right`

**Available motions:**

| Value | Behavior |
|-------|---------|
| `none` | Sprite stays at its position |
| `bounce` | Bounces around the screen |
| `scroll` | Scrolls across the screen in one direction |
| `float` | Gentle floating up/down oscillation |
| `orbit` | Orbits around the center of the screen |

---

## Step 9: Enable Shell Integration for Reactive Events

Reactive events let the sprite (or any other CSS property) change in response to shell activity. The events are:

| Event selector | Triggers when |
|---------------|--------------|
| `:terminal::on-command-success` | A command exits with code 0 |
| `:terminal::on-command-fail` | A command exits with a non-zero code |
| `:terminal::on-bell` | The terminal bell fires (`\a` or `Ctrl+G`) |
| `:terminal::on-focus` | The window gains focus |
| `:terminal::on-blur` | The window loses focus |

For `on-command-success` and `on-command-fail` to work, your shell must emit **OSC 133 semantic prompt markers**. This is a standard protocol supported by most modern shells.

**Enable semantic prompts in config.toml:**

```toml
[shell]
semantic_prompts = true
```

Save. CRT will now pass the `PROMPT_COMMAND` mechanism (or equivalent) to your shell to inject OSC 133 markers.

**Verify it is working:** Run a command that succeeds (`ls`) and one that fails (`ls /nonexistent`). If reactive events are wired up correctly, you will see the effects you configure in the next step.

> **Note:** If you use a custom prompt framework like Starship, Oh My Zsh, or Powerlevel10k, check their documentation for OSC 133 support. Many frameworks emit these markers automatically or have a configuration option to enable them.

---

## Step 10: Add Reactive Event Styles

Now wire the sprite to react to shell events. Append these blocks to `myscout.css`:

```css
/* Command succeeded — sprite bounces briefly in celebration */
:terminal::on-command-success {
    --duration: 1200ms;
    --sprite-fps: 18;
    --sprite-scale: 2.5;
    --sprite-opacity: 1.0;
    --sprite-motion: bounce;
}

/* Command failed — sprite slows to a stop, fades slightly */
:terminal::on-command-fail {
    --duration: 2500ms;
    --sprite-fps: 4;
    --sprite-scale: 2.0;
    --sprite-opacity: 0.5;
    --sprite-motion: none;
    text-shadow: 0 0 16px rgba(255, 80, 80, 0.5);
}

/* Bell — sprite animates faster for a brief moment */
:terminal::on-bell {
    --duration: 600ms;
    --sprite-fps: 24;
    --sprite-scale: 3.0;
    --sprite-opacity: 1.0;
}
```

Save. Run a few commands and watch the sprite:

- A successful `ls` — the sprite should speed up and bounce for about 1.2 seconds.
- A failed `ls /doesnotexist` — the sprite slows down and the screen gets a faint red glow for 2.5 seconds.
- Trigger the bell (`echo -e "\a"`) — the sprite briefly enlarges.

**How `--duration` works:**

The `--duration` property (in milliseconds) controls how long the event override stays active before the sprite returns to its default state defined in `::backdrop`. After the duration expires, CRT smoothly transitions back to the base values.

**You can override any CSS property in an event block**, not just sprite properties. For example, adding `text-shadow` in `::on-command-fail` applies a red glow to all terminal text while the event is active.

---

## Step 11: Add a Second Sprite for Failure State (Optional)

If you have a second sprite sheet representing a different animation (e.g., a sad or confused idle animation), you can swap sprite sheets entirely on failure:

```css
:terminal::on-command-fail {
    --duration: 3000ms;
    --sprite-path: "images/runcycle-sad.png";
    --sprite-frame-width: 128;
    --sprite-frame-height: 72;
    --sprite-columns: 6;
    --sprite-rows: 1;
    --sprite-frame-count: 6;
    --sprite-fps: 6;
    --sprite-scale: 2.0;
    --sprite-opacity: 0.8;
    --sprite-motion: none;
    text-shadow: 0 0 16px rgba(255, 80, 80, 0.5);
}
```

This completely replaces the sprite with a different animation for the duration of the failure event, then switches back automatically. You would need to create `runcycle-sad.png` using the same frame extraction and montage process from Steps 3–6.

---

## Complete Theme File

Here is the complete `myscout.css` after all steps:

```css
:terminal {
    font-family: "JetBrains Mono", "Fira Code", "Menlo", monospace;
    font-size: 14;
    line-height: 1.5;
    color: #e0e0e0;
    background: #1a1a2e;
}

:terminal::cursor {
    background: #e94560;
}

:terminal::selection {
    background: #16213e;
    color: #e0e0e0;
}

:terminal::backdrop {
    --sprite-enabled: true;
    --sprite-path: "images/runcycle.png";
    --sprite-frame-width: 128;
    --sprite-frame-height: 72;
    --sprite-columns: 8;
    --sprite-rows: 1;
    --sprite-frame-count: 8;
    --sprite-fps: 10;
    --sprite-scale: 2.0;
    --sprite-opacity: 0.9;
    --sprite-position: bottom-right;
    --sprite-motion: none;
}

:terminal::on-command-success {
    --duration: 1200ms;
    --sprite-fps: 18;
    --sprite-scale: 2.5;
    --sprite-opacity: 1.0;
    --sprite-motion: bounce;
}

:terminal::on-command-fail {
    --duration: 2500ms;
    --sprite-fps: 4;
    --sprite-scale: 2.0;
    --sprite-opacity: 0.5;
    --sprite-motion: none;
    text-shadow: 0 0 16px rgba(255, 80, 80, 0.5);
}

:terminal::on-bell {
    --duration: 600ms;
    --sprite-fps: 24;
    --sprite-scale: 3.0;
    --sprite-opacity: 1.0;
}
```

---

## Troubleshooting

**Sprite is not visible:**
- Confirm `--sprite-enabled: true` is set.
- Check that the path in `--sprite-path` is correct. Paths are relative to `~/.config/crt/themes/`. Use `"images/runcycle.png"` if the file is at `~/.config/crt/themes/images/runcycle.png`.
- Verify the file exists: `ls ~/.config/crt/themes/images/runcycle.png`

**Sprite animation looks wrong (frames jumping or out of order):**
- Verify `--sprite-frame-width` and `--sprite-frame-height` match the actual pixel dimensions of one frame in the sheet.
- Verify `--sprite-columns` and `--sprite-rows` match the grid you built with `montage`.
- Check the sprite sheet dimensions: `identify ~/.config/crt/themes/images/runcycle.png`

**Background not fully transparent:**
- Increase the `-fuzz` percentage: `-fuzz 20%` or `-fuzz 25%`
- Use a more precise color value. Sample the exact hex from a corner pixel: `convert frame_0001.png -format "%[pixel:u.p{0,0}]" info:`
- For GIFs that use palette blending near edges, try alpha trimming instead: `convert frame.png -alpha set -fuzz 15% -transparent "#fffffe" output.png`

**Reactive events not firing:**
- Confirm `semantic_prompts = true` is set in the `[shell]` section of `config.toml`.
- Restart CRT after changing `config.toml` for shell integration changes to take effect.
- Test with `echo -e "\a"` for the bell event (which does not require shell integration).

**Animation speed is wrong:**
- Check the original GIF delay: `identify -verbose runcycle.gif | grep Delay`
- Delay is in `centiseconds/100`. Delay `10x100` = 10/100 seconds = 0.1 seconds per frame = 10 fps.
- Set `--sprite-fps` to match.

**Sprite sheet is too large (slow load or high memory):**
- Resize frames before building the sheet: `convert frame.png -resize 64x36 frame_small.png`
- Use fewer frames by extracting every second frame in ffmpeg (see Step 3).

---

## Next Steps

- **Build a complete theme around your sprite** — [Build Your First Theme](./build-your-first-theme.md)
- **Explore all sprite properties** — [Theme CSS Properties Reference](../reference/theme-css-properties.md)
- **Look at the built-in pokemon and robco-reactive themes** for real working sprite examples — they are at `assets/themes/pokemon.css` and `assets/themes/robco-reactive.css` in the CRT source repository
- **How-to guide for sprite sheets** — [How to Create a Sprite Sheet from a GIF](../how-to/create-sprite-from-gif.md)
