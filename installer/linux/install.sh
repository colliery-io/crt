#!/bin/bash
# crt Installer for Linux
# Installs binary, config, themes, and fonts
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

# Default installation paths
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"
CONFIG_DIR="$HOME/.config/crt"
DESKTOP_DIR="$HOME/.local/share/applications"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

info() { echo -e "${GREEN}[INFO]${NC} $1"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
error() { echo -e "${RED}[ERROR]${NC} $1"; exit 1; }

# Extract version from Cargo.toml
VERSION=$(grep -E "^version" "$PROJECT_ROOT/Cargo.toml" | head -1 | sed 's/.*"\(.*\)".*/\1/')

echo ""
echo "crt Installer v$VERSION"
echo "======================="
echo ""

# Check for cargo
if ! command -v cargo &> /dev/null; then
    error "Cargo not found. Please install Rust: https://rustup.rs"
fi

# Build release binary
info "Building release binary..."
cd "$PROJECT_ROOT"
cargo build --release

# Create directories
info "Creating directories..."
mkdir -p "$INSTALL_DIR"
mkdir -p "$CONFIG_DIR/themes"
mkdir -p "$CONFIG_DIR/fonts"
mkdir -p "$DESKTOP_DIR"

# Install binary
info "Installing binary to $INSTALL_DIR..."
cp "$PROJECT_ROOT/target/release/crt" "$INSTALL_DIR/"
chmod +x "$INSTALL_DIR/crt"

# Always update default_config.toml (reference for upgrades)
info "Installing default_config.toml (reference config)..."
cp "$PROJECT_ROOT/assets/config.toml" "$CONFIG_DIR/default_config.toml"

# Only create config.toml if it doesn't exist (preserve user customizations)
if [ ! -f "$CONFIG_DIR/config.toml" ]; then
    info "Installing config.toml..."
    cp "$PROJECT_ROOT/assets/config.toml" "$CONFIG_DIR/config.toml"
else
    info "Preserving existing config.toml (see default_config.toml for new options)"
fi

# Always overwrite default themes
info "Installing default themes..."
if [ -d "$PROJECT_ROOT/assets/themes" ]; then
    for theme in "$PROJECT_ROOT/assets/themes/"*.css; do
        if [ -f "$theme" ]; then
            basename=$(basename "$theme")
            cp "$theme" "$CONFIG_DIR/themes/$basename"
            echo "  - $basename"
        fi
    done
fi

# Copy fonts
info "Installing fonts..."
if [ -d "$PROJECT_ROOT/assets/fonts" ]; then
    for font in "$PROJECT_ROOT/assets/fonts/"*.ttf "$PROJECT_ROOT/assets/fonts/"*.otf; do
        if [ -f "$font" ]; then
            basename=$(basename "$font")
            cp "$font" "$CONFIG_DIR/fonts/$basename"
        fi
    done
fi

# Install icons
info "Installing icons..."
ICON_DIR="$HOME/.local/share/icons/hicolor"
if [ -d "$PROJECT_ROOT/assets/icons" ]; then
    for size in 16 24 32 48 64 128 256 512; do
        icon_file="$PROJECT_ROOT/assets/icons/crt-${size}x${size}.png"
        if [ -f "$icon_file" ]; then
            mkdir -p "$ICON_DIR/${size}x${size}/apps"
            cp "$icon_file" "$ICON_DIR/${size}x${size}/apps/crt.png"
        fi
    done
    # Update icon cache if gtk-update-icon-cache is available
    if command -v gtk-update-icon-cache &> /dev/null; then
        gtk-update-icon-cache -f -t "$ICON_DIR" 2>/dev/null || true
    fi
fi

# Install .desktop file
info "Installing desktop entry..."
cat > "$DESKTOP_DIR/crt.desktop" << EOF
[Desktop Entry]
Name=crt
Comment=GPU-accelerated terminal emulator with retro CRT effects
Exec=$INSTALL_DIR/crt
Icon=crt
Terminal=false
Type=Application
Categories=System;TerminalEmulator;
Keywords=terminal;console;command;prompt;shell;
EOF

# Check if INSTALL_DIR is in PATH
if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
    warn "$INSTALL_DIR is not in your PATH"
    echo ""
    echo "Add it to your shell profile:"
    echo "  echo 'export PATH=\"\$PATH:$INSTALL_DIR\"' >> ~/.bashrc"
    echo "  # or for zsh:"
    echo "  echo 'export PATH=\"\$PATH:$INSTALL_DIR\"' >> ~/.zshrc"
fi

echo ""
info "Installation complete!"
echo ""
echo "Configuration:"
echo "  $CONFIG_DIR/config.toml         - Your config (edit this)"
echo "  $CONFIG_DIR/default_config.toml - Reference config (updated on install)"
echo ""
echo "Themes (copy and rename to customize):"
ls -1 "$CONFIG_DIR/themes/" 2>/dev/null | sed 's/^/  /'
echo ""
echo "Note: Default themes are overwritten on install/upgrade."
echo "      To customize, copy a theme: cp mytheme.css custom.css"
echo ""
echo "Run 'crt' to start the terminal."
