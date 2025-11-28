//! CRT Terminal with Effects
//!
//! Two-pass rendering:
//! 1. Render text to offscreen texture using swash-based glyph cache
//! 2. Composite text with effects (gradient, grid, glow) to screen

mod config;
mod font;
mod gpu;
mod input;
mod menu;
mod render;
mod state;
mod watcher;
mod window;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use config::Config;
use crt_core::{ShellTerminal, Size, Scroll};
use crt_renderer::{GlyphCache, GridRenderer, RectRenderer, EffectPipeline, TabBar, BackgroundImagePipeline, BackgroundImageState, EffectsRenderer, GridEffect, StarfieldEffect, RainEffect, ParticleEffect, MatrixEffect, ShapeEffect, SpriteEffect, EffectConfig, SpriteAnimationState, SpriteConfig, SpritePosition, SpriteMotion, CrtPipeline};
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
use window::{ContextMenuItem, WindowState, SearchMatch};

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


struct App {
    windows: HashMap<WindowId, WindowState>,
    shared_gpu: Option<SharedGpuState>,
    focused_window: Option<WindowId>,
    config: Config,
    modifiers: Modifiers,
    pending_new_window: bool,
    config_watcher: Option<watcher::ConfigWatcher>,
    /// Last frame time for throttling redraws (prevents Metal memory leak)
    last_frame_time: Instant,
    #[cfg(target_os = "macos")]
    menu: Option<Menu>,
    #[cfg(target_os = "macos")]
    menu_ids: Option<MenuIds>,
}

impl App {
    fn new() -> Self {
        let config = Config::load();
        let config_watcher = watcher::ConfigWatcher::new();

        Self {
            windows: HashMap::new(),
            shared_gpu: None,
            focused_window: None,
            config,
            modifiers: Modifiers::default(),
            pending_new_window: false,
            config_watcher,
            last_frame_time: Instant::now(),
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

        // Initialize glyph caches with font variants from config
        let scaled_font_size = self.config.font.size * scale_factor;
        let line_height_multiplier = self.config.font.line_height;
        let font_variants = font::load_font_variants(&self.config.font);
        let mut glyph_cache = GlyphCache::with_variants(&shared.device, font_variants.clone(), scaled_font_size, line_height_multiplier)
            .expect("Failed to create glyph cache");
        glyph_cache.precache_ascii();
        glyph_cache.flush(&shared.queue);

        let mut grid_renderer = GridRenderer::new(&shared.device, format);
        grid_renderer.set_glyph_cache(&shared.device, &glyph_cache);
        grid_renderer.update_screen_size(&shared.queue, size.width as f32, size.height as f32);

        // Separate renderer for output text (rendered flat, no glow)
        let mut output_grid_renderer = GridRenderer::new(&shared.device, format);
        output_grid_renderer.set_glyph_cache(&shared.device, &glyph_cache);
        output_grid_renderer.update_screen_size(&shared.queue, size.width as f32, size.height as f32);

        // Tab bar uses same font at smaller size with fixed line height
        let tab_font_size = 12.0 * scale_factor;
        let mut tab_glyph_cache = GlyphCache::with_variants(&shared.device, font_variants, tab_font_size, 1.3)
            .expect("Failed to create tab glyph cache");
        tab_glyph_cache.precache_ascii();
        tab_glyph_cache.flush(&shared.queue);

        let mut tab_title_renderer = GridRenderer::new(&shared.device, format);
        tab_title_renderer.set_glyph_cache(&shared.device, &tab_glyph_cache);
        tab_title_renderer.update_screen_size(&shared.queue, size.width as f32, size.height as f32);

        // Effect pipeline for background rendering
        let theme = self.load_theme();
        let mut effect_pipeline = EffectPipeline::new(&shared.device, format);
        effect_pipeline.set_theme(theme.clone());

        // Backdrop effects renderer (grid, starfield, rain, particles, etc.)
        let mut effects_renderer = EffectsRenderer::new(&shared.device, shared.vello_renderer_arc(), format);
        // Add effects (disabled by default, enabled via CSS)
        effects_renderer.add_effect(Box::new(GridEffect::new()));
        effects_renderer.add_effect(Box::new(StarfieldEffect::new()));
        effects_renderer.add_effect(Box::new(RainEffect::new()));
        effects_renderer.add_effect(Box::new(ParticleEffect::new()));
        effects_renderer.add_effect(Box::new(MatrixEffect::new()));
        effects_renderer.add_effect(Box::new(ShapeEffect::new()));
        effects_renderer.add_effect(Box::new(SpriteEffect::new()));
        // Configure effects from theme
        configure_effects_from_theme(&mut effects_renderer, &theme);

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

        // Rect renderer for cell backgrounds and tab bar
        let rect_renderer = RectRenderer::new(&shared.device, format);

        // Separate rect renderer for overlays (cursor, selection, underlines)
        // to avoid buffer conflicts with tab bar rendering
        let overlay_rect_renderer = RectRenderer::new(&shared.device, format);

        // Background image pipeline (always created, state only if theme has background image)
        let background_image_pipeline = BackgroundImagePipeline::new(&shared.device, format);
        let (background_image_state, background_image_bind_group) = if let Some(ref bg_image) = theme.background_image {
            match BackgroundImageState::new(&shared.device, &shared.queue, bg_image) {
                Ok(state) => {
                    let bind_group = background_image_pipeline.create_bind_group(
                        &shared.device,
                        &state.texture.view,
                    );
                    log::info!("Loaded background image: {:?}", bg_image.path);
                    (Some(state), Some(bind_group))
                }
                Err(e) => {
                    log::warn!("Failed to load background image: {}", e);
                    (None, None)
                }
            }
        } else {
            (None, None)
        };

        // Sprite animation state (bypasses vello for memory efficiency)
        log::info!("Checking sprite config: {:?}", theme.sprite.as_ref().map(|s| (s.enabled, &s.path)));
        let sprite_state = if let Some(ref sprite) = theme.sprite {
            if sprite.enabled {
                if let Some(ref path_str) = sprite.path {
                    // Resolve path relative to theme base directory
                    let path = std::path::PathBuf::from(path_str);
                    let resolved_path = if let Some(ref bg) = theme.background_image {
                        if let Some(ref base_dir) = bg.base_dir {
                            if path.is_relative() {
                                base_dir.join(&path)
                            } else {
                                path.clone()
                            }
                        } else {
                            path.clone()
                        }
                    } else {
                        path.clone()
                    };

                    log::info!("Resolved sprite path: {:?}", resolved_path);
                    let config = SpriteConfig {
                        path: resolved_path.clone(),
                        frame_width: sprite.frame_width,
                        frame_height: sprite.frame_height,
                        columns: sprite.columns,
                        rows: sprite.rows,
                        frame_count: sprite.frame_count,
                        fps: sprite.fps,
                        scale: sprite.scale,
                        opacity: sprite.opacity,
                        position: SpritePosition::from_str(sprite.position.as_str()),
                        motion: SpriteMotion::from_str(sprite.motion.as_str()),
                        motion_speed: sprite.motion_speed,
                    };

                    match SpriteAnimationState::new(&shared.device, &shared.queue, config, format) {
                        Ok(state) => {
                            log::info!("Loaded sprite animation using raw wgpu (bypassing vello)");
                            Some(state)
                        }
                        Err(e) => {
                            log::warn!("Failed to load sprite animation: {}", e);
                            None
                        }
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        // Create intermediate text texture for glow effect
        let text_texture = shared.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Text Texture"),
            size: wgpu::Extent3d {
                width: size.width,
                height: size.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let text_texture_view = text_texture.create_view(&Default::default());
        let composite_bind_group = effect_pipeline.composite.create_bind_group(
            &shared.device,
            &text_texture_view,
        );

        // CRT post-processing pipeline
        let mut crt_pipeline = CrtPipeline::new(&shared.device, format);
        crt_pipeline.set_effect(theme.crt);
        let (crt_texture, crt_texture_view, crt_bind_group) = if crt_pipeline.is_enabled() {
            log::info!("CRT effect enabled - creating intermediate texture");
            let texture = shared.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("CRT Intermediate Texture"),
                size: wgpu::Extent3d {
                    width: size.width,
                    height: size.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let view = texture.create_view(&Default::default());
            let bind_group = crt_pipeline.create_bind_group(&shared.device, &view);
            (Some(texture), Some(view), Some(bind_group))
        } else {
            (None, None, None)
        };

        let gpu = WindowGpuState {
            surface,
            config: surface_config,
            glyph_cache,
            grid_renderer,
            output_grid_renderer,
            tab_glyph_cache,
            tab_title_renderer,
            effect_pipeline,
            effects_renderer,
            tab_bar,
            terminal_vello,
            rect_renderer,
            overlay_rect_renderer,
            background_image_pipeline,
            background_image_state,
            background_image_bind_group,
            sprite_state,
            text_texture,
            text_texture_view,
            composite_bind_group,
            crt_pipeline,
            crt_texture,
            crt_texture_view,
            crt_bind_group,
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
            search: Default::default(),
            bell: window::BellState::from_config(&self.config.bell),
            context_menu: window::ContextMenu::default(),
            // New testable state modules
            tab_state: state::TabState::new(),
            ui_state: state::UiState::new(),
        };

        self.windows.insert(window_id, window_state);
        self.focused_window = Some(window_id);
        log::info!("Created window {:?}, total: {}", window_id, self.windows.len());
        window_id
    }

    fn load_theme(&self) -> Theme {
        match self.config.theme_css_with_path() {
            Some((css, base_dir)) => Theme::from_css_with_base(&css, &base_dir).unwrap_or_else(|e| {
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
                // Compute delta to get back to scale 1.0
                if let Some(state) = self.windows.get(&self.focused_window.unwrap_or(WindowId::dummy())) {
                    let delta = 1.0 - state.font_scale;
                    if delta.abs() > 0.001 {
                        self.adjust_font_scale(delta);
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
            MenuAction::Find => {
                if let Some(state) = self.focused_window_mut() {
                    // Toggle search mode
                    state.search.active = !state.search.active;
                    if !state.search.active {
                        // Clear search when closing
                        state.search.query.clear();
                        state.search.matches.clear();
                        state.search.current_match = 0;
                    }
                    state.force_active_tab_redraw();
                    state.window.request_redraw();
                }
            }
            _ => log::info!("{:?} not yet implemented", action),
        }
    }

    #[cfg(target_os = "macos")]
    fn adjust_font_scale(&mut self, delta: f32) {
        use crt_core::Size;

        let base_font_size = self.config.font.size;
        let focused_id = match self.focused_window {
            Some(id) => id,
            None => return,
        };

        let shared = match self.shared_gpu.as_ref() {
            Some(s) => s,
            None => return,
        };

        let state = match self.windows.get_mut(&focused_id) {
            Some(s) => s,
            None => return,
        };

        let new_scale = (state.font_scale + delta).clamp(MIN_FONT_SCALE, MAX_FONT_SCALE);
        if (new_scale - state.font_scale).abs() > 0.001 {
            state.font_scale = new_scale;

            // Update glyph cache with new font size
            let new_font_size = base_font_size * new_scale * state.scale_factor;
            state.gpu.glyph_cache.set_font_size(&shared.queue, new_font_size);
            state.gpu.glyph_cache.precache_ascii();
            state.gpu.glyph_cache.flush(&shared.queue);

            // Update grid renderers with new glyph cache
            state.gpu.grid_renderer.set_glyph_cache(&shared.device, &state.gpu.glyph_cache);
            state.gpu.output_grid_renderer.set_glyph_cache(&shared.device, &state.gpu.glyph_cache);

            // Recalculate terminal grid size (like resize does)
            let cell_width = state.gpu.glyph_cache.cell_width();
            let line_height = state.gpu.glyph_cache.line_height();
            let tab_bar_height = state.gpu.tab_bar.height();

            let padding_physical = 20.0 * state.scale_factor;
            let tab_bar_physical = tab_bar_height * state.scale_factor;

            let content_width = (state.gpu.config.width as f32 - padding_physical).max(60.0);
            let content_height = (state.gpu.config.height as f32 - padding_physical - tab_bar_physical).max(40.0);

            let new_cols = ((content_width / cell_width) as usize).max(10);
            let new_rows = ((content_height / line_height) as usize).max(4);

            state.cols = new_cols;
            state.rows = new_rows;

            // Resize all shells to match new grid size
            for shell in state.shells.values_mut() {
                shell.resize(Size::new(new_cols, new_rows));
            }

            // Force full redraw
            state.dirty = true;
            for hash in state.content_hashes.values_mut() {
                *hash = 0;
            }
            state.window.request_redraw();
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

    /// Reload config from disk and apply changes
    fn reload_config(&mut self) {
        log::info!("Reloading config...");
        let new_config = Config::load();

        // Check if theme changed
        let theme_changed = new_config.theme.name != self.config.theme.name;

        self.config = new_config;

        // Reload theme if it changed
        if theme_changed {
            self.reload_theme();
        }

        // Apply other config changes to all windows
        for state in self.windows.values_mut() {
            // Force redraw
            state.dirty = true;
            for hash in state.content_hashes.values_mut() {
                *hash = 0;
            }
        }
    }

    /// Reload theme CSS and apply to all windows
    fn reload_theme(&mut self) {
        log::info!("Reloading theme...");
        let theme = self.load_theme();

        for state in self.windows.values_mut() {
            // Update effect pipeline with new theme
            state.gpu.effect_pipeline.set_theme(theme.clone());

            // Update backdrop effects from theme
            configure_effects_from_theme(&mut state.gpu.effects_renderer, &theme);

            // Update tab bar theme
            state.gpu.tab_bar.set_theme(theme.tabs.clone());

            // Force full redraw
            state.dirty = true;
            for hash in state.content_hashes.values_mut() {
                *hash = 0;
            }
        }
    }
}

/// Configure backdrop effects from theme settings
fn configure_effects_from_theme(effects_renderer: &mut EffectsRenderer, theme: &Theme) {
    let mut config = EffectConfig::new();

    // Grid effect configuration from theme
    if let Some(ref grid) = theme.grid {
        config.insert("grid-enabled", if grid.enabled { "true" } else { "false" });
        // Convert Color to rgba() string
        let c = grid.color;
        config.insert("grid-color", format!(
            "rgba({}, {}, {}, {})",
            (c.r * 255.0) as u8,
            (c.g * 255.0) as u8,
            (c.b * 255.0) as u8,
            c.a
        ));
        config.insert("grid-spacing", grid.spacing.to_string());
        config.insert("grid-line-width", grid.line_width.to_string());
        config.insert("grid-perspective", grid.perspective.to_string());
        config.insert("grid-horizon", grid.horizon.to_string());
        config.insert("grid-animation-speed", grid.animation_speed.to_string());
        config.insert("grid-glow-radius", grid.glow_radius.to_string());
        config.insert("grid-glow-intensity", grid.glow_intensity.to_string());
        config.insert("grid-vanishing-spread", grid.vanishing_spread.to_string());
        config.insert("grid-curved", if grid.curved { "true" } else { "false" });
    }

    // Starfield effect configuration from theme
    if let Some(ref starfield) = theme.starfield {
        config.insert("starfield-enabled", if starfield.enabled { "true" } else { "false" });
        // Convert Color to rgba() string
        let c = starfield.color;
        config.insert("starfield-color", format!(
            "rgba({}, {}, {}, {})",
            (c.r * 255.0) as u8,
            (c.g * 255.0) as u8,
            (c.b * 255.0) as u8,
            c.a
        ));
        config.insert("starfield-density", starfield.density.to_string());
        config.insert("starfield-layers", starfield.layers.to_string());
        config.insert("starfield-speed", starfield.speed.to_string());
        config.insert("starfield-direction", starfield.direction.as_str().to_string());
        config.insert("starfield-glow-radius", starfield.glow_radius.to_string());
        config.insert("starfield-glow-intensity", starfield.glow_intensity.to_string());
        config.insert("starfield-twinkle", if starfield.twinkle { "true" } else { "false" });
        config.insert("starfield-twinkle-speed", starfield.twinkle_speed.to_string());
        config.insert("starfield-min-size", starfield.min_size.to_string());
        config.insert("starfield-max-size", starfield.max_size.to_string());
    }

    // Rain effect configuration from theme
    if let Some(ref rain) = theme.rain {
        config.insert("rain-enabled", if rain.enabled { "true" } else { "false" });
        let c = rain.color;
        config.insert("rain-color", format!(
            "rgba({}, {}, {}, {})",
            (c.r * 255.0) as u8,
            (c.g * 255.0) as u8,
            (c.b * 255.0) as u8,
            c.a
        ));
        config.insert("rain-density", rain.density.to_string());
        config.insert("rain-speed", rain.speed.to_string());
        config.insert("rain-angle", rain.angle.to_string());
        config.insert("rain-length", rain.length.to_string());
        config.insert("rain-thickness", rain.thickness.to_string());
        config.insert("rain-glow-radius", rain.glow_radius.to_string());
        config.insert("rain-glow-intensity", rain.glow_intensity.to_string());
    }

    // Particle effect configuration from theme
    if let Some(ref particles) = theme.particles {
        config.insert("particles-enabled", if particles.enabled { "true" } else { "false" });
        let c = particles.color;
        config.insert("particles-color", format!(
            "rgba({}, {}, {}, {})",
            (c.r * 255.0) as u8,
            (c.g * 255.0) as u8,
            (c.b * 255.0) as u8,
            c.a
        ));
        config.insert("particles-count", particles.count.to_string());
        config.insert("particles-shape", particles.shape.as_str().to_string());
        config.insert("particles-behavior", particles.behavior.as_str().to_string());
        config.insert("particles-size", particles.size.to_string());
        config.insert("particles-speed", particles.speed.to_string());
        config.insert("particles-glow-radius", particles.glow_radius.to_string());
        config.insert("particles-glow-intensity", particles.glow_intensity.to_string());
    }

    // Matrix effect configuration from theme
    if let Some(ref matrix) = theme.matrix {
        config.insert("matrix-enabled", if matrix.enabled { "true" } else { "false" });
        let c = matrix.color;
        config.insert("matrix-color", format!(
            "rgba({}, {}, {}, {})",
            (c.r * 255.0) as u8,
            (c.g * 255.0) as u8,
            (c.b * 255.0) as u8,
            c.a
        ));
        config.insert("matrix-density", matrix.density.to_string());
        config.insert("matrix-speed", matrix.speed.to_string());
        config.insert("matrix-font-size", matrix.font_size.to_string());
        config.insert("matrix-charset", matrix.charset.clone());
    }

    // Shape effect configuration from theme
    if let Some(ref shape) = theme.shape {
        config.insert("shape-enabled", if shape.enabled { "true" } else { "false" });
        config.insert("shape-type", shape.shape_type.as_str().to_string());
        config.insert("shape-size", shape.size.to_string());
        if let Some(ref fill) = shape.fill {
            config.insert("shape-fill", format!(
                "rgba({}, {}, {}, {})",
                (fill.r * 255.0) as u8,
                (fill.g * 255.0) as u8,
                (fill.b * 255.0) as u8,
                fill.a
            ));
        } else {
            config.insert("shape-fill", "none".to_string());
        }
        if let Some(ref stroke) = shape.stroke {
            config.insert("shape-stroke", format!(
                "rgba({}, {}, {}, {})",
                (stroke.r * 255.0) as u8,
                (stroke.g * 255.0) as u8,
                (stroke.b * 255.0) as u8,
                stroke.a
            ));
        } else {
            config.insert("shape-stroke", "none".to_string());
        }
        config.insert("shape-stroke-width", shape.stroke_width.to_string());
        config.insert("shape-glow-radius", shape.glow_radius.to_string());
        if let Some(ref glow_color) = shape.glow_color {
            config.insert("shape-glow-color", format!(
                "rgba({}, {}, {}, {})",
                (glow_color.r * 255.0) as u8,
                (glow_color.g * 255.0) as u8,
                (glow_color.b * 255.0) as u8,
                glow_color.a
            ));
        }
        config.insert("shape-rotation", shape.rotation.as_str().to_string());
        config.insert("shape-rotation-speed", shape.rotation_speed.to_string());
        config.insert("shape-motion", shape.motion.as_str().to_string());
        config.insert("shape-motion-speed", shape.motion_speed.to_string());
        config.insert("shape-polygon-sides", shape.polygon_sides.to_string());
    }

    // Sprite effect: DISABLED - now using raw wgpu SpriteRenderer to avoid vello memory issues
    // The vello-based SpriteEffect causes unbounded memory growth due to atlas texture system.
    // Sprite rendering is handled by SpriteAnimationState in render.rs instead.
    config.insert("sprite-enabled", "false");

    effects_renderer.configure(&config);
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

                // Handle context menu dismissal with Escape
                if state.context_menu.visible {
                    if let Key::Named(NamedKey::Escape) = &event.logical_key {
                        state.context_menu.hide();
                        state.dirty = true;
                        state.window.request_redraw();
                        return;
                    }
                }

                // Handle tab editing first
                if let TabEditResult::Handled = handle_tab_editing(state, &event.logical_key, mod_pressed) {
                    return;
                }

                // Handle search input when search is active
                if state.search.active {
                    match &event.logical_key {
                        Key::Named(NamedKey::Escape) => {
                            // Close search
                            state.search.active = false;
                            state.search.query.clear();
                            state.search.matches.clear();
                            state.search.current_match = 0;
                            state.force_active_tab_redraw();
                            state.window.request_redraw();
                            return;
                        }
                        Key::Named(NamedKey::Enter) => {
                            // Next match on Enter
                            if !state.search.matches.is_empty() {
                                state.search.current_match = (state.search.current_match + 1) % state.search.matches.len();
                                scroll_to_current_match(state);
                                state.force_active_tab_redraw();
                                state.window.request_redraw();
                            }
                            return;
                        }
                        Key::Named(NamedKey::Backspace) => {
                            // Delete last char from query
                            state.search.query.pop();
                            update_search_matches(state);
                            state.force_active_tab_redraw();
                            state.window.request_redraw();
                            return;
                        }
                        Key::Character(c) if !mod_pressed => {
                            // Add character to query
                            state.search.query.push_str(c.as_str());
                            update_search_matches(state);
                            state.force_active_tab_redraw();
                            state.window.request_redraw();
                            return;
                        }
                        _ => {}
                    }
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
                        Key::Character(c) if c.as_str() == "f" => {
                            // Toggle search mode
                            log::info!("Cmd+F pressed, toggling search mode");
                            state.search.active = !state.search.active;
                            log::info!("Search active: {}", state.search.active);
                            if !state.search.active {
                                // Clear search when closing
                                state.search.query.clear();
                                state.search.matches.clear();
                                state.search.current_match = 0;
                            }
                            state.force_active_tab_redraw();
                            state.window.request_redraw();
                            return;
                        }
                        Key::Character(c) if c.as_str() == "g" => {
                            // Next/prev match
                            if state.search.active && !state.search.matches.is_empty() {
                                if shift_pressed {
                                    // Previous match
                                    if state.search.current_match == 0 {
                                        state.search.current_match = state.search.matches.len() - 1;
                                    } else {
                                        state.search.current_match -= 1;
                                    }
                                } else {
                                    // Next match
                                    state.search.current_match = (state.search.current_match + 1) % state.search.matches.len();
                                }
                                scroll_to_current_match(state);
                                state.force_active_tab_redraw();
                                state.window.request_redraw();
                            }
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
                let ctrl_pressed = self.modifiers.state().control_key();
                if handle_shell_input(state, &event.logical_key, mod_pressed, ctrl_pressed) {
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

                // Update context menu hover state
                if state.context_menu.visible {
                    let old_hover = state.context_menu.hovered_item;
                    state.context_menu.update_hover(x, y);
                    if old_hover != state.context_menu.hovered_item {
                        state.dirty = true;
                        state.window.request_redraw();
                    }
                }

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

                // Handle context menu interactions first
                if state.context_menu.visible {
                    match (button, button_state) {
                        (winit::event::MouseButton::Left, ElementState::Pressed) => {
                            // Check if clicking on a menu item
                            if let Some(item) = state.context_menu.item_at(x, y) {
                                handle_context_menu_action(state, item);
                                state.context_menu.hide();
                                state.dirty = true;
                                state.window.request_redraw();
                                return;
                            }
                            // Clicking outside the menu dismisses it
                            state.context_menu.hide();
                            state.dirty = true;
                            state.window.request_redraw();
                            // Fall through to normal click handling
                        }
                        (winit::event::MouseButton::Right, ElementState::Pressed) => {
                            // Right-click while menu is open moves the menu
                            state.context_menu.show(x, y);
                            state.dirty = true;
                            state.window.request_redraw();
                            return;
                        }
                        _ => {}
                    }
                }

                // Right-click shows context menu
                if button == winit::event::MouseButton::Right && button_state == ElementState::Pressed {
                    state.context_menu.show(x, y);
                    state.dirty = true;
                    state.window.request_redraw();
                    return;
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
                let shared = self.shared_gpu.as_mut().unwrap();
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

        // Check for config/theme file changes - collect events first to avoid borrow issues
        let events: Vec<_> = self.config_watcher.as_mut()
            .map(|w| std::iter::from_fn(|| w.poll()).collect())
            .unwrap_or_default();

        for event in events {
            match event {
                watcher::ConfigEvent::ConfigChanged => self.reload_config(),
                watcher::ConfigEvent::ThemeChanged => self.reload_theme(),
            }
        }

        if self.pending_new_window {
            self.pending_new_window = false;
            self.create_window(event_loop);
        }

        // FRAME THROTTLING - Critical fix for Metal/wgpu memory leak (November 2024)
        //
        // WHY: wgpu/Metal on macOS has a bug where IOAccelerator drawable allocations grow
        // unboundedly when frames are rendered at high rates. With ControlFlow::Poll and
        // continuous request_redraw(), we were seeing 1500+ GPU allocations per second,
        // causing memory to balloon from ~130MB to 4-9GB within minutes.
        //
        // WHAT: Limits redraws to ~60fps by only calling request_redraw() when at least
        // 16.6ms has elapsed since the last frame. This keeps IOAccelerator regions
        // at ~500 instead of 40,000+ and memory stable at ~150-220MB.
        //
        // RE-EVALUATE WHEN:
        // - wgpu updates to 24.x+ (check if Metal backend fixes drawable allocation)
        // - Testing on non-macOS platforms (this may not be needed on Windows/Linux)
        // - If we need variable refresh rate support (would need smarter throttling)
        // - If Apple fixes the IOAccelerator memory management in a future macOS version
        //
        // Related: https://github.com/gfx-rs/wgpu/issues/3292 (Metal memory growth issues)
        const TARGET_FRAME_TIME: std::time::Duration = std::time::Duration::from_micros(16666); // ~60fps
        let elapsed = self.last_frame_time.elapsed();

        if elapsed >= TARGET_FRAME_TIME {
            self.last_frame_time = Instant::now();

            for state in self.windows.values() {
                // Always request redraw at throttled rate to check for PTY output
                state.window.request_redraw();
            }
        }
    }
}

/// Scroll terminal to make current search match visible
fn scroll_to_current_match(state: &mut WindowState) {
    if state.search.matches.is_empty() {
        return;
    }

    let current_match = &state.search.matches[state.search.current_match];
    let match_line = current_match.line;

    // Get active shell
    let active_tab_id = state.gpu.tab_bar.active_tab_id();
    let shell = active_tab_id.and_then(|id| state.shells.get_mut(&id));
    let Some(shell) = shell else { return };

    let terminal = shell.terminal();
    let screen_lines = terminal.screen_lines() as i32;
    let display_offset = terminal.display_offset() as i32;

    // Calculate viewport line (what line would the match be at in current viewport)
    // viewport_line = grid_line + display_offset
    let viewport_line = match_line + display_offset;

    // If match is outside visible range (0 to screen_lines-1), scroll to center it
    if viewport_line < 0 || viewport_line >= screen_lines {
        // Target: put match roughly in the middle of the screen
        let target_viewport_line = screen_lines / 2;
        // New display_offset needed: match_line + new_offset = target_viewport_line
        // new_offset = target_viewport_line - match_line
        let new_offset = target_viewport_line - match_line;

        // The scroll delta is the change in display_offset
        // Positive delta scrolls up (increases display_offset)
        let scroll_delta = new_offset - display_offset;

        if scroll_delta != 0 {
            shell.scroll(Scroll::Delta(scroll_delta));
            if let Some(tab_id) = active_tab_id {
                state.content_hashes.insert(tab_id, 0);
            }
        }
    }
}

/// Update search matches based on current query
fn update_search_matches(state: &mut WindowState) {
    state.search.matches.clear();
    state.search.current_match = 0;

    let query = &state.search.query;
    if query.is_empty() {
        return;
    }

    // Get active shell's terminal content
    let active_tab_id = state.gpu.tab_bar.active_tab_id();
    let shell = active_tab_id.and_then(|id| state.shells.get(&id));
    let Some(shell) = shell else { return };

    let terminal = shell.terminal();

    // Get all lines including history
    let all_lines = terminal.all_lines_text();

    // Search each line for the query (case-insensitive)
    let query_lower = query.to_lowercase();
    for (line_idx, line_text) in &all_lines {
        let line_lower = line_text.to_lowercase();
        let mut start = 0;
        while let Some(pos) = line_lower[start..].find(&query_lower) {
            let match_start = start + pos;
            state.search.matches.push(SearchMatch {
                line: *line_idx,
                start_col: match_start,
                end_col: match_start + query.len(),
            });
            start = match_start + 1;
        }
    }

    // Scroll to first match if any found
    if !state.search.matches.is_empty() {
        scroll_to_current_match(state);
    }
}

/// Handle context menu action (copy, paste, select all)
fn handle_context_menu_action(state: &mut WindowState, item: ContextMenuItem) {
    match item {
        ContextMenuItem::Copy => {
            if let Some(text) = get_terminal_selection_text(state) {
                set_clipboard_content(&text);
            }
        }
        ContextMenuItem::Paste => {
            if let Some(content) = get_clipboard_content() {
                paste_to_terminal(state, &content);
            }
        }
        ContextMenuItem::SelectAll => {
            // Select all visible content
            if let Some(tab_id) = state.gpu.tab_bar.active_tab_id() {
                if let Some(shell) = state.shells.get_mut(&tab_id) {
                    use crt_core::{Point, Line, Column, SelectionType};
                    let terminal = shell.terminal_mut();
                    let screen_lines = terminal.screen_lines();
                    let columns = terminal.columns();

                    // Start selection at top-left
                    terminal.start_selection(
                        Point { line: Line(0), column: Column(0) },
                        SelectionType::Simple,
                    );
                    // Extend to bottom-right
                    terminal.update_selection(
                        Point {
                            line: Line(screen_lines as i32 - 1),
                            column: Column(columns - 1),
                        },
                    );
                    state.dirty = true;
                }
            }
        }
    }
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn,crt=info")).init();
    log::info!("CRT Terminal starting");

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);
    event_loop.run_app(&mut App::new()).unwrap();
}
