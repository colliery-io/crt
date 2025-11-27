#!/bin/bash
# Build PKG installer for crt
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
BUILD_DIR="$PROJECT_ROOT/target/release"
APP_DIR="$BUILD_DIR/crt.app"
PKG_DIR="$BUILD_DIR/pkg"
IDENTIFIER="com.colliery.crt"

# Extract version from Cargo.toml
VERSION=$(grep -E "^version" "$PROJECT_ROOT/Cargo.toml" | head -1 | sed 's/.*"\(.*\)".*/\1/')
echo "Building crt PKG installer v$VERSION..."

# First, build the app bundle
"$SCRIPT_DIR/build-app.sh"

# Create pkg staging directory
rm -rf "$PKG_DIR"
mkdir -p "$PKG_DIR/root/Applications"
mkdir -p "$PKG_DIR/scripts"

# Copy app bundle to staging
cp -R "$APP_DIR" "$PKG_DIR/root/Applications/"

# Copy postinstall script
cp "$SCRIPT_DIR/scripts/postinstall" "$PKG_DIR/scripts/"
chmod +x "$PKG_DIR/scripts/postinstall"

# Build the component package
echo "Building component package..."
pkgbuild \
    --root "$PKG_DIR/root" \
    --scripts "$PKG_DIR/scripts" \
    --identifier "$IDENTIFIER" \
    --version "$VERSION" \
    --install-location "/" \
    "$PKG_DIR/crt-component.pkg"

# Create distribution.xml for productbuild
cat > "$PKG_DIR/distribution.xml" << EOF
<?xml version="1.0" encoding="utf-8"?>
<installer-gui-script minSpecVersion="2">
    <title>crt</title>
    <organization>com.colliery</organization>
    <domains enable_localSystem="true"/>
    <options customize="never" require-scripts="true" rootVolumeOnly="true"/>

    <welcome file="welcome.html" mime-type="text/html"/>
    <conclusion file="conclusion.html" mime-type="text/html"/>

    <choices-outline>
        <line choice="default">
            <line choice="com.colliery.crt"/>
        </line>
    </choices-outline>

    <choice id="default"/>
    <choice id="com.colliery.crt" visible="false">
        <pkg-ref id="com.colliery.crt"/>
    </choice>

    <pkg-ref id="com.colliery.crt" version="$VERSION" onConclusion="none">crt-component.pkg</pkg-ref>
</installer-gui-script>
EOF

# Create welcome page
cat > "$PKG_DIR/welcome.html" << EOF
<!DOCTYPE html>
<html>
<head>
    <style>
        body { font-family: -apple-system, BlinkMacSystemFont, sans-serif; padding: 20px; }
        h1 { color: #333; }
        .feature { margin: 10px 0; }
    </style>
</head>
<body>
    <h1>crt v$VERSION</h1>
    <p>A GPU-accelerated terminal emulator with retro CRT effects.</p>

    <h2>This installer will:</h2>
    <div class="feature">Install crt.app to /Applications</div>
    <div class="feature">Set up configuration in ~/.config/crt/</div>
    <div class="feature">Install default themes and fonts</div>

    <p><strong>Note:</strong> Default themes are updated on each install. To customize a theme, copy it with a new name.</p>
</body>
</html>
EOF

# Create conclusion page
cat > "$PKG_DIR/conclusion.html" << 'EOF'
<!DOCTYPE html>
<html>
<head>
    <style>
        body { font-family: -apple-system, BlinkMacSystemFont, sans-serif; padding: 20px; }
        h1 { color: #333; }
        code { background: #f0f0f0; padding: 2px 6px; border-radius: 3px; }
    </style>
</head>
<body>
    <h1>Installation Complete</h1>
    <p>crt has been installed successfully.</p>

    <h2>Configuration</h2>
    <p>Your configuration is stored in <code>~/.config/crt/</code></p>
    <ul>
        <li><code>config.toml</code> - Your settings</li>
        <li><code>default_config.toml</code> - Reference (updated on install)</li>
        <li><code>themes/</code> - Theme files</li>
        <li><code>fonts/</code> - Font files</li>
    </ul>

    <h2>Get Started</h2>
    <p>Launch crt from your Applications folder or Spotlight.</p>
</body>
</html>
EOF

# Build the final product package
echo "Building product package..."
productbuild \
    --distribution "$PKG_DIR/distribution.xml" \
    --resources "$PKG_DIR" \
    --package-path "$PKG_DIR" \
    "$BUILD_DIR/crt-$VERSION.pkg"

# Clean up
rm -rf "$PKG_DIR"

echo ""
echo "PKG installer created: $BUILD_DIR/crt-$VERSION.pkg"
echo ""
echo "To install:"
echo "  open '$BUILD_DIR/crt-$VERSION.pkg'"
