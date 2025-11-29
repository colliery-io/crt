---
id: medium-priority-memory
level: initiative
title: "Medium Priority Memory Optimizations"
short_code: "CRT-I-0018"
created_at: 2025-11-29T03:03:19.150113+00:00
updated_at: 2025-11-29T03:03:19.150113+00:00
parent: CRT-V-0001
blocked_by: []
archived: false

tags:
  - "#initiative"
  - "#phase/discovery"


exit_criteria_met: false
estimated_complexity: M
strategy_id: NULL
initiative_id: medium-priority-memory
---

# Medium Priority Memory Optimizations Initiative

## Context

Following the high-priority memory optimizations, these are additional improvements that affect less frequent code paths or have lower per-occurrence impact. These optimizations will improve long-running session stability and reduce memory footprint for power users.

## Goals & Non-Goals

**Goals:**
- Limit profiling snapshot memory consumption
- Share font data across windows instead of cloning
- Reduce effect configuration string allocations

**Non-Goals:**
- Per-frame hot path optimizations (covered in CRT-I-0017)
- GPU memory management (already well-optimized)
- Startup time optimizations

## Optimization Targets

### 1. Profiling Snapshot Memory

**Location:** `src/render.rs:692-698`

**Current Problem:**
```rust
let all_lines = terminal.all_lines_text();  // Allocates ENTIRE history as strings
let visible_content: Vec<String> = all_lines
    .into_iter()
    .filter(|(idx, _)| *idx >= 0)
    .map(|(_, text)| text)
    .collect();
```

**Impact:** Megabytes when profiling enabled with large scrollback

**Solution:**
- Only snapshot visible lines (already filtered, but allocation happens first)
- Implement streaming iterator that only allocates visible lines
- Add configurable limit (e.g., last 1000 lines max)
- Consider sampling for very long histories

### 2. ~~Glyph Cache Unbounded Growth~~ (NOT AN ISSUE)

**Location:** `crates/crt-renderer/src/glyph_cache.rs:191`

**Analysis:** After review, this is NOT a problem. The cache is naturally bounded by:
- Fixed 1024x1024 texture atlas - `AtlasPacker::allocate()` returns `None` when full
- Font size changes call `glyphs.clear()` and reset the packer
- At ~14px font, atlas fits ~3000-4000 glyphs max
- Typical terminal usage is <500 unique characters

**Conclusion:** No action needed - atlas size is the natural LRU equivalent

### 3. Font Variants Cloning

**Location:** `src/main.rs:167`

**Current Problem:**
```rust
font_variants.clone()  // Clones 4 font buffers (~500KB total)
```

**Impact:** ~500KB per window creation

**Solution:**
- Wrap FontVariants in `Arc<FontVariants>`
- Share single font instance across all windows
- Only clone the Arc, not the underlying data
- Font data is immutable after load, safe to share

### 4. Effect Configuration String Allocations

**Location:** `src/main.rs:769-907`

**Current Problem:**
```rust
config.insert("grid-color", format!(
    "rgba({}, {}, {}, {})",
    (c.r * 255.0) as u8,
    (c.g * 255.0) as u8,
    (c.b * 255.0) as u8,
    c.a
));  // Many format!() calls creating strings
```

**Impact:** Kilobytes on theme load (infrequent)

**Solution:**
- Pass color values directly as f32 tuples instead of formatting to strings
- Define typed configuration struct instead of HashMap<String, String>
- Parse CSS once and cache parsed values
- Only re-parse on theme file change

## Testing Strategy

### Memory Profiling
- Use `heaptrack` or `dhat` to measure allocation patterns
- Run extended sessions (1+ hour) to verify glyph cache bounds
- Test multi-window scenarios for font sharing

### Validation
- Verify profiling output still works correctly
- Ensure glyph rendering quality unchanged after cache eviction
- Test font rendering across multiple windows

## Implementation Plan

1. **Font sharing with Arc** - Simple refactor, good starting point
2. **Profiling snapshot limits** - Protects against worst-case memory
3. **Effect config typing** - Larger refactor, lower priority