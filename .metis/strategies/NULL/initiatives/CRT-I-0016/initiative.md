---
id: responsive-theming-event-driven
level: initiative
title: "Responsive Theming - Event-Driven Visual Effects"
short_code: "CRT-I-0016"
created_at: 2025-11-29T02:19:11.377178+00:00
updated_at: 2025-12-31T17:25:46.410581+00:00
parent: CRT-V-0001
blocked_by: []
archived: false

tags:
  - "#initiative"
  - "#phase/completed"


exit_criteria_met: false
estimated_complexity: L
strategy_id: NULL
initiative_id: responsive-theming-event-driven
---

# Responsive Theming - Event-Driven Visual Effects Initiative

## Context

CRT terminal currently supports static theming via CSS-like syntax. Themes define colors, fonts, backgrounds, and backdrop effects. However, themes cannot respond to terminal events - they are purely declarative and static.

Users want visual feedback when events occur: a flash when the bell rings, color changes when commands fail, sprite animations on certain triggers. This creates a more dynamic, responsive terminal experience.

## Goals & Non-Goals

**Goals:**
- Enable themes to define temporary visual overrides triggered by terminal events
- Support bell event (BEL character) as a trigger
- Support command exit events (success/fail) via OSC 133;D semantic prompts
- Support focus gained/lost events (window activation)
- Allow any theme property to be temporarily overridden (background, cursor, text-shadow, grid, etc.)
- Support optional sprite animations/swaps alongside property overrides
- Keep CSS syntax consistent with existing theme patterns
- Provide opt-in automatic OSC 133 shell integration for bash/zsh

**Non-Goals:**
- Sound effects
- Easing/transition animations between states
- Multiple simultaneous effect layering
- Custom event definitions beyond v1 events
- Complex effect composition or blending

## V1 Event Scope

| Event | Selector | Source | Detection |
|-------|----------|--------|-----------|
| Bell | `::on-bell` | BEL char (0x07) | Terminal level, always works |
| Command fail | `::on-command-fail` | OSC 133;D;N (N≠0) | Requires shell integration |
| Command success | `::on-command-success` | OSC 133;D;0 | Requires shell integration |
| Focus gained | `::on-focus` | Window event | Terminal level, always works |
| Focus lost | `::on-blur` | Window event | Terminal level, always works |

**Future candidates (not v1):** idle, activity, resize, selection, search match

## Use Cases

### Use Case 1: Bell Visual Feedback
- **Actor**: Terminal user
- **Scenario**: Application sends BEL character (Ctrl+G or `echo -e '\a'`)
- **Expected Outcome**: Terminal temporarily applies ::on-bell overrides (e.g., red flash, explosion sprite) for configured duration, then reverts to base theme

### Use Case 2: Command Failure Indication
- **Actor**: Terminal user with OSC 133-enabled shell (starship, zsh plugin)
- **Scenario**: User runs a command that exits with non-zero status
- **Expected Outcome**: Terminal applies ::on-command-fail overrides (e.g., red tint, error cursor) for configured duration

### Use Case 3: Command Success Celebration
- **Actor**: Terminal user with OSC 133-enabled shell
- **Scenario**: User runs a command that exits successfully (exit code 0)
- **Expected Outcome**: Terminal applies ::on-command-success overrides (e.g., sparkle sprite) for configured duration

### Use Case 4: Focus Lost Indication
- **Actor**: Terminal user multitasking between windows
- **Scenario**: User switches to another application, CRT window loses focus
- **Expected Outcome**: Terminal applies ::on-blur overrides (e.g., dimmed colors, paused sprite) until focus returns

### Use Case 5: Focus Gained Indication
- **Actor**: Terminal user returning to CRT
- **Scenario**: User clicks back on CRT window or Alt-Tabs to it
- **Expected Outcome**: Terminal applies ::on-focus overrides briefly (e.g., highlight flash) or restores from blur state

## Architecture

### Overview
Event-driven theming treats event blocks as "temporary theme overlays". When an event fires, the renderer applies property overrides for a specified duration, then reverts to base theme values.

### Data Flow
1. Event source fires (terminal escape sequence, window system, etc.)
2. Event handler emits TerminalEvent to renderer
3. Renderer looks up corresponding EventOverride from theme
4. If override exists, creates ActiveOverride with timestamp
5. Each frame: applies override properties if not expired
6. When duration elapses, clears ActiveOverride

**Special case: Focus events**
- `::on-blur` typically has no duration - it persists until `FocusGained` clears it
- `::on-focus` can have a brief duration (e.g., 200ms flash) or no duration
- When focus is regained: clear blur override, then optionally trigger focus override

**Event precedence (v1: simple model)**
- Only one override active at a time (no layering per Non-Goals)
- New event replaces current override (newest wins)
- Exception: `::on-blur` is lower priority - bell/command events can still fire while blurred

### Key Types
- `EventOverride`: Partial theme with sprite patch, overlay, and duration
- `SpritePatch`: Overrides for existing backdrop sprite (keeps position/motion)
- `SpriteOverlay`: One-shot effect sprite at specified position
- `ActiveOverride`: Runtime state tracking override + start time
- `TerminalEvent`: Enum (Bell, CommandSuccess, CommandFail, FocusGained, FocusLost)

## Detailed Design

### CSS Syntax

**Block Merging:** Multiple `::on-*` blocks for the same event are merged (CSS cascade style). This lets theme authors organize by concern:

```css
/* ══════════════════════════════════════════════════════════════════
   SPRITE PROPERTIES IN EVENT BLOCKS
   
   --sprite-*         = Patch existing backdrop sprite (keeps position/motion)
   --sprite-overlay-* = New one-shot effect sprite at specified position
   ══════════════════════════════════════════════════════════════════ */

/* Example: Nyan Cat explodes on command fail */

/* Block 1: Patch backdrop sprite to exploding animation */
:terminal::on-command-fail {
    --duration: 1500ms;
    --sprite-path: "nyancat/nyan-exploding.png";
    --sprite-columns: 8;
    --sprite-fps: 24;
}

/* Block 2: Add explosion overlay at sprite's current position */
:terminal::on-command-fail {
    --sprite-overlay: "effects/explosion.png";
    --sprite-overlay-position: sprite;  /* center | cursor | sprite | random */
    --sprite-overlay-columns: 8;
    --sprite-overlay-rows: 4;
    --sprite-overlay-fps: 24;
    --sprite-overlay-scale: 2.0;
}

/* Block 3: Also flash the background red */
:terminal::on-command-fail {
    background: #400020;
    text-shadow: 0 0 20px rgba(255, 0, 0, 0.8);
}

/* ══════════════════════════════════════════════════════════════════ */

:terminal::on-bell {
    --duration: 500ms;
    --sprite-overlay: "effects/flash.png";
    --sprite-overlay-position: center;
    --sprite-overlay-columns: 4;
    --sprite-overlay-fps: 16;
    background: #400000;
    text-shadow: 0 0 30px rgba(255, 0, 0, 0.9);
}

:terminal::on-command-success {
    --duration: 300ms;
    --sprite-overlay: "effects/sparkle.png";
    --sprite-overlay-position: cursor;
    --sprite-overlay-columns: 4;
    --sprite-overlay-fps: 16;
}

:terminal::on-focus {
    --duration: 200ms;
    text-shadow: 0 0 20px rgba(255, 255, 255, 0.8);
}

:terminal::on-blur {
    /* No duration - persists until focus regained */
    background: linear-gradient(to bottom, #1a1a1a, #0a0a0a);
    --sprite-opacity: 0.3;
    --sprite-motion-speed: 0.1;
    --starfield-speed: 0.1;
}
```

### Data Structures (crt-theme/src/lib.rs)
```rust
#[derive(Debug, Clone, Default)]
pub struct EventOverride {
    /// Duration in milliseconds. 0 = persist until cleared by another event
    /// (used for ::on-blur which persists until focus regained)
    pub duration_ms: u32,

    /// Patches to existing backdrop sprite (keeps position/motion)
    /// Only specified fields override; None = keep current value
    pub sprite_patch: Option<SpritePatch>,
    
    /// Separate overlay sprite (one-shot effect at specified position)
    pub sprite_overlay: Option<SpriteOverlay>,

    // Theme property overrides (all optional - None means "keep base theme value")
    pub foreground: Option<Color>,
    pub background: Option<Background>,
    pub cursor_color: Option<Color>,
    pub text_shadow: Option<TextShadow>,
    pub grid_color: Option<Color>,
    pub starfield_speed: Option<f32>,
    // ... other overridable properties
}

/// Patches to existing backdrop sprite - keeps position and motion
#[derive(Debug, Clone, Default)]
pub struct SpritePatch {
    pub path: Option<String>,       // --sprite-path
    pub columns: Option<u32>,       // --sprite-columns
    pub rows: Option<u32>,          // --sprite-rows
    pub fps: Option<f32>,           // --sprite-fps
    pub opacity: Option<f32>,       // --sprite-opacity
    pub scale: Option<f32>,         // --sprite-scale
    pub motion_speed: Option<f32>,  // --sprite-motion-speed
}

/// One-shot overlay sprite at specified position
#[derive(Debug, Clone)]
pub struct SpriteOverlay {
    pub path: String,                   // --sprite-overlay (required)
    pub position: SpriteOverlayPosition, // --sprite-overlay-position
    pub columns: u32,                   // --sprite-overlay-columns
    pub rows: u32,                      // --sprite-overlay-rows
    pub fps: f32,                       // --sprite-overlay-fps
    pub scale: f32,                     // --sprite-overlay-scale
    pub opacity: f32,                   // --sprite-overlay-opacity
}

#[derive(Debug, Clone, Default)]
pub enum SpriteOverlayPosition {
    #[default]
    Center,
    Cursor,
    Sprite,  // At current backdrop sprite position
    Random,
}

// Add to Theme struct:
pub on_bell: Option<EventOverride>,
pub on_command_fail: Option<EventOverride>,
pub on_command_success: Option<EventOverride>,
pub on_focus: Option<EventOverride>,
pub on_blur: Option<EventOverride>,
```

### CSS Block Merging
Multiple `::on-*` blocks for the same event are merged during parsing:
```rust
impl EventOverride {
    /// Merge another EventOverride into this one (later values win)
    pub fn merge(&mut self, other: EventOverride) {
        if other.duration_ms > 0 {
            self.duration_ms = other.duration_ms;
        }
        // Merge sprite_patch fields individually
        if let Some(patch) = other.sprite_patch {
            let existing = self.sprite_patch.get_or_insert_default();
            if patch.path.is_some() { existing.path = patch.path; }
            if patch.fps.is_some() { existing.fps = patch.fps; }
            // ... etc
        }
        // Overlay replaces entirely if specified
        if other.sprite_overlay.is_some() {
            self.sprite_overlay = other.sprite_overlay;
        }
        // Theme properties: later wins
        if other.background.is_some() { self.background = other.background; }
        // ... etc
    }
}
```

### Event Types and Runtime State
```rust
// In crt-theme or shared location
pub enum TerminalEvent {
    Bell,
    CommandSuccess,
    CommandFail(i32),  // exit code
    FocusGained,
    FocusLost,
}

// Runtime state for active override
pub struct ActiveOverride {
    pub override_theme: EventOverride,
    pub started_at: Instant,
    pub sprite_effect: Option<SpriteEffect>,
}
```

### OSC 133;D Exit Code Extraction (crt-core/src/lib.rs)
```rust
// Parse "133;D;{exit_code}" format
if params.starts_with("133;D") {
    let exit_code = params.split(';').nth(2)
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(0);
    // Emit appropriate event based on exit_code
}
```

### Theme Compositor
```rust
impl Theme {
    pub fn with_override(&self, active: &EventOverride) -> EffectiveTheme {
        EffectiveTheme {
            foreground: active.foreground.unwrap_or(self.foreground),
            background: active.background.clone().unwrap_or(self.background.clone()),
            cursor_color: active.cursor_color.unwrap_or(self.cursor_color),
            // ... other properties
        }
    }
}
```

### Files to Modify
1. `crates/crt-theme/src/lib.rs` - Add EventOverride, SpriteConfig, TerminalEvent types
2. `crates/crt-theme/src/parser.rs` - Parse ::on-bell, ::on-command-fail, ::on-command-success, ::on-focus, ::on-blur
3. `crates/crt-core/src/lib.rs` - Extract exit code from OSC 133;D, emit command events
4. `crates/crt-renderer/src/effects/renderer.rs` - Add ActiveOverride state, theme compositor
5. `src/main.rs` - Wire events to renderer, handle WindowEvent::Focused for focus/blur
6. `src/input/mod.rs` - Forward focus events to event system (if not already in main.rs)

## Testing Strategy

### Bell Event Testing
1. Create test theme with ::on-bell override (background flash + optional sprite)
2. Press Ctrl+G or `echo -e '\a'` to trigger bell
3. Verify background/cursor color changes for duration
4. Verify sprite animation plays if configured

### Command Exit Testing
1. Create test theme with ::on-command-fail override
2. Run `false` or `(exit 1)` command
3. Verify effect triggers (requires OSC 133-enabled shell like starship)

### Duration Testing
1. Set various --duration values (100ms, 500ms, 2000ms)
2. Verify override reverts to base theme after duration

### Focus Event Testing
1. Create test theme with ::on-blur override (dimmed background, reduced sprite opacity)
2. Click away from CRT window to another application
3. Verify blur overrides apply immediately and persist
4. Click back to CRT window
5. Verify ::on-focus override triggers briefly (if configured), then blur clears
6. Test with ::on-blur having no duration (should persist until focus regained)

## Alternatives Considered

### State Machine Approach
Could model events as state transitions with enter/exit animations. Rejected as over-engineered for v1 - simple duration-based overlays are sufficient.

### Sprite-Only Events
Original design only supported sprite animations. Extended to full property overrides for more flexibility (e.g., flash without sprite).

### Keyframe Animations
Could support CSS-like @keyframes for smooth transitions. Deferred - adds significant complexity for minimal benefit in v1.

## Implementation Plan

### Phase 0: Semantic Prompt Shell Integration
CRT can automatically inject OSC 133 hooks for shells that don't natively support them.

**Config option:**
```toml
[shell]
semantic_prompts = true  # default: false initially
```

**When enabled:**
1. Detect shell type from spawn command (bash/zsh/fish)
2. Fish: No action needed (built-in support since 3.4)
3. Bash: Spawn with `--rcfile /path/to/crt-bash-init` where init file:
   - Sources user's `~/.bashrc` if exists
   - Adds `__crt_precmd` function to emit `OSC 133;D;$?`
   - Prepends to `PROMPT_COMMAND`
   - Wraps `PS1` with `OSC 133;A` and `OSC 133;B`
4. Zsh: Use custom rcfile or ZDOTDIR approach:
   - Sources user's `.zshrc`
   - Adds precmd/preexec hooks via `add-zsh-hook`

**Init scripts (bundled with CRT):**
```bash
# crt-bash-init
[ -f ~/.bashrc ] && source ~/.bashrc

__crt_precmd() {
    printf '\e]133;D;%d\a' "$?"
}
PROMPT_COMMAND="__crt_precmd${PROMPT_COMMAND:+;$PROMPT_COMMAND}"
PS1='\[\e]133;A\a\]'"${PS1}"'\[\e]133;B\a\]'
```

```zsh
# crt-zsh-init
[ -f ~/.zshrc ] && source ~/.zshrc

__crt_precmd() {
    printf '\e]133;D;%d\a' "$?"
    printf '\e]133;A\a'
}
__crt_preexec() {
    printf '\e]133;B\a'
}
autoload -Uz add-zsh-hook
add-zsh-hook precmd __crt_precmd
add-zsh-hook preexec __crt_preexec
```

**Fallback:** If user has starship/oh-my-zsh with semantic prompts, the hooks are redundant but harmless (double-emission is fine).

### Phase 1: Theme Data Structures
- Add EventOverride and SpriteConfig structs to crt-theme
- Add on_bell, on_command_fail, on_command_success fields to Theme

### Phase 2: CSS Parser Extension
- Parse ::on-bell, ::on-command-fail, ::on-command-success pseudo-elements
- Reuse existing property parsing for overridable values
- Parse sprite custom properties (--sprite-image, etc.)

### Phase 3: Event Detection
- Create TerminalEvent enum in crt-theme (shared across crates)
- **Terminal-level events** (crt-core):
  - Bell: Detect BEL character (0x07) during terminal processing
  - Command events: Extract exit code from OSC 133;D sequences
- **Window-level events** (main.rs):
  - Focus: Handle winit's WindowEvent::Focused(true/false)
  - These bypass crt-core entirely, fired directly from event loop

### Phase 4: Renderer Integration
- Add ActiveOverride state to renderer
- Implement trigger_event() method
- Apply theme.with_override() in render loop
- Handle duration expiry and revert

### Phase 5: Sprite Support
- **SpritePatch**: Apply property overrides to existing backdrop sprite
  - Preserve current position and motion pattern
  - Swap sprite sheet, adjust fps/opacity/speed
- **SpriteOverlay**: Render one-shot effect sprite
  - Support position variants (center, cursor, sprite, random)
  - `sprite` position tracks backdrop sprite location in real-time
  - Clean up overlay when animation completes or event duration expires

### Phase 6: Testing and Polish
- Create demo theme showcasing all event types
- Manual testing with various shells
- Documentation updates