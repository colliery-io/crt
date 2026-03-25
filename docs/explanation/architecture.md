# Architecture Overview

CRT Terminal is a GPU-accelerated terminal emulator written in Rust. Its name is recursive: CRT's a Ridiculous Terminal. The project is organized as a Cargo workspace with three library crates and one binary. This document explains why the code is structured the way it is, what each piece is responsible for, and the reasoning behind the major design decisions.

## Workspace Structure

```
crt/
├── crates/
│   ├── crt-core/       # Terminal emulation and PTY management
│   ├── crt-renderer/   # GPU rendering
│   └── crt-theme/      # CSS-like theme parsing
└── src/                # Main binary: app lifecycle, input, window management
```

The three-crate split is not arbitrary. Each crate has a clearly bounded responsibility and a different pace of change:

- **crt-core** changes when terminal protocol behavior changes (rarely)
- **crt-theme** changes when new visual properties are added
- **crt-renderer** changes when rendering techniques evolve
- The **main binary** changes when platform integration, input handling, or app lifecycle logic changes

This separation means you can work on glyph rendering without touching PTY code, or extend the theme parser without recompiling the renderer. In practice it also keeps compile times manageable because unchanged crates are not recompiled.

The dependency graph only flows one direction: the main binary depends on all three crates. The renderer depends on the theme crate (to read `Theme` structs for shader uniforms). The core and theme crates do not depend on each other or on the renderer.

## crt-core: Terminal Emulation

This crate provides the terminal grid state and PTY management. It does not do any rendering — it only knows about character cells, their attributes, and the bytes flowing through the pseudo-terminal.

### Why alacritty_terminal?

Building a correct VT100/VT220/xterm terminal emulator from scratch is a multi-year project. The escape sequence specification is enormous and riddled with edge cases. `alacritty_terminal` is a battle-tested implementation that handles the full ANSI/VT state machine, including:

- All standard escape sequences and CSI/OSC sequences
- The 16/256-color model and true-color (RGB) sequences
- Alternate screen buffer switching
- Scrollback history with configurable capacity
- Selection and scrolling APIs

By wrapping `alacritty_terminal` rather than reimplementing it, CRT inherits years of compatibility fixes. The `Terminal` struct in `crt-core/src/lib.rs` is a thin wrapper: it holds a `Term<TerminalEventProxy>` and an `ansi::Processor`, exposes a clean API to the rest of the application, and adds a few CRT-specific concerns on top.

### OSC 133 Semantic Zones

One addition over the raw `alacritty_terminal` API is OSC 133 parsing. These are escape sequences that shells like zsh and bash (or prompt tools like Starship) emit to mark the boundaries between prompt, user input, and command output. CRT scans the raw byte stream for these sequences *before* passing it to the VTE parser, building a `BTreeMap<i32, SemanticZone>` keyed on line number.

This allows the renderer to apply different visual treatment to different regions — for example, applying a glow effect only to the prompt and input lines. The shell integration works with any tool that emits OSC 133 markers; CRT can also inject hooks into bash/zsh itself when `semantic_prompts = true` in the config.

### PTY Management

`ShellTerminal` combines a `Terminal` with a `Pty` (from the `portable-pty` crate). The `Pty` spawns a child shell process and provides cross-platform pseudo-terminal I/O. The separation between `Terminal` (grid state) and `ShellTerminal` (grid state + live process) is intentional: it allows tests to drive the terminal through mock PTY input without needing a real shell process. `MockPty` provides a synchronous in-memory implementation used throughout the test suite.

### Event Handling

Terminal events (like bell, title changes, and clipboard requests) travel from the VTE parser back to the application through `TerminalEventProxy`. This is a lock-free `SegQueue` (crossbeam) rather than a `Mutex<Vec>`: the rendering loop calls `take_events()` every frame, and contention on a mutex would add latency on the hot path. The queue is pushed from the PTY reader thread and drained from the main render loop.

## crt-renderer: GPU Rendering

This crate owns all GPU resources and rendering logic. It uses two GPU libraries:

- **wgpu** for direct GPU access — buffers, textures, render pipelines, command encoders
- **vello** for 2D vector rendering — paths, shapes, bezier curves, text rendering in the vello model

### Why wgpu and vello together?

wgpu is a safe, cross-platform Rust abstraction over native GPU APIs (Metal on macOS, Vulkan on Linux, D3D12 on Windows). It gives full control over the rendering pipeline: you write your own WGSL shaders, manage your own buffers, and submit your own command encoders. This control is necessary for the performance-sensitive hot path — rendering thousands of glyphs per frame as instanced quads with a shared atlas texture.

Vello is a higher-level 2D vector renderer that targets wgpu as its backend. It excels at path-based drawing: curves, gradients, compositing. Writing a retro perspective grid or a particle system using raw wgpu geometry code would be tedious and fragile; vello's scene API makes those effects straightforward. The trade-off is that vello has internal texture atlases that can accumulate memory over time, which is why the renderer periodically resets the vello `Renderer` (every ~300 frames when effects are active).

Both libraries coexist because they serve different rendering tasks. The text rendering pipeline is pure wgpu with hand-written shaders. The backdrop effects pipeline uses vello to build a scene, then vello's wgpu backend renders that scene to an intermediate texture, and a final wgpu blit pass composites it onto the frame.

### SharedPipelines

The most significant design decision in the renderer is `SharedPipelines`. On macOS, compiling a Metal shader pipeline from WGSL source involves shader compilation, reflection, and caching — this costs roughly 5–15 MB of memory per pipeline object, even when different windows use identical shaders. With naive per-window pipeline creation, three open windows would hold three independent copies of each compiled shader.

`SharedPipelines` holds `Arc`-wrapped pipeline objects. All windows share the same `Arc` references. When a window is created, it calls `SharedGpuState::ensure_shared_pipelines()`, which creates the pipelines once on first call and is a no-op on subsequent calls. Window-specific state (uniform buffers, bind groups, instance data) remains per-window. The shared objects are immutable after creation, so no synchronization is needed to read them.

This design achieved roughly a 77% reduction in GPU memory use when multiple windows are open.

### Texture and Buffer Pools

When a window is closed and a new one opened, naive code would allocate fresh GPU textures for the text rendering intermediate and the CRT post-processing intermediate. GPU memory allocations are expensive, and wgpu does not automatically reuse recently freed allocations.

`TexturePool` maintains a small pool of textures bucketed by size. When a window needs a texture, it calls `pool.checkout()`, which returns a `PooledTexture` wrapping the wgpu texture in an RAII guard. Dropping the `PooledTexture` returns it to the pool rather than destroying it. When a window closes, the pool is shrunk to release excess entries. Pool depth is capped at 2 textures per size bucket to avoid holding onto too much memory during normal operation.

`BufferPool` provides the same service for instance and uniform buffers. It is architecturally ready but not yet fully integrated — the texture pool carries most of the benefit in practice.

## crt-theme: CSS-Like Theming

This crate parses theme files written in a CSS-like syntax and produces a `Theme` struct that the renderer can consume. It has no GPU dependencies and no runtime FFI — it is purely a data transformation crate.

The design decision to use CSS syntax is covered in detail in the theming system document. From an architecture perspective, the key point is that `crt-theme` is self-contained: it uses `lightningcss` for parsing, defines its own `Color`, `LinearGradient`, `TextShadow`, and effect configuration types, and exposes a `Mergeable` trait for implementing the CSS cascade.

## The Main Binary

The binary (the `crt` package at the workspace root) is responsible for everything that requires platform integration:

- The winit event loop
- Window creation and lifecycle
- Keyboard and mouse input handling
- The native menu bar (macOS only, via `muda`)
- Config file loading and watching
- Theme registry management
- Profiling and diagnostics

### Event Loop Architecture

CRT uses winit's `ApplicationHandler` trait with `Poll` control flow. The `App` struct implements the handler:

- `resumed()`: Called once on startup. Creates the first window and, on macOS, initializes the native menu bar.
- `window_event()`: Handles per-window events — keyboard input, mouse input, resize, close, focus changes, and redraw requests.
- `about_to_wait()`: Called after all pending events are processed. This is where PTY output is polled for *unfocused* windows, file watchers are checked, and redraws are requested.

Frame timing is managed in `about_to_wait`. The focused window renders at approximately 60 fps; unfocused windows render at approximately 1 fps, running the PTY pump but not submitting full GPU commands. This keeps unfocused shells responsive without burning GPU resources.

### Window Management

Each open window corresponds to a `WindowState` in the `App::windows` HashMap, keyed by `winit::WindowId`. A window contains:

- A `WindowGpuState` with all GPU resources (surface, glyph cache, pipeline instances, pools)
- A `HashMap<TabId, ShellTerminal>` — each tab is an independent shell process
- Interaction state (selection, scroll, search)
- UI overlay state (context menu, toast notifications, zoom indicator)
- An active theme name and theme override state

When a new window is created, it inherits the current working directory from the active tab of the focused window. This is done by reading `/proc/<pid>/cwd` on Linux or using platform-appropriate APIs — the goal is that `Cmd+N` opens a new window already in the same directory you are working in.

When the display scale factor changes (moving a window between a retina and non-retina display, for example), CRT recreates the glyph caches for that window. Font metrics are resolution-dependent, so a cache built at 1x DPI would produce blurry glyphs at 2x.

### Config System

Configuration lives in `~/.config/crt/config.toml` (overridable via `CRT_CONFIG_DIR`). The `Config` struct is deserialized from TOML via serde. It covers shell, font, theme selection, and feature flags.

A `ConfigWatcher` polls the config file for changes at ~1 second intervals (using the `notify` crate). On detection, `reload_config()` re-parses the file and applies changes. If the theme name changed, `reload_theme()` is also called. Toast notifications are shown for parse errors so the user gets immediate feedback about a broken config.

Theme files are watched separately from the config. The theme directory (`~/.config/crt/themes/`) is monitored for file changes. When a `.css` file changes, all themes in the registry are reloaded and all windows re-render with the updated theme. Hot reload is the primary development workflow for theme authors.

## Dependency Flow Summary

```
crt (binary)
├── crt-core      (Terminal, ShellTerminal, PTY)
├── crt-renderer  (GPU rendering, pipelines, effects)
│   └── crt-theme (Theme struct, CSS properties)
└── crt-theme     (Theme struct for direct access)

External crates:
  alacritty_terminal ── vte ──> crt-core
  wgpu ────────────────────────> crt-renderer
  vello (on wgpu) ─────────────> crt-renderer
  lightningcss ────────────────> crt-theme
  winit ───────────────────────> crt (binary)
  muda (macOS) ────────────────> crt (binary)
  notify ──────────────────────> crt (binary)
  portable-pty ────────────────> crt-core
```

Each dependency is deliberately limited to the crate that actually needs it. winit does not appear in `crt-renderer`. `portable-pty` does not appear in `crt-theme`. This keeps the crates reusable and their compile-time requirements minimal.
