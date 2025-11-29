---
id: high-priority-memory-optimizations
level: initiative
title: "High Priority Memory Optimizations"
short_code: "CRT-I-0017"
created_at: 2025-11-29T03:03:19.080501+00:00
updated_at: 2025-11-29T03:03:19.080501+00:00
parent: CRT-V-0001
blocked_by: []
archived: false

tags:
  - "#initiative"
  - "#phase/discovery"


exit_criteria_met: false
estimated_complexity: M
strategy_id: NULL
initiative_id: high-priority-memory-optimizations
---

# High Priority Memory Optimizations Initiative

## Context

Performance analysis of CRT identified several memory allocation hotspots that occur in hot paths (per-frame or per-keystroke). These allocations contribute to memory pressure and potential GC pauses. The codebase already has critical protections in place (frame throttling, vello reset), but these remaining issues affect steady-state performance.

## Goals & Non-Goals

**Goals:**
- Eliminate per-keystroke string allocations in search functionality
- Remove per-frame allocations in URL detection
- Avoid unnecessary vector cloning in decoration rendering
- Reduce memory churn during normal terminal operation

**Non-Goals:**
- Architectural changes to rendering pipeline
- GPU memory optimizations (already well-handled)
- One-time startup allocations

## Optimization Targets

### 1. Search String Allocations (Highest Impact)

**Location:** `src/main.rs:1570-1577`

**Current Problem:**
```rust
let all_lines = terminal.all_lines_text();  // Allocates Vec of Strings
let query_lower = query.to_lowercase();     // String allocation
for (line_idx, line_text) in &all_lines {
    let line_lower = line_text.to_lowercase();  // String allocation PER LINE
    // ...
}
```

**Impact:** 100s of KB per search keystroke with large scrollback history

**Solution:**
- Pre-lowercase query once (already done)
- Use byte-level case-insensitive comparison without allocating per-line copies
- Consider `str::to_ascii_lowercase()` for ASCII-only fast path
- Alternative: Use `unicase` crate for zero-alloc case-insensitive comparison

### 2. URL Detection Allocation

**Location:** `src/window.rs:466-478`

**Current Problem:**
```rust
let mut line_texts: std::collections::BTreeMap<i32, String> = BTreeMap::new();
for cell in content.display_iter {
    line_texts.entry(viewport_line).or_default().push(cell.c);
}
self.detected_urls.clear();
for (viewport_line, line_text) in &line_texts {
    let urls = detect_urls_in_line(line_text, *viewport_line as usize);
    self.detected_urls.extend(urls);
}
```

**Impact:** ~1KB per frame for 80x24 terminal

**Solution:**
- Only re-detect URLs when viewport content actually changes (use content hash)
- Reuse line buffer across frames instead of creating new BTreeMap
- Consider inline detection without intermediate collection

### 3. Decoration Vector Cloning

**Location:** `src/render.rs:308`

**Current Problem:**
```rust
result.decorations.clone()  // Deep copies entire decoration vector
```

**Impact:** Kilobytes per frame depending on terminal content

**Solution:**
- Use `std::mem::take()` or `std::mem::swap()` to move instead of clone
- If shared ownership needed, use `Arc<Vec<TextDecoration>>`
- Consider making decorations Copy if small enough

## Testing Strategy

### Benchmarking
- Use existing `src/bin/benchmark.rs` to measure RSS before/after
- Add microbenchmarks for search and URL detection functions
- Measure allocation counts with `dhat` or similar profiler

### Validation
- Ensure search functionality works correctly after optimization
- Verify URL detection still catches all URLs
- Check decoration rendering is visually identical

## Implementation Plan

1. **Search optimization** - Highest impact, implement first
2. **URL detection caching** - Add dirty flag based on content hash
3. **Decoration move semantics** - Quick win with std::mem::take