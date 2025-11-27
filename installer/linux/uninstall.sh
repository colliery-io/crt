#!/bin/bash
# crt Uninstaller for Linux
set -e

INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"
CONFIG_DIR="$HOME/.config/crt"
DESKTOP_DIR="$HOME/.local/share/applications"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

info() { echo -e "${GREEN}[INFO]${NC} $1"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }

echo ""
echo "crt Uninstaller"
echo "==============="
echo ""

# Remove binary
if [ -f "$INSTALL_DIR/crt" ]; then
    info "Removing binary..."
    rm "$INSTALL_DIR/crt"
else
    warn "Binary not found at $INSTALL_DIR/crt"
fi

# Remove desktop entry
if [ -f "$DESKTOP_DIR/crt.desktop" ]; then
    info "Removing desktop entry..."
    rm "$DESKTOP_DIR/crt.desktop"
fi

# Remove icons
info "Removing icons..."
ICON_DIR="$HOME/.local/share/icons/hicolor"
for size in 16 24 32 48 64 128 256 512; do
    icon_file="$ICON_DIR/${size}x${size}/apps/crt.png"
    if [ -f "$icon_file" ]; then
        rm "$icon_file"
    fi
done

echo ""
info "Uninstall complete!"
echo ""
echo "Configuration preserved at: $CONFIG_DIR"
echo "To remove all config data: rm -rf $CONFIG_DIR"
