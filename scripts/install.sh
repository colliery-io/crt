#!/bin/sh
# crt installer script
# Usage: curl -sSL https://raw.githubusercontent.com/colliery-io/crt/main/scripts/install.sh | sh
#
# Environment variables:
#   CRT_INSTALL_DIR  - Binary install location (default: ~/.local/bin)
#   CRT_VERSION      - Specific version to install (default: latest)

set -e

REPO="colliery-io/crt"
GITHUB_API="https://api.github.com/repos/${REPO}"
GITHUB_RELEASES="https://github.com/${REPO}/releases"

# Colors (disabled if not a terminal)
if [ -t 1 ]; then
    RED='\033[0;31m'
    GREEN='\033[0;32m'
    YELLOW='\033[0;33m'
    BLUE='\033[0;34m'
    BOLD='\033[1m'
    NC='\033[0m' # No Color
else
    RED=''
    GREEN=''
    YELLOW=''
    BLUE=''
    BOLD=''
    NC=''
fi

info() {
    printf "${BLUE}==>${NC} ${BOLD}%s${NC}\n" "$1"
}

success() {
    printf "${GREEN}==>${NC} ${BOLD}%s${NC}\n" "$1"
}

warn() {
    printf "${YELLOW}Warning:${NC} %s\n" "$1"
}

error() {
    printf "${RED}Error:${NC} %s\n" "$1" >&2
    exit 1
}

# Detect OS
detect_os() {
    case "$(uname -s)" in
        Darwin*)
            echo "macos"
            ;;
        Linux*)
            echo "linux"
            ;;
        *)
            error "Unsupported operating system: $(uname -s)"
            ;;
    esac
}

# Detect architecture
detect_arch() {
    case "$(uname -m)" in
        x86_64|amd64)
            echo "x86_64"
            ;;
        arm64|aarch64)
            echo "aarch64"
            ;;
        *)
            error "Unsupported architecture: $(uname -m)"
            ;;
    esac
}

# Check for required commands
check_dependencies() {
    for cmd in curl tar; do
        if ! command -v "$cmd" >/dev/null 2>&1; then
            error "Required command not found: $cmd"
        fi
    done
}

# Get latest release version from GitHub
get_latest_version() {
    curl -sSL "${GITHUB_API}/releases/latest" | \
        grep '"tag_name":' | \
        sed -E 's/.*"tag_name": *"([^"]+)".*/\1/' | \
        sed 's/^v//'
}

# Download and extract release
download_release() {
    local version="$1"
    local os="$2"
    local arch="$3"
    local tmp_dir="$4"

    # Construct download URL
    local filename="crt-${version}-${os}-${arch}.tar.gz"
    local url="${GITHUB_RELEASES}/download/v${version}/${filename}"

    info "Downloading crt v${version} for ${os}-${arch}..."

    if ! curl -sSL -o "${tmp_dir}/${filename}" "$url"; then
        error "Failed to download from: $url"
    fi

    info "Extracting..."
    tar -xzf "${tmp_dir}/${filename}" -C "${tmp_dir}"
}

# Install binary
install_binary() {
    local tmp_dir="$1"
    local install_dir="$2"
    local os="$3"

    # Create install directory
    mkdir -p "$install_dir"

    if [ "$os" = "macos" ]; then
        # macOS: Install the .app bundle to /Applications
        if [ -d "${tmp_dir}/crt.app" ]; then
            info "Installing crt.app to /Applications..."

            # Remove old installation if exists
            if [ -d "/Applications/crt.app" ]; then
                rm -rf "/Applications/crt.app"
            fi

            cp -R "${tmp_dir}/crt.app" "/Applications/"

            # Remove quarantine attribute
            xattr -rd com.apple.quarantine "/Applications/crt.app" 2>/dev/null || true

            # Also install CLI binary to install_dir for terminal access
            if [ -f "${tmp_dir}/crt.app/Contents/MacOS/crt" ]; then
                cp "${tmp_dir}/crt.app/Contents/MacOS/crt" "${install_dir}/crt"
                chmod +x "${install_dir}/crt"
            fi
        else
            # Fallback: just binary
            cp "${tmp_dir}/crt" "${install_dir}/crt"
            chmod +x "${install_dir}/crt"
        fi
    else
        # Linux: Install binary
        cp "${tmp_dir}/crt" "${install_dir}/crt"
        chmod +x "${install_dir}/crt"
    fi
}

# Set up config directory
setup_config() {
    local tmp_dir="$1"
    local os="$2"
    local config_dir="${HOME}/.config/crt"

    info "Setting up configuration in ${config_dir}..."

    # Create directory structure
    mkdir -p "${config_dir}/themes"
    mkdir -p "${config_dir}/fonts"

    # Find assets directory
    local assets_dir=""
    if [ "$os" = "macos" ] && [ -d "${tmp_dir}/crt.app/Contents/Resources/assets" ]; then
        assets_dir="${tmp_dir}/crt.app/Contents/Resources/assets"
    elif [ -d "${tmp_dir}/assets" ]; then
        assets_dir="${tmp_dir}/assets"
    fi

    if [ -n "$assets_dir" ]; then
        # Copy default config (only if user doesn't have one)
        if [ ! -f "${config_dir}/config.toml" ]; then
            if [ -f "${assets_dir}/config.toml" ]; then
                cp "${assets_dir}/config.toml" "${config_dir}/config.toml"
            fi
        fi

        # Always update default_config.toml for reference
        if [ -f "${assets_dir}/config.toml" ]; then
            cp "${assets_dir}/config.toml" "${config_dir}/default_config.toml"
        fi

        # Copy themes (always update defaults, preserve user customizations)
        if [ -d "${assets_dir}/themes" ]; then
            for theme in "${assets_dir}/themes/"*.css; do
                if [ -f "$theme" ]; then
                    cp "$theme" "${config_dir}/themes/"
                fi
            done
        fi

        # Copy fonts
        if [ -d "${assets_dir}/fonts" ]; then
            for font in "${assets_dir}/fonts/"*.ttf "${assets_dir}/fonts/"*.otf; do
                if [ -f "$font" ]; then
                    cp "$font" "${config_dir}/fonts/"
                fi
            done
        fi

        # Copy theme assets (e.g., wh40k sprites)
        for subdir in "${assets_dir}/themes/"*/; do
            if [ -d "$subdir" ]; then
                dirname=$(basename "$subdir")
                mkdir -p "${config_dir}/themes/${dirname}"
                cp -R "$subdir"* "${config_dir}/themes/${dirname}/" 2>/dev/null || true
            fi
        done
    fi
}

# Check if install directory is in PATH
check_path() {
    local install_dir="$1"

    case ":${PATH}:" in
        *":${install_dir}:"*)
            return 0
            ;;
        *)
            return 1
            ;;
    esac
}

# Suggest PATH addition
suggest_path() {
    local install_dir="$1"
    local shell_name="$(basename "$SHELL")"
    local rc_file=""

    case "$shell_name" in
        bash)
            if [ -f "${HOME}/.bash_profile" ]; then
                rc_file="${HOME}/.bash_profile"
            else
                rc_file="${HOME}/.bashrc"
            fi
            ;;
        zsh)
            rc_file="${HOME}/.zshrc"
            ;;
        fish)
            rc_file="${HOME}/.config/fish/config.fish"
            ;;
        *)
            rc_file="your shell's rc file"
            ;;
    esac

    warn "${install_dir} is not in your PATH"
    echo ""
    echo "Add it by running:"
    echo ""
    if [ "$shell_name" = "fish" ]; then
        printf "  ${BOLD}fish_add_path %s${NC}\n" "$install_dir"
    else
        printf "  ${BOLD}echo 'export PATH=\"%s:\$PATH\"' >> %s${NC}\n" "$install_dir" "$rc_file"
    fi
    echo ""
    echo "Then restart your shell or run:"
    if [ "$shell_name" = "fish" ]; then
        printf "  ${BOLD}source %s${NC}\n" "$rc_file"
    else
        printf "  ${BOLD}source %s${NC}\n" "$rc_file"
    fi
}

# Main installation
main() {
    echo ""
    printf "${BOLD}crt installer${NC}\n"
    echo "============="
    echo ""

    check_dependencies

    local os=$(detect_os)
    local arch=$(detect_arch)
    local install_dir="${CRT_INSTALL_DIR:-${HOME}/.local/bin}"
    local version="${CRT_VERSION:-$(get_latest_version)}"

    if [ -z "$version" ]; then
        error "Could not determine latest version. Set CRT_VERSION manually."
    fi

    info "Installing crt v${version}"
    info "Platform: ${os}-${arch}"
    info "Install directory: ${install_dir}"
    echo ""

    # Create temp directory
    local tmp_dir=$(mktemp -d)
    trap "rm -rf '$tmp_dir'" EXIT

    # Download and extract
    download_release "$version" "$os" "$arch" "$tmp_dir"

    # Install binary
    install_binary "$tmp_dir" "$install_dir" "$os"

    # Set up config
    setup_config "$tmp_dir" "$os"

    echo ""
    success "crt v${version} installed successfully!"
    echo ""

    if [ "$os" = "macos" ]; then
        echo "  App installed to: /Applications/crt.app"
        echo "  CLI installed to: ${install_dir}/crt"
        echo "  Config directory: ~/.config/crt/"
        echo ""
        echo "Launch from Spotlight or Finder, or run 'crt' from terminal."
    else
        echo "  Binary: ${install_dir}/crt"
        echo "  Config: ~/.config/crt/"
    fi
    echo ""

    # Check PATH
    if ! check_path "$install_dir"; then
        suggest_path "$install_dir"
    fi
}

main "$@"
