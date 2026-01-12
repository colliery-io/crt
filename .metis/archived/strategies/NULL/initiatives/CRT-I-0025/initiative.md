---
id: per-window-theme-switching
level: initiative
title: "Per-Window Theme Switching"
short_code: "CRT-I-0025"
created_at: 2025-12-31T14:28:08.905973+00:00
updated_at: 2025-12-31T15:47:11.563620+00:00
parent: CRT-V-0001
blocked_by: []
archived: true

tags:
  - "#initiative"
  - "#phase/completed"


exit_criteria_met: false
estimated_complexity: M
strategy_id: NULL
initiative_id: per-window-theme-switching
---

# Per-Window Theme Switching Initiative

## Context

Currently, CRT loads themes from CSS files (`~/.config/crt/themes/{name}.css`) configured in `config.toml`. The theme is loaded once at startup and applied globally to all windows via `App::theme`. This means:
- Changing themes requires editing `config.toml` and restarting
- All windows share the same theme
- No runtime theme switching capability

Users want the flexibility to have different visual environments for different contexts (e.g., a dark theme for coding, a light theme for documentation, a fun retro theme for personal projects).

## Goals & Non-Goals

**Goals:**
- Allow runtime theme switching per-window via a dropdown UI
- Config.toml provides the default theme for new windows
- All tabs within a window share that window's theme
- Theme changes apply immediately without restart
- Discover available themes from the themes directory

**Non-Goals:**
- Per-tab theming (complexity outweighs benefit, GPU resources tied to window)
- Theme editing/creation UI (out of scope)
- Theme synchronization across windows
- Persisting per-window theme choices across sessions

## Architecture

### Current State
```
App::theme (single Theme) 
    └── Applied to all WindowState instances
        └── WindowGpuState::effect_pipeline.set_theme()
        └── WindowGpuState::tab_bar.set_theme()
```

### Target State
```
App::available_themes: HashMap<String, Theme>  // Preloaded theme cache
App::default_theme_name: String                // From config

WindowState::theme: Theme                      // Per-window theme
WindowState::theme_name: String                // Current theme name
    └── WindowGpuState::effect_pipeline.set_theme()
    └── WindowGpuState::tab_bar.set_theme()
```

### Key Changes

1. **Theme Registry** (`src/theme_registry.rs` - new)
   - Scan `~/.config/crt/themes/` for `.css` files at startup
   - Load and validate all themes into `HashMap<String, Theme>`
   - Provide `list_themes() -> Vec<String>` for UI
   - Provide `get_theme(name) -> Option<&Theme>`

2. **WindowState Changes** (`src/window.rs`)
   - Add `theme: Theme` field (move from App)
   - Add `theme_name: String` field
   - Add `set_theme(name: &str, registry: &ThemeRegistry)` method

3. **UI: Theme Dropdown** (`src/ui/theme_selector.rs` - new)
   - Rendered in tab bar area (right side) or via context menu
   - Lists available themes from registry
   - Indicates current selection
   - On selection: calls `window_state.set_theme()`

4. **Window Menu Integration** (`src/menu.rs`)
   - Add "Theme" submenu to Window menu
   - Dynamically populate from theme registry

## Detailed Design

### ThemeRegistry Implementation

```rust
pub struct ThemeRegistry {
    themes: HashMap<String, Theme>,
    theme_dir: PathBuf,
}

impl ThemeRegistry {
    pub fn new(theme_dir: PathBuf) -> Self;
    pub fn scan_and_load(&mut self) -> Result<(), ThemeLoadError>;
    pub fn list_themes(&self) -> Vec<&str>;
    pub fn get_theme(&self, name: &str) -> Option<&Theme>;
    pub fn reload_theme(&mut self, name: &str) -> Result<(), ThemeLoadError>;
}
```

### Theme Switching Flow

1. User selects theme from dropdown/menu
2. `UserEvent::SetWindowTheme { window_id, theme_name }` dispatched
3. Event handler looks up theme from registry
4. Calls `window_state.set_theme(theme.clone())`
5. `set_theme` updates:
   - `self.theme = theme`
   - `self.gpu.effect_pipeline.set_theme(&theme)`
   - `self.gpu.tab_bar.set_theme(&theme.tabs)`
   - Marks window dirty for re-render

### UI Placement Options

**Option A: Tab Bar Dropdown (Recommended)**
- Small theme icon/dropdown at right edge of tab bar
- Minimal UI footprint, always accessible
- Consistent with browser-style tab bar extras

**Option B: Context Menu**
- Right-click on tab bar background
- Add "Theme >" submenu
- Less discoverable but cleaner

**Option C: Window Menu Only**
- macOS Window menu → Theme submenu
- Most native feel on macOS
- Cross-platform: add to context menu

**Recommendation:** Implement Option C (Window menu) first for clean native integration, add Option B (context menu) for discoverability.

## Alternatives Considered

1. **Hot-reload config.toml** - Rejected: Still global, doesn't solve per-window need
2. **Store theme per-tab** - Rejected: GPU resources (effect pipeline) are per-window, would require major refactor
3. **Theme as window attribute in config** - Rejected: Config is static, doesn't allow runtime switching

## Implementation Plan

### Phase 1: Theme Registry
- Create `ThemeRegistry` struct
- Scan themes directory on startup
- Replace single `App::theme` with registry + default

### Phase 2: Per-Window Theme State
- Move theme from `App` to `WindowState`
- Add `set_theme()` method with GPU resource updates
- New windows get default theme from registry

### Phase 3: Window Menu Integration
- Add Theme submenu to Window menu
- Wire menu selection to `SetWindowTheme` event
- Handle event in main loop

### Phase 4: Context Menu Integration
- Add Theme submenu to tab bar context menu
- Reuse menu building logic from Phase 3

### Phase 5: Polish
- Add visual indicator of current theme in menu
- Handle theme load errors gracefully
- Consider theme preview on hover (stretch goal)