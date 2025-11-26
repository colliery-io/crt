//! CRT Terminal with Effects
//!
//! Two-pass rendering:
//! 1. Render text to offscreen texture using swash-based glyph cache
//! 2. Composite text with effects (gradient, grid, glow) to screen

mod config;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use config::Config;
use crt_core::{ShellTerminal, Size};
use crt_renderer::{GlyphCache, GridRenderer, EffectPipeline, TextRenderTarget, TabBar};
use crt_theme::Theme;

use winit::{
    application::ApplicationHandler,
    event::{ElementState, Modifiers, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::{Window, WindowId},
};

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

    let shell_menu = Submenu::with_items(
        "Shell",
        true,
        &[
            &new_tab,
            &new_window,
            &PredefinedMenuItem::separator(),
            &close_tab,
            &close_window,
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

struct GpuState {
    device: wgpu::Device,
    queue: wgpu::Queue,
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

struct App {
    window: Option<Arc<Window>>,
    gpu: Option<GpuState>,
    // Map of tab_id -> shell terminal (one shell per tab)
    shells: HashMap<u64, ShellTerminal>,
    modifiers: Modifiers,
    dirty: bool,
    cols: usize,
    rows: usize,
    // Content hash to skip reshaping when unchanged (per tab)
    content_hashes: HashMap<u64, u64>,
    // Mouse position for tab bar interaction
    cursor_position: (f32, f32),
    // DPI scale factor
    scale_factor: f32,
    // User font scale multiplier (1.0 = default)
    font_scale: f32,
    // Frame counter for startup settling
    frame_count: u32,
    // Double-click detection for tab editing
    last_click_time: Option<Instant>,
    last_click_tab: Option<u64>,
    // Configuration loaded from file
    config: Config,
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
            window: None,
            gpu: None,
            shells: HashMap::new(),
            modifiers: Modifiers::default(),
            dirty: true,
            cols: config.window.columns,
            rows: config.window.rows,
            content_hashes: HashMap::new(),
            cursor_position: (0.0, 0.0),
            scale_factor: 1.0,
            font_scale: 1.0,
            frame_count: 0,
            last_click_time: None,
            last_click_tab: None,
            config,
            #[cfg(target_os = "macos")]
            menu: None,
            #[cfg(target_os = "macos")]
            menu_ids: None,
        }
    }

    /// Create a new shell for a tab
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

    /// Remove shell when tab is closed
    fn remove_shell_for_tab(&mut self, tab_id: u64) {
        self.shells.remove(&tab_id);
        self.content_hashes.remove(&tab_id);
    }

    /// Force re-render of the active tab by clearing its content hash
    fn force_active_tab_redraw(&mut self) {
        if let Some(tab_id) = self.gpu.as_ref().and_then(|g| g.tab_bar.active_tab_id()) {
            self.content_hashes.insert(tab_id, 0);
        }
        self.dirty = true;
    }

    /// Rebuild glyph cache with current font scale
    /// This recreates the atlas with new font size and updates renderers
    fn rebuild_glyph_cache(&mut self) {
        let Some(gpu) = &mut self.gpu else { return };

        // Calculate effective font size (base * DPI scale * user scale)
        let effective_font_size = self.config.font.size * self.scale_factor * self.font_scale;
        log::info!(
            "Rebuilding glyph cache: base={}, scale_factor={}, font_scale={}, effective={}",
            self.config.font.size, self.scale_factor, self.font_scale, effective_font_size
        );

        // Create new glyph cache
        match GlyphCache::new(&gpu.device, FONT_DATA, effective_font_size) {
            Ok(mut new_cache) => {
                new_cache.precache_ascii();
                new_cache.flush(&gpu.queue);

                // Update terminal grid renderer to use new cache
                // (tab_title_renderer keeps using fixed-size tab_glyph_cache)
                gpu.grid_renderer.set_glyph_cache(&gpu.device, &new_cache);

                // Replace the cache
                gpu.glyph_cache = new_cache;

                // Clear all content hashes to force full re-render
                for hash in self.content_hashes.values_mut() {
                    *hash = 0;
                }

                // Recalculate terminal grid size based on new cell dimensions
                if let Some(window) = &self.window {
                    let size = window.inner_size();
                    let cell_width = gpu.glyph_cache.cell_width();
                    let line_height = gpu.glyph_cache.line_height();
                    let tab_bar_height = gpu.tab_bar.height();

                    let padding_physical = 20.0 * self.scale_factor;
                    let tab_bar_physical = tab_bar_height * self.scale_factor;

                    let content_width = (size.width as f32 - padding_physical).max(60.0);
                    let content_height = (size.height as f32 - padding_physical - tab_bar_physical).max(40.0);

                    let new_cols = (content_width / cell_width).max(10.0) as usize;
                    let new_rows = (content_height / line_height).max(4.0) as usize;

                    if new_cols != self.cols || new_rows != self.rows {
                        self.cols = new_cols;
                        self.rows = new_rows;

                        // Resize all shells to new grid dimensions
                        for shell in self.shells.values_mut() {
                            shell.resize(Size::new(new_cols, new_rows));
                        }
                        log::info!("Terminal grid resized to {}x{}", new_cols, new_rows);
                    }
                }

                self.dirty = true;
            }
            Err(e) => {
                log::error!("Failed to rebuild glyph cache: {}", e);
            }
        }
    }

    fn update_text_buffer(&mut self) -> bool {
        let Some(gpu) = &mut self.gpu else { return false };
        let Some(tab_id) = gpu.tab_bar.active_tab_id() else { return false };
        let Some(shell) = self.shells.get(&tab_id) else { return false };

        let term = shell.terminal();
        let content = term.renderable_content();
        let cursor = content.cursor;
        let cursor_point = cursor.point;

        // Collect cells with their grid positions
        // (row, col, char) where row/col are grid indices
        let mut cells: Vec<(i32, usize, char)> = Vec::new();
        let mut current_row = 0i32;
        let mut last_line_raw = i32::MIN;

        // Compute hash while collecting
        let mut hash: u64 = 0xcbf29ce484222325;

        for indexed in content.display_iter {
            let cell = &indexed.cell;
            let point = indexed.point;
            let line_raw = point.line.0;

            // Track row changes
            if last_line_raw != line_raw {
                if last_line_raw != i32::MIN {
                    current_row += 1;
                }
                last_line_raw = line_raw;
            }

            // Determine character (cursor or cell)
            let c = if point.line == cursor_point.line && point.column == cursor_point.column {
                '\u{258F}' // Thin bar cursor (left one-eighth block)
            } else {
                cell.c
            };

            // Hash it
            hash ^= c as u64;
            hash = hash.wrapping_mul(0x100000001b3);

            // Store non-empty characters with their grid position
            if c != ' ' && c != '\0' {
                cells.push((current_row, point.column.0, c));
            }
        }

        // Skip rendering if content unchanged for this tab
        let prev_hash = self.content_hashes.get(&tab_id).copied().unwrap_or(0);
        if hash == prev_hash {
            return false;
        }
        self.content_hashes.insert(tab_id, hash);

        // Clear previous instances
        gpu.grid_renderer.clear();

        // Get cell dimensions (in physical pixels since font is scaled)
        let cell_width = gpu.glyph_cache.cell_width();
        let line_height = gpu.glyph_cache.line_height();
        let text_color = [1.0, 1.0, 1.0, 1.0];

        // Scale logical values to physical pixels
        let scale_factor = self.scale_factor;
        let padding = 10.0 * scale_factor;
        let tab_bar_height = gpu.tab_bar.height() * scale_factor;

        // Position each character at its exact grid cell (offset by tab bar)
        let mut glyphs = Vec::new();
        for (row, col, c) in cells {
            let cell_x = padding + (col as f32) * cell_width;
            let cell_y = tab_bar_height + padding + (row as f32) * line_height;

            if let Some(glyph) = gpu.glyph_cache.position_char(c, cell_x, cell_y) {
                glyphs.push(glyph);
            }
        }
        gpu.grid_renderer.push_glyphs(&glyphs, text_color);

        // Upload any new glyphs to the atlas
        gpu.glyph_cache.flush(&gpu.queue);

        true
    }

    /// Handle menu actions (macOS menu bar)
    #[cfg(target_os = "macos")]
    fn handle_menu_action(&mut self, action: MenuAction, event_loop: &ActiveEventLoop) {
        match action {
            MenuAction::NewTab => {
                if let Some(gpu) = &mut self.gpu {
                    let tab_num = gpu.tab_bar.tab_count() + 1;
                    let tab_id = gpu.tab_bar.add_tab(format!("Terminal {}", tab_num));
                    gpu.tab_bar.select_tab_index(gpu.tab_bar.tab_count() - 1);
                    self.create_shell_for_tab(tab_id);
                    self.dirty = true;
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                }
            }
            MenuAction::NewWindow => {
                // TODO: Implement new window creation
                log::info!("New Window requested (not yet implemented)");
            }
            MenuAction::CloseTab => {
                if let Some(gpu) = &mut self.gpu {
                    if gpu.tab_bar.tab_count() > 1 {
                        if let Some(id) = gpu.tab_bar.active_tab_id() {
                            gpu.tab_bar.close_tab(id);
                            self.remove_shell_for_tab(id);
                            self.dirty = true;
                            if let Some(window) = &self.window {
                                window.request_redraw();
                            }
                        }
                    } else {
                        event_loop.exit();
                    }
                }
            }
            MenuAction::CloseWindow => {
                event_loop.exit();
            }
            MenuAction::Copy => {
                // TODO: Implement copy to clipboard
                log::info!("Copy requested (not yet implemented)");
            }
            MenuAction::Paste => {
                // TODO: Implement paste from clipboard
                log::info!("Paste requested (not yet implemented)");
            }
            MenuAction::SelectAll => {
                // TODO: Implement select all
                log::info!("Select All requested (not yet implemented)");
            }
            MenuAction::Find => {
                // TODO: Implement find
                log::info!("Find requested (not yet implemented)");
            }
            MenuAction::ClearScrollback => {
                // TODO: Implement clear scrollback
                log::info!("Clear Scrollback requested (not yet implemented)");
            }
            MenuAction::ToggleFullScreen => {
                if let Some(window) = &self.window {
                    let is_fullscreen = window.fullscreen().is_some();
                    if is_fullscreen {
                        window.set_fullscreen(None);
                    } else {
                        window.set_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
                    }
                }
            }
            MenuAction::IncreaseFontSize => {
                let new_scale = (self.font_scale + FONT_SCALE_STEP).min(MAX_FONT_SCALE);
                if (new_scale - self.font_scale).abs() > 0.001 {
                    self.font_scale = new_scale;
                    self.rebuild_glyph_cache();
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                }
            }
            MenuAction::DecreaseFontSize => {
                let new_scale = (self.font_scale - FONT_SCALE_STEP).max(MIN_FONT_SCALE);
                if (new_scale - self.font_scale).abs() > 0.001 {
                    self.font_scale = new_scale;
                    self.rebuild_glyph_cache();
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                }
            }
            MenuAction::ResetFontSize => {
                if (self.font_scale - 1.0).abs() > 0.001 {
                    self.font_scale = 1.0;
                    self.rebuild_glyph_cache();
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                }
            }
            MenuAction::Minimize => {
                if let Some(window) = &self.window {
                    window.set_minimized(true);
                }
            }
            MenuAction::NextTab => {
                if let Some(gpu) = &mut self.gpu {
                    gpu.tab_bar.next_tab();
                }
                self.force_active_tab_redraw();
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            MenuAction::PrevTab => {
                if let Some(gpu) = &mut self.gpu {
                    gpu.tab_bar.prev_tab();
                }
                self.force_active_tab_redraw();
                if let Some(window) = &self.window {
                    window.request_redraw();
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

    /// Select tab by index (used by menu actions)
    #[cfg(target_os = "macos")]
    fn select_tab_by_index(&mut self, index: usize) {
        if let Some(gpu) = &mut self.gpu {
            gpu.tab_bar.select_tab_index(index);
        }
        self.force_active_tab_redraw();
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        // Initial window size - will be refined after font loads
        // Use approximate cell width (actual is ~8.4 for 14pt JetBrains Mono)
        let font_size = self.config.font.size;
        let line_height = font_size * self.config.font.line_height;
        let approx_cell_width = font_size * 0.6;
        let tab_bar_height = 36; // Default tab bar height
        let width = (self.cols as f32 * approx_cell_width) as u32 + 20; // 10px padding each side
        let height = (self.rows as f32 * line_height) as u32 + 20 + tab_bar_height;

        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title(&self.config.window.title)
                        .with_inner_size(winit::dpi::LogicalSize::new(width, height)),
                )
                .expect("Failed to create window"),
        );

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let surface = instance.create_surface(window.clone()).unwrap();

        let (adapter, device, queue) = pollster::block_on(async {
            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    compatible_surface: Some(&surface),
                    ..Default::default()
                })
                .await
                .unwrap();
            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor::default())
                .await
                .unwrap();
            (adapter, device, queue)
        });

        let size = window.inner_size();
        let scale_factor = window.scale_factor() as f32;
        self.scale_factor = scale_factor;
        log::info!("Window scale factor: {}", scale_factor);

        let caps = surface.get_capabilities(&adapter);
        let format = caps.formats[0];

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        // Initialize swash glyph cache with scaled font size for HiDPI
        let scaled_font_size = self.config.font.size * scale_factor;
        let mut glyph_cache = GlyphCache::new(&device, FONT_DATA, scaled_font_size)
            .expect("Failed to create glyph cache");
        glyph_cache.precache_ascii();
        glyph_cache.flush(&queue);

        let mut grid_renderer = GridRenderer::new(&device, format);
        grid_renderer.set_glyph_cache(&device, &glyph_cache);
        grid_renderer.update_screen_size(&queue, size.width as f32, size.height as f32);

        // Create fixed-size glyph cache for tab titles (12pt, doesn't scale with zoom)
        let tab_font_size = 12.0 * scale_factor;
        let mut tab_glyph_cache = GlyphCache::new(&device, FONT_DATA, tab_font_size)
            .expect("Failed to create tab glyph cache");
        tab_glyph_cache.precache_ascii();
        tab_glyph_cache.flush(&queue);

        // Create separate renderer for tab titles (avoids buffer conflicts)
        let mut tab_title_renderer = GridRenderer::new(&device, format);
        tab_title_renderer.set_glyph_cache(&device, &tab_glyph_cache);
        tab_title_renderer.update_screen_size(&queue, size.width as f32, size.height as f32);

        // Create offscreen text render target
        let text_target = TextRenderTarget::new(&device, size.width, size.height, format);

        // Load theme from config (tries ~/.config/crt/themes/{name}.css, then embedded fallback)
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

        // Create effect pipeline with loaded theme
        let mut effect_pipeline = EffectPipeline::new(&device, format);
        effect_pipeline.set_theme(theme.clone());

        // Create composite bind group
        let composite_bind_group = Some(effect_pipeline.create_bind_group(&device, &text_target.view));

        // Create tab bar with scale factor and theme
        let mut tab_bar = TabBar::new(&device, format);
        tab_bar.set_scale_factor(scale_factor);
        tab_bar.set_theme(theme.tabs);
        tab_bar.resize(size.width as f32, size.height as f32);

        self.gpu = Some(GpuState {
            device,
            queue,
            surface,
            config,
            glyph_cache,
            grid_renderer,
            tab_glyph_cache,
            tab_title_renderer,
            text_target,
            effect_pipeline,
            composite_bind_group,
            tab_bar,
        });

        // Create shell for the initial tab (tab id 0)
        self.create_shell_for_tab(0);

        self.window = Some(window);

        // Initialize macOS menu bar
        #[cfg(target_os = "macos")]
        {
            let (menu, ids) = build_menu_bar();
            menu.init_for_nsapp();
            self.menu = Some(menu); // Keep menu alive for the app lifetime
            self.menu_ids = Some(ids);
            log::info!("macOS menu bar initialized");
        }

        log::info!("Minimal terminal initialized: {}x{}", self.cols, self.rows);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),

            WindowEvent::ModifiersChanged(new_modifiers) => {
                self.modifiers = new_modifiers;
            }

            WindowEvent::KeyboardInput { event, .. } => {
                if event.state != ElementState::Pressed {
                    return;
                }

                // Use Cmd on macOS, Ctrl on other platforms
                #[cfg(target_os = "macos")]
                let mod_pressed = self.modifiers.state().super_key();
                #[cfg(not(target_os = "macos"))]
                let mod_pressed = self.modifiers.state().control_key();

                // Check if we're editing a tab title - route input there first
                // But let Cmd/Ctrl shortcuts pass through
                let is_editing = self.gpu.as_ref().map(|g| g.tab_bar.is_editing()).unwrap_or(false);
                if is_editing && !mod_pressed {
                    let mut handled = true;
                    let mut need_redraw = true;

                    if let Some(gpu) = &mut self.gpu {
                        match &event.logical_key {
                            Key::Named(NamedKey::Enter) => {
                                gpu.tab_bar.confirm_editing();
                            }
                            Key::Named(NamedKey::Escape) => {
                                gpu.tab_bar.cancel_editing();
                            }
                            Key::Named(NamedKey::Backspace) => {
                                gpu.tab_bar.edit_backspace();
                            }
                            Key::Named(NamedKey::Delete) => {
                                gpu.tab_bar.edit_delete();
                            }
                            Key::Named(NamedKey::ArrowLeft) => {
                                gpu.tab_bar.edit_cursor_left();
                            }
                            Key::Named(NamedKey::ArrowRight) => {
                                gpu.tab_bar.edit_cursor_right();
                            }
                            Key::Named(NamedKey::Home) => {
                                gpu.tab_bar.edit_cursor_home();
                            }
                            Key::Named(NamedKey::End) => {
                                gpu.tab_bar.edit_cursor_end();
                            }
                            Key::Named(NamedKey::Space) => {
                                gpu.tab_bar.edit_insert_char(' ');
                            }
                            Key::Character(c) => {
                                // Insert characters (but not control sequences)
                                for ch in c.chars() {
                                    if !ch.is_control() {
                                        gpu.tab_bar.edit_insert_char(ch);
                                    }
                                }
                            }
                            _ => {
                                handled = false;
                                need_redraw = false;
                            }
                        }
                    }

                    if need_redraw {
                        self.dirty = true;
                        if let Some(window) = &self.window {
                            window.request_redraw();
                        }
                    }

                    if handled {
                        return;
                    }
                }

                if mod_pressed {
                    // If editing a tab title, confirm it before processing shortcuts
                    if let Some(gpu) = &mut self.gpu {
                        if gpu.tab_bar.is_editing() {
                            gpu.tab_bar.confirm_editing();
                            self.dirty = true;
                        }
                    }

                    match &event.logical_key {
                        Key::Character(c) if c.as_str() == "q" => {
                            event_loop.exit();
                            return;
                        }
                        Key::Character(c) if c.as_str() == "w" => {
                            // Close current tab (or exit if last tab)
                            if let Some(gpu) = &mut self.gpu {
                                if gpu.tab_bar.tab_count() > 1 {
                                    if let Some(id) = gpu.tab_bar.active_tab_id() {
                                        gpu.tab_bar.close_tab(id);
                                        self.remove_shell_for_tab(id);
                                        self.dirty = true;
                                        if let Some(window) = &self.window {
                                            window.request_redraw();
                                        }
                                        return;
                                    }
                                }
                            }
                            event_loop.exit();
                            return;
                        }
                        Key::Character(c) if c.as_str() == "t" => {
                            // New tab
                            if let Some(gpu) = &mut self.gpu {
                                let tab_num = gpu.tab_bar.tab_count() + 1;
                                let tab_id = gpu.tab_bar.add_tab(format!("Terminal {}", tab_num));
                                gpu.tab_bar.select_tab_index(gpu.tab_bar.tab_count() - 1);
                                // Create shell for the new tab
                                self.create_shell_for_tab(tab_id);
                                self.dirty = true;
                                if let Some(window) = &self.window {
                                    window.request_redraw();
                                }
                            }
                            return;
                        }
                        Key::Character(c) if c.as_str() == "[" && self.modifiers.state().shift_key() => {
                            // Previous tab (Cmd+Shift+[)
                            if let Some(gpu) = &mut self.gpu {
                                gpu.tab_bar.prev_tab();
                            }
                            self.force_active_tab_redraw();
                            if let Some(window) = &self.window {
                                window.request_redraw();
                            }
                            return;
                        }
                        Key::Character(c) if c.as_str() == "]" && self.modifiers.state().shift_key() => {
                            // Next tab (Cmd+Shift+])
                            if let Some(gpu) = &mut self.gpu {
                                gpu.tab_bar.next_tab();
                            }
                            self.force_active_tab_redraw();
                            if let Some(window) = &self.window {
                                window.request_redraw();
                            }
                            return;
                        }
                        // Tab selection with Cmd+1-9
                        Key::Character(c) if c.len() == 1 => {
                            if let Some(digit) = c.chars().next().and_then(|ch| ch.to_digit(10)) {
                                if digit >= 1 && digit <= 9 {
                                    if let Some(gpu) = &mut self.gpu {
                                        let index = (digit - 1) as usize;
                                        gpu.tab_bar.select_tab_index(index);
                                    }
                                    self.force_active_tab_redraw();
                                    if let Some(window) = &self.window {
                                        window.request_redraw();
                                    }
                                    return;
                                }
                            }
                        }
                        _ => {}
                    }
                }

                // Send input to active shell
                let tab_id = self.gpu.as_ref().and_then(|g| g.tab_bar.active_tab_id());
                if let Some(tab_id) = tab_id {
                    if let Some(shell) = self.shells.get(&tab_id) {
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
                            self.dirty = true;
                            if let Some(window) = &self.window {
                                window.request_redraw();
                            }
                        }
                    }
                }
            }

            WindowEvent::Resized(new_size) => {
                if new_size.width < 100 || new_size.height < 80 {
                    return;
                }

                // Use stored scale factor for DPI-aware sizing
                let scale_factor = self.scale_factor;

                // Use actual font metrics (in physical pixels since font is scaled)
                let (cell_width, line_height, tab_bar_height) = if let Some(gpu) = &self.gpu {
                    (gpu.glyph_cache.cell_width(), gpu.glyph_cache.line_height(), gpu.tab_bar.height())
                } else {
                    // Fallback using config values
                    let font_size = self.config.font.size;
                    (font_size * 0.6 * scale_factor, font_size * self.config.font.line_height * scale_factor, 36.0)
                };

                // Work in physical pixels - scale logical values to physical
                let padding_physical = 20.0 * scale_factor; // 10px on each side
                let tab_bar_physical = tab_bar_height * scale_factor;

                let content_width = (new_size.width as f32 - padding_physical).max(60.0);
                let content_height = (new_size.height as f32 - padding_physical - tab_bar_physical).max(40.0);
                let new_cols = (content_width / cell_width) as usize;
                let new_rows = (content_height / line_height) as usize;
                let new_cols = new_cols.max(10);
                let new_rows = new_rows.max(4);

                self.cols = new_cols;
                self.rows = new_rows;

                // Resize all shells
                for shell in self.shells.values_mut() {
                    shell.resize(Size::new(new_cols, new_rows));
                }

                if let Some(gpu) = &mut self.gpu {
                    gpu.config.width = new_size.width;
                    gpu.config.height = new_size.height;
                    gpu.surface.configure(&gpu.device, &gpu.config);

                    gpu.grid_renderer.update_screen_size(
                        &gpu.queue,
                        new_size.width as f32,
                        new_size.height as f32,
                    );
                    gpu.tab_title_renderer.update_screen_size(
                        &gpu.queue,
                        new_size.width as f32,
                        new_size.height as f32,
                    );

                    // Resize tab bar
                    gpu.tab_bar.resize(new_size.width as f32, new_size.height as f32);

                    // Resize text render target
                    gpu.text_target.resize(&gpu.device, new_size.width, new_size.height, gpu.config.format);

                    // Recreate composite bind group with new text target
                    gpu.composite_bind_group = Some(
                        gpu.effect_pipeline.create_bind_group(&gpu.device, &gpu.text_target.view)
                    );
                }

                self.dirty = true;
                // Force re-render by clearing all content hashes
                for hash in self.content_hashes.values_mut() {
                    *hash = 0;
                }
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }

            WindowEvent::CursorMoved { position, .. } => {
                self.cursor_position = (position.x as f32, position.y as f32);
            }

            WindowEvent::MouseInput { state: ElementState::Pressed, button: winit::event::MouseButton::Left, .. } => {
                let (x, y) = self.cursor_position;
                let now = Instant::now();
                let double_click_threshold = std::time::Duration::from_millis(400);

                // Handle tab bar clicks
                let mut tab_closed = None;
                let mut tab_switched = false;
                let mut started_editing = false;

                if let Some(gpu) = &mut self.gpu {
                    // If we're editing and clicked outside the tab being edited, confirm and save
                    if gpu.tab_bar.is_editing() {
                        if let Some(editing_id) = gpu.tab_bar.editing_tab_id() {
                            // Check if we clicked on a different area
                            if let Some((tab_id, _)) = gpu.tab_bar.hit_test(x, y) {
                                if tab_id != editing_id {
                                    // Clicked on different tab - confirm edit and select new tab
                                    gpu.tab_bar.confirm_editing();
                                    gpu.tab_bar.select_tab(tab_id);
                                    tab_switched = true;
                                }
                            } else {
                                // Clicked outside tabs - confirm edit (save changes)
                                gpu.tab_bar.confirm_editing();
                            }
                        }
                    } else {
                        // Not editing - check for tab clicks
                        if let Some((tab_id, is_close)) = gpu.tab_bar.hit_test(x, y) {
                            if is_close {
                                // Close button clicked
                                if gpu.tab_bar.tab_count() > 1 {
                                    gpu.tab_bar.close_tab(tab_id);
                                    tab_closed = Some(tab_id);
                                    tab_switched = true;
                                }
                            } else {
                                // Check for double-click on same tab
                                let is_double_click = self.last_click_time
                                    .map(|t| now.duration_since(t) < double_click_threshold)
                                    .unwrap_or(false)
                                    && self.last_click_tab == Some(tab_id);

                                if is_double_click {
                                    // Double-click - start editing
                                    gpu.tab_bar.start_editing(tab_id);
                                    started_editing = true;
                                    self.last_click_time = None;
                                    self.last_click_tab = None;
                                } else {
                                    // Single click - select tab and record for double-click detection
                                    gpu.tab_bar.select_tab(tab_id);
                                    tab_switched = true;
                                    self.last_click_time = Some(now);
                                    self.last_click_tab = Some(tab_id);
                                }
                            }
                        } else {
                            // Clicked outside tabs
                            self.last_click_time = None;
                            self.last_click_tab = None;
                        }
                    }
                }

                // Remove shell for closed tab (after releasing borrow of self.gpu)
                if let Some(tab_id) = tab_closed {
                    self.remove_shell_for_tab(tab_id);
                }

                // Force redraw when tab changes or editing starts
                if tab_switched || started_editing {
                    self.force_active_tab_redraw();
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                }
            }

            WindowEvent::RedrawRequested => {
                // Track frame count for startup settling
                self.frame_count = self.frame_count.saturating_add(1);

                // Check for PTY output from active shell
                let active_tab_id = self.gpu.as_ref().and_then(|g| g.tab_bar.active_tab_id());
                if let Some(tab_id) = active_tab_id {
                    if let Some(shell) = self.shells.get_mut(&tab_id) {
                        if shell.process_pty_output() {
                            self.dirty = true;
                        }

                        // Sync terminal title to tab bar (OSC 0/2 escape sequences)
                        if let Some(title) = shell.check_title_change() {
                            if let Some(gpu) = &mut self.gpu {
                                gpu.tab_bar.set_tab_title(tab_id, title);
                            }
                        }
                    }
                }

                // Force re-renders during first 60 frames to let shell output settle
                if self.frame_count < 60 {
                    self.dirty = true;
                    // Clear content hash to force re-render
                    if let Some(tab_id) = active_tab_id {
                        self.content_hashes.insert(tab_id, 0);
                    }
                }

                // Update text buffer if needed
                let text_changed = if self.dirty {
                    self.dirty = false;
                    self.update_text_buffer()
                } else {
                    false
                };

                if let Some(gpu) = &mut self.gpu {
                    let frame = gpu.surface.get_current_texture().unwrap();
                    let frame_view = frame.texture.create_view(&Default::default());

                    let mut encoder = gpu.device.create_command_encoder(&Default::default());

                    // Pass 1: Render text to offscreen texture (only if text changed)
                    if text_changed {
                        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: Some("Text Render Pass"),
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                view: &gpu.text_target.view,
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

                        gpu.grid_renderer.render(&gpu.queue, &mut pass);
                    }

                    // Update effect uniforms
                    gpu.effect_pipeline.update_uniforms(
                        &gpu.queue,
                        gpu.config.width as f32,
                        gpu.config.height as f32,
                    );

                    // Pass 2: Render background to frame
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

                        gpu.effect_pipeline.background.render(&mut pass);
                    }

                    // Pass 3: Composite text with effects
                    if let Some(bind_group) = &gpu.composite_bind_group {
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

                        gpu.effect_pipeline.composite.render(&mut pass, bind_group);
                    }

                    // Pass 4: Render tab bar on top
                    {
                        gpu.tab_bar.prepare(&gpu.queue);

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

                        gpu.tab_bar.render(&mut pass);
                    }

                    // Pass 5: Render tab title text with glow
                    // Uses separate tab_title_renderer to avoid buffer conflicts with terminal text
                    {
                        // Get tab labels and render them
                        let tab_labels = gpu.tab_bar.get_tab_labels();
                        if !tab_labels.is_empty() {
                            gpu.tab_title_renderer.clear();

                            // Get tab colors and shadows from theme
                            let active_color = gpu.tab_bar.active_tab_color();
                            let inactive_color = gpu.tab_bar.inactive_tab_color();
                            let active_shadow = gpu.tab_bar.active_tab_text_shadow();
                            let _inactive_shadow = gpu.tab_bar.inactive_tab_text_shadow();

                            // First pass: render glow layers for active tabs
                            // Render text multiple times with offsets for a pseudo-glow effect
                            if let Some((radius, glow_color)) = active_shadow {
                                let offsets = [
                                    (-1.5, -1.5), (1.5, -1.5), (-1.5, 1.5), (1.5, 1.5),
                                    (-2.0, 0.0), (2.0, 0.0), (0.0, -2.0), (0.0, 2.0),
                                    (-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0),
                                ];

                                // Scale glow alpha based on radius (more radius = stronger glow)
                                let glow_alpha = (glow_color[3] * (radius / 20.0).min(1.0)).min(0.6);
                                let glow_render_color = [glow_color[0], glow_color[1], glow_color[2], glow_alpha];

                                for (x, y, title, is_active, _is_editing) in &tab_labels {
                                    if *is_active {
                                        // Render glow at each offset
                                        for (ox, oy) in &offsets {
                                            let mut glyphs = Vec::new();
                                            let mut char_x = *x + ox;
                                            for c in title.chars() {
                                                if let Some(glyph) = gpu.tab_glyph_cache.position_char(c, char_x, *y + oy) {
                                                    glyphs.push(glyph);
                                                }
                                                char_x += gpu.tab_glyph_cache.cell_width();
                                            }
                                            gpu.tab_title_renderer.push_glyphs(&glyphs, glow_render_color);
                                        }
                                    }
                                }
                            }

                            // Second pass: render actual text on top
                            for (x, y, title, is_active, is_editing) in tab_labels {
                                let mut glyphs = Vec::new();
                                let mut char_x = x;
                                for c in title.chars() {
                                    if let Some(glyph) = gpu.tab_glyph_cache.position_char(c, char_x, y) {
                                        glyphs.push(glyph);
                                    }
                                    char_x += gpu.tab_glyph_cache.cell_width();
                                }

                                // Use theme colors for tabs (brighten active when editing)
                                let text_color = if is_editing {
                                    // Brighten the active color when editing
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
                                gpu.tab_title_renderer.push_glyphs(&glyphs, text_color);
                            }

                            gpu.tab_glyph_cache.flush(&gpu.queue);

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

                            gpu.tab_title_renderer.render(&gpu.queue, &mut pass);
                        }
                    }

                    gpu.queue.submit(std::iter::once(encoder.finish()));
                    frame.present();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // Note: PTY output is now processed exclusively in RedrawRequested
        // to avoid race conditions during startup where chunks with erase
        // sequences could clear content between event callbacks.

        // Handle menu events on macOS
        #[cfg(target_os = "macos")]
        if let Some(ids) = &self.menu_ids {
            if let Ok(event) = MenuEvent::receiver().try_recv() {
                if let Some(action) = menu_id_to_action(event.id(), ids) {
                    self.handle_menu_action(action, event_loop);
                }
            }
        }

        // Always request redraw for continuous animation
        if let Some(window) = &self.window {
            window.request_redraw();
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
