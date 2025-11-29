---
id: responsive-theming-event-driven
level: initiative
title: "Responsive Theming - Event-Driven Visual Effects"
short_code: "CRT-I-0016"
created_at: 2025-11-29T02:19:11.377178+00:00
updated_at: 2025-11-29T02:19:11.377178+00:00
parent: CRT-V-0001
blocked_by: []
archived: false

tags:
  - "#initiative"
  - "#phase/discovery"


exit_criteria_met: false
estimated_complexity: L
strategy_id: NULL
initiative_id: responsive-theming-event-driven
---

# Responsive Theming - Event-Driven Visual Effects Initiative

*This template includes sections for various types of initiatives. Delete sections that don't apply to your specific use case.*

## Context

CRT terminal currently supports static theming via CSS-like syntax. Themes define colors, fonts, backgrounds, and backdrop effects. However, themes cannot respond to terminal events - they are purely declarative and static.

Users want visual feedback when events occur: a flash when the bell rings, color changes when commands fail, sprite animations on certain triggers. This creates a more dynamic, responsive terminal experience.

## Goals & Non-Goals

**Goals:**
- Enable themes to define temporary visual overrides triggered by terminal events
- Support bell event (BEL character) as a trigger
- Support command exit events (success/fail) via OSC 133;D semantic prompts
- Allow any theme property to be temporarily overridden (background, cursor, text-shadow, grid, etc.)
- Support optional sprite animations alongside property overrides
- Keep CSS syntax consistent with existing theme patterns

**Non-Goals:**
- Sound effects
- Easing/transition animations between states
- Multiple simultaneous effect layering
- Custom event definitions beyond bell/command exit
- Complex effect composition or blending

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

## Architecture

### Overview
Event-driven theming treats event blocks as "temporary theme overlays". When an event fires, the renderer applies property overrides for a specified duration, then reverts to base theme values.

### Data Flow
1. Terminal receives event (bell character or OSC 133;D)
2. crt-core detects event and emits TerminalEvent
3. Renderer looks up corresponding EventOverride from theme
4. If override exists, creates ActiveOverride with timestamp
5. Each frame: applies override properties if not expired
6. When duration elapses, clears ActiveOverride

### Key Types
- `EventOverride`: Partial theme with optional sprite config and duration
- `SpriteConfig`: Animation parameters (image, frames, fps, position)
- `ActiveOverride`: Runtime state tracking override + start time
- `TerminalEvent`: Enum (Bell, CommandSuccess, CommandFail)

## Detailed Design

### CSS Syntax
```css
:terminal::on-bell {
    --duration: 500ms;
    --sprite-image: "explosion.png";
    --sprite-columns: 8;
    --sprite-rows: 4;
    --sprite-fps: 24;
    --sprite-position: center;
    cursor-color: #ff0000;
    background: #400000;
    text-shadow: 0 0 30px rgba(255, 0, 0, 0.9);
}

:terminal::on-command-fail {
    --duration: 1000ms;
    cursor-color: #ff3333;
    background: linear-gradient(to bottom, #300000, #100000);
}

:terminal::on-command-success {
    --duration: 300ms;
    --sprite-image: "sparkle.png";
    --sprite-columns: 4;
    --sprite-rows: 1;
    --sprite-fps: 16;
    --sprite-position: cursor;
}
```

### Data Structures (crt-theme/src/lib.rs)
```rust
#[derive(Debug, Clone, Default)]
pub struct EventOverride {
    pub duration_ms: u32,

    // Sprite config (optional)
    pub sprite: Option<SpriteConfig>,

    // Property overrides (all optional - None means "keep base theme value")
    pub foreground: Option<Color>,
    pub background: Option<Background>,
    pub cursor_color: Option<Color>,
    pub text_shadow: Option<TextShadow>,
    pub grid_color: Option<Color>,
    // ... other overridable properties
}

#[derive(Debug, Clone)]
pub struct SpriteConfig {
    pub image: String,
    pub columns: u32,
    pub rows: u32,
    pub fps: f32,
    pub position: SpritePosition,
    pub scale: f32,
}

#[derive(Debug, Clone, Default)]
pub enum SpritePosition {
    #[default]
    Center,
    Cursor,
    Random,
}

// Add to Theme struct:
pub on_bell: Option<EventOverride>,
pub on_command_fail: Option<EventOverride>,
pub on_command_success: Option<EventOverride>,
```

### Event Types and Runtime State
```rust
// In crt-theme or shared location
pub enum TerminalEvent {
    Bell,
    CommandSuccess,
    CommandFail(i32),  // exit code
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
1. `crates/crt-theme/src/lib.rs` - Add EventOverride, SpriteConfig structs
2. `crates/crt-theme/src/parser.rs` - Parse ::on-bell, ::on-command-fail, ::on-command-success
3. `crates/crt-core/src/lib.rs` - Extract exit code from OSC 133;D
4. `crates/crt-renderer/src/effects/renderer.rs` - Add ActiveOverride state
5. `src/main.rs` - Wire events to renderer, apply overrides in render loop

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

## Alternatives Considered

### State Machine Approach
Could model events as state transitions with enter/exit animations. Rejected as over-engineered for v1 - simple duration-based overlays are sufficient.

### Sprite-Only Events
Original design only supported sprite animations. Extended to full property overrides for more flexibility (e.g., flash without sprite).

### Keyframe Animations
Could support CSS-like @keyframes for smooth transitions. Deferred - adds significant complexity for minimal benefit in v1.

## Implementation Plan

### Phase 1: Theme Data Structures
- Add EventOverride and SpriteConfig structs to crt-theme
- Add on_bell, on_command_fail, on_command_success fields to Theme

### Phase 2: CSS Parser Extension
- Parse ::on-bell, ::on-command-fail, ::on-command-success pseudo-elements
- Reuse existing property parsing for overridable values
- Parse sprite custom properties (--sprite-image, etc.)

### Phase 3: Event Detection
- Extract exit code from OSC 133;D in crt-core
- Create TerminalEvent enum
- Emit events from terminal processing

### Phase 4: Renderer Integration
- Add ActiveOverride state to renderer
- Implement trigger_event() method
- Apply theme.with_override() in render loop
- Handle duration expiry and revert

### Phase 5: Sprite Support
- Wire SpriteEffect creation from SpriteConfig
- Support position variants (center, cursor, random)

### Phase 6: Testing and Polish
- Create demo theme showcasing all event types
- Manual testing with various shells
- Documentation updates