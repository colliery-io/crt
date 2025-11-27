#!/bin/bash
# Release build script for crt
# Builds release artifacts for the current platform
#
# Outputs:
#   macOS:  crt-VERSION-macos-ARCH.tar.gz (contains crt.app bundle)
#   Linux:  crt-VERSION-linux-ARCH.tar.gz (contains binary + assets)
#
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

# Detect architecture
case "$(uname -m)" in
    x86_64|amd64)
        ARCH="x86_64"
        ;;
    arm64|aarch64)
        ARCH="aarch64"
        ;;
    *)
        echo "Unsupported architecture: $(uname -m)"
        exit 1
        ;;
esac

echo "Platform: $PLATFORM"
echo "Architecture: $ARCH"
echo ""

# Build release binary
echo "Building release binary..."
cd "$PROJECT_ROOT"
cargo build --release

# Create release directory
RELEASE_DIR="$PROJECT_ROOT/target/release/dist"
rm -rf "$RELEASE_DIR"
mkdir -p "$RELEASE_DIR"

# Artifact filename
ARTIFACT_NAME="crt-${VERSION}-${PLATFORM}-${ARCH}"

if [ "$PLATFORM" = "macos" ]; then
    echo ""
    echo "Building macOS release..."

    # Build app bundle
    "$PROJECT_ROOT/installer/macos/build-app.sh"

    # Update Info.plist version dynamically
    APP_DIR="$PROJECT_ROOT/target/release/crt.app"
    /usr/libexec/PlistBuddy -c "Set :CFBundleShortVersionString $VERSION" "$APP_DIR/Contents/Info.plist"
    /usr/libexec/PlistBuddy -c "Set :CFBundleVersion $VERSION" "$APP_DIR/Contents/Info.plist"

    # Create tarball containing the app bundle
    echo "Creating release tarball..."
    cd "$PROJECT_ROOT/target/release"
    tar -czf "$RELEASE_DIR/${ARTIFACT_NAME}.tar.gz" crt.app

    echo ""
    echo "macOS release artifact:"
    ls -lh "$RELEASE_DIR/${ARTIFACT_NAME}.tar.gz"

elif [ "$PLATFORM" = "linux" ]; then
    echo ""
    echo "Building Linux release..."

    # Create staging directory with expected structure
    STAGING_DIR="$PROJECT_ROOT/target/release/staging"
    rm -rf "$STAGING_DIR"
    mkdir -p "$STAGING_DIR"

    # Copy binary
    cp "$PROJECT_ROOT/target/release/crt" "$STAGING_DIR/"

    # Copy assets
    mkdir -p "$STAGING_DIR/assets/themes"
    mkdir -p "$STAGING_DIR/assets/fonts"

    cp "$PROJECT_ROOT/assets/config.toml" "$STAGING_DIR/assets/"

    # Copy themes
    for theme in "$PROJECT_ROOT/assets/themes/"*.css; do
        if [ -f "$theme" ]; then
            cp "$theme" "$STAGING_DIR/assets/themes/"
        fi
    done

    # Copy theme asset directories (sprites, images)
    for subdir in "$PROJECT_ROOT/assets/themes/"*/; do
        if [ -d "$subdir" ]; then
            cp -R "$subdir" "$STAGING_DIR/assets/themes/"
        fi
    done

    # Copy fonts
    for font in "$PROJECT_ROOT/assets/fonts/"*.ttf "$PROJECT_ROOT/assets/fonts/"*.otf; do
        if [ -f "$font" ]; then
            cp "$font" "$STAGING_DIR/assets/fonts/"
        fi
    done

    # Copy desktop entry if exists
    if [ -f "$PROJECT_ROOT/assets/crt.desktop" ]; then
        cp "$PROJECT_ROOT/assets/crt.desktop" "$STAGING_DIR/"
    fi

    # Copy icons if exists
    if [ -d "$PROJECT_ROOT/assets/icons" ]; then
        cp -R "$PROJECT_ROOT/assets/icons" "$STAGING_DIR/"
    fi

    # Create tarball (contents at root level, not in subdirectory)
    echo "Creating release tarball..."
    cd "$STAGING_DIR"
    tar -czf "$RELEASE_DIR/${ARTIFACT_NAME}.tar.gz" .

    # Cleanup staging
    rm -rf "$STAGING_DIR"

    echo ""
    echo "Linux release artifact:"
    ls -lh "$RELEASE_DIR/${ARTIFACT_NAME}.tar.gz"
fi

echo ""
echo "Release build complete!"
echo ""
echo "Artifact: $RELEASE_DIR/${ARTIFACT_NAME}.tar.gz"
echo ""
echo "To create a GitHub release:"
echo "  gh release create v${VERSION} '$RELEASE_DIR/${ARTIFACT_NAME}.tar.gz' --title 'v${VERSION}' --notes 'Release notes here'"
