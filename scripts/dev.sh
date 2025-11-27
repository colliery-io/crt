#!/bin/bash
# Development helper script for crt
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

usage() {
    echo "Usage: $0 <command>"
    echo ""
    echo "Commands:"
    echo "  build     Build debug binary"
    echo "  release   Build release binary"
    echo "  run       Build and run debug binary"
    echo "  test      Run tests"
    echo "  check     Run cargo check"
    echo "  clippy    Run clippy lints"
    echo "  install   Install config files to ~/.config/crt"
    echo "  clean     Clean build artifacts"
    echo ""
}

cd "$PROJECT_ROOT"

case "${1:-}" in
    build)
        cargo build
        ;;
    release)
        cargo build --release
        ;;
    run)
        cargo run
        ;;
    test)
        cargo test
        ;;
    check)
        cargo check
        ;;
    clippy)
        cargo clippy -- -W clippy::all
        ;;
    install)
        "$PROJECT_ROOT/installer/macos/install-config.sh"
        ;;
    clean)
        cargo clean
        ;;
    *)
        usage
        exit 1
        ;;
esac
