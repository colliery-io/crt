# How to Build from Source

Compile CRT Terminal from source for development, customization, or contributing.

## Prerequisites

### Rust

CRT requires the Rust 2024 edition. Install via [rustup](https://rustup.rs/):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Verify your version supports edition 2024:

```bash
rustc --version
# Needs rustc 1.85.0 or later
```

### Platform Dependencies

**macOS**: No additional system dependencies. Xcode command-line tools are required (installed with Rust).

**Linux (Ubuntu/Debian)**:

```bash
sudo apt-get update
sudo apt-get install -y libglib2.0-dev libgtk-3-dev libxkbcommon-dev
```

**Linux (Fedora)**:

```bash
sudo dnf install glib2-devel gtk3-devel libxkbcommon-devel
```

## Clone and Build

```bash
git clone https://github.com/colliery-io/crt.git
cd crt
```

### Debug Build (Fast Compilation, Slow Runtime)

```bash
cargo build
```

Binary at `target/debug/crt`.

### Release Build (Slow Compilation, Fast Runtime)

```bash
cargo build --release
```

Binary at `target/release/crt`.

### Run Directly

```bash
cargo run              # Debug build
cargo run --release    # Release build
```

## Install Config Files

After building, install the default themes and config to `~/.config/crt/`:

```bash
./scripts/dev.sh install
```

Or manually:

```bash
mkdir -p ~/.config/crt/themes ~/.config/crt/fonts
cp assets/themes/*.css ~/.config/crt/themes/
cp assets/config.toml ~/.config/crt/config.toml
```

Also copy theme asset directories (sprites, images):

```bash
for dir in assets/themes/*/; do
    cp -R "$dir" ~/.config/crt/themes/
done
```

## Dev Script

The `scripts/dev.sh` helper provides common commands:

| Command | Description |
|---------|-------------|
| `./scripts/dev.sh build` | Debug build |
| `./scripts/dev.sh release` | Release build |
| `./scripts/dev.sh run` | Build and run (debug) |
| `./scripts/dev.sh test` | Run all tests |
| `./scripts/dev.sh check` | Cargo check (fast type checking) |
| `./scripts/dev.sh clippy` | Run clippy lints |
| `./scripts/dev.sh install` | Install config files to ~/.config/crt |
| `./scripts/dev.sh clean` | Clean build artifacts |

## Running Tests

### All Tests

```bash
cargo test --all-targets
```

This runs unit tests across all crates, integration tests (shell tests, terminal tests), and visual/golden image tests.

### Specific Test Suites

```bash
cargo test --test shell_tests      # Shell integration tests (spawns real PTY)
cargo test --test terminal_tests   # Terminal emulation tests
cargo test --test visual_tests     # Visual golden image tests (macOS only)
```

Visual tests compare rendered output against golden images stored in `tests/visual/golden/`. These golden images were generated on macOS, so visual tests are skipped on Linux CI.

### Single Crate Tests

```bash
cargo test -p crt-core       # Core terminal tests
cargo test -p crt-renderer   # Renderer tests
cargo test -p crt-theme      # Theme parser tests
```

## Project Structure

```
crt/
├── src/                    # Main binary
│   ├── main.rs             # Entry point, event loop
│   ├── app/                # Application state, lifecycle
│   ├── config.rs           # Config loading (TOML)
│   ├── font.rs             # Font loading
│   ├── gpu/                # GPU resource management
│   ├── input/              # Keyboard and mouse handling
│   ├── menu.rs             # macOS menu bar
│   ├── render/             # UI overlays, context menu
│   ├── window/             # Per-window state
│   ├── watcher.rs          # File change watching
│   ├── theme_registry.rs   # Runtime theme management
│   └── profiling.rs        # Built-in profiler
├── crates/
│   ├── crt-core/           # Terminal emulation (alacritty_terminal wrapper)
│   ├── crt-renderer/       # GPU rendering (wgpu, vello, effects)
│   └── crt-theme/          # CSS theme parser (lightningcss)
├── assets/
│   ├── themes/             # Built-in theme CSS files
│   ├── fonts/              # Bundled fonts
│   └── config.toml         # Default config
├── tests/                  # Integration tests
├── scripts/                # Dev and release scripts
└── installer/              # Platform installers
```

## Creating a macOS App Bundle

```bash
./installer/macos/build-app.sh
```

This creates `target/release/crt.app` with the binary, assets, and Info.plist. The app bundle can be copied to `/Applications/`.

To ad-hoc sign the bundle (required after replacing binaries):

```bash
codesign --force --deep --sign - target/release/crt.app
```

## Creating a Release Artifact

```bash
./scripts/release.sh
```

This builds a release binary and packages it:
- **macOS**: `target/release/dist/crt-{version}-macos-{arch}.tar.gz` containing `crt.app`
- **Linux**: `target/release/dist/crt-{version}-linux-{arch}.tar.gz` containing the binary and assets

## Running Benchmarks

### Criterion Benchmarks

```bash
cargo bench
```

Runs criterion benchmarks for terminal processing (`crates/crt-core/benches/terminal.rs`) and theme parsing (`crates/crt-theme/benches/theme_parsing.rs`). HTML reports are generated in `target/criterion/`.

### Application Benchmarks

```bash
cargo run --release --bin benchmark          # CPU-side throughput
cargo run --release --bin benchmark_gpu      # GPU rendering (opens window)
cargo run --release --bin profile_memory     # Memory profiling
cargo run --release --bin profile_gpu_memory # GPU memory profiling
```

### Benchmark Script

```bash
./scripts/benchmark.sh quick    # CPU benchmark only
./scripts/benchmark.sh gpu      # GPU benchmark (opens window)
./scripts/benchmark.sh memory   # RSS monitoring
./scripts/benchmark.sh all      # Everything
```

## CI Pipeline

The GitHub Actions CI (`.github/workflows/test.yml`) runs:

1. **test**: `cargo build --all-targets` and `cargo test --all-targets` on both macOS and Ubuntu
2. **visual-tests**: `cargo test --test visual_tests` on macOS only (golden images are macOS-specific)
3. **coverage**: `cargo llvm-cov` on Ubuntu, uploads lcov report

## Contributing Workflow

1. Fork the repository
2. Create a feature branch
3. Make changes
4. Run tests: `cargo test --all-targets`
5. Run lints: `cargo clippy -- -W clippy::all`
6. Open a pull request against `main`

## Heap Profiling with dhat

CRT supports heap profiling via the `dhat-heap` feature:

```bash
cargo run --release --features dhat-heap
```

This writes a `dhat-heap.json` file that can be viewed with the [dhat viewer](https://nnethercote.github.io/dh_view/dh_view.html).

## See Also

- [Architecture Overview](../explanation/architecture.md) for understanding the codebase
- [How to Profile Performance](profile-performance.md) for runtime profiling
- [Configuration Reference](../reference/configuration.md) for config options
