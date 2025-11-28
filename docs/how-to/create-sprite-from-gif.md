# How to Create a Sprite Sheet from a GIF

This guide shows how to convert an animated GIF into a sprite sheet for use with CRT Terminal themes.

## Prerequisites

- ffmpeg installed (`brew install ffmpeg`)
- ImageMagick installed (`brew install imagemagick`)
- An animated GIF file

## Overview

CRT Terminal uses sprite sheets - a single image containing all animation frames arranged in a grid. You'll need to:

1. Extract frames from the GIF
2. Remove the background (make it transparent)
3. Combine frames into a sprite sheet grid
4. Configure the theme CSS

## Steps

### 1. Create a working directory

```bash
mkdir sprite-work && cd sprite-work
```

### 2. Extract frames from the GIF

```bash
ffmpeg -i input.gif -vsync 0 frame_%04d.png
```

This extracts each frame as a separate PNG file (`frame_0001.png`, `frame_0002.png`, etc.).

Check how many frames were extracted:

```bash
ls frame_*.png | wc -l
```

### 3. Remove the background

If your GIF has a solid color background, remove it with ImageMagick:

```bash
for f in frame_*.png; do
    convert "$f" -fuzz 10% -transparent white "${f%.png}_transparent.png"
done
```

Replace `white` with your background color (`black`, `#ff00ff`, etc.).

The `-fuzz 10%` allows for slight color variations. Increase for more aggressive removal, decrease for precision.

For green screen / chroma key removal:

```bash
for f in frame_*.png; do
    convert "$f" -fuzz 15% -transparent "#00ff00" "${f%.png}_transparent.png"
done
```

### 4. Choose sprite sheet layout

CRT Terminal supports two sprite sheet layouts:

**Linear (1xN or Nx1)** - All frames in a single row or column:
```
[frame1][frame2][frame3][frame4][frame5]...
```

**Grid (MxN)** - Frames arranged in rows and columns:
```
[frame1][frame2][frame3][frame4]
[frame5][frame6][frame7][frame8]
[frame9][frame10][frame11][frame12]
```

For grid layouts, choose dimensions that create a roughly square image:

| Frames | Grid | Example |
|--------|------|---------|
| 12 | 4x3 | 4 columns, 3 rows |
| 16 | 4x4 | 4 columns, 4 rows |
| 24 | 6x4 | 6 columns, 4 rows |
| 25 | 5x5 | 5 columns, 5 rows |
| 30 | 6x5 | 6 columns, 5 rows |

### 5. Create the sprite sheet

**For linear sprite (horizontal row):**

```bash
montage frame_*_transparent.png -tile x1 -geometry +0+0 -background none sprite-sheet.png
```

**For linear sprite (vertical column):**

```bash
montage frame_*_transparent.png -tile 1x -geometry +0+0 -background none sprite-sheet.png
```

**For grid layout:**

```bash
montage frame_*_transparent.png -tile 5x5 -geometry +0+0 -background none sprite-sheet.png
```

Parameters:
- `-tile 5x5` - Grid layout (columns x rows)
- `-tile x1` - Single horizontal row (linear)
- `-tile 1x` - Single vertical column (linear)
- `-geometry +0+0` - No padding between frames
- `-background none` - Transparent background

### 6. Get frame dimensions

Check the dimensions of a single frame:

```bash
identify frame_0001.png
```

Output example: `frame_0001.png PNG 512x512 ...`

The sprite sheet dimensions will be: `frame_width * columns` x `frame_height * rows`

### 7. Copy to themes directory

```bash
mkdir -p ~/.config/crt/themes/images
cp sprite-sheet.png ~/.config/crt/themes/images/my-sprite.png
```

### 8. Configure the theme

Add the sprite configuration to your theme CSS.

**For linear sprite (horizontal):**

```css
:terminal::backdrop {
    --sprite-enabled: true;
    --sprite-path: "images/my-sprite.png";
    --sprite-frame-width: 512;
    --sprite-frame-height: 512;
    --sprite-columns: 25;
    --sprite-rows: 1;
    --sprite-frame-count: 25;
    --sprite-fps: 12;
    --sprite-scale: 0.5;
    --sprite-opacity: 0.8;
    --sprite-position: bottom-right;
}
```

**For grid sprite:**

```css
:terminal::backdrop {
    --sprite-enabled: true;
    --sprite-path: "images/my-sprite.png";
    --sprite-frame-width: 512;
    --sprite-frame-height: 512;
    --sprite-columns: 5;
    --sprite-rows: 5;
    --sprite-frame-count: 25;
    --sprite-fps: 12;
    --sprite-scale: 0.5;
    --sprite-opacity: 0.8;
    --sprite-position: bottom-right;
    --sprite-motion: float;
    --sprite-motion-speed: 0.5;
}
```

## Complete Example Script

Here's a complete script to automate the process:

```bash
#!/bin/bash
# gif-to-sprite.sh - Convert GIF to sprite sheet

INPUT_GIF="$1"
OUTPUT_NAME="${2:-sprite}"
COLUMNS="${3:-4}"
ROWS="${4:-4}"
BG_COLOR="${5:-white}"

if [ -z "$INPUT_GIF" ]; then
    echo "Usage: gif-to-sprite.sh input.gif [output_name] [columns] [rows] [bg_color]"
    exit 1
fi

# Create temp directory
WORK_DIR=$(mktemp -d)
cd "$WORK_DIR"

# Extract frames
echo "Extracting frames..."
ffmpeg -i "$INPUT_GIF" -vsync 0 frame_%04d.png 2>/dev/null

FRAME_COUNT=$(ls frame_*.png | wc -l | tr -d ' ')
echo "Extracted $FRAME_COUNT frames"

# Remove background
echo "Removing background ($BG_COLOR)..."
for f in frame_*.png; do
    convert "$f" -fuzz 10% -transparent "$BG_COLOR" "${f%.png}_t.png"
done

# Create sprite sheet
echo "Creating sprite sheet (${COLUMNS}x${ROWS})..."
montage frame_*_t.png -tile ${COLUMNS}x${ROWS} -geometry +0+0 -background none "${OUTPUT_NAME}.png"

# Get frame dimensions
DIMENSIONS=$(identify frame_0001.png | awk '{print $3}')
WIDTH=$(echo $DIMENSIONS | cut -d'x' -f1)
HEIGHT=$(echo $DIMENSIONS | cut -d'x' -f2)

# Move result and cleanup
mv "${OUTPUT_NAME}.png" "$OLDPWD/"
cd "$OLDPWD"
rm -rf "$WORK_DIR"

echo ""
echo "Created: ${OUTPUT_NAME}.png"
echo ""
echo "CSS configuration:"
echo "    --sprite-frame-width: $WIDTH;"
echo "    --sprite-frame-height: $HEIGHT;"
echo "    --sprite-columns: $COLUMNS;"
echo "    --sprite-rows: $ROWS;"
echo "    --sprite-frame-count: $FRAME_COUNT;"
```

Make it executable and run:

```bash
chmod +x gif-to-sprite.sh
./gif-to-sprite.sh animation.gif my-sprite 5 5 white
```

## Troubleshooting

**Background not fully removed?**
- Increase the fuzz percentage: `-fuzz 20%`
- Use a more precise color value: `-transparent "#f0f0f0"`
- For complex backgrounds, use `-alpha set -channel A -evaluate set 0%` on specific regions

**Animation too fast/slow?**
- Adjust `--sprite-fps` in the CSS
- Original GIF frame rate: `identify -verbose input.gif | grep Delay`

**Sprite sheet too large?**
- Resize frames before combining: `convert frame.png -resize 256x256 frame_resized.png`
- Use fewer frames by extracting every Nth frame: `ffmpeg -i input.gif -vf "select=not(mod(n\,2))" frame_%04d.png`

**Frames not aligning?**
- Ensure all frames are the same size
- Use `-geometry 512x512+0+0` to force specific frame size

## Sprite Properties Reference

| Property | Description |
|----------|-------------|
| `--sprite-enabled` | Enable sprite animation |
| `--sprite-path` | Path to sprite sheet image |
| `--sprite-frame-width` | Width of each frame in pixels |
| `--sprite-frame-height` | Height of each frame in pixels |
| `--sprite-columns` | Number of columns in the grid |
| `--sprite-rows` | Number of rows in the grid |
| `--sprite-frame-count` | Total number of frames |
| `--sprite-fps` | Animation frames per second |
| `--sprite-scale` | Display scale multiplier |
| `--sprite-opacity` | Sprite opacity (0.0-1.0) |
| `--sprite-position` | Position: `center`, `bottom-right`, `top-left`, etc. |
| `--sprite-motion` | Movement: `none`, `float`, `bounce`, `pace` |
| `--sprite-motion-speed` | Motion speed multiplier |

## See Also

- [Theme CSS Properties Reference](../reference/theme-css-properties.md)
- [How to Create a Custom Theme](create-custom-theme.md)
