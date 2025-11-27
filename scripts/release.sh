#!/bin/bash
# Release build script for crt
# Builds release artifacts for the current platform
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Extract version from Cargo.toml
VERSION=$(grep -E "^version" "$PROJECT_ROOT/Cargo.toml" | head -1 | sed 's/.*"\(.*\)".*/\1/')

echo ""
echo "crt Release Build v$VERSION"
echo "==========================="
echo ""

# Detect platform
case "$(uname -s)" in
    Darwin*)
        PLATFORM="macos"
        ;;
    Linux*)
        PLATFORM="linux"
        ;;
    *)
        echo "Unsupported platform: $(uname -s)"
        exit 1
        ;;
esac

echo "Platform: $PLATFORM"
echo ""

# Build release binary
echo "Building release binary..."
cd "$PROJECT_ROOT"
cargo build --release

# Create release directory
RELEASE_DIR="$PROJECT_ROOT/target/release/dist"
mkdir -p "$RELEASE_DIR"

if [ "$PLATFORM" = "macos" ]; then
    echo ""
    echo "Building macOS artifacts..."

    # Build app bundle
    "$PROJECT_ROOT/installer/macos/build-app.sh"

    # Create tarball of app bundle
    echo "Creating app bundle tarball..."
    cd "$PROJECT_ROOT/target/release"
    tar -czf "$RELEASE_DIR/crt-$VERSION-macos.tar.gz" crt.app

    # Build PKG installer
    "$PROJECT_ROOT/installer/macos/build-pkg.sh"
    mv "$PROJECT_ROOT/target/release/crt-$VERSION.pkg" "$RELEASE_DIR/"

    echo ""
    echo "macOS release artifacts:"
    ls -la "$RELEASE_DIR"

elif [ "$PLATFORM" = "linux" ]; then
    echo ""
    echo "Building Linux artifacts..."

    # Create tarball with binary and assets
    echo "Creating release tarball..."
    STAGING_DIR="$PROJECT_ROOT/target/release/crt-$VERSION"
    mkdir -p "$STAGING_DIR"

    # Copy binary
    cp "$PROJECT_ROOT/target/release/crt" "$STAGING_DIR/"

    # Copy assets
    cp -r "$PROJECT_ROOT/assets/config.toml" "$STAGING_DIR/"
    cp -r "$PROJECT_ROOT/assets/themes" "$STAGING_DIR/"
    cp -r "$PROJECT_ROOT/assets/fonts" "$STAGING_DIR/"
    cp -r "$PROJECT_ROOT/assets/icons" "$STAGING_DIR/"
    cp "$PROJECT_ROOT/assets/crt.desktop" "$STAGING_DIR/"

    # Copy installer scripts
    cp "$PROJECT_ROOT/installer/linux/install.sh" "$STAGING_DIR/"
    cp "$PROJECT_ROOT/installer/linux/uninstall.sh" "$STAGING_DIR/"

    # Create tarball
    cd "$PROJECT_ROOT/target/release"
    tar -czf "$RELEASE_DIR/crt-$VERSION-linux-$(uname -m).tar.gz" "crt-$VERSION"
    rm -rf "$STAGING_DIR"

    echo ""
    echo "Linux release artifacts:"
    ls -la "$RELEASE_DIR"
fi

echo ""
echo "Release build complete!"
echo "Artifacts in: $RELEASE_DIR"
