//! CRT Terminal with Effects
//!
//! Two-pass rendering:
//! 1. Render text to offscreen texture using swash-based glyph cache
//! 2. Composite text with effects (gradient, grid, glow) to screen

mod config;
mod gpu;
mod input;
mod menu;
mod render;
mod window;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use config::Config;
use crt_core::{ShellTerminal, Size, Scroll};
use crt_renderer::{GlyphCache, GridRenderer, RectRenderer, EffectPipeline, TextRenderTarget, TabBar, FontVariants};
use crt_theme::Theme;
use gpu::{SharedGpuState, WindowGpuState};
use input::{
    TabEditResult, handle_tab_editing, handle_shell_input, handle_tab_click, handle_resize,
    handle_terminal_mouse_button, handle_terminal_mouse_move,
    handle_terminal_mouse_release, handle_terminal_scroll,
    clear_terminal_selection, get_terminal_selection_text, get_clipboard_content, set_clipboard_content,
    paste_to_terminal, MOUSE_BUTTON_LEFT, MOUSE_BUTTON_MIDDLE, MOUSE_BUTTON_RIGHT,
    find_url_at_position, find_url_index_at_position, open_url,
};
use menu::MenuAction;
use render::render_frame;
use window::WindowState;

use winit::{
    application::ApplicationHandler,
    event::{ElementState, Modifiers, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::{Window, WindowId},
};

#[cfg(target_os = "macos")]
use winit::platform::macos::WindowAttributesExtMacOS;

#[cfg(target_os = "macos")]
use muda::{Menu, MenuEvent};

#[cfg(target_os = "macos")]
use menu::{MenuIds, build_menu_bar, menu_id_to_action};

// Font scale bounds
const MIN_FONT_SCALE: f32 = 0.5;
const MAX_FONT_SCALE: f32 = 3.0;
const FONT_SCALE_STEP: f32 = 0.1;

// Embedded fonts - MesloLGS NF (Nerd Font with powerline glyphs)
const FONT_REGULAR: &[u8] = include_bytes!("../assets/fonts/MesloLGS-NF-Regular.ttf");
const FONT_BOLD: &[u8] = include_bytes!("../assets/fonts/MesloLGS-NF-Bold.ttf");
const FONT_ITALIC: &[u8] = include_bytes!("../assets/fonts/MesloLGS-NF-Italic.ttf");
const FONT_BOLD_ITALIC: &[u8] = include_bytes!("../assets/fonts/MesloLGS-NF-BoldItalic.ttf");

struct App {
    windows: HashMap<WindowId, WindowState>,
    shared_gpu: Option<SharedGpuState>,
    focused_window: Option<WindowId>,
    config: Config,
    modifiers: Modifiers,
    pending_new_window: bool,
    #[cfg(target_os = "macos")]
    menu: Option<Menu>,
    #[cfg(target_os = "macos")]
    menu_ids: Option<MenuIds>,
}

impl App {
    fn new() -> Self {
        Self {
            windows: HashMap::new(),
            shared_gpu: None,
            focused_window: None,
            config: Config::load(),
            modifiers: Modifiers::default(),
            pending_new_window: false,
            #[cfg(target_os = "macos")]
            menu: None,
            #[cfg(target_os = "macos")]
            menu_ids: None,
        }
    }

    fn init_shared_gpu(&mut self) {
        if self.shared_gpu.is_none() {
            self.shared_gpu = Some(SharedGpuState::new());
        }
    }

    fn create_window(&mut self, event_loop: &ActiveEventLoop) -> WindowId {
        self.init_shared_gpu();
        let shared = self.shared_gpu.as_ref().unwrap();

        // Calculate initial window size
        let font_size = self.config.font.size;
        let line_height = font_size * self.config.font.line_height;
        let approx_cell_width = font_size * 0.6;
        let tab_bar_height = 36;
        let cols = self.config.window.columns;
        let rows = self.config.window.rows;
        let width = (cols as f32 * approx_cell_width) as u32 + 20;
        let height = (rows as f32 * line_height) as u32 + 20 + tab_bar_height;

        // Build window
        let mut window_attrs = Window::default_attributes()
            .with_title(&self.config.window.title)
            .with_inner_size(winit::dpi::LogicalSize::new(width, height));

        #[cfg(target_os = "macos")]
        {
            let unique_id = format!("crt-window-{}", self.windows.len());
            window_attrs = window_attrs.with_tabbing_identifier(&unique_id);
        }

        let window = Arc::new(event_loop.create_window(window_attrs).expect("Failed to create window"));
        let window_id = window.id();
        let size = window.inner_size();
        let scale_factor = window.scale_factor() as f32;

        // Create GPU resources
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

        // Initialize glyph caches with font variants
        let scaled_font_size = self.config.font.size * scale_factor;
        let font_variants = FontVariants::new(FONT_REGULAR.to_vec())
            .with_bold(FONT_BOLD.to_vec())
            .with_italic(FONT_ITALIC.to_vec())
            .with_bold_italic(FONT_BOLD_ITALIC.to_vec());
        let mut glyph_cache = GlyphCache::with_variants(&shared.device, font_variants, scaled_font_size)
            .expect("Failed to create glyph cache");
        glyph_cache.precache_ascii();
        glyph_cache.flush(&shared.queue);

        let mut grid_renderer = GridRenderer::new(&shared.device, format);
        grid_renderer.set_glyph_cache(&shared.device, &glyph_cache);
        grid_renderer.update_screen_size(&shared.queue, size.width as f32, size.height as f32);

        let tab_font_size = 12.0 * scale_factor;
        let mut tab_glyph_cache = GlyphCache::new(&shared.device, FONT_REGULAR, tab_font_size)
            .expect("Failed to create tab glyph cache");
        tab_glyph_cache.precache_ascii();
        tab_glyph_cache.flush(&shared.queue);

        let mut tab_title_renderer = GridRenderer::new(&shared.device, format);
        tab_title_renderer.set_glyph_cache(&shared.device, &tab_glyph_cache);
        tab_title_renderer.update_screen_size(&shared.queue, size.width as f32, size.height as f32);

        // Text render target and effects
        let text_target = TextRenderTarget::new(&shared.device, size.width, size.height, format);
        let theme = self.load_theme();
        let mut effect_pipeline = EffectPipeline::new(&shared.device, format);
        effect_pipeline.set_theme(theme.clone());
        let composite_bind_group = Some(effect_pipeline.create_bind_group(&shared.device, &text_target.view));

        // Tab bar (always at top)
        let mut tab_bar = TabBar::new(&shared.device, format);
        tab_bar.set_scale_factor(scale_factor);
        tab_bar.set_theme(theme.tabs);
        tab_bar.resize(size.width as f32, size.height as f32);

        // Terminal vello renderer for cursor and selection
        let mut terminal_vello = crt_renderer::TerminalVelloRenderer::new(&shared.device);
        // Apply cursor config
        let cursor_shape = match self.config.cursor.style {
            config::CursorStyle::Block => crt_renderer::CursorShape::Block,
            config::CursorStyle::Bar => crt_renderer::CursorShape::Bar,
            config::CursorStyle::Underline => crt_renderer::CursorShape::Underline,
        };
        terminal_vello.set_cursor_shape(cursor_shape);
        terminal_vello.set_blink_enabled(self.config.cursor.blink);
        terminal_vello.set_blink_interval_ms(self.config.cursor.blink_interval_ms);

        // Rect renderer for cell backgrounds
        let rect_renderer = RectRenderer::new(&shared.device, format);

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
            terminal_vello,
            rect_renderer,
        };

        // Create initial shell
        let mut shells = HashMap::new();
        let mut content_hashes = HashMap::new();
        if let Ok(shell) = ShellTerminal::new(Size::new(cols, rows)) {
            log::info!("Shell spawned for initial tab");
            shells.insert(0, shell);
            content_hashes.insert(0, 0);
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
            mouse_pressed: false,
            selection_click_count: 0,
            last_selection_click_time: None,
            last_selection_click_pos: None,
            cached_render: Default::default(),
            detected_urls: Vec::new(),
            hovered_url_index: None,
        };

        self.windows.insert(window_id, window_state);
        self.focused_window = Some(window_id);
        log::info!("Created window {:?}, total: {}", window_id, self.windows.len());
        window_id
    }

    fn load_theme(&self) -> Theme {
        match self.config.theme_css() {
            Some(css) => Theme::from_css(&css).unwrap_or_else(|e| {
                log::warn!("Failed to parse theme: {:?}", e);
                Theme::default()
            }),
            None => {
                log::warn!("Theme not found, using default");
                Theme::default()
            }
        }
    }

    fn focused_window_mut(&mut self) -> Option<&mut WindowState> {
        self.focused_window.and_then(|id| self.windows.get_mut(&id))
    }

    fn close_window(&mut self, window_id: WindowId) {
        if self.windows.remove(&window_id).is_some() {
            log::info!("Closed window {:?}, remaining: {}", window_id, self.windows.len());
            if self.focused_window == Some(window_id) {
                self.focused_window = self.windows.keys().next().copied();
            }
        }
    }

    #[cfg(target_os = "macos")]
    fn handle_menu_action(&mut self, action: MenuAction, event_loop: &ActiveEventLoop) {
        match action {
            MenuAction::NewTab => {
                if let Some(state) = self.focused_window_mut() {
                    let tab_num = state.gpu.tab_bar.tab_count() + 1;
                    let tab_id = state.gpu.tab_bar.add_tab(format!("Terminal {}", tab_num));
                    state.gpu.tab_bar.select_tab_index(state.gpu.tab_bar.tab_count() - 1);
                    state.create_shell_for_tab(tab_id);
                    state.dirty = true;
                    state.window.request_redraw();
                }
            }
            MenuAction::NewWindow => {
                self.pending_new_window = true;
            }
            MenuAction::CloseTab => {
                let should_close = if let Some(state) = self.focused_window_mut() {
                    if state.gpu.tab_bar.tab_count() > 1 {
                        if let Some(id) = state.gpu.tab_bar.active_tab_id() {
                            state.gpu.tab_bar.close_tab(id);
                            state.remove_shell_for_tab(id);
                            state.dirty = true;
                            state.window.request_redraw();
                        }
                        false
                    } else { true }
                } else { false };
                if should_close {
                    if let Some(id) = self.focused_window { self.close_window(id); }
                }
            }
            MenuAction::CloseWindow => {
                if let Some(id) = self.focused_window { self.close_window(id); }
            }
            MenuAction::Quit => event_loop.exit(),
            MenuAction::ToggleFullScreen => {
                if let Some(state) = self.focused_window_mut() {
                    let fs = state.window.fullscreen().is_some();
                    state.window.set_fullscreen(if fs { None } else {
                        Some(winit::window::Fullscreen::Borderless(None))
                    });
                }
            }
            MenuAction::IncreaseFontSize => self.adjust_font_scale(FONT_SCALE_STEP),
            MenuAction::DecreaseFontSize => self.adjust_font_scale(-FONT_SCALE_STEP),
            MenuAction::ResetFontSize => {
                if let Some(state) = self.focused_window_mut() {
                    if (state.font_scale - 1.0).abs() > 0.001 {
                        state.font_scale = 1.0;
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
            MenuAction::NextTab => self.navigate_tab(true),
            MenuAction::PrevTab => self.navigate_tab(false),
            MenuAction::SelectTab1 => self.select_tab_index(0),
            MenuAction::SelectTab2 => self.select_tab_index(1),
            MenuAction::SelectTab3 => self.select_tab_index(2),
            MenuAction::SelectTab4 => self.select_tab_index(3),
            MenuAction::SelectTab5 => self.select_tab_index(4),
            MenuAction::SelectTab6 => self.select_tab_index(5),
            MenuAction::SelectTab7 => self.select_tab_index(6),
            MenuAction::SelectTab8 => self.select_tab_index(7),
            MenuAction::SelectTab9 => self.select_tab_index(8),
            MenuAction::Paste => {
                if let Some(state) = self.focused_window_mut() {
                    if let Some(content) = get_clipboard_content() {
                        paste_to_terminal(state, &content);
                    }
                }
            }
            MenuAction::Copy => {
                if let Some(state) = self.focused_window_mut() {
                    if let Some(text) = get_terminal_selection_text(state) {
                        set_clipboard_content(&text);
                    }
                }
            }
            _ => log::info!("{:?} not yet implemented", action),
        }
    }

    #[cfg(target_os = "macos")]
    fn adjust_font_scale(&mut self, delta: f32) {
        if let Some(state) = self.focused_window_mut() {
            let new_scale = (state.font_scale + delta).clamp(MIN_FONT_SCALE, MAX_FONT_SCALE);
            if (new_scale - state.font_scale).abs() > 0.001 {
                state.font_scale = new_scale;
                state.dirty = true;
                state.window.request_redraw();
            }
        }
    }

    #[cfg(target_os = "macos")]
    fn navigate_tab(&mut self, next: bool) {
        if let Some(state) = self.focused_window_mut() {
            if next { state.gpu.tab_bar.next_tab(); }
            else { state.gpu.tab_bar.prev_tab(); }
            state.force_active_tab_redraw();
            state.window.request_redraw();
        }
    }

    #[cfg(target_os = "macos")]
    fn select_tab_index(&mut self, index: usize) {
        if let Some(state) = self.focused_window_mut() {
            state.gpu.tab_bar.select_tab_index(index);
            state.force_active_tab_redraw();
            state.window.request_redraw();
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.windows.is_empty() {
            self.create_window(event_loop);

            #[cfg(target_os = "macos")]
            if self.menu.is_none() {
                let (menu, ids) = build_menu_bar();
                menu.init_for_nsapp();
                self.menu = Some(menu);
                self.menu_ids = Some(ids);
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let Some(state) = self.windows.get_mut(&id) else { return };

        match event {
            WindowEvent::CloseRequested => {
                self.windows.remove(&id);
                if self.focused_window == Some(id) {
                    self.focused_window = self.windows.keys().next().copied();
                }
            }

            WindowEvent::Focused(focused) => {
                if focused { self.focused_window = Some(id); }
            }

            WindowEvent::ModifiersChanged(m) => { self.modifiers = m; }

            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                let Some(state) = self.windows.get_mut(&id) else { return };

                #[cfg(target_os = "macos")]
                let mod_pressed = self.modifiers.state().super_key();
                #[cfg(not(target_os = "macos"))]
                let mod_pressed = self.modifiers.state().control_key();

                let shift_pressed = self.modifiers.state().shift_key();

                // Handle scroll shortcuts (Shift+PageUp/PageDown/Home/End)
                if shift_pressed && !mod_pressed {
                    let scroll_action = match &event.logical_key {
                        Key::Named(NamedKey::PageUp) => Some(Scroll::PageUp),
                        Key::Named(NamedKey::PageDown) => Some(Scroll::PageDown),
                        Key::Named(NamedKey::Home) => Some(Scroll::Top),
                        Key::Named(NamedKey::End) => Some(Scroll::Bottom),
                        _ => None,
                    };

                    if let Some(scroll) = scroll_action {
                        let tab_id = state.gpu.tab_bar.active_tab_id();
                        if let Some(tab_id) = tab_id {
                            if let Some(shell) = state.shells.get_mut(&tab_id) {
                                shell.scroll(scroll);
                                state.dirty = true;
                                state.content_hashes.insert(tab_id, 0);
                                state.window.request_redraw();
                            }
                        }
                        return;
                    }
                }

                // Handle tab editing first
                if let TabEditResult::Handled = handle_tab_editing(state, &event.logical_key, mod_pressed) {
                    return;
                }

                // Handle keyboard shortcuts
                if mod_pressed {
                    if state.gpu.tab_bar.is_editing() {
                        state.gpu.tab_bar.confirm_editing();
                        state.dirty = true;
                    }

                    match &event.logical_key {
                        Key::Character(c) if c.as_str() == "c" => {
                            // Copy selection to clipboard
                            if let Some(text) = get_terminal_selection_text(state) {
                                set_clipboard_content(&text);
                                return;
                            }
                        }
                        Key::Character(c) if c.as_str() == "v" => {
                            // Paste from clipboard
                            if let Some(content) = get_clipboard_content() {
                                paste_to_terminal(state, &content);
                            }
                            return;
                        }
                        Key::Character(c) if c.as_str() == "q" => { event_loop.exit(); return; }
                        Key::Character(c) if c.as_str() == "w" => {
                            if state.gpu.tab_bar.tab_count() > 1 {
                                if let Some(tab_id) = state.gpu.tab_bar.active_tab_id() {
                                    state.gpu.tab_bar.close_tab(tab_id);
                                    state.remove_shell_for_tab(tab_id);
                                    state.dirty = true;
                                    state.window.request_redraw();
                                    return;
                                }
                            }
                            self.windows.remove(&id);
                            if self.focused_window == Some(id) {
                                self.focused_window = self.windows.keys().next().copied();
                            }
                            return;
                        }
                        Key::Character(c) if c.as_str() == "n" => {
                            self.pending_new_window = true;
                            return;
                        }
                        Key::Character(c) if c.as_str() == "t" => {
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

                // Send to shell (clears selection on input)
                if handle_shell_input(state, &event.logical_key, mod_pressed) {
                    clear_terminal_selection(state);
                }
            }

            WindowEvent::Resized(size) => {
                let shared = self.shared_gpu.as_ref().unwrap();
                handle_resize(state, shared, size.width, size.height);
            }

            WindowEvent::CursorMoved { position, .. } => {
                let x = position.x as f32;
                let y = position.y as f32;
                state.cursor_position = (x, y);

                // Update selection if dragging
                handle_terminal_mouse_move(state, x, y);

                // Check for URL hover and update underline state
                let (offset_x, offset_y) = state.gpu.tab_bar.content_offset();
                let padding = 10.0 * state.scale_factor;
                let cell_width = state.gpu.glyph_cache.cell_width();
                let line_height = state.gpu.glyph_cache.line_height();

                let rel_x = x - offset_x - padding;
                let rel_y = y - offset_y - padding;

                let new_hovered = if rel_x >= 0.0 && rel_y >= 0.0 {
                    let col = (rel_x / cell_width) as usize;
                    let line = (rel_y / line_height) as usize;
                    find_url_index_at_position(&state.detected_urls, col, line)
                } else {
                    None
                };

                // Redraw if hover state changed
                if new_hovered != state.hovered_url_index {
                    state.hovered_url_index = new_hovered;
                    // Force content re-render to update decorations
                    state.force_active_tab_redraw();
                }
            }

            WindowEvent::MouseInput { state: button_state, button, .. } => {
                let (x, y) = state.cursor_position;

                // Check for Cmd+click (Super on macOS, Ctrl on Linux) to open URLs
                #[cfg(target_os = "macos")]
                let cmd_pressed = self.modifiers.state().super_key();
                #[cfg(not(target_os = "macos"))]
                let cmd_pressed = self.modifiers.state().control_key();

                if cmd_pressed
                    && button == winit::event::MouseButton::Left
                    && button_state == ElementState::Pressed
                {
                    // Calculate cell position from pixel coordinates
                    let (offset_x, offset_y) = state.gpu.tab_bar.content_offset();
                    let padding = 10.0 * state.scale_factor;
                    let cell_width = state.gpu.glyph_cache.cell_width();
                    let line_height = state.gpu.glyph_cache.line_height();

                    let rel_x = x - offset_x - padding;
                    let rel_y = y - offset_y - padding;

                    if rel_x >= 0.0 && rel_y >= 0.0 {
                        let col = (rel_x / cell_width) as usize;
                        let line = (rel_y / line_height) as usize;

                        // Check if there's a URL at this position
                        if let Some(url) = find_url_at_position(&state.detected_urls, col, line) {
                            log::info!("Opening URL: {}", url.url);
                            open_url(&url.url);
                            return;
                        }
                    }
                }

                let mouse_button = match button {
                    winit::event::MouseButton::Left => Some(MOUSE_BUTTON_LEFT),
                    winit::event::MouseButton::Middle => Some(MOUSE_BUTTON_MIDDLE),
                    winit::event::MouseButton::Right => Some(MOUSE_BUTTON_RIGHT),
                    _ => None,
                };

                if let Some(btn) = mouse_button {
                    match button_state {
                        ElementState::Pressed => {
                            // Try terminal (mouse reporting or selection) first, then tab bar
                            if !handle_terminal_mouse_button(state, x, y, Instant::now(), btn, true) {
                                if btn == MOUSE_BUTTON_LEFT {
                                    handle_tab_click(state, x, y, Instant::now());
                                }
                            }
                        }
                        ElementState::Released => {
                            handle_terminal_mouse_release(state, x, y);
                        }
                    }
                }
            }

            WindowEvent::MouseWheel { delta, .. } => {
                let (x, y) = state.cursor_position;
                let delta_y = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(pos) => {
                        let line_height = state.gpu.glyph_cache.line_height();
                        (pos.y / line_height as f64) as f32
                    }
                };

                // Check if mouse reporting should handle this
                if handle_terminal_scroll(state, x, y, delta_y) {
                    // Mouse reporting handled the scroll
                    return;
                }

                // Fall back to local scrollback
                let tab_id = state.gpu.tab_bar.active_tab_id();
                if let Some(tab_id) = tab_id {
                    if let Some(shell) = state.shells.get_mut(&tab_id) {
                        let lines = delta_y as i32;
                        if lines != 0 {
                            shell.scroll(Scroll::Delta(lines));
                            state.dirty = true;
                            state.content_hashes.insert(tab_id, 0);
                            state.window.request_redraw();
                        }
                    }
                }
            }

            WindowEvent::RedrawRequested => {
                let shared = self.shared_gpu.as_ref().unwrap();
                render_frame(state, shared);
            }

            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        #[cfg(target_os = "macos")]
        if let Some(ids) = &self.menu_ids {
            if let Ok(event) = MenuEvent::receiver().try_recv() {
                if let Some(action) = menu_id_to_action(event.id(), ids) {
                    self.handle_menu_action(action, event_loop);
                }
            }
        }

        if self.pending_new_window {
            self.pending_new_window = false;
            self.create_window(event_loop);
        }

        for state in self.windows.values() {
            state.window.request_redraw();
        }
    }
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn,crt=info")).init();
    log::info!("CRT Terminal - swash renderer + effect pipeline");

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);
    event_loop.run_app(&mut App::new()).unwrap();
}
