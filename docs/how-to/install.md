# How to Install CRT

This guide covers every method for installing CRT Terminal: the one-line install script, building from source, and creating a macOS app bundle. By the end you will have a working CRT installation with its configuration directory set up.

---

## Method 1: One-line install script (recommended)

The install script downloads a pre-built release from GitHub, installs it to the right location for your platform, and sets up `~/.config/crt/` with default themes and fonts.

```sh
curl -sSL https://raw.githubusercontent.com/colliery-io/crt/main/scripts/install.sh | sh
```

The script requires `curl` and `tar`. It detects your OS and CPU architecture automatically.

### What the script does

**On macOS:**
1. Downloads `crt-VERSION-macos-ARCH.tar.gz` from GitHub Releases.
2. Installs `crt.app` to `/Applications/`.
3. Removes the macOS quarantine attribute (may prompt for your password).
4. Copies themes, fonts, and `config.toml` to `~/.config/crt/`.

**On Linux:**
1. Downloads `crt-VERSION-linux-ARCH.tar.gz` from GitHub Releases.
2. Copies the `crt` binary to `~/.local/bin/` (or your chosen directory).
3. Makes the binary executable.
4. Copies themes, fonts, and `config.toml` to `~/.config/crt/`.

### Customising the install with environment variables

| Variable | Default | Description |
|---|---|---|
| `CRT_INSTALL_DIR` | `~/.local/bin` | Where to install the binary on Linux |
| `CRT_VERSION` | latest | Pin to a specific release tag (e.g. `0.1.0`) |

Example — install a specific version to a custom path:

```sh
CRT_VERSION=0.1.0 CRT_INSTALL_DIR=/usr/local/bin \
  curl -sSL https://raw.githubusercontent.com/colliery-io/crt/main/scripts/install.sh | sh
```

### Adding the binary to your PATH (Linux)

If `~/.local/bin` is not already in your `PATH`, the script will tell you. Add it permanently:

**Bash (`~/.bashrc` or `~/.bash_profile`):**
```sh
export PATH="$HOME/.local/bin:$PATH"
```

**Zsh (`~/.zshrc`):**
```sh
export PATH="$HOME/.local/bin:$PATH"
```

**Fish:**
```sh
fish_add_path ~/.local/bin
```

Then reload your shell: `source ~/.zshrc` (or restart the terminal).

---

## Method 2: Build from source

See [How to Build from Source](./build-from-source.md) for full instructions. The short version:

```sh
git clone https://github.com/colliery-io/crt.git
cd crt
cargo build --release
# Binary is at target/release/crt
```

After building, run the config installer to populate `~/.config/crt/`:

```sh
./scripts/dev.sh install
```

---

## Method 3: macOS app bundle

If you want to distribute or install a self-contained `.app` bundle without using the release script:

```sh
git clone https://github.com/colliery-io/crt.git
cd crt
./installer/macos/build-app.sh
```

This builds `target/release/crt.app`. Copy it to `/Applications/` manually:

```sh
cp -r target/release/crt.app /Applications/
```

To build a `.pkg` installer instead:

```sh
./installer/macos/build-pkg.sh
open target/release/crt-*.pkg
```

The `.pkg` installer places `crt.app` in `/Applications/` and runs a postinstall script that sets up `~/.config/crt/`.

---

## Post-install verification

**macOS:** Launch CRT from Spotlight (`Cmd+Space`, type `crt`), Launchpad, or Finder. A window should open with the Synthwave theme.

**Linux:** Run:
```sh
crt
```

If the command is not found, check that `~/.local/bin` is in your `PATH` (see above).

Verify the config directory was created:
```sh
ls ~/.config/crt/
```

You should see:
```
config.toml          ← your settings (edit this)
default_config.toml  ← reference copy, updated on each install
themes/              ← CSS theme files
fonts/               ← bundled NerdFonts (MesloLGS NF)
shell/               ← optional shell integration scripts
```

---

## Config directory structure

CRT reads all configuration from `~/.config/crt/` by default. To use a different location, set the `CRT_CONFIG_DIR` environment variable before launching:

```sh
CRT_CONFIG_DIR=/path/to/config crt
```

Key files:

| Path | Purpose |
|---|---|
| `~/.config/crt/config.toml` | Main configuration file |
| `~/.config/crt/default_config.toml` | Annotated reference; updated on every install |
| `~/.config/crt/themes/{name}.css` | Theme files; set `[theme] name = "..."` in config |
| `~/.config/crt/fonts/` | Fonts loaded before system fonts |
| `~/.config/crt/shell/` | Shell integration scripts for reactive themes |

---

## Updating

Re-run the install script to update to the latest release. The script preserves your existing `config.toml` and overwrites default themes to deliver any upstream changes. Your custom theme files (files not matching a built-in name) are left untouched.

```sh
curl -sSL https://raw.githubusercontent.com/colliery-io/crt/main/scripts/install.sh | sh
```

To update to a specific version:

```sh
CRT_VERSION=0.2.0 curl -sSL https://raw.githubusercontent.com/colliery-io/crt/main/scripts/install.sh | sh
```

---

## Uninstalling

**macOS:**
```sh
rm -rf /Applications/crt.app
rm -rf ~/.config/crt
```

**Linux:**
```sh
rm ~/.local/bin/crt
rm -rf ~/.config/crt
```

Your `config.toml` and any custom themes live in `~/.config/crt/`. Delete that directory only if you want to remove all personalisation.
