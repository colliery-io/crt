# How to Profile Performance

CRT Terminal includes a built-in profiler that records frame timing, memory usage, and terminal state. Use it to diagnose performance issues or collect data for bug reports.

## Enable Profiling

Three ways to start profiling:

### 1. Environment Variable (Startup)

```bash
CRT_PROFILE=1 crt
```

On macOS with the app bundle:

```bash
CRT_PROFILE=1 /Applications/crt.app/Contents/MacOS/crt
```

This also enables debug-level logging automatically.

### 2. Keyboard Shortcut (Runtime)

Press **Cmd+Option+P** to toggle profiling on or off while CRT is running. A toast notification confirms the state change and shows the log file path.

### 3. Menu (Runtime)

**View > Start Profiling** / **View > Stop Profiling** (macOS only).

## Profile Output

Profile data is written to:

```
~/.config/crt/profile-{unix_timestamp}.log
```

Example: `~/.config/crt/profile-1711234567.log`

The profiler also prints the path to stderr when starting and stopping.

## Reading the Profile Log

### System Info Header

```
=== CRT Terminal Profile Log ===
Version: 0.1.0
OS: macos aarch64
macOS: 15.3
Initial RSS: 45000 KB
=== Begin Profiling ===
```

### Frame Timing

Periodic summaries appear every 300 frames (~5 seconds at 60fps):

```
STATS: frames=300 avg=2.45ms p99=8.12ms fps=408.2
MEMORY: rss=87.5MB
```

- **avg**: Average frame time in milliseconds
- **p99**: 99th percentile frame time (worst 1% of frames)
- **fps**: Estimated frames per second

### Slow Frame Alerts

Any frame exceeding 16ms (below 60fps) is logged:

```
SLOW FRAME: total=24.50ms update=1.20ms render=18.30ms present=3.00ms effects=2.00ms
```

The breakdown shows where time was spent:
- **update**: Processing PTY output, terminal state updates
- **render**: Text rendering, glyph shaping, buffer uploads
- **present**: GPU presentation (swap chain)
- **effects**: Backdrop effects (grid, starfield, particles, etc.)

### Grid Snapshots

Every 5 seconds, the profiler captures a terminal state snapshot:

```
=== Grid Snapshot #1 ===
Grid: 80x24 cursor: (0,23) visible: true shape: Block offset: 0 history: 150
Content:
    0| $ ls -la
    1| total 42
    ...
=== End Snapshot ===
```

### Session Summary

When profiling stops, a complete summary is written:

```
=== Profile Summary ===
Session duration: 45.2s
Total frames: 2712
Frame timing: avg=2.45ms min=0.80ms max=28.50ms p50=1.90ms p99=8.12ms fps=408.2
Breakdown:
  Update:  avg=0.35ms p99=2.10ms
  Render:  avg=1.20ms p99=5.50ms
  Present: avg=0.60ms p99=3.20ms
  Effects: avg=0.30ms p99=1.50ms
Memory:
  Start:  45.0MB
  End:    87.5MB
  Peak:   92.0MB
  Growth: +42.5MB
Events logged: 15
Grid snapshots: 9
```

## Interpreting Results

### Healthy Performance

- Average frame time under 4ms
- P99 under 16ms
- No slow frame alerts
- Memory growth stabilizes after startup

### Common Issues

**High render time**: Text re-rendering is expensive. Check if content is changing every frame (e.g., continuous output from `yes` or `cat /dev/urandom`). This is expected behavior under heavy output.

**High effects time**: Multiple backdrop effects compound. The `stress` theme enables everything simultaneously. Try disabling effects to isolate: set `--grid-enabled: false`, `--starfield-enabled: false`, etc. in your theme.

**Growing memory**: Some memory growth is normal (glyph cache, scrollback history). If it grows continuously, note the rate and include the profile in a bug report.

**High present time**: GPU is bottlenecked. Can happen on integrated GPUs with high-resolution displays and multiple effects.

## Benchmark Binaries

CRT includes standalone benchmarks for targeted testing:

### CPU Benchmark

Tests terminal processing throughput without GPU rendering:

```bash
cargo run --release --bin benchmark
```

Measures: `process_input` (ANSI parsing), `all_lines_text` (text extraction), `content_hash` (change detection), `damage_check` (dirty tracking).

### GPU Benchmark

Tests GPU rendering with a live window:

```bash
cargo run --release --bin benchmark_gpu
```

Opens a terminal window and logs frame timing to stderr.

### Memory Profiler

Monitors memory allocation patterns:

```bash
cargo run --release --bin profile_memory
```

### GPU Memory Profiler

Tracks GPU resource allocation:

```bash
cargo run --release --bin profile_gpu_memory
```

## Benchmark Script

The `scripts/benchmark.sh` script provides convenient modes:

```bash
./scripts/benchmark.sh quick    # CPU-only benchmark (no window)
./scripts/benchmark.sh gpu      # GPU benchmark (opens window)
./scripts/benchmark.sh memory   # Monitor RSS over time
./scripts/benchmark.sh stress   # Automated stress test (macOS)
./scripts/benchmark.sh all      # Run all benchmarks
```

## Verbose Logging

For detailed debug output without the profiler overhead:

```bash
RUST_LOG=debug crt
```

Or target specific crates:

```bash
RUST_LOG="warn,crt=debug,crt_renderer=debug" crt
```

Logs go to stderr (the terminal where you launched CRT, not inside CRT).

## Sharing Profile Data

When filing a bug report:

1. Enable profiling: `CRT_PROFILE=1 crt`
2. Reproduce the issue
3. Quit CRT (Cmd+Q)
4. Attach the profile log from `~/.config/crt/profile-*.log`
5. Include your `config.toml` and the theme CSS you were using

## See Also

- [Environment Variables Reference](../reference/environment-variables.md) for `CRT_PROFILE` and `RUST_LOG`
- [Build from Source](build-from-source.md) for running benchmarks
- [Troubleshooting](../troubleshooting.md) for common performance issues
