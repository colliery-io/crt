#!/bin/bash
# Build crt.app bundle for macOS
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
BUILD_DIR="$PROJECT_ROOT/target/release"
APP_NAME="crt.app"
APP_DIR="$BUILD_DIR/$APP_NAME"

echo "Building crt for macOS..."
echo "Project root: $PROJECT_ROOT"

# Build release binary
echo "Building release binary..."
cd "$PROJECT_ROOT"
cargo build --release

# Create app bundle structure
echo "Creating app bundle..."
rm -rf "$APP_DIR"
mkdir -p "$APP_DIR/Contents/MacOS"
mkdir -p "$APP_DIR/Contents/Resources"

# Copy binary
cp "$BUILD_DIR/crt" "$APP_DIR/Contents/MacOS/"

# Copy Info.plist
cp "$SCRIPT_DIR/Info.plist" "$APP_DIR/Contents/"

# Copy icon if it exists
if [ -f "$SCRIPT_DIR/crt.icns" ]; then
    cp "$SCRIPT_DIR/crt.icns" "$APP_DIR/Contents/Resources/"
elif [ -f "$PROJECT_ROOT/assets/crt.icns" ]; then
    cp "$PROJECT_ROOT/assets/crt.icns" "$APP_DIR/Contents/Resources/"
else
    echo "Warning: No icon file found (crt.icns)"
fi

# Copy assets for installer (config, themes, fonts)
echo "Bundling assets..."
ASSETS_DEST="$APP_DIR/Contents/Resources/assets"
mkdir -p "$ASSETS_DEST/themes"
mkdir -p "$ASSETS_DEST/fonts"

# Copy config template
cp "$PROJECT_ROOT/assets/config.toml" "$ASSETS_DEST/"

# Copy themes (CSS files)
if [ -d "$PROJECT_ROOT/assets/themes" ]; then
    cp "$PROJECT_ROOT/assets/themes/"*.css "$ASSETS_DEST/themes/" 2>/dev/null || true
fi

# Copy theme asset directories (sprites, images, etc.)
# Preserve directory structure so paths like "wh40k/sprite.png" work
if [ -d "$PROJECT_ROOT/assets/themes" ]; then
    for subdir in "$PROJECT_ROOT/assets/themes/"*/; do
        if [ -d "$subdir" ]; then
            dirname=$(basename "$subdir")
            mkdir -p "$ASSETS_DEST/themes/$dirname"
            cp -R "$subdir"* "$ASSETS_DEST/themes/$dirname/" 2>/dev/null || true
        fi
    done
fi

# Copy fonts
if [ -d "$PROJECT_ROOT/assets/fonts" ]; then
    cp "$PROJECT_ROOT/assets/fonts/"*.ttf "$ASSETS_DEST/fonts/" 2>/dev/null || true
    cp "$PROJECT_ROOT/assets/fonts/"*.otf "$ASSETS_DEST/fonts/" 2>/dev/null || true
fi

# Make binary executable
chmod +x "$APP_DIR/Contents/MacOS/crt"

echo ""
echo "App bundle created at: $APP_DIR"
echo ""
echo "To install manually:"
echo "  cp -r '$APP_DIR' /Applications/"
echo ""
echo "To create a DMG, run:"
echo "  $SCRIPT_DIR/create-dmg.sh"
