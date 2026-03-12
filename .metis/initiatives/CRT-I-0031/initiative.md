---
id: performance-and-memory-optimization
level: initiative
title: "Performance and Memory Optimization"
short_code: "CRT-I-0031"
created_at: 2026-03-11T14:33:12.478246+00:00
updated_at: 2026-03-11T21:17:31.656294+00:00
parent: CRT-V-0001
blocked_by: []
archived: false

tags:
  - "#initiative"
  - "#phase/decompose"


exit_criteria_met: false
estimated_complexity: L
initiative_id: performance-and-memory-optimization
---

# Performance and Memory Optimization Initiative

## Context

CRT's performance is production-ready overall, with several smart optimizations already in place:

- **Frame throttling** at 60fps (`main.rs:1537-1583`) prevents the Metal/wgpu IOAccelerator memory leak that otherwise causes unbounded allocation growth
- **Content hashing** (`window.rs:1170-1214`) skips rendering when terminal content hasn't changed, achieving <1% CPU at idle
- **Glyph caching** (`glyph_cache.rs`) renders each character once and reuses from a 1024x1024 atlas
- **Damage tracking** via alacritty_terminal's `TermDamage` detects which lines changed

However, profiling analysis identified specific optimization opportunities, primarily on the CPU side:

| Issue | Location | Estimated Impact | Effort |
|-------|----------|-----------------|--------|
| PTY read allocates new Vec per read | `pty.rs:147-164` | 5-15% CPU on scrolling | Medium |
| `all_lines_text()` double-allocates | `lib.rs:481-503` | 10-20% for search/log | Low |
| Event queue uses Mutex | `lib.rs:70-115` | 1-2% CPU + latency | Medium |
| Buffer pool API exists but unused | `buffer_pool.rs` | ~2MB/window savings | Medium |
| Damage tracking not used for partial updates | `render/mod.rs` | 10-30% GPU reduction | High |
| Glyph atlas fixed at 1024x1024 | `glyph_cache.rs:174` | Risk for Unicode-heavy | Medium |
| Debug logging allocates per-byte | `lib.rs:562-586` | Minor (debug only) | Low |

**Key insight:** CPU is the bottleneck, not GPU. PTY I/O, terminal parsing, and text buffer updates drive the load. The GPU is waiting most of the time.

## Goals & Non-Goals

**Goals:**
- Reduce per-frame CPU allocations in PTY I/O and text processing hot paths
- Integrate the existing buffer pool infrastructure into the rendering pipeline
- Leverage damage tracking for partial grid updates (only re-render changed lines)
- Establish performance baselines and regression detection via integrated benchmarks in CI
- Add glyph atlas capacity monitoring and graceful handling when full

**Non-Goals:**
- Rewriting the rendering pipeline (architectural changes are CRT-I-0028/CRT-I-0030)
- Optimizing cold paths (config loading, theme parsing, startup)
- GPU shader optimization (current shaders are simple and fast)
- Multi-GPU support
- Reducing idle memory footprint below current ~150MB (most is GPU texture allocation, inherent to wgpu)

## Detailed Design

### 1. PTY Buffer Reuse (High Priority)

**Problem:** Every PTY read creates a new `Vec<u8>` via `buf[..n].to_vec()` (`pty.rs:153, 260`). At typical shell output rates, this means hundreds of allocator calls per frame during scrolling output.

**Solution:** Pool of pre-allocated buffers:

```rust
struct BufferPool {
    buffers: Vec<Vec<u8>>,  // Pre-allocated with capacity 4096
}

impl BufferPool {
    fn checkout(&mut self) -> Vec<u8> {
        self.buffers.pop().unwrap_or_else(|| Vec::with_capacity(4096))
    }
    fn return_buf(&mut self, mut buf: Vec<u8>) {
        buf.clear();
        if self.buffers.len() < 16 { self.buffers.push(buf); }
    }
}
```

Alternative: Use a ring buffer with `Arc<[u8]>` slices to avoid copies entirely. Needs benchmarking to compare approaches.

**Measurement:** Before/after benchmark with `scrolling_output` scenario from `src/bin/benchmark.rs`. Target: measurable reduction in allocator calls per frame.

### 2. String Allocation Fix in `all_lines_text()` (High Priority, Low Effort)

**Problem:** Double allocation per line — `collect()` creates a String, then `trim_end().to_string()` creates another copy.

**Fix:**
```rust
pub fn all_lines_text(&self) -> Vec<(i32, String)> {
    let mut lines = Vec::with_capacity(total_lines);
    for row in grid_rows {
        let mut text = String::with_capacity(self.columns());
        for cell in row { text.push(cell.c); }
        let trimmed_len = text.trim_end().len();
        text.truncate(trimmed_len);  // In-place, no allocation
        lines.push((line_idx, text));
    }
    lines
}
```

### 3. Lock-Free Event Queue (Medium Priority)

**Problem:** `TerminalEventProxy` uses `Arc<Mutex<EventStorage>>` for event passing (`lib.rs:70-115`). While events are infrequent, the Mutex adds latency on the rendering hot path since `take_events()` is called every frame.

**Solution:** Replace with `crossbeam::queue::SegQueue` (lock-free MPSC queue):

```rust
pub struct TerminalEventProxy {
    events: Arc<SegQueue<Event>>,
}

impl EventListener for TerminalEventProxy {
    fn send_event(&self, event: Event) {
        self.events.push(event);  // Lock-free
    }
}

pub fn take_events(&self) -> Vec<Event> {
    let mut events = Vec::new();
    while let Some(event) = self.events.pop() {
        events.push(event);
    }
    events
}
```

Adds `crossbeam` dependency (commonly used in Rust ecosystem, minimal overhead).

### 4. Buffer Pool Integration (Medium Priority)

**Problem:** `BufferPool` and `TexturePool` in `src/gpu/` are fully implemented with RAII patterns, statistics tracking, and proper alignment — but not wired into `WindowGpuState`. GPU buffers are created directly instead.

**Solution:** Wire `BufferPool` into `WindowGpuState` and use `PooledBuffer` for:
- Grid instance buffer (32K instances x 48 bytes = 1.5MB)
- Rect instance buffer (16K instances x 32 bytes = 512KB)
- Small uniform buffers (256 bytes)

This saves ~2MB of re-allocation per window lifecycle (create, resize, close).

### 5. Partial Grid Rendering via Damage Tracking (High Impact, High Effort)

**Problem:** When a single character is typed, the entire visible grid is re-rendered even though only 1-2 lines changed. `has_damage()` is used as a boolean dirty flag, but the damage _bounds_ (which specific lines changed) are discarded.

**Solution:** Use `TermDamage` line-level information to only update changed grid rows:

1. Query damage for specific line ranges from alacritty_terminal
2. Only rebuild cell data for damaged lines
3. Keep previous frame's cell data for undamaged lines in a retained buffer
4. Submit partial updates to the grid renderer

This requires changes in both `crates/crt-core/src/lib.rs` (expose damage bounds) and the grid rendering path. The content hashing already provides a coarse skip, so the benefit is primarily when the terminal is partially updating (typing, single-line output).

**Estimated impact:** 10-30% GPU work reduction for interactive use (typing, prompt rendering). Less impact during full-screen scrolling (everything is damaged anyway).

### 6. Glyph Atlas Monitoring and Overflow (Low Priority)

**Problem:** Atlas is fixed at 1024x1024 (1MB). When full, `allocate()` returns `None` and glyphs silently fail to render. No warning, no recovery.

**Solution:**
- Log warning at 80% capacity with stats (unique glyphs cached, atlas utilization)
- When full, implement LRU eviction: clear least-recently-used glyphs and re-pack atlas
- Consider dynamic atlas growth (1024 → 2048) as alternative to eviction

### 7. Benchmark Integration in CI

**Current:** `src/bin/benchmark.rs` measures frame timing, FPS, and memory but is a manual tool run via `cargo run --release --bin benchmark`.

**Solution:**
- Integrate `criterion` benchmarks (already in `dev-dependencies`) for:
  - Terminal text processing throughput (bytes/second)
  - Glyph cache lookup performance
  - Content hash computation
  - Theme parsing
- Run benchmarks in CI with regression detection (criterion's built-in comparison)
- Keep existing `benchmark.rs` for manual end-to-end profiling

## Testing Strategy

Each optimization must be validated with before/after measurements:

- **PTY buffer reuse:** `scrolling_output` benchmark scenario — measure allocations per frame
- **String fix:** Micro-benchmark `all_lines_text()` on a 10K-line scrollback buffer
- **Lock-free queue:** Latency measurement — time from event emission to consumption
- **Buffer pool:** Memory RSS measurement during window lifecycle
- **Partial rendering:** Frame time measurement during interactive typing vs current
- **Glyph atlas:** Test with Unicode-heavy content (CJK + emoji), verify graceful handling at capacity

All existing tests must pass unchanged after each optimization.

## Alternatives Considered

**Custom allocator (jemalloc/mimalloc):** Considered as a blanket fix for allocation overhead. Rejected — targeted buffer reuse is more effective and doesn't add a global dependency. Can revisit if profiling shows general allocator pressure.

**Async PTY I/O (tokio):** Replacing threads with async would reduce thread overhead but adds significant complexity. The current thread-per-PTY model is simple and well-isolated. Not worth the architectural cost.

**GPU compute for text processing:** Using compute shaders for terminal grid processing was considered but rejected — the bottleneck is CPU-side parsing and state management, not data transfer to GPU.

## Implementation Plan

**Phase 1 — Quick wins (low effort, measurable impact):**
- String allocation fix in `all_lines_text()`
- Debug logging optimization
- Glyph cache capacity logging

**Phase 2 — Core optimizations:**
- PTY buffer reuse implementation
- Lock-free event queue migration
- Buffer pool integration

**Phase 3 — Advanced:**
- Partial grid rendering via damage tracking
- Glyph atlas overflow handling (LRU eviction or growth)
- Criterion benchmark suite + CI integration

**Measurement:** Each phase starts with baseline measurements using `src/bin/benchmark.rs` and ends with comparison. No optimization is merged without demonstrated improvement.