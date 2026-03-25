# Getting Started with CRT Terminal

CRT Terminal ("CRT's a Ridiculous Terminal") is a GPU-accelerated terminal emulator with CSS-based theming and visual effects. This tutorial walks you from a fresh install to a working, customized terminal. You will install CRT, explore its built-in themes, change your font and window size, and see live hot reload in action by editing a theme file.

No prior experience with CRT is required. You need a macOS or Linux system and a text editor.

---

## Prerequisites

- macOS 12+ or a modern Linux distribution
- `curl` available in your current shell
- A text editor (any editor works; examples use `nano`)

---

## Step 1: Install CRT Terminal

Run the one-line installer:

```bash
curl -sSL https://raw.githubusercontent.com/colliery-io/crt/main/scripts/install.sh | sh
```

The installer detects your platform automatically:

- **macOS**: CRT is installed to `/Applications/crt.app`
- **Linux**: CRT is installed to `~/.local/bin/crt`

After installation finishes, the installer prints the path and any first-run instructions.

**Verify the installation:**

On macOS, open Finder and look for `crt.app` in your Applications folder, or launch it from Spotlight with `Cmd+Space` then type `crt`.

On Linux, verify the binary is on your PATH:

```bash
which crt
# Expected: /home/yourname/.local/bin/crt
```

If `which crt` returns nothing, add `~/.local/bin` to your PATH:

```bash
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.bashrc
source ~/.bashrc
```

---

## Step 2: Launch CRT Terminal

**macOS**: Double-click `crt.app` in Applications, or launch it with:

```bash
open /Applications/crt.app
```

**Linux**:

```bash
crt
```

CRT opens with the default **synthwave** theme — a deep purple background with cyan text and a neon perspective grid in the backdrop. You should see a working shell prompt.

---

## Step 3: Explore Themes with the Context Menu

CRT ships with 19 built-in themes. The fastest way to browse them is the right-click context menu.

1. Right-click anywhere in the terminal window.
2. A context menu appears. Look for the **Theme** submenu.
3. Click any theme name to switch to it immediately.

Try a few themes to get a feel for the range of visual styles:

| Theme | Style |
|-------|-------|
| `dracula` | Classic dark purple with bright pastels |
| `matrix` | Green-on-black cascading characters |
| `tron` | Electric cyan on deep black |
| `vaporwave` | Pink and purple pastel gradients |
| `rain` | Animated rainfall backdrop |
| `particles` | Floating particle backdrop |
| `pokemon` | Animated Pikachu sprite with an indigo night background |
| `minimal` | Clean, no-effects, distraction-free |

Theme switching is instant — no restart required.

---

## Step 4: Understand the Config Directory

CRT reads its configuration from `~/.config/crt/`. The directory structure looks like this:

```
~/.config/crt/
├── config.toml        # Main configuration file
└── themes/            # Theme CSS files live here
    ├── mytheme.css    # (you will create this later)
    └── images/        # Images used by themes
```

The main config file uses [TOML](https://toml.io) format. If it does not exist yet, CRT creates it with defaults on first run.

Open the config file in your editor:

```bash
nano ~/.config/crt/config.toml
```

If the file is empty or does not exist, create it:

```bash
mkdir -p ~/.config/crt
nano ~/.config/crt/config.toml
```

---

## Step 5: Change the Active Theme

Inside `config.toml`, find or add the `[theme]` section:

```toml
[theme]
name = "tron"
```

Save the file. CRT detects the change and reloads immediately — you do not need to restart.

Switch back to synthwave (the default release theme):

```toml
[theme]
name = "synthwave"
```

The `name` value matches the filename of the CSS file, without the `.css` extension. Built-in themes are always available by name. Custom themes go in `~/.config/crt/themes/` and are referenced the same way.

---

## Step 6: Adjust Font and Window Size

Still in `config.toml`, add `[font]` and `[window]` sections. Here is a complete example with common customizations:

```toml
[theme]
name = "synthwave"

[font]
family = ["JetBrains Mono", "Fira Code", "SF Mono", "Menlo"]
size = 16.0
line_height = 1.4

[window]
columns = 120
rows = 36
title = "My Terminal"
```

**Font settings explained:**

- `family` — An ordered list of font names. CRT tries each in order and uses the first one installed on your system. The default list is `["MesloLGS NF", "JetBrains Mono", "Fira Code", "SF Mono", "Menlo"]`.
- `size` — Font size in points. Default is `14.0`.
- `line_height` — Line spacing multiplier. Default is `1.5`. Values between `1.2` and `1.6` are common.

**Window settings explained:**

- `columns` — Terminal width in character columns. Default is `80`.
- `rows` — Terminal height in rows. Default is `24`.
- `title` — The window title bar text.

Save the file. Font size and window title update immediately via hot reload. Window dimensions apply on next launch.

**Font size keyboard shortcuts:**

You can also adjust font size interactively without editing the config:

| Shortcut | Action |
|----------|--------|
| `Cmd+=` | Increase font size |
| `Cmd+-` | Decrease font size |
| `Cmd+0` | Reset font to configured size |

---

## Step 7: Configure the Cursor

Add a `[cursor]` section to customize the cursor style and blink behavior:

```toml
[cursor]
style = "bar"
blink = true
blink_interval_ms = 530
```

Available cursor styles:

| Style | Appearance |
|-------|-----------|
| `block` | Full character cell (default) |
| `bar` | Thin vertical bar |
| `underline` | Horizontal line under character |

Save the file to apply the change immediately.

---

## Step 8: Experience Hot Reload by Editing a Theme File

Hot reload is one of CRT's most useful features for theme development. Any time you save a theme CSS file, CRT reloads it instantly — no restart, no flicker.

Let's create a minimal custom theme to see this in action.

**Create the theme file:**

```bash
touch ~/.config/crt/themes/mytest.css
```

**Set it as the active theme in config.toml:**

```toml
[theme]
name = "mytest"
```

Save `config.toml`. The terminal will appear blank or use default colors because `mytest.css` is empty.

**Open the theme file in your editor** (use a second terminal window, or your graphical editor):

```bash
nano ~/.config/crt/themes/mytest.css
```

**Add some initial styling:**

```css
:terminal {
    color: #e0e0e0;
    background: #1a1a2e;
    font-size: 14;
    line-height: 1.5;
}

:terminal::cursor {
    background: #e94560;
}
```

Save the file. Watch the CRT window — it should update immediately with a dark blue background, light gray text, and a red cursor.

**Now change the background color** while CRT is still open:

```css
:terminal {
    color: #e0e0e0;
    background: #0f3460;
    font-size: 14;
    line-height: 1.5;
}
```

Save again. The background updates in real time — this is hot reload working. You can iterate on your theme as quickly as you can save the file.

---

## Step 9: Learn the Essential Keyboard Shortcuts

Get familiar with these shortcuts before moving on:

| Shortcut | Action |
|----------|--------|
| `Cmd+T` | New tab |
| `Cmd+W` | Close current tab |
| `Cmd+1` through `Cmd+9` | Switch to tab by number |
| `Cmd+Shift+[` | Switch to previous tab |
| `Cmd+Shift+]` | Switch to next tab |
| `Cmd+N` | New window |
| `Cmd+C` | Copy selection |
| `Cmd+V` | Paste |
| `Cmd+F` | Search |
| `Cmd+Q` | Quit |

On Linux, substitute `Ctrl` for `Cmd`.

---

## Next Steps

You now have CRT installed, configured with a custom font size and window size, and know how to use hot reload to iterate on themes in real time.

Where to go next:

- **Build a complete theme from scratch** — [Build Your First Theme](./build-your-first-theme.md)
- **Add animated sprites to a theme** — [Add Animated Sprites to Your Theme](./add-animated-sprites.md)
- **All CSS properties available in themes** — [Theme CSS Properties Reference](../reference/theme-css-properties.md)
- **All config.toml options** — [Configuration Reference](../reference/configuration.md)
- **Backdrop effects in detail** — [Theme CSS Properties Reference](../reference/theme-css-properties.md)
