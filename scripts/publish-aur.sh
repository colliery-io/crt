#!/bin/bash
# Publish crt to the AUR
# Usage: ./scripts/publish-aur.sh <version>
#
# Environment variables:
#   AUR_SSH_PRIVATE_KEY - SSH private key for AUR authentication (required in CI)

set -euo pipefail

VERSION="${1:-}"
if [ -z "$VERSION" ]; then
    echo "Usage: $0 <version>"
    echo "Example: $0 0.0.9"
    exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(dirname "$SCRIPT_DIR")"
WORK_DIR=$(mktemp -d)
trap "rm -rf '$WORK_DIR'" EXIT

echo "Publishing crt v${VERSION} to AUR..."

# Download release artifacts to compute checksums
echo "Downloading release artifacts..."
X86_64_URL="https://github.com/colliery-io/crt/releases/download/v${VERSION}/crt-${VERSION}-linux-x86_64.tar.gz"
AARCH64_URL="https://github.com/colliery-io/crt/releases/download/v${VERSION}/crt-${VERSION}-linux-aarch64.tar.gz"

# Download with retry (release assets may take a moment to be available)
download_with_retry() {
    local url="$1"
    local output="$2"
    local max_attempts=5
    local attempt=1

    while [ $attempt -le $max_attempts ]; do
        if curl -sSL -f -o "$output" "$url" 2>/dev/null; then
            return 0
        fi
        echo "  Attempt $attempt/$max_attempts failed, waiting 10s..."
        sleep 10
        attempt=$((attempt + 1))
    done
    return 1
}

echo "Downloading x86_64 artifact..."
if ! download_with_retry "$X86_64_URL" "$WORK_DIR/x86_64.tar.gz"; then
    echo "Error: x86_64 artifact not found"
    exit 1
fi

echo "Downloading aarch64 artifact..."
if ! download_with_retry "$AARCH64_URL" "$WORK_DIR/aarch64.tar.gz"; then
    echo "Error: aarch64 artifact not found"
    exit 1
fi

# Compute checksums
SHA256_X86_64=$(sha256sum "$WORK_DIR/x86_64.tar.gz" | cut -d' ' -f1)
echo "x86_64 checksum: $SHA256_X86_64"

SHA256_AARCH64=$(sha256sum "$WORK_DIR/aarch64.tar.gz" | cut -d' ' -f1)
echo "aarch64 checksum: $SHA256_AARCH64"

# Generate PKGBUILD from template
echo "Generating PKGBUILD..."
sed -e "s/\${PKGVER}/${VERSION}/g" \
    -e "s/\${SHA256_X86_64}/${SHA256_X86_64}/g" \
    -e "s/\${SHA256_AARCH64}/${SHA256_AARCH64}/g" \
    "$REPO_ROOT/pkg/aur/PKGBUILD.template" > "$WORK_DIR/PKGBUILD"

# Generate .SRCINFO
echo "Generating .SRCINFO..."
cat > "$WORK_DIR/.SRCINFO" <<EOF
pkgbase = crt-bin
	pkgdesc = GPU-accelerated terminal emulator with CSS theming and visual effects
	pkgver = ${VERSION}
	pkgrel = 1
	url = https://github.com/colliery-io/crt
	arch = x86_64
	arch = aarch64
	license = MIT
	license = Apache-2.0
	depends = fontconfig
	depends = freetype2
	depends = libxkbcommon
	depends = wayland
	depends = libx11
	depends = vulkan-icd-loader
	depends = hicolor-icon-theme
	optdepends = vulkan-driver: for Vulkan rendering backend
	provides = crt
	conflicts = crt
	conflicts = crt-git
	source_x86_64 = crt-${VERSION}-linux-x86_64.tar.gz::https://github.com/colliery-io/crt/releases/download/v${VERSION}/crt-${VERSION}-linux-x86_64.tar.gz
	sha256sums_x86_64 = ${SHA256_X86_64}
	source_aarch64 = crt-${VERSION}-linux-aarch64.tar.gz::https://github.com/colliery-io/crt/releases/download/v${VERSION}/crt-${VERSION}-linux-aarch64.tar.gz
	sha256sums_aarch64 = ${SHA256_AARCH64}

pkgname = crt-bin
EOF

# Set up SSH for AUR
if [ -n "${AUR_SSH_PRIVATE_KEY:-}" ]; then
    echo "Setting up SSH authentication..."
    mkdir -p ~/.ssh
    echo "$AUR_SSH_PRIVATE_KEY" > ~/.ssh/aur_key
    chmod 600 ~/.ssh/aur_key

    cat >> ~/.ssh/config <<EOF
Host aur.archlinux.org
    IdentityFile ~/.ssh/aur_key
    User aur
    StrictHostKeyChecking accept-new
EOF
fi

# Clone AUR repo and push update
echo "Cloning AUR repository..."
cd "$WORK_DIR"
git clone ssh://aur@aur.archlinux.org/crt-bin.git aur-repo || {
    echo "Failed to clone AUR repo. Make sure:"
    echo "  1. The crt-bin package exists on AUR"
    echo "  2. SSH key is configured correctly"
    exit 1
}

cd aur-repo
cp ../PKGBUILD .
cp ../.SRCINFO .

# Check if there are changes
if git diff --quiet && git diff --staged --quiet; then
    echo "No changes to publish"
    exit 0
fi

# Commit and push
git config user.email "hello@colliery.io"
git config user.name "Colliery CI"
git add PKGBUILD .SRCINFO
git commit -m "Update to v${VERSION}"
git push

echo "Successfully published crt-bin v${VERSION} to AUR!"
