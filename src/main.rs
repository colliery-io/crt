//! CRT Terminal with Effects
//!
//! Two-pass rendering:
//! 1. Render text to offscreen texture using swash-based glyph cache
//! 2. Composite text with effects (gradient, grid, glow) to screen

mod config;

use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::Hasher;
use std::sync::Arc;
use std::time::Instant;

use config::Config;
use crt_core::{ShellTerminal, Size};
use crt_renderer::{GlyphCache, GridRenderer, EffectPipeline, TextRenderTarget, TabBar, TabPosition};
use crt_theme::Theme;

use winit::{
    application::ApplicationHandler,
    event::{ElementState, Modifiers, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::{Window, WindowId},
};

#[cfg(target_os = "macos")]
use winit::platform::macos::WindowAttributesExtMacOS;

#[cfg(target_os = "macos")]
use muda::{
    accelerator::{Accelerator, Code, Modifiers as AccelMods},
    Menu, MenuEvent, MenuItem, MenuId, PredefinedMenuItem, Submenu,
};

// Font scale bounds
const MIN_FONT_SCALE: f32 = 0.5;
const MAX_FONT_SCALE: f32 = 3.0;
const FONT_SCALE_STEP: f32 = 0.1;

// Embedded fonts - MesloLGS NF (Nerd Font with powerline glyphs)
const FONT_DATA: &[u8] = include_bytes!("../assets/fonts/MesloLGS-NF-Regular.ttf");
const FONT_DATA_BOLD: &[u8] = include_bytes!("../assets/fonts/MesloLGS-NF-Bold.ttf");
const FONT_DATA_ITALIC: &[u8] = include_bytes!("../assets/fonts/MesloLGS-NF-Italic.ttf");
const FONT_DATA_BOLD_ITALIC: &[u8] = include_bytes!("../assets/fonts/MesloLGS-NF-BoldItalic.ttf");

/// Menu action identifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MenuAction {
    // Shell menu
    NewTab,
    NewWindow,
    CloseTab,
    CloseWindow,
    Quit,
    // Edit menu
    Copy,
    Paste,
    SelectAll,
    Find,
    ClearScrollback,
    // View menu
    ToggleFullScreen,
    IncreaseFontSize,
    DecreaseFontSize,
    ResetFontSize,
    // Window menu
    Minimize,
    NextTab,
    PrevTab,
    SelectTab1,
    SelectTab2,
    SelectTab3,
    SelectTab4,
    SelectTab5,
    SelectTab6,
    SelectTab7,
    SelectTab8,
    SelectTab9,
}

/// Menu item IDs stored for event handling
#[cfg(target_os = "macos")]
struct MenuIds {
    new_tab: MenuId,
    new_window: MenuId,
    close_tab: MenuId,
    close_window: MenuId,
    quit: MenuId,
    copy: MenuId,
    paste: MenuId,
    select_all: MenuId,
    find: MenuId,
    clear_scrollback: MenuId,
    toggle_fullscreen: MenuId,
    increase_font: MenuId,
    decrease_font: MenuId,
    reset_font: MenuId,
    minimize: MenuId,
    next_tab: MenuId,
    prev_tab: MenuId,
    select_tab: [MenuId; 9],
}

#[cfg(target_os = "macos")]
fn build_menu_bar() -> (Menu, MenuIds) {
    let menu = Menu::new();

    // Shell menu
    let new_tab = MenuItem::with_id(
        "new_tab",
        "New Tab",
        true,
        Some(Accelerator::new(Some(AccelMods::SUPER), Code::KeyT)),
    );
    let new_window = MenuItem::with_id(
        "new_window",
        "New Window",
        true,
        Some(Accelerator::new(Some(AccelMods::SUPER), Code::KeyN)),
    );
    let close_tab = MenuItem::with_id(
        "close_tab",
        "Close Tab",
        true,
        Some(Accelerator::new(Some(AccelMods::SUPER), Code::KeyW)),
    );
    let close_window = MenuItem::with_id(
        "close_window",
        "Close Window",
        true,
        Some(Accelerator::new(Some(AccelMods::SUPER | AccelMods::SHIFT), Code::KeyW)),
    );
    let quit = MenuItem::with_id(
        "quit",
        "Quit CRT",
        true,
        Some(Accelerator::new(Some(AccelMods::SUPER), Code::KeyQ)),
    );

    let shell_menu = Submenu::with_items(
        "Shell",
        true,
        &[
            &new_tab,
            &new_window,
            &PredefinedMenuItem::separator(),
            &close_tab,
            &close_window,
            &PredefinedMenuItem::separator(),
            &quit,
        ],
    ).unwrap();

    // Edit menu
    let copy = MenuItem::with_id(
        "copy",
        "Copy",
        true,
        Some(Accelerator::new(Some(AccelMods::SUPER), Code::KeyC)),
    );
    let paste = MenuItem::with_id(
        "paste",
        "Paste",
        true,
        Some(Accelerator::new(Some(AccelMods::SUPER), Code::KeyV)),
    );
    let select_all = MenuItem::with_id(
        "select_all",
        "Select All",
        true,
        Some(Accelerator::new(Some(AccelMods::SUPER), Code::KeyA)),
    );
    let find = MenuItem::with_id(
        "find",
        "Find...",
        true,
        Some(Accelerator::new(Some(AccelMods::SUPER), Code::KeyF)),
    );
    let clear_scrollback = MenuItem::with_id(
        "clear_scrollback",
        "Clear Scrollback",
        true,
        Some(Accelerator::new(Some(AccelMods::SUPER), Code::KeyK)),
    );

    let edit_menu = Submenu::with_items(
        "Edit",
        true,
        &[
            &copy,
            &paste,
            &select_all,
            &PredefinedMenuItem::separator(),
            &find,
            &PredefinedMenuItem::separator(),
            &clear_scrollback,
        ],
    ).unwrap();

    // View menu
    let toggle_fullscreen = MenuItem::with_id(
        "toggle_fullscreen",
        "Enter Full Screen",
        true,
        Some(Accelerator::new(Some(AccelMods::SUPER | AccelMods::CONTROL), Code::KeyF)),
    );
    let increase_font = MenuItem::with_id(
        "increase_font",
        "Increase Font Size",
        true,
        Some(Accelerator::new(Some(AccelMods::SUPER), Code::Equal)),
    );
    let decrease_font = MenuItem::with_id(
        "decrease_font",
        "Decrease Font Size",
        true,
        Some(Accelerator::new(Some(AccelMods::SUPER), Code::Minus)),
    );
    let reset_font = MenuItem::with_id(
        "reset_font",
        "Reset Font Size",
        true,
        Some(Accelerator::new(Some(AccelMods::SUPER), Code::Digit0)),
    );

    let view_menu = Submenu::with_items(
        "View",
        true,
        &[
            &toggle_fullscreen,
            &PredefinedMenuItem::separator(),
            &increase_font,
            &decrease_font,
            &reset_font,
        ],
    ).unwrap();

    // Window menu
    let minimize = MenuItem::with_id(
        "minimize",
        "Minimize",
        true,
        Some(Accelerator::new(Some(AccelMods::SUPER), Code::KeyM)),
    );
    let next_tab = MenuItem::with_id(
        "next_tab",
        "Show Next Tab",
        true,
        Some(Accelerator::new(Some(AccelMods::SUPER | AccelMods::SHIFT), Code::BracketRight)),
    );
    let prev_tab = MenuItem::with_id(
        "prev_tab",
        "Show Previous Tab",
        true,
        Some(Accelerator::new(Some(AccelMods::SUPER | AccelMods::SHIFT), Code::BracketLeft)),
    );

    // Tab selection items
    let select_tab_1 = MenuItem::with_id(
        "select_tab_1",
        "Select Tab 1",
        true,
        Some(Accelerator::new(Some(AccelMods::SUPER), Code::Digit1)),
    );
    let select_tab_2 = MenuItem::with_id(
        "select_tab_2",
        "Select Tab 2",
        true,
        Some(Accelerator::new(Some(AccelMods::SUPER), Code::Digit2)),
    );
    let select_tab_3 = MenuItem::with_id(
        "select_tab_3",
        "Select Tab 3",
        true,
        Some(Accelerator::new(Some(AccelMods::SUPER), Code::Digit3)),
    );
    let select_tab_4 = MenuItem::with_id(
        "select_tab_4",
        "Select Tab 4",
        true,
        Some(Accelerator::new(Some(AccelMods::SUPER), Code::Digit4)),
    );
    let select_tab_5 = MenuItem::with_id(
        "select_tab_5",
        "Select Tab 5",
        true,
        Some(Accelerator::new(Some(AccelMods::SUPER), Code::Digit5)),
    );
    let select_tab_6 = MenuItem::with_id(
        "select_tab_6",
        "Select Tab 6",
        true,
        Some(Accelerator::new(Some(AccelMods::SUPER), Code::Digit6)),
    );
    let select_tab_7 = MenuItem::with_id(
        "select_tab_7",
        "Select Tab 7",
        true,
        Some(Accelerator::new(Some(AccelMods::SUPER), Code::Digit7)),
    );
    let select_tab_8 = MenuItem::with_id(
        "select_tab_8",
        "Select Tab 8",
        true,
        Some(Accelerator::new(Some(AccelMods::SUPER), Code::Digit8)),
    );
    let select_tab_9 = MenuItem::with_id(
        "select_tab_9",
        "Select Tab 9",
        true,
        Some(Accelerator::new(Some(AccelMods::SUPER), Code::Digit9)),
    );

    let window_menu = Submenu::with_items(
        "Window",
        true,
        &[
            &minimize,
            &PredefinedMenuItem::separator(),
            &next_tab,
            &prev_tab,
            &PredefinedMenuItem::separator(),
            &select_tab_1,
            &select_tab_2,
            &select_tab_3,
            &select_tab_4,
            &select_tab_5,
            &select_tab_6,
            &select_tab_7,
            &select_tab_8,
            &select_tab_9,
        ],
    ).unwrap();

    // Build the menu bar
    menu.append(&shell_menu).unwrap();
    menu.append(&edit_menu).unwrap();
    menu.append(&view_menu).unwrap();
    menu.append(&window_menu).unwrap();

    let ids = MenuIds {
        new_tab: new_tab.id().clone(),
        new_window: new_window.id().clone(),
        close_tab: close_tab.id().clone(),
        close_window: close_window.id().clone(),
        quit: quit.id().clone(),
        copy: copy.id().clone(),
        paste: paste.id().clone(),
        select_all: select_all.id().clone(),
        find: find.id().clone(),
        clear_scrollback: clear_scrollback.id().clone(),
        toggle_fullscreen: toggle_fullscreen.id().clone(),
        increase_font: increase_font.id().clone(),
        decrease_font: decrease_font.id().clone(),
        reset_font: reset_font.id().clone(),
        minimize: minimize.id().clone(),
        next_tab: next_tab.id().clone(),
        prev_tab: prev_tab.id().clone(),
        select_tab: [
            select_tab_1.id().clone(),
            select_tab_2.id().clone(),
            select_tab_3.id().clone(),
            select_tab_4.id().clone(),
            select_tab_5.id().clone(),
            select_tab_6.id().clone(),
            select_tab_7.id().clone(),
            select_tab_8.id().clone(),
            select_tab_9.id().clone(),
        ],
    };

    (menu, ids)
}

#[cfg(target_os = "macos")]
fn menu_id_to_action(id: &MenuId, ids: &MenuIds) -> Option<MenuAction> {
    if *id == ids.new_tab { return Some(MenuAction::NewTab); }
    if *id == ids.new_window { return Some(MenuAction::NewWindow); }
    if *id == ids.close_tab { return Some(MenuAction::CloseTab); }
    if *id == ids.close_window { return Some(MenuAction::CloseWindow); }
    if *id == ids.quit { return Some(MenuAction::Quit); }
    if *id == ids.copy { return Some(MenuAction::Copy); }
    if *id == ids.paste { return Some(MenuAction::Paste); }
    if *id == ids.select_all { return Some(MenuAction::SelectAll); }
    if *id == ids.find { return Some(MenuAction::Find); }
    if *id == ids.clear_scrollback { return Some(MenuAction::ClearScrollback); }
    if *id == ids.toggle_fullscreen { return Some(MenuAction::ToggleFullScreen); }
    if *id == ids.increase_font { return Some(MenuAction::IncreaseFontSize); }
    if *id == ids.decrease_font { return Some(MenuAction::DecreaseFontSize); }
    if *id == ids.reset_font { return Some(MenuAction::ResetFontSize); }
    if *id == ids.minimize { return Some(MenuAction::Minimize); }
    if *id == ids.next_tab { return Some(MenuAction::NextTab); }
    if *id == ids.prev_tab { return Some(MenuAction::PrevTab); }
    if *id == ids.select_tab[0] { return Some(MenuAction::SelectTab1); }
    if *id == ids.select_tab[1] { return Some(MenuAction::SelectTab2); }
    if *id == ids.select_tab[2] { return Some(MenuAction::SelectTab3); }
    if *id == ids.select_tab[3] { return Some(MenuAction::SelectTab4); }
    if *id == ids.select_tab[4] { return Some(MenuAction::SelectTab5); }
    if *id == ids.select_tab[5] { return Some(MenuAction::SelectTab6); }
    if *id == ids.select_tab[6] { return Some(MenuAction::SelectTab7); }
    if *id == ids.select_tab[7] { return Some(MenuAction::SelectTab8); }
    if *id == ids.select_tab[8] { return Some(MenuAction::SelectTab9); }
    None
}

/// Shared GPU resources across all windows
struct SharedGpuState {
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
}

/// Per-window GPU state (surface tied to specific window)
struct WindowGpuState {
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,

    // Text rendering with swash glyph cache (scales with zoom)
    glyph_cache: GlyphCache,
    grid_renderer: GridRenderer,

    // Fixed-size glyph cache for tab titles (doesn't scale with zoom)
    tab_glyph_cache: GlyphCache,
    // Separate renderer for tab titles to avoid buffer conflicts
    // (terminal and tab titles render in different passes but the GPU
    // commands are batched, so they need separate instance buffers)
    tab_title_renderer: GridRenderer,

    // Offscreen text target
    text_target: TextRenderTarget,

    // Effect pipeline
    effect_pipeline: EffectPipeline,
    composite_bind_group: Option<wgpu::BindGroup>,

    // Tab bar
    tab_bar: TabBar,
}

/// Per-window state containing window handle, GPU state, shells, and interaction state
struct WindowState {
    window: Arc<Window>,
    gpu: WindowGpuState,
    // Map of tab_id -> shell (each window has its own tabs)
    shells: HashMap<u64, ShellTerminal>,
    // Content hash to skip reshaping when unchanged (per tab)
    content_hashes: HashMap<u64, u64>,
    // Window-specific sizing
    cols: usize,
    rows: usize,
    scale_factor: f32,
    // User font scale multiplier (1.0 = default)
    font_scale: f32,
    // Rendering state
    dirty: bool,
    frame_count: u32,
    // Interaction state
    cursor_position: (f32, f32),
    last_click_time: Option<Instant>,
    last_click_tab: Option<u64>,
    // Tab ID counter for this window
    next_tab_id: u64,
}

struct App {
    // Multiple windows keyed by winit WindowId
    windows: HashMap<WindowId, WindowState>,
    // Shared GPU resources (device, queue, etc.)
    shared_gpu: Option<SharedGpuState>,
    // Track which window is focused (for menu action routing)
    focused_window: Option<WindowId>,
    // Shared state across all windows
    config: Config,
    modifiers: Modifiers,
    // Flag to request new window creation (used by menu actions)
    pending_new_window: bool,
    // macOS menu bar - must keep Menu alive for the duration of the app
    #[cfg(target_os = "macos")]
    menu: Option<Menu>,
    #[cfg(target_os = "macos")]
    menu_ids: Option<MenuIds>,
}

impl App {
    fn new() -> Self {
        let config = Config::load();

        Self {
            windows: HashMap::new(),
            shared_gpu: None,
            focused_window: None,
            config,
            modifiers: Modifiers::default(),
            pending_new_window: false,
            #[cfg(target_os = "macos")]
            menu: None,
            #[cfg(target_os = "macos")]
            menu_ids: None,
        }
    }

    /// Initialize shared GPU resources (called once)
    fn init_shared_gpu(&mut self) {
        if self.shared_gpu.is_some() {
            return;
        }

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());

        // Request adapter without a surface first (we'll create surfaces per-window)
        let adapter = pollster::block_on(async {
            instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::default(),
                    compatible_surface: None,
                    force_fallback_adapter: false,
                })
                .await
                .expect("Failed to find suitable GPU adapter")
        });

        let (device, queue) = pollster::block_on(async {
            adapter
                .request_device(&wgpu::DeviceDescriptor::default())
                .await
                .expect("Failed to create device")
        });

        self.shared_gpu = Some(SharedGpuState {
            instance,
            adapter,
            device,
            queue,
        });
    }

    /// Create a new window and add it to the windows map
    fn create_window(&mut self, event_loop: &ActiveEventLoop) -> WindowId {
        // Ensure shared GPU is initialized
        self.init_shared_gpu();
        let shared = self.shared_gpu.as_ref().unwrap();

        // Initial window size
        let font_size = self.config.font.size;
        let line_height = font_size * self.config.font.line_height;
        let approx_cell_width = font_size * 0.6;
        let tab_bar_height = 36;
        let cols = self.config.window.columns;
        let rows = self.config.window.rows;
        let width = (cols as f32 * approx_cell_width) as u32 + 20;
        let height = (rows as f32 * line_height) as u32 + 20 + tab_bar_height;

        // Build window attributes
        let mut window_attrs = Window::default_attributes()
            .with_title(&self.config.window.title)
            .with_inner_size(winit::dpi::LogicalSize::new(width, height));

        // On macOS, give each window a unique tabbing identifier to prevent
        // automatic window tabbing (where new windows appear as tabs in the same frame)
        #[cfg(target_os = "macos")]
        {
            // Generate unique tabbing identifier based on window count
            let unique_id = format!("crt-window-{}", self.windows.len());
            window_attrs = window_attrs.with_tabbing_identifier(&unique_id);
        }

        let window = Arc::new(
            event_loop
                .create_window(window_attrs)
                .expect("Failed to create window"),
        );

        let window_id = window.id();
        let size = window.inner_size();
        let scale_factor = window.scale_factor() as f32;

        // Create surface for this window
        let surface = shared.instance.create_surface(window.clone()).unwrap();
        let caps = surface.get_capabilities(&shared.adapter);
        let format = caps.formats[0];

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&shared.device, &surface_config);

        // Initialize glyph cache with scaled font size
        let scaled_font_size = self.config.font.size * scale_factor;
        let mut glyph_cache = GlyphCache::new(&shared.device, FONT_DATA, scaled_font_size)
            .expect("Failed to create glyph cache");
        glyph_cache.precache_ascii();
        glyph_cache.flush(&shared.queue);

        let mut grid_renderer = GridRenderer::new(&shared.device, format);
        grid_renderer.set_glyph_cache(&shared.device, &glyph_cache);
        grid_renderer.update_screen_size(&shared.queue, size.width as f32, size.height as f32);

        // Tab title glyph cache (fixed size)
        let tab_font_size = 12.0 * scale_factor;
        let mut tab_glyph_cache = GlyphCache::new(&shared.device, FONT_DATA, tab_font_size)
            .expect("Failed to create tab glyph cache");
        tab_glyph_cache.precache_ascii();
        tab_glyph_cache.flush(&shared.queue);

        let mut tab_title_renderer = GridRenderer::new(&shared.device, format);
        tab_title_renderer.set_glyph_cache(&shared.device, &tab_glyph_cache);
        tab_title_renderer.update_screen_size(&shared.queue, size.width as f32, size.height as f32);

        // Text render target
        let text_target = TextRenderTarget::new(&shared.device, size.width, size.height, format);

        // Load theme
        let theme = match self.config.theme_css() {
            Some(css) => match Theme::from_css(&css) {
                Ok(t) => t,
                Err(e) => {
                    log::warn!("Failed to parse theme '{}': {:?}, using default", self.config.theme.name, e);
                    Theme::default()
                }
            },
            None => {
                log::warn!("Theme '{}' not found, using default", self.config.theme.name);
                Theme::default()
            }
        };

        // Effect pipeline
        let mut effect_pipeline = EffectPipeline::new(&shared.device, format);
        effect_pipeline.set_theme(theme.clone());

        let composite_bind_group = Some(effect_pipeline.create_bind_group(&shared.device, &text_target.view));

        // Tab bar
        let mut tab_bar = TabBar::new(&shared.device, format);
        tab_bar.set_scale_factor(scale_factor);
        tab_bar.set_theme(theme.tabs);
        tab_bar.set_position(match self.config.window.tab_position {
            config::TabPosition::Top => TabPosition::Top,
            config::TabPosition::Bottom => TabPosition::Bottom,
            config::TabPosition::Left => TabPosition::Left,
            config::TabPosition::Right => TabPosition::Right,
        });
        tab_bar.resize(size.width as f32, size.height as f32);

        // Create GPU state for this window
        let gpu = WindowGpuState {
            surface,
            config: surface_config,
            glyph_cache,
            grid_renderer,
            tab_glyph_cache,
            tab_title_renderer,
            text_target,
            effect_pipeline,
            composite_bind_group,
            tab_bar,
        };

        // Create initial shell
        let mut shells = HashMap::new();
        let mut content_hashes = HashMap::new();
        match ShellTerminal::new(Size::new(cols, rows)) {
            Ok(shell) => {
                log::info!("Shell spawned for initial tab in new window");
                shells.insert(0, shell);
                content_hashes.insert(0, 0);
            }
            Err(e) => {
                log::error!("Failed to spawn shell: {}", e);
            }
        }

        let window_state = WindowState {
            window,
            gpu,
            shells,
            content_hashes,
            cols,
            rows,
            scale_factor,
            font_scale: 1.0,
            dirty: true,
            frame_count: 0,
            cursor_position: (0.0, 0.0),
            last_click_time: None,
            last_click_tab: None,
            next_tab_id: 1, // Tab 0 already created
        };

        self.windows.insert(window_id, window_state);
        self.focused_window = Some(window_id);

        log::info!("Created new window {:?}, total windows: {}", window_id, self.windows.len());
        window_id
    }

    /// Get the focused window state (for menu actions)
    fn focused_window_mut(&mut self) -> Option<&mut WindowState> {
        self.focused_window.and_then(|id| self.windows.get_mut(&id))
    }

    /// Close a specific window
    fn close_window(&mut self, window_id: WindowId) {
        if let Some(_state) = self.windows.remove(&window_id) {
            log::info!("Closed window {:?}, remaining windows: {}", window_id, self.windows.len());
            // Clear focused window if it was the one we closed
            if self.focused_window == Some(window_id) {
                self.focused_window = self.windows.keys().next().copied();
            }
        }
    }

    /// Handle menu actions (macOS menu bar)
    #[cfg(target_os = "macos")]
    fn handle_menu_action(&mut self, action: MenuAction, event_loop: &ActiveEventLoop) {
        match action {
            MenuAction::NewTab => {
                if let Some(state) = self.focused_window_mut() {
                    let tab_num = state.gpu.tab_bar.tab_count() + 1;
                    let tab_id = state.gpu.tab_bar.add_tab(format!("Terminal {}", tab_num));
                    state.gpu.tab_bar.select_tab_index(state.gpu.tab_bar.tab_count() - 1);
                    // Create shell for new tab
                    match ShellTerminal::new(Size::new(state.cols, state.rows)) {
                        Ok(shell) => {
                            state.shells.insert(tab_id, shell);
                            state.content_hashes.insert(tab_id, 0);
                        }
                        Err(e) => log::error!("Failed to spawn shell: {}", e),
                    }
                    state.dirty = true;
                    state.window.request_redraw();
                }
            }
            MenuAction::NewWindow => {
                log::info!("New Window requested");
                self.pending_new_window = true;
            }
            MenuAction::CloseTab => {
                let should_close_window = if let Some(state) = self.focused_window_mut() {
                    if state.gpu.tab_bar.tab_count() > 1 {
                        if let Some(id) = state.gpu.tab_bar.active_tab_id() {
                            state.gpu.tab_bar.close_tab(id);
                            state.shells.remove(&id);
                            state.content_hashes.remove(&id);
                            state.dirty = true;
                            state.window.request_redraw();
                        }
                        false
                    } else {
                        true // Close window when last tab
                    }
                } else {
                    false
                };
                if should_close_window {
                    if let Some(id) = self.focused_window {
                        self.close_window(id);
                    }
                }
            }
            MenuAction::CloseWindow => {
                if let Some(id) = self.focused_window {
                    self.close_window(id);
                }
            }
            MenuAction::Quit => {
                log::info!("Quit requested from menu");
                event_loop.exit();
            }
            MenuAction::Copy => {
                log::info!("Copy requested (not yet implemented)");
            }
            MenuAction::Paste => {
                log::info!("Paste requested (not yet implemented)");
            }
            MenuAction::SelectAll => {
                log::info!("Select All requested (not yet implemented)");
            }
            MenuAction::Find => {
                log::info!("Find requested (not yet implemented)");
            }
            MenuAction::ClearScrollback => {
                log::info!("Clear Scrollback requested (not yet implemented)");
            }
            MenuAction::ToggleFullScreen => {
                if let Some(state) = self.focused_window_mut() {
                    let is_fullscreen = state.window.fullscreen().is_some();
                    if is_fullscreen {
                        state.window.set_fullscreen(None);
                    } else {
                        state.window.set_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
                    }
                }
            }
            MenuAction::IncreaseFontSize => {
                if let Some(state) = self.focused_window_mut() {
                    let new_scale = (state.font_scale + FONT_SCALE_STEP).min(MAX_FONT_SCALE);
                    if (new_scale - state.font_scale).abs() > 0.001 {
                        state.font_scale = new_scale;
                        // TODO: Rebuild glyph cache for this window
                        state.dirty = true;
                        state.window.request_redraw();
                    }
                }
            }
            MenuAction::DecreaseFontSize => {
                if let Some(state) = self.focused_window_mut() {
                    let new_scale = (state.font_scale - FONT_SCALE_STEP).max(MIN_FONT_SCALE);
                    if (new_scale - state.font_scale).abs() > 0.001 {
                        state.font_scale = new_scale;
                        // TODO: Rebuild glyph cache for this window
                        state.dirty = true;
                        state.window.request_redraw();
                    }
                }
            }
            MenuAction::ResetFontSize => {
                if let Some(state) = self.focused_window_mut() {
                    if (state.font_scale - 1.0).abs() > 0.001 {
                        state.font_scale = 1.0;
                        // TODO: Rebuild glyph cache for this window
                        state.dirty = true;
                        state.window.request_redraw();
                    }
                }
            }
            MenuAction::Minimize => {
                if let Some(state) = self.focused_window_mut() {
                    state.window.set_minimized(true);
                }
            }
            MenuAction::NextTab => {
                if let Some(state) = self.focused_window_mut() {
                    state.gpu.tab_bar.next_tab();
                    // Force redraw of active tab
                    if let Some(tab_id) = state.gpu.tab_bar.active_tab_id() {
                        state.content_hashes.insert(tab_id, 0);
                    }
                    state.dirty = true;
                    state.window.request_redraw();
                }
            }
            MenuAction::PrevTab => {
                if let Some(state) = self.focused_window_mut() {
                    state.gpu.tab_bar.prev_tab();
                    if let Some(tab_id) = state.gpu.tab_bar.active_tab_id() {
                        state.content_hashes.insert(tab_id, 0);
                    }
                    state.dirty = true;
                    state.window.request_redraw();
                }
            }
            MenuAction::SelectTab1 => self.select_tab_by_index(0),
            MenuAction::SelectTab2 => self.select_tab_by_index(1),
            MenuAction::SelectTab3 => self.select_tab_by_index(2),
            MenuAction::SelectTab4 => self.select_tab_by_index(3),
            MenuAction::SelectTab5 => self.select_tab_by_index(4),
            MenuAction::SelectTab6 => self.select_tab_by_index(5),
            MenuAction::SelectTab7 => self.select_tab_by_index(6),
            MenuAction::SelectTab8 => self.select_tab_by_index(7),
            MenuAction::SelectTab9 => self.select_tab_by_index(8),
        }
    }

    /// Select tab by index in focused window
    #[cfg(target_os = "macos")]
    fn select_tab_by_index(&mut self, index: usize) {
        if let Some(state) = self.focused_window_mut() {
            state.gpu.tab_bar.select_tab_index(index);
            if let Some(tab_id) = state.gpu.tab_bar.active_tab_id() {
                state.content_hashes.insert(tab_id, 0);
            }
            state.dirty = true;
            state.window.request_redraw();
        }
    }
}

/// WindowState helper methods for per-window operations
impl WindowState {
    /// Update text buffer for this window's active shell
    fn update_text_buffer(&mut self, shared_gpu: &SharedGpuState) -> bool {
        let active_tab_id = self.gpu.tab_bar.active_tab_id();
        let shell = active_tab_id.and_then(|id| self.shells.get(&id));

        if shell.is_none() {
            return false;
        }
        let shell = shell.unwrap();
        let terminal = shell.terminal();

        // Compute content hash to avoid re-rendering unchanged content
        let mut hasher = DefaultHasher::new();
        let content = terminal.renderable_content();
        hasher.write_i32(content.cursor.point.line.0);
        hasher.write_usize(content.cursor.point.column.0);
        for cell in content.display_iter {
            hasher.write_u32(cell.c as u32);
        }
        let content_hash = hasher.finish();

        // Check if content changed
        let tab_id = active_tab_id.unwrap();
        let prev_hash = self.content_hashes.get(&tab_id).copied().unwrap_or(0);
        if content_hash == prev_hash && prev_hash != 0 {
            return false; // No changes
        }
        self.content_hashes.insert(tab_id, content_hash);

        // Get content offset (excluding tab bar)
        let (offset_x, offset_y) = self.gpu.tab_bar.content_offset();

        // Re-read content since we consumed it above
        let content = terminal.renderable_content();
        self.gpu.grid_renderer.clear();

        let cell_width = self.gpu.glyph_cache.cell_width();
        let line_height = self.gpu.glyph_cache.line_height();
        let padding = 10.0 * self.scale_factor;

        // Cursor info
        let cursor = content.cursor;
        let cursor_point = cursor.point;

        // Render cells
        for cell in content.display_iter {
            let col = cell.point.column.0;
            let row = cell.point.line.0;

            let is_cursor = cell.point.column == cursor_point.column
                && cell.point.line == cursor_point.line;

            let x = offset_x + padding + (col as f32 * cell_width);
            let y = offset_y + padding + (row as f32 * line_height);

            let c = cell.c;
            if c == ' ' && !is_cursor {
                continue;
            }

            // Default text color (could be extended to support ANSI colors)
            let color = [0.9, 0.9, 0.9, 1.0];

            if let Some(glyph) = self.gpu.glyph_cache.position_char(c, x, y) {
                self.gpu.grid_renderer.push_glyphs(&[glyph], color);
            }

            // Render cursor as a highlighted space
            if is_cursor {
                let cursor_color = [0.8, 0.8, 0.2, 0.8];
                // Use a block character for cursor visualization
                if let Some(glyph) = self.gpu.glyph_cache.position_char('\u{2588}', x, y) {
                    self.gpu.grid_renderer.push_glyphs(&[glyph], cursor_color);
                }
            }
        }

        self.gpu.glyph_cache.flush(&shared_gpu.queue);
        true
    }

    /// Create a shell for a new tab
    fn create_shell_for_tab(&mut self, tab_id: u64) {
        match ShellTerminal::new(Size::new(self.cols, self.rows)) {
            Ok(shell) => {
                log::info!("Shell spawned for tab {}", tab_id);
                self.shells.insert(tab_id, shell);
                self.content_hashes.insert(tab_id, 0);
            }
            Err(e) => {
                log::error!("Failed to spawn shell for tab {}: {}", tab_id, e);
            }
        }
    }

    /// Remove shell for a closed tab
    fn remove_shell_for_tab(&mut self, tab_id: u64) {
        self.shells.remove(&tab_id);
        self.content_hashes.remove(&tab_id);
        log::info!("Removed shell for tab {}", tab_id);
    }

    /// Force redraw of active tab by clearing its content hash
    fn force_active_tab_redraw(&mut self) {
        if let Some(tab_id) = self.gpu.tab_bar.active_tab_id() {
            self.content_hashes.insert(tab_id, 0);
            self.dirty = true;
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // Only create first window if none exist
        if !self.windows.is_empty() {
            return;
        }

        // Create the first window
        self.create_window(event_loop);

        // Initialize macOS menu bar (once per app)
        #[cfg(target_os = "macos")]
        if self.menu.is_none() {
            let (menu, ids) = build_menu_bar();
            menu.init_for_nsapp();
            self.menu = Some(menu);
            self.menu_ids = Some(ids);
            log::info!("macOS menu bar initialized");
        }

        log::info!("CRT Terminal initialized with {} window(s)", self.windows.len());
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        // Route event to the specific window
        let Some(state) = self.windows.get_mut(&id) else {
            return; // Window not found (already closed)
        };

        match event {
            WindowEvent::CloseRequested => {
                // Close this specific window
                log::info!("Window {:?} close requested", id);
                self.windows.remove(&id);
                if self.focused_window == Some(id) {
                    self.focused_window = self.windows.keys().next().copied();
                }
                log::info!("Remaining windows: {}", self.windows.len());
                // Note: Don't exit app when all windows close (macOS behavior)
            }

            WindowEvent::Focused(focused) => {
                if focused {
                    self.focused_window = Some(id);
                    log::debug!("Window {:?} focused", id);
                }
            }

            WindowEvent::ModifiersChanged(new_modifiers) => {
                self.modifiers = new_modifiers;
            }

            WindowEvent::KeyboardInput { event, .. } => {
                if event.state != ElementState::Pressed {
                    return;
                }

                // Re-borrow state after modifiers update
                let Some(state) = self.windows.get_mut(&id) else { return };

                // Use Cmd on macOS, Ctrl on other platforms
                #[cfg(target_os = "macos")]
                let mod_pressed = self.modifiers.state().super_key();
                #[cfg(not(target_os = "macos"))]
                let mod_pressed = self.modifiers.state().control_key();

                // Check if we're editing a tab title - route input there first
                let is_editing = state.gpu.tab_bar.is_editing();
                if is_editing && !mod_pressed {
                    let mut handled = true;
                    let mut need_redraw = true;

                    match &event.logical_key {
                        Key::Named(NamedKey::Enter) => {
                            state.gpu.tab_bar.confirm_editing();
                        }
                        Key::Named(NamedKey::Escape) => {
                            state.gpu.tab_bar.cancel_editing();
                        }
                        Key::Named(NamedKey::Backspace) => {
                            state.gpu.tab_bar.edit_backspace();
                        }
                        Key::Named(NamedKey::Delete) => {
                            state.gpu.tab_bar.edit_delete();
                        }
                        Key::Named(NamedKey::ArrowLeft) => {
                            state.gpu.tab_bar.edit_cursor_left();
                        }
                        Key::Named(NamedKey::ArrowRight) => {
                            state.gpu.tab_bar.edit_cursor_right();
                        }
                        Key::Named(NamedKey::Home) => {
                            state.gpu.tab_bar.edit_cursor_home();
                        }
                        Key::Named(NamedKey::End) => {
                            state.gpu.tab_bar.edit_cursor_end();
                        }
                        Key::Named(NamedKey::Space) => {
                            state.gpu.tab_bar.edit_insert_char(' ');
                        }
                        Key::Character(c) => {
                            for ch in c.chars() {
                                if !ch.is_control() {
                                    state.gpu.tab_bar.edit_insert_char(ch);
                                }
                            }
                        }
                        _ => {
                            handled = false;
                            need_redraw = false;
                        }
                    }

                    if need_redraw {
                        state.dirty = true;
                        state.window.request_redraw();
                    }

                    if handled {
                        return;
                    }
                }

                if mod_pressed {
                    // If editing a tab title, confirm it before processing shortcuts
                    if state.gpu.tab_bar.is_editing() {
                        state.gpu.tab_bar.confirm_editing();
                        state.dirty = true;
                    }

                    match &event.logical_key {
                        Key::Character(c) if c.as_str() == "q" => {
                            event_loop.exit();
                            return;
                        }
                        Key::Character(c) if c.as_str() == "w" => {
                            // Close current tab (or close window if last tab)
                            if state.gpu.tab_bar.tab_count() > 1 {
                                if let Some(tab_id) = state.gpu.tab_bar.active_tab_id() {
                                    state.gpu.tab_bar.close_tab(tab_id);
                                    state.remove_shell_for_tab(tab_id);
                                    state.dirty = true;
                                    state.window.request_redraw();
                                    return;
                                }
                            }
                            // Close window when last tab is closed
                            self.windows.remove(&id);
                            if self.focused_window == Some(id) {
                                self.focused_window = self.windows.keys().next().copied();
                            }
                            return;
                        }
                        Key::Character(c) if c.as_str() == "n" => {
                            // New window - set flag to create in about_to_wait
                            log::info!("New window requested via Cmd+N");
                            self.pending_new_window = true;
                            return;
                        }
                        Key::Character(c) if c.as_str() == "t" => {
                            // New tab
                            let tab_num = state.gpu.tab_bar.tab_count() + 1;
                            let tab_id = state.gpu.tab_bar.add_tab(format!("Terminal {}", tab_num));
                            state.gpu.tab_bar.select_tab_index(state.gpu.tab_bar.tab_count() - 1);
                            state.create_shell_for_tab(tab_id);
                            state.dirty = true;
                            state.window.request_redraw();
                            return;
                        }
                        Key::Character(c) if c.as_str() == "[" && self.modifiers.state().shift_key() => {
                            state.gpu.tab_bar.prev_tab();
                            state.force_active_tab_redraw();
                            state.window.request_redraw();
                            return;
                        }
                        Key::Character(c) if c.as_str() == "]" && self.modifiers.state().shift_key() => {
                            state.gpu.tab_bar.next_tab();
                            state.force_active_tab_redraw();
                            state.window.request_redraw();
                            return;
                        }
                        Key::Character(c) if c.len() == 1 => {
                            if let Some(digit) = c.chars().next().and_then(|ch| ch.to_digit(10)) {
                                if digit >= 1 && digit <= 9 {
                                    state.gpu.tab_bar.select_tab_index((digit - 1) as usize);
                                    state.force_active_tab_redraw();
                                    state.window.request_redraw();
                                    return;
                                }
                            }
                        }
                        _ => {}
                    }
                }

                // Send input to active shell
                let tab_id = state.gpu.tab_bar.active_tab_id();
                if let Some(tab_id) = tab_id {
                    if let Some(shell) = state.shells.get(&tab_id) {
                        let mut input_sent = false;
                        match &event.logical_key {
                            Key::Named(NamedKey::Escape) => {
                                shell.send_input(b"\x1b");
                                input_sent = true;
                            }
                            Key::Named(NamedKey::Enter) => {
                                shell.send_input(b"\r");
                                input_sent = true;
                            }
                            Key::Named(NamedKey::Backspace) => {
                                shell.send_input(b"\x7f");
                                input_sent = true;
                            }
                            Key::Named(NamedKey::Tab) => {
                                shell.send_input(b"\t");
                                input_sent = true;
                            }
                            Key::Named(NamedKey::ArrowUp) => {
                                shell.send_input(b"\x1b[A");
                                input_sent = true;
                            }
                            Key::Named(NamedKey::ArrowDown) => {
                                shell.send_input(b"\x1b[B");
                                input_sent = true;
                            }
                            Key::Named(NamedKey::ArrowRight) => {
                                shell.send_input(b"\x1b[C");
                                input_sent = true;
                            }
                            Key::Named(NamedKey::ArrowLeft) => {
                                shell.send_input(b"\x1b[D");
                                input_sent = true;
                            }
                            Key::Named(NamedKey::Space) => {
                                shell.send_input(b" ");
                                input_sent = true;
                            }
                            Key::Character(c) => {
                                if !mod_pressed {
                                    shell.send_input(c.as_bytes());
                                    input_sent = true;
                                }
                            }
                            _ => {}
                        }
                        if input_sent {
                            state.dirty = true;
                            state.window.request_redraw();
                        }
                    }
                }
            }

            WindowEvent::Resized(new_size) => {
                if new_size.width < 100 || new_size.height < 80 {
                    return;
                }

                let scale_factor = state.scale_factor;
                let cell_width = state.gpu.glyph_cache.cell_width();
                let line_height = state.gpu.glyph_cache.line_height();
                let is_horizontal = state.gpu.tab_bar.is_horizontal();
                let tab_bar_size = if is_horizontal {
                    state.gpu.tab_bar.height()
                } else {
                    state.gpu.tab_bar.width()
                };

                let padding_physical = 20.0 * scale_factor;
                let tab_bar_physical = tab_bar_size * scale_factor;

                let (content_width, content_height) = if is_horizontal {
                    (
                        (new_size.width as f32 - padding_physical).max(60.0),
                        (new_size.height as f32 - padding_physical - tab_bar_physical).max(40.0),
                    )
                } else {
                    (
                        (new_size.width as f32 - padding_physical - tab_bar_physical).max(60.0),
                        (new_size.height as f32 - padding_physical).max(40.0),
                    )
                };

                let new_cols = ((content_width / cell_width) as usize).max(10);
                let new_rows = ((content_height / line_height) as usize).max(4);

                state.cols = new_cols;
                state.rows = new_rows;

                // Resize all shells in this window
                for shell in state.shells.values_mut() {
                    shell.resize(Size::new(new_cols, new_rows));
                }

                // Update GPU resources
                let shared = self.shared_gpu.as_ref().unwrap();
                state.gpu.config.width = new_size.width;
                state.gpu.config.height = new_size.height;
                state.gpu.surface.configure(&shared.device, &state.gpu.config);

                state.gpu.grid_renderer.update_screen_size(
                    &shared.queue,
                    new_size.width as f32,
                    new_size.height as f32,
                );
                state.gpu.tab_title_renderer.update_screen_size(
                    &shared.queue,
                    new_size.width as f32,
                    new_size.height as f32,
                );

                state.gpu.tab_bar.resize(new_size.width as f32, new_size.height as f32);
                state.gpu.text_target.resize(&shared.device, new_size.width, new_size.height, state.gpu.config.format);
                state.gpu.composite_bind_group = Some(
                    state.gpu.effect_pipeline.create_bind_group(&shared.device, &state.gpu.text_target.view)
                );

                state.dirty = true;
                for hash in state.content_hashes.values_mut() {
                    *hash = 0;
                }
                state.window.request_redraw();
            }

            WindowEvent::CursorMoved { position, .. } => {
                state.cursor_position = (position.x as f32, position.y as f32);
            }

            WindowEvent::MouseInput { state: ElementState::Pressed, button: winit::event::MouseButton::Left, .. } => {
                let (x, y) = state.cursor_position;
                let now = Instant::now();
                let double_click_threshold = std::time::Duration::from_millis(400);

                let mut tab_closed = None;
                let mut tab_switched = false;
                let mut started_editing = false;

                if state.gpu.tab_bar.is_editing() {
                    if let Some(editing_id) = state.gpu.tab_bar.editing_tab_id() {
                        if let Some((tab_id, _)) = state.gpu.tab_bar.hit_test(x, y) {
                            if tab_id != editing_id {
                                state.gpu.tab_bar.confirm_editing();
                                state.gpu.tab_bar.select_tab(tab_id);
                                tab_switched = true;
                            }
                        } else {
                            state.gpu.tab_bar.confirm_editing();
                        }
                    }
                } else {
                    if let Some((tab_id, is_close)) = state.gpu.tab_bar.hit_test(x, y) {
                        if is_close {
                            if state.gpu.tab_bar.tab_count() > 1 {
                                state.gpu.tab_bar.close_tab(tab_id);
                                tab_closed = Some(tab_id);
                                tab_switched = true;
                            }
                        } else {
                            let is_double_click = state.last_click_time
                                .map(|t| now.duration_since(t) < double_click_threshold)
                                .unwrap_or(false)
                                && state.last_click_tab == Some(tab_id);

                            if is_double_click {
                                state.gpu.tab_bar.start_editing(tab_id);
                                started_editing = true;
                                state.last_click_time = None;
                                state.last_click_tab = None;
                            } else {
                                state.gpu.tab_bar.select_tab(tab_id);
                                tab_switched = true;
                                state.last_click_time = Some(now);
                                state.last_click_tab = Some(tab_id);
                            }
                        }
                    } else {
                        state.last_click_time = None;
                        state.last_click_tab = None;
                    }
                }

                if let Some(tab_id) = tab_closed {
                    state.remove_shell_for_tab(tab_id);
                }

                if tab_switched || started_editing {
                    state.force_active_tab_redraw();
                    state.window.request_redraw();
                }
            }

            WindowEvent::RedrawRequested => {
                state.frame_count = state.frame_count.saturating_add(1);

                // Process PTY output from active shell
                let active_tab_id = state.gpu.tab_bar.active_tab_id();
                if let Some(tab_id) = active_tab_id {
                    if let Some(shell) = state.shells.get_mut(&tab_id) {
                        if shell.process_pty_output() {
                            state.dirty = true;
                        }

                        if let Some(title) = shell.check_title_change() {
                            state.gpu.tab_bar.set_tab_title(tab_id, title);
                        }
                    }
                }

                // Force re-renders during first 60 frames
                if state.frame_count < 60 {
                    state.dirty = true;
                    if let Some(tab_id) = active_tab_id {
                        state.content_hashes.insert(tab_id, 0);
                    }
                }

                // Update text buffer
                let shared = self.shared_gpu.as_ref().unwrap();
                let text_changed = if state.dirty {
                    state.dirty = false;
                    state.update_text_buffer(shared)
                } else {
                    false
                };

                // Render
                let frame = match state.gpu.surface.get_current_texture() {
                    Ok(f) => f,
                    Err(e) => {
                        log::warn!("Failed to get surface texture: {:?}", e);
                        return;
                    }
                };
                let frame_view = frame.texture.create_view(&Default::default());

                let mut encoder = shared.device.create_command_encoder(&Default::default());

                // Pass 1: Render text to offscreen texture
                if text_changed {
                    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("Text Render Pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &state.gpu.text_target.view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });

                    state.gpu.grid_renderer.render(&shared.queue, &mut pass);
                }

                // Update effect uniforms
                state.gpu.effect_pipeline.update_uniforms(
                    &shared.queue,
                    state.gpu.config.width as f32,
                    state.gpu.config.height as f32,
                );

                // Pass 2: Render background
                {
                    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("Background Render Pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &frame_view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });

                    state.gpu.effect_pipeline.background.render(&mut pass);
                }

                // Pass 3: Composite text with effects
                if let Some(bind_group) = &state.gpu.composite_bind_group {
                    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("Composite Render Pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &frame_view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });

                    state.gpu.effect_pipeline.composite.render(&mut pass, bind_group);
                }

                // Pass 4: Render tab bar
                {
                    state.gpu.tab_bar.prepare(&shared.queue);

                    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("Tab Bar Render Pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &frame_view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });

                    state.gpu.tab_bar.render(&mut pass);
                }

                // Pass 5: Render tab title text with glow
                {
                    let tab_labels = state.gpu.tab_bar.get_tab_labels();
                    if !tab_labels.is_empty() {
                        state.gpu.tab_title_renderer.clear();

                        let active_color = state.gpu.tab_bar.active_tab_color();
                        let inactive_color = state.gpu.tab_bar.inactive_tab_color();
                        let active_shadow = state.gpu.tab_bar.active_tab_text_shadow();

                        // First pass: render glow layers for active tabs
                        if let Some((radius, glow_color)) = active_shadow {
                            let offsets = [
                                (-1.5, -1.5), (1.5, -1.5), (-1.5, 1.5), (1.5, 1.5),
                                (-2.0, 0.0), (2.0, 0.0), (0.0, -2.0), (0.0, 2.0),
                                (-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0),
                            ];

                            let glow_alpha = (glow_color[3] * (radius / 20.0).min(1.0)).min(0.6);
                            let glow_render_color = [glow_color[0], glow_color[1], glow_color[2], glow_alpha];

                            for (x, y, title, is_active, _is_editing) in &tab_labels {
                                if *is_active {
                                    for (ox, oy) in &offsets {
                                        let mut glyphs = Vec::new();
                                        let mut char_x = *x + ox;
                                        for c in title.chars() {
                                            if let Some(glyph) = state.gpu.tab_glyph_cache.position_char(c, char_x, *y + oy) {
                                                glyphs.push(glyph);
                                            }
                                            char_x += state.gpu.tab_glyph_cache.cell_width();
                                        }
                                        state.gpu.tab_title_renderer.push_glyphs(&glyphs, glow_render_color);
                                    }
                                }
                            }
                        }

                        // Second pass: render actual text on top
                        for (x, y, title, is_active, is_editing) in tab_labels {
                            let mut glyphs = Vec::new();
                            let mut char_x = x;
                            for c in title.chars() {
                                if let Some(glyph) = state.gpu.tab_glyph_cache.position_char(c, char_x, y) {
                                    glyphs.push(glyph);
                                }
                                char_x += state.gpu.tab_glyph_cache.cell_width();
                            }

                            let text_color = if is_editing {
                                [
                                    (active_color[0] * 1.2).min(1.0),
                                    (active_color[1] * 1.2).min(1.0),
                                    (active_color[2] * 1.2).min(1.0),
                                    active_color[3],
                                ]
                            } else if is_active {
                                active_color
                            } else {
                                inactive_color
                            };
                            state.gpu.tab_title_renderer.push_glyphs(&glyphs, text_color);
                        }

                        state.gpu.tab_glyph_cache.flush(&shared.queue);

                        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: Some("Tab Title Render Pass"),
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                view: &frame_view,
                                resolve_target: None,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Load,
                                    store: wgpu::StoreOp::Store,
                                },
                            })],
                            depth_stencil_attachment: None,
                            timestamp_writes: None,
                            occlusion_query_set: None,
                        });

                        state.gpu.tab_title_renderer.render(&shared.queue, &mut pass);
                    }
                }

                shared.queue.submit(std::iter::once(encoder.finish()));
                frame.present();
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // Handle menu events on macOS
        #[cfg(target_os = "macos")]
        if let Some(ids) = &self.menu_ids {
            if let Ok(event) = MenuEvent::receiver().try_recv() {
                if let Some(action) = menu_id_to_action(event.id(), ids) {
                    self.handle_menu_action(action, event_loop);
                }
            }
        }

        // Handle pending new window request
        if self.pending_new_window {
            self.pending_new_window = false;
            log::info!("Creating new window from pending request");
            self.create_window(event_loop);
        }

        // Request redraw for all windows (continuous animation)
        for state in self.windows.values() {
            state.window.request_redraw();
        }
    }
}

fn main() {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("warn,crt=info"),
    )
    .init();

    log::info!("CRT Terminal with Effects - swash renderer + effect pipeline");

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App::new();
    event_loop.run_app(&mut app).unwrap();
}
