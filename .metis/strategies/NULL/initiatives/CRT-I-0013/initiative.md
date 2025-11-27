---
id: osc-133-semantic-prompt-support
level: initiative
title: "OSC 133 Semantic Prompt Support"
short_code: "CRT-I-0013"
created_at: 2025-11-27T12:20:30.265466+00:00
updated_at: 2025-11-27T12:42:36.835730+00:00
parent: CRT-V-0001
blocked_by: []
archived: false

tags:
  - "#initiative"
  - "#phase/completed"


exit_criteria_met: false
estimated_complexity: L
strategy_id: NULL
initiative_id: osc-133-semantic-prompt-support
---

# OSC 133 Semantic Prompt Support Initiative

## Context

CRT currently applies a text glow effect to terminal content. We want to apply glow only to the PS1 prompt and user input, while rendering command output flat with ANSI colors. 

Currently, we use a heuristic (cursor line + 1 line above) to guess what's prompt vs output. This is fragile and doesn't work reliably with multi-line prompts or complex shell configurations.

OSC 133 (FinalTerm/Semantic Prompt) is a standardized set of escape sequences that shells can emit to mark semantic boundaries:
- `OSC 133;A ST` - Prompt start
- `OSC 133;B ST` - Prompt end / Command start (user input begins)
- `OSC 133;C ST` - Command end / Output start
- `OSC 133;D ST` - Output end

Many shell configurations (oh-my-zsh, starship, powerlevel10k, fish) already emit these - they're just ignored by terminals that don't support them.

## Goals & Non-Goals

**Goals:**
- Parse OSC 133 escape sequences in the terminal
- Track semantic zones (Prompt, Input, Output) per line
- Expose zone information to the renderer
- Route text to glow vs flat renderers based on zone type
- Maintain fallback heuristic for shells without OSC 133 support

**Non-Goals:**
- Implementing other shell integration features (command history navigation, etc.)
- Requiring users to modify their shell configuration (it should just work if they already emit OSC 133)

## Architecture

### Current Data Flow
```
Shell PTY -> crt-core::Terminal::process_input(bytes) -> vte::Processor::advance()
          -> alacritty_terminal::Term handles escape sequences -> Grid<Cell>
```

VTE already parses OSC sequences but alacritty_terminal ignores OSC 133. We don't need to fork - we can intercept at the crt-core layer.

### Proposed Changes (No Fork Required)

1. **Intercept in process_input()** - Scan bytes for OSC 133 before passing to parser
2. **Add SemanticZone tracking** to crt-core Terminal struct
3. **Get cursor position** from alacritty_terminal when zone markers are found
4. **Expose zone lookup** method for renderer to query line zones
5. **Update window.rs** to route cells based on zone

### Data Flow
```
PTY bytes arrive at Terminal::process_input()
  -> Scan for OSC 133 sequences
  -> On OSC 133;A: line_zones[cursor_line] = Prompt
  -> On OSC 133;B: line_zones[cursor_line] = Input  
  -> On OSC 133;C: line_zones[cursor_line] = Output
  -> Pass all bytes through to parser unchanged

Rendering: query terminal.get_line_zone(line) -> route to glow or flat renderer
```

## Detailed Design

### Phase 1: Add SemanticZone Types to crt-core

In `crates/crt-core/src/lib.rs`:
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SemanticZone {
    #[default]
    Unknown,
    Prompt,   // Between OSC 133;A and OSC 133;B
    Input,    // Between OSC 133;B and OSC 133;C
    Output,   // Between OSC 133;C and next OSC 133;A
}
```

### Phase 2: Add Zone Tracking to Terminal

Add to Terminal struct:
```rust
pub struct Terminal {
    term: Term<TerminalEventProxy>,
    parser: Processor,
    // ... existing fields ...
    
    /// Semantic zones per line (from OSC 133)
    line_zones: BTreeMap<i32, SemanticZone>,
    /// Current zone state (for marking new lines)
    current_zone: SemanticZone,
}
```

### Phase 3: Parse OSC 133 in process_input()

Scan input bytes for OSC 133 pattern (`\x1b]133;X[\x07\x1b\\]`):
```rust
pub fn process_input(&mut self, bytes: &[u8]) {
    // Scan for OSC 133 sequences and update zone state
    self.scan_osc133(bytes);
    
    // Pass through to parser unchanged
    self.parser.advance(&mut self.term, bytes);
}

fn scan_osc133(&mut self, bytes: &[u8]) {
    // Look for \x1b]133;A, \x1b]133;B, \x1b]133;C, \x1b]133;D
    // On match, get cursor line from self.term and update line_zones
}
```

### Phase 4: Expose Zone Query API

```rust
impl Terminal {
    /// Get semantic zone for a given line
    pub fn get_line_zone(&self, line: i32) -> SemanticZone {
        self.line_zones.get(&line).copied().unwrap_or(SemanticZone::Unknown)
    }
    
    /// Check if OSC 133 is active (any zones detected)
    pub fn has_semantic_zones(&self) -> bool {
        !self.line_zones.is_empty()
    }
}
```

### Phase 5: Update Renderer

In window.rs `update_text_buffer()`:
```rust
// Get zone info from terminal
let zone = shell.terminal().get_line_zone(grid_line);
let has_osc133 = shell.terminal().has_semantic_zones();

let use_glow = if has_osc133 {
    matches!(zone, SemanticZone::Prompt | SemanticZone::Input)
} else {
    // Fallback heuristic for non-OSC 133 shells
    viewport_line >= cursor_viewport_line - 1 && viewport_line <= cursor_viewport_line
};
```

## Alternatives Considered

1. **Cursor position heuristic only**: Fragile, doesn't work with multi-line prompts
2. **Time-based detection**: Track when text was typed vs received - unreliable
3. **Pattern matching on prompt strings**: Too shell-specific, breaks with custom prompts
4. **Intercept at crt-core level**: Would require duplicate state tracking, messier than forking

## Implementation Plan

### Task 1: Add SemanticZone types to crt-core (CRT-T-0067)
- Define SemanticZone enum (Unknown, Prompt, Input, Output)
- Add line_zones BTreeMap and current_zone to Terminal struct
- Initialize in Terminal::new()

### Task 2: Implement OSC 133 scanner (CRT-T-0068)
- Create scan_osc133() function to find OSC 133 sequences in byte stream
- Handle patterns: `\x1b]133;A\x07`, `\x1b]133;B\x07`, etc.
- Also handle ST terminator variant: `\x1b]133;A\x1b\\`

### Task 3: Wire scanner into process_input (CRT-T-0068)
- Call scan_osc133() before parser.advance()
- On OSC 133;A/B/C/D detection, get cursor line and update line_zones
- Update current_zone state for marking subsequent lines

### Task 4: Expose zone query API (CRT-T-0069)
- Add get_line_zone(line: i32) -> SemanticZone method
- Add has_semantic_zones() -> bool method
- Re-export SemanticZone from crt-core

### Task 5: Update renderer to use zones (CRT-T-0070)
- Query zone for each cell's line during rendering
- Route Prompt/Input to glow renderer
- Route Output/Unknown to flat renderer (with cursor fallback)