---
id: terminal-polish-and-ux
level: initiative
title: "Terminal Polish and UX"
short_code: "CRT-I-0011"
created_at: 2025-11-26T21:25:00.335016+00:00
updated_at: 2025-11-27T02:48:24.207484+00:00
parent: CRT-V-0001
blocked_by: []
archived: true

tags:
  - "#initiative"
  - "#phase/completed"


exit_criteria_met: false
estimated_complexity: M
strategy_id: NULL
initiative_id: terminal-polish-and-ux
---

# Terminal Polish and UX Initiative

*This template includes sections for various types of initiatives. Delete sections that don't apply to your specific use case.*

## Context

With core features in place, these enhancements improve the daily usability and polish of the terminal. These are quality-of-life features that make the terminal feel complete and professional.

## Goals & Non-Goals

**Goals:**
- URL detection with clickable links (Cmd+click)
- Search in terminal output (Cmd+F with highlight)
- Font configuration via config.toml (size, family, weight)
- Terminal bell support (visual flash and/or system sound)
- Right-click context menu (copy, paste, search selection)
- CSS background images and animated GIFs

**Non-Goals:**
- Browser-style URL preview tooltip
- Regex-based custom link patterns (future)
- Multiple font fallback chains (future)
- Sophisticated search with regex (basic substring first)

## Detailed Design

### URL Detection
- Scan terminal content for URL patterns (http://, https://, file://)
- Highlight URLs with underline on hover
- Cmd+click opens URL in default browser
- Store URL positions in render metadata

### Search (Cmd+F)
- Overlay search bar at top or bottom of terminal
- Highlight all matches in terminal content
- Navigate between matches with Enter/Shift+Enter
- Close with Escape
- Search in scrollback buffer too

### Font Configuration
- Add to config.toml: font.family, font.size, font.weight
- Support system fonts and bundled fonts
- Reload font on config change
- Validate font exists, fallback to bundled

### Terminal Bell
- Listen for BEL character (0x07) from terminal
- Visual: brief screen flash or border highlight
- Audio: system beep (optional, configurable)
- Config: bell.visual, bell.audio, bell.enabled

### Right-Click Context Menu
- Native context menu via winit/muda
- Options: Copy, Paste, Select All, Search Selection
- If URL under cursor: Open Link option
- Respect current selection state

### CSS Background Images
- Parse `background-image: url(...)` in CSS
- Support static images (PNG, JPG) and animated GIFs
- Load image via `image` crate, upload to GPU texture
- GIF animation: decode frames, track timing, update texture
- Blend with existing background shader (image behind gradient)
- CSS properties: background-size, background-position, background-repeat

## Implementation Plan

### Phase 1: URL Detection and Click
- Add URL regex scanner
- Track URL positions during render
- Handle Cmd+click to open browser

### Phase 2: Search Overlay
- Create search UI component
- Implement search through terminal content
- Add match highlighting

### Phase 3: Font Configuration
- Add font config to config.toml
- Load fonts dynamically
- Handle missing fonts gracefully

### Phase 4: Bell and Context Menu
- Implement bell handling
- Create context menu
- Wire up menu actions