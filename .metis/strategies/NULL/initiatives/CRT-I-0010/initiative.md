---
id: core-terminal-features
level: initiative
title: "Core Terminal Features"
short_code: "CRT-I-0010"
created_at: 2025-11-26T21:25:00.228160+00:00
updated_at: 2025-11-26T21:41:32.246893+00:00
parent: CRT-V-0001
blocked_by: []
archived: false

tags:
  - "#initiative"
  - "#phase/decompose"


exit_criteria_met: false
estimated_complexity: L
strategy_id: NULL
initiative_id: core-terminal-features
---

# Core Terminal Features Initiative

*This template includes sections for various types of initiatives. Delete sections that don't apply to your specific use case.*

## Context

The terminal has basic functionality but lacks essential features that users expect from a daily-driver terminal emulator. These are blocking issues that prevent using CRT as a primary terminal.

## Goals & Non-Goals

**Goals:**
- Scrollback buffer with mouse wheel and keyboard navigation
- Paste support (Cmd+V) from system clipboard
- Bold/italic/underline text rendering from ANSI SGR codes
- Full 24-bit true color support (wire up existing Spec variant)
- Mouse reporting protocol for vim/tmux/htop interaction
- Alternate screen buffer for full-screen TUI apps (vim, less, htop)

**Non-Goals:**
- Infinite scrollback (reasonable default limit of 10k lines)
- Sixel/image protocol support (future initiative)
- OSC 52 clipboard integration (future)

## Detailed Design

### Scrollback Buffer
- alacritty_terminal already supports scrollback via `history_size` in config
- Need to wire up mouse wheel events to scroll viewport
- Add Shift+PageUp/PageDown keyboard shortcuts
- Scrollbar rendering optional (can add later)

### Paste Support
- Cmd+V reads from system clipboard (pbpaste on macOS)
- Send clipboard content to PTY as if typed
- Handle bracketed paste mode for proper paste in vim/zsh

### Text Styles (Bold/Italic/Underline)
- alacritty_terminal Cell has Flags for BOLD, ITALIC, UNDERLINE, etc.
- Load bold/italic font variants or synthesize via glyph transforms
- Underline: render line below baseline via vello

### True Color (24-bit)
- AnsiColor::Spec variant already parsed
- Wire Spec(Rgb) through to renderer (currently only Named/Indexed)

### Mouse Reporting
- alacritty_terminal handles escape sequence generation
- Need to forward mouse events (press, release, move, scroll) to terminal
- Support SGR mouse mode (modern) and legacy X10/X11 modes

### Alternate Screen Buffer
- alacritty_terminal handles this internally
- May need to clear selection when switching buffers
- Ensure scrollback is preserved on primary screen

## Implementation Plan

### Phase 1: Scrollback and Navigation
- Wire mouse wheel to terminal scroll
- Add keyboard shortcuts (Shift+PgUp/PgDn)
- Test with long command output

### Phase 2: Clipboard Integration  
- Implement Cmd+V paste
- Handle bracketed paste mode
- Test with vim, zsh paste

### Phase 3: Text Styles
- Read cell flags for bold/italic/underline
- Render underline via vello
- Bold: increase font weight or synthesize

### Phase 4: True Color and Mouse
- Wire Spec color variant to renderer
- Forward mouse events to terminal
- Test with vim, htop, tmux