## `CRT's a Ridiculous Terminal`

 <p align="center">
    <img src="assets/icons/crt-256x256.png" alt="CRT" width="256">
  </p>


## Why 

I really like [Hyper.js](https://github.com/vercel/hyper), being able to use CSS to style a terminal just made it fun, but it's not been maintained and has been regressing in performance recently. So like anyone 
who hasn't tried to do the thing before I decided "How hard could it be ?". This is the result, it's not as performant as [rio](https://rioterm.com/) or [alacritty](https://alacritty.org/), but I'm **pretty** sure we're eating up less memory than Hyper and I'm not getting weird pinwheels of doom as I use it - and I'm doing way more rendering so I'm taking it as a win. 

Also, my 6 year old thinks its cool so what other possible endorsements could you want ?

 <div align="center">

  > ### *"This is cool!"*
  >
  > — Mat (6 years old)

  </div>

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

### Linux (un tested ) 

```sh
curl -sSL https://raw.githubusercontent.com/colliery-io/crt/main/scripts/install.sh | sh
```

Binary installs to `~/.local/bin/crt` with config at `~/.config/crt/`.

### Windows Support

[Soon™](https://wowpedia.fandom.com/wiki/Soon)

### Building from Source

Requires Rust 2024 edition:

```sh
git clone https://github.com/colliery-io/crt.git
cd crt
cargo build --release
```

## Configuration

Configuration lives at `~/.config/crt/config.toml`:

## Themes

CRT includes 15 built-in themes:

| Theme | Description |
|-------|-------------|
| `alien` | Amber phosphor CRT (Weyland-Yutani MU/TH/UR 6000) |
| `dracula` | Classic Dracula color scheme |
| `matrix` | Green falling code with CRT effect |
| `minimal` | Clean, pure black background |
| `nyancat` | Bouncing Nyan Cat with stars and sparkles |
| `particles` | Floating particle effect |
| `rain` | Animated rain drops |
| `robco` | Fallout Pip-Boy green phosphor CRT |
| `shape` | Floating geometric shapes |
| `solarized` | Solarized Dark color scheme |
| `starfield` | Twinkling stars background |
| `stress` | All effects at once (for testing) |
| `synthwave` | 80s neon with perspective grid |
| `tron` | Cyan grid aesthetic |
| `vaporwave` | Pink and cyan aesthetic with perspective grid |
| `wh40k` | Warhammer 40K Adeptus Mechanicus with servo skull |

### Custom Themes

Create custom themes at `~/.config/crt/themes/mytheme.css` and set `name = "mytheme"` in config.

See the [Theming Guide](docs/theming.md) for details, or jump to:
- [How to Create a Custom Theme](docs/how-to/create-custom-theme.md)
- [CSS Properties Reference](docs/reference/theme-css-properties.md)

## Basic Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| Cmd+T | New tab |
| Cmd+W | Close tab |
| Cmd+1-9 | Switch to tab 1-9 |
| Cmd+= | Increase font size |
| Cmd+- | Decrease font size |
| Cmd+0 | Reset font size |

## Reporting Issues

Found a bug? Enable profiling with `Cmd+Option+P`, reproduce the issue, then submit a bug report with the profile log from `~/.config/crt/`.

[Open an issue](https://github.com/colliery-io/crt/issues)

## Contributing

[Open a PR](https://github.com/colliery-io/crt/pulls)

## License

MIT
