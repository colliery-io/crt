# CRT Terminal

A GPU-accelerated terminal emulator with retro aesthetics and modern performance.

## Features

- **GPU Rendering** - Hardware-accelerated rendering via wgpu/vello for smooth 60fps performance
- **CSS Theming** - Fully customizable themes using CSS syntax with hot-reload support
- **Visual Effects** - CRT scanlines, matrix rain, particle systems, perspective grids, animated sprites
- **256-Color Support** - Full ANSI color palette with per-theme overrides for tools like LS_COLORS
- **Tabs** - Multi-tab support with customizable tab bar styling
- **Font Ligatures** - Programming ligature support with configurable font variants

## Installation

### macOS

```sh
curl -sSL https://raw.githubusercontent.com/colliery-io/crt/main/scripts/install.sh | sh
```

This installs `crt.app` to `/Applications` and sets up config at `~/.config/crt/`.

### Linux

```sh
curl -sSL https://raw.githubusercontent.com/colliery-io/crt/main/scripts/install.sh | sh
```

Binary installs to `~/.local/bin/crt` with config at `~/.config/crt/`.

### Building from Source

Requires Rust 2024 edition:

```sh
git clone https://github.com/colliery-io/crt.git
cd crt
cargo build --release
```

## Configuration

Configuration lives at `~/.config/crt/config.toml`:

```toml
[shell]
# program = "/bin/zsh"

[font]
family = ["MesloLGS NF", "JetBrains Mono", "Fira Code", "Menlo"]
size = 14.0
line_height = 1.4

[window]
columns = 80
rows = 24

[theme]
name = "synthwave"

[cursor]
style = "block"
blink = true
```

## Themes

CRT includes 14 built-in themes:

| Theme | Description |
|-------|-------------|
| `alien` | Amber phosphor CRT (Weyland-Yutani MU/TH/UR 6000) |
| `dracula` | Classic Dracula color scheme |
| `matrix` | Green falling code with CRT effect |
| `minimal` | Clean, pure black background |
| `particles` | Floating particle effect |
| `rain` | Animated rain drops |
| `robco` | Fallout Pip-Boy green phosphor CRT |
| `shape` | Floating geometric shapes |
| `solarized` | Solarized Dark color scheme |
| `starfield` | Twinkling stars background |
| `stress` | All effects at once (for testing) |
| `synthwave` | 80s neon with perspective grid |
| `tron` | Cyan grid aesthetic |
| `wh40k` | Warhammer 40K Adeptus Mechanicus with servo skull |

### Custom Themes

Create custom themes at `~/.config/crt/themes/mytheme.css` and set `name = "mytheme"` in config.

See the [Theming Guide](docs/theming.md) for details, or jump to:
- [How to Create a Custom Theme](docs/how-to/create-custom-theme.md)
- [CSS Properties Reference](docs/reference/theme-css-properties.md)

## Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| Cmd+T | New tab |
| Cmd+W | Close tab |
| Cmd+Shift+[ | Previous tab |
| Cmd+Shift+] | Next tab |
| Cmd+1-9 | Switch to tab 1-9 |
| Cmd+= | Increase font size |
| Cmd+- | Decrease font size |
| Cmd+0 | Reset font size |
| Cmd+C | Copy |
| Cmd+V | Paste |
| Cmd+Q | Quit |

## Architecture

CRT is built with a modular crate structure:

- `crt-core` - Terminal emulation (alacritty_terminal), PTY handling
- `crt-renderer` - GPU rendering, effects, tab bar, glyph cache
- `crt-theme` - CSS parsing, theme hot-reload

## License

MIT
