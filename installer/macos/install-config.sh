#!/bin/bash
# Install crt configuration files to ~/.config/crt
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
CONFIG_DIR="$HOME/.config/crt"

echo "Installing crt configuration..."

# Create config directory structure
mkdir -p "$CONFIG_DIR/themes"
mkdir -p "$CONFIG_DIR/fonts"

# Always update default_config.toml (reference for upgrades)
echo "Installing default_config.toml (reference config)..."
cp "$PROJECT_ROOT/assets/config.toml" "$CONFIG_DIR/default_config.toml"

# Only create config.toml if it doesn't exist (preserve user customizations)
if [ ! -f "$CONFIG_DIR/config.toml" ]; then
    echo "Installing config.toml..."
    cp "$PROJECT_ROOT/assets/config.toml" "$CONFIG_DIR/config.toml"
else
    echo "Preserving existing config.toml (see default_config.toml for new options)"
fi

# Always overwrite default themes (users should copy + customize, not modify defaults)
echo "Installing default themes..."
if [ -d "$PROJECT_ROOT/assets/themes" ]; then
    for theme in "$PROJECT_ROOT/assets/themes/"*.css; do
        if [ -f "$theme" ]; then
            basename=$(basename "$theme")
            cp "$theme" "$CONFIG_DIR/themes/$basename"
            echo "  - $basename"
        fi
    done
fi

# Copy fonts (overwrite to update)
echo "Installing fonts..."
if [ -d "$PROJECT_ROOT/assets/fonts" ]; then
    cp "$PROJECT_ROOT/assets/fonts/"*.ttf "$CONFIG_DIR/fonts/" 2>/dev/null || true
    cp "$PROJECT_ROOT/assets/fonts/"*.otf "$CONFIG_DIR/fonts/" 2>/dev/null || true
fi

echo ""
echo "Configuration installed to: $CONFIG_DIR"
echo ""
echo "Files:"
echo "  config.toml         - Your config (edit this)"
echo "  default_config.toml - Reference config (updated on install)"
echo ""
echo "Themes (copy and rename to customize):"
ls -1 "$CONFIG_DIR/themes/" 2>/dev/null | sed 's/^/  /'
echo ""
echo "Note: Default themes are overwritten on install/upgrade."
echo "      To customize, copy a theme: cp mytheme.css custom.css"
