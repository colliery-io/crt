---
id: testing-infrastructure-and
level: initiative
title: "Testing Infrastructure and Architectural Improvements"
short_code: "CRT-I-0015"
created_at: 2025-11-28T14:43:59.394220+00:00
updated_at: 2025-11-29T17:19:34.220837+00:00
parent: CRT-V-0001
blocked_by: []
archived: true

tags:
  - "#initiative"
  - "#phase/completed"


exit_criteria_met: false
estimated_complexity: L
strategy_id: NULL
initiative_id: testing-infrastructure-and
---

# Testing Infrastructure and Architectural Improvements Initiative

## Context

The CRT terminal emulator has grown to ~12,500 LOC across 4 crates with sophisticated GPU rendering, theming, and effects. However, testing coverage is limited (rating: 3/10) with only ~27 tests total, primarily in `crt-core` and `crt-theme`. The main application layer has zero tests.

Key architectural issues impacting testability:
- **Monolithic modules**: `main.rs` (1,458 LOC), `render.rs` (1,096 LOC), `input.rs` (772 LOC)
- **Tight coupling**: `WindowState` mixes GUI state, GPU resources, and terminal state
- **No rendering abstraction**: Direct wgpu calls prevent mocking
- **GPU dependency**: Most rendering code requires actual GPU context

## Goals & Non-Goals

**Goals:**
- Enable unit testing of input handling, text layout, and state management without GPU
- Reduce coupling between UI state, rendering, and terminal logic
- Split monolithic modules into focused, testable components
- Create integration test infrastructure with fake PTY support
- Improve code maintainability and reduce cognitive load per module

**Non-Goals:**
- Full GPU rendering tests (headless GPU testing is complex and fragile)
- 100% test coverage (diminishing returns)
- Major architectural rewrites (incremental improvements preferred)
- ECS/MVVM migration (separate initiative if pursued)

## Architecture

### Target Pattern: Hybrid ViewModel

Rather than a full ECS or MVVM migration, we adopt a lightweight hybrid that takes the best of both patterns while preserving the existing crate structure:

```
┌─────────────────────────────────────────────────────────────────┐
│                    CRATE STRUCTURE (unchanged)                  │
├─────────────────────────────────────────────────────────────────┤
│  crt-core     - Terminal Model (already clean, testable)       │
│  crt-theme    - Theme/Config Model (already clean, testable)   │
│  crt-renderer - View layer (GPU rendering)                     │
│  crt (app)    - ViewModel + Orchestration (refactored)         │
└─────────────────────────────────────────────────────────────────┘
```

### App Crate Target Structure

```
src/
├── main.rs                      # Thin: event loop + App::handle()
│
├── state/                       # ViewModel layer (testable)
│   ├── mod.rs
│   ├── app_state.rs             # Windows, focused, config
│   ├── window_state.rs          # Tabs, UI state, shells
│   ├── selection.rs             # Pure selection logic
│   ├── search.rs                # Pure search logic (exists)
│   ├── tab_state.rs             # Pure tab management
│   └── ui_state.rs              # Aggregates UI concerns
│
├── input/                       # Pure input processing
│   ├── mod.rs
│   ├── keyboard.rs              # Key -> Command/bytes
│   ├── mouse.rs                 # Mouse -> selection/scroll
│   ├── url_detection.rs         # Text -> URLs
│   └── commands.rs              # Command enum
│
├── layout/                      # Pure layout calculation
│   ├── mod.rs
│   ├── cell_metrics.rs          # Cell sizing
│   └── text_layout.rs           # Glyph positioning
│
├── managers/                    # Stateful orchestration
│   ├── mod.rs
│   ├── window_manager.rs        # Window lifecycle
│   ├── config_manager.rs        # Config + hot-reload
│   └── event_handler.rs         # Event dispatch
│
├── render.rs                    # Thin: calls crt-renderer
├── gpu.rs                       # GPU resource management
├── font.rs                      # Font loading
├── menu.rs                      # macOS menu
└── watcher.rs                   # File watching
```

### Key Architectural Principles

1. **State modules are ViewModels**: Contain UI logic, fully testable without GPU
2. **Input modules return Commands**: No side effects, just data transformations
3. **Layout modules are pure functions**: Math only, no state
4. **Managers coordinate**: Stateful but thin, delegate to state/input/layout
5. **Render is a thin adapter**: Translates ViewModel to crt-renderer calls

### Command Pattern for Input

```rust
// input/commands.rs
pub enum Command {
    // Terminal
    SendToPty(Vec<u8>),
    ScrollLines(i32),
    ScrollToTop,
    ScrollToBottom,
    
    // Selection
    StartSelection(Point, SelectionMode),
    UpdateSelection(Point),
    EndSelection,
    ClearSelection,
    
    // Clipboard
    Copy(String),
    Paste,
    
    // Navigation
    OpenUrl(String),
    SwitchTab(TabId),
    NewTab,
    CloseTab(TabId),
    
    // Window
    NewWindow,
    CloseWindow,
    ToggleFullscreen,
    
    // Search
    OpenSearch,
    CloseSearch,
    FindNext,
    FindPrevious,
    
    // Display
    RequestRedraw,
    AdjustFontScale(f32),
}
```

Input handlers return `Vec<Command>`, managers execute them. This makes input handling fully testable.

### Data Flow

```
winit Event
    │
    ▼
EventHandler (thin dispatch)
    │
    ▼
Input Module (pure: event -> Commands)
    │
    ▼
Commands
    │
    ▼
Managers (execute commands, update state)
    │
    ▼
State Modules (ViewModel: updated state)
    │
    ▼
Layout Module (pure: state -> positions)
    │
    ▼
Render (thin: positions -> crt-renderer)
    │
    ▼
GPU
```

### Testing Strategy by Layer

| Layer | Testing Approach |
|-------|------------------|
| State modules | Unit tests with mock data |
| Input modules | Unit tests (pure functions) |
| Layout modules | Unit tests (pure math) |
| Managers | Integration tests with fake PTY |
| Render | Manual/visual only |

### Migration Path

Tasks 1-7 implement this architecture incrementally:
- **Tasks 1, 5**: Create `input/` and `layout/` pure modules
- **Task 2**: Create `state/` ViewModel modules
- **Task 3**: Create `managers/` orchestration layer
- **Task 4**: Define traits for render abstraction
- **Tasks 6-7**: Add test infrastructure

### When to Consider Full ECS

This hybrid approach is sufficient for current complexity. Consider ECS migration if:
- Adding split panes, floating windows, or plugin system
- Performance requires parallel system execution
- Team grows and needs stricter boundaries

Potential libraries: `bevy_ecs`, `hecs`, `specs`

## Detailed Design

### Task 1: Extract Input Processing to Pure Functions

**Current State:**
```rust
// input.rs - tightly coupled to WindowState
fn handle_shell_input(state: &mut WindowState, key: Key) -> bool
```

**Target State:**
```rust
// input/key_mapping.rs - pure, testable
pub fn key_to_terminal_bytes(key: Key, mods: Modifiers, mode: TerminalMode) -> Option<Vec<u8>>
pub fn should_handle_as_shortcut(key: Key, mods: Modifiers) -> bool

// input/url_detection.rs - pure, testable  
pub fn detect_urls_in_text(text: &str) -> Vec<UrlMatch>
pub fn url_at_position(urls: &[UrlMatch], col: usize) -> Option<&UrlMatch>

// input/mouse.rs - pure, testable
pub fn calculate_grid_position(pixel: (f32, f32), cell_size: (f32, f32), padding: Padding) -> Point
pub fn selection_range(start: Point, end: Point, mode: SelectionMode) -> Range
```

**Files to modify:** `src/input.rs` -> split into `src/input/mod.rs`, `key_mapping.rs`, `url_detection.rs`, `mouse.rs`

### Task 2: Separate UI State from GPU State

**Current State:**
```rust
pub struct WindowState {
    pub window: Arc<Window>,
    pub gpu: WindowGpuState,           // GPU resources
    pub shells: HashMap<u64, ShellTerminal>,
    pub cursor_position: (f32, f32),   // UI state mixed in
    pub detected_urls: Vec<DetectedUrl>,
    pub search: SearchState,
    // ... 20+ fields mixing concerns
}
```

**Target State:**
```rust
// state/selection.rs
pub struct SelectionState {
    start: Option<Point>,
    end: Option<Point>,
    mode: SelectionMode,
    in_progress: bool,
}
impl SelectionState {
    pub fn start(&mut self, point: Point, mode: SelectionMode);
    pub fn update(&mut self, point: Point);
    pub fn finish(&mut self) -> Option<Selection>;
    pub fn clear(&mut self);
    pub fn to_range(&self) -> Option<Range>;
}

// state/tab_state.rs
pub struct TabState {
    tabs: Vec<TabInfo>,
    active_index: usize,
}
impl TabState {
    pub fn add_tab(&mut self, info: TabInfo) -> TabId;
    pub fn close_tab(&mut self, id: TabId) -> Option<TabInfo>;
    pub fn switch_to(&mut self, id: TabId) -> bool;
    pub fn active(&self) -> Option<&TabInfo>;
}

// state/ui_state.rs  
pub struct UiState {
    pub selection: SelectionState,
    pub search: SearchState,  // Already exists
    pub context_menu: ContextMenuState,
    pub hovered_url: Option<usize>,
    pub cursor_position: (f32, f32),
}

// WindowState becomes thinner
pub struct WindowState {
    pub window: Arc<Window>,
    pub gpu: WindowGpuState,
    pub shells: HashMap<TabId, ShellTerminal>,
    pub tabs: TabState,
    pub ui: UiState,
    // ... fewer fields, better organized
}
```

**Files to create:** `src/state/mod.rs`, `selection.rs`, `tab_state.rs`, `ui_state.rs`

### Task 3: Split main.rs into Focused Modules

**Current State:** `main.rs` handles events, windows, config, menus, rendering coordination

**Target State:**
```rust
// event_handler.rs
pub struct EventHandler;
impl EventHandler {
    pub fn handle_window_event(&self, app: &mut App, event: WindowEvent) -> EventResult;
    pub fn handle_device_event(&self, app: &mut App, event: DeviceEvent);
}

// window_manager.rs
pub struct WindowManager {
    windows: HashMap<WindowId, WindowState>,
    focused: Option<WindowId>,
}
impl WindowManager {
    pub fn create_window(&mut self, gpu: &SharedGpuState, config: &Config) -> WindowId;
    pub fn close_window(&mut self, id: WindowId);
    pub fn get_focused(&self) -> Option<&WindowState>;
    pub fn get_focused_mut(&mut self) -> Option<&mut WindowState>;
}

// config_manager.rs
pub struct ConfigManager {
    config: Config,
    watcher: Option<ConfigWatcher>,
}
impl ConfigManager {
    pub fn reload_config(&mut self) -> Result<(), ConfigError>;
    pub fn reload_theme(&mut self) -> Result<Theme, ThemeError>;
    pub fn watch_for_changes(&mut self, callback: impl Fn(ConfigChange));
}

// main.rs becomes thin orchestration
fn main() {
    let event_loop = EventLoop::new();
    let mut app = App::new();
    event_loop.run(|event, target| app.handle(event, target));
}
```

**Files to create:** `src/event_handler.rs`, `src/window_manager.rs`, `src/config_manager.rs`

### Task 4: Create Renderer Trait for Testing

**Current State:** Direct wgpu calls in `render.rs`

**Target State:**
```rust
// render/traits.rs
pub trait TextRenderer {
    fn render_grid(&mut self, content: &GridContent, theme: &Theme);
    fn render_cursor(&mut self, pos: Point, style: &CursorStyle, blink_on: bool);
    fn render_selection(&mut self, ranges: &[SelectionRange], color: Color);
}

pub trait UiRenderer {
    fn render_tab_bar(&mut self, tabs: &TabState, theme: &TabTheme);
    fn render_search_matches(&mut self, matches: &[Match], current: usize);
    fn render_context_menu(&mut self, menu: &ContextMenuState, theme: &MenuTheme);
}

pub trait EffectsRenderer {
    fn render_backdrop(&mut self, effects: &[Box<dyn BackdropEffect>], dt: f32);
    fn render_bell_flash(&mut self, intensity: f32);
}

// render/wgpu_renderer.rs - real implementation
pub struct WgpuRenderer { /* existing GPU resources */ }
impl TextRenderer for WgpuRenderer { ... }
impl UiRenderer for WgpuRenderer { ... }

// render/mock_renderer.rs - for testing
pub struct MockRenderer {
    pub text_calls: Vec<TextRenderCall>,
    pub ui_calls: Vec<UiRenderCall>,
}
impl TextRenderer for MockRenderer {
    fn render_grid(&mut self, content: &GridContent, theme: &Theme) {
        self.text_calls.push(TextRenderCall::Grid { ... });
    }
}
```

### Task 5: Extract Text Layout Logic

**Current State:** Text positioning buried in `update_text_buffer()` in `window.rs`

**Target State:**
```rust
// layout/text_layout.rs
pub struct CellMetrics {
    pub cell_width: f32,
    pub cell_height: f32,
    pub baseline_offset: f32,
    pub padding: Padding,
}

pub fn cell_to_pixel(col: usize, row: usize, metrics: &CellMetrics) -> (f32, f32) {
    let x = metrics.padding.left + (col as f32 * metrics.cell_width);
    let y = metrics.padding.top + (row as f32 * metrics.cell_height);
    (x, y)
}

pub fn pixel_to_cell(x: f32, y: f32, metrics: &CellMetrics) -> (usize, usize) {
    let col = ((x - metrics.padding.left) / metrics.cell_width).floor() as usize;
    let row = ((y - metrics.padding.top) / metrics.cell_height).floor() as usize;
    (col, row)
}

pub fn calculate_terminal_size(
    window_size: (u32, u32),
    metrics: &CellMetrics,
) -> (usize, usize) {
    // cols, rows calculation
}

// layout/glyph_positioning.rs
pub struct GlyphInstance {
    pub position: (f32, f32),
    pub uv: (f32, f32, f32, f32),
    pub color: [f32; 4],
}

pub fn layout_line(
    line: &str,
    row: usize,
    metrics: &CellMetrics,
    glyph_lookup: impl Fn(char) -> GlyphInfo,
) -> Vec<GlyphInstance> {
    // Pure layout logic, no GPU
}
```

### Task 6: Add Integration Test Infrastructure

```rust
// tests/common/fake_pty.rs
pub struct FakePty {
    input_buffer: Vec<u8>,
    output_queue: VecDeque<Vec<u8>>,
}

impl FakePty {
    pub fn new() -> Self { ... }
    pub fn queue_output(&mut self, data: &[u8]) { ... }
    pub fn read_input(&mut self) -> Vec<u8> { ... }
}

// tests/common/test_terminal.rs
pub struct TestTerminal {
    terminal: Terminal,
    fake_pty: FakePty,
}

impl TestTerminal {
    pub fn new(cols: usize, rows: usize) -> Self { ... }
    
    pub fn send(&mut self, data: &str) {
        self.fake_pty.queue_output(data.as_bytes());
        self.terminal.process_input(&mut self.fake_pty);
    }
    
    pub fn cursor_position(&self) -> (usize, usize) { ... }
    pub fn cell_content(&self, col: usize, row: usize) -> char { ... }
    pub fn line_content(&self, row: usize) -> String { ... }
}

// tests/integration/cursor_movement.rs
#[test]
fn test_cursor_home() {
    let mut t = TestTerminal::new(80, 24);
    t.send("Hello");
    t.send("\x1b[H");  // Cursor home
    assert_eq!(t.cursor_position(), (0, 0));
}

#[test]
fn test_cursor_absolute_position() {
    let mut t = TestTerminal::new(80, 24);
    t.send("\x1b[5;10H");  // Move to row 5, col 10
    assert_eq!(t.cursor_position(), (9, 4));  // 0-indexed
}
```

### Task 7: Effects Configuration Validation

```rust
// crt-theme/src/effects_config.rs
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_starfield_config_parsing() {
        let css = r#"
            :terminal::backdrop {
                --starfield-enabled: true;
                --starfield-density: 150;
                --starfield-speed: 0.8;
            }
        "#;
        let config = parse_effect_config(css).unwrap();
        assert!(config.starfield.enabled);
        assert_eq!(config.starfield.density, 150);
        assert_eq!(config.starfield.speed, 0.8);
    }
    
    #[test]
    fn test_invalid_effect_values_use_defaults() {
        let css = r#"
            :terminal::backdrop {
                --starfield-density: -50;  // Invalid
                --rain-angle: 999;         // Out of range
            }
        "#;
        let config = parse_effect_config(css).unwrap();
        assert_eq!(config.starfield.density, DEFAULT_STARFIELD_DENSITY);
        assert_eq!(config.rain.angle, DEFAULT_RAIN_ANGLE);
    }
}
```

## Alternatives Considered

1. **Full ECS Architecture (e.g., bevy_ecs)**: Would provide excellent separation but requires significant rewrite. Consider as separate initiative if complexity grows.

2. **MVVM Pattern**: Good for UI-heavy apps but terminal is more data-flow oriented. Partial adoption possible.

3. **Headless GPU Testing**: Using wgpu with null backend or software renderer. Complex to set up and maintain, provides less value than unit testing pure logic.

4. **Snapshot Testing for Rendering**: Capture rendered frames and compare. Fragile across GPU drivers and platforms.

## Implementation Plan

**Phase 1: Foundation (Tasks 1-2)**
- Extract pure input functions
- Create state modules
- Add tests for extracted code

**Phase 2: Architecture (Tasks 3-4)**
- Split main.rs
- Create renderer traits
- Add mock renderer

**Phase 3: Layout & Integration (Tasks 5-7)**
- Extract text layout
- Create fake PTY infrastructure
- Add effect config validation