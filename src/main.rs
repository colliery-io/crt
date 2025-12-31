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
pub mod profiling;
mod render;
mod theme_registry;
mod watcher;
mod window;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use config::{Config, ConfigPaths};
use crt_core::{ShellTerminal, Size, SpawnOptions};
use crt_renderer::{
    BackgroundImagePipeline, BackgroundImageState, CrtPipeline, EffectConfig, EffectPipeline,
    EffectsRenderer, GlyphCache, GridEffect, GridRenderer, MatrixEffect, ParticleEffect,
    RainEffect, RectRenderer, ShapeEffect, SpriteAnimationState, SpriteConfig, SpriteEffect,
    SpriteMotion, SpritePosition, StarfieldEffect, TabBar,
};
use crt_theme::Theme;
use gpu::{SharedGpuState, WindowGpuState};
use input::{
    KeyboardAction, get_clipboard_content, get_terminal_selection_text, handle_cursor_moved,
    handle_keyboard_input, handle_mouse_input, handle_mouse_wheel, handle_resize,
    paste_to_terminal, set_clipboard_content,
};
use menu::MenuAction;
use render::render_frame;
use theme_registry::ThemeRegistry;
use window::WindowState;

use winit::{
    application::ApplicationHandler,
    event::{ElementState, Modifiers, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

#[cfg(target_os = "macos")]
use winit::platform::macos::WindowAttributesExtMacOS;

#[cfg(target_os = "macos")]
use muda::{Menu, MenuEvent};

#[cfg(target_os = "macos")]
use menu::{MenuIds, build_menu_bar, menu_id_to_action, set_windows_menu};

// Font scale bounds
const MIN_FONT_SCALE: f32 = 0.5;
const MAX_FONT_SCALE: f32 = 3.0;
const FONT_SCALE_STEP: f32 = 0.1;

struct App {
    windows: HashMap<WindowId, WindowState>,
    shared_gpu: Option<SharedGpuState>,
    focused_window: Option<WindowId>,
    config: Config,
    /// Current theme (stored for event override access)
    theme: Theme,
    /// Registry of available themes for runtime switching
    theme_registry: ThemeRegistry,
    modifiers: Modifiers,
    pending_new_window: bool,
    config_watcher: Option<watcher::ConfigWatcher>,
    /// Last frame time for throttling focused window redraws (~60fps)
    last_frame_time: Instant,
    /// Last frame time for unfocused windows (~1fps for PTY updates)
    last_unfocused_frame_time: Instant,
    #[cfg(target_os = "macos")]
    menu: Option<Menu>,
    #[cfg(target_os = "macos")]
    menu_ids: Option<MenuIds>,
}

impl App {
    fn new() -> Self {
        let config = Config::load();
        let config_watcher = watcher::ConfigWatcher::new();

        // Initialize theme registry from themes directory
        let theme_registry = ConfigPaths::from_env_or_default()
            .map(|paths| ThemeRegistry::new(paths.themes_dir(), config.theme.name.clone()))
            .unwrap_or_else(|| {
                log::warn!("Could not determine config paths, using empty theme registry");
                ThemeRegistry::new(std::path::PathBuf::new(), config.theme.name.clone())
            });

        Self {
            windows: HashMap::new(),
            shared_gpu: None,
            focused_window: None,
            config,
            theme: Theme::default(), // Will be loaded properly in resumed()
            theme_registry,
            modifiers: Modifiers::default(),
            pending_new_window: false,
            config_watcher,
            last_frame_time: Instant::now(),
            last_unfocused_frame_time: Instant::now(),
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
        log::debug!("Creating new window");
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
        log::debug!(
            "Window dimensions: {}x{} ({}cols x {}rows, font_size={})",
            width,
            height,
            cols,
            rows,
            font_size
        );

        // Build window
        let mut window_attrs = Window::default_attributes()
            .with_title(&self.config.window.title)
            .with_inner_size(winit::dpi::LogicalSize::new(width, height));

        #[cfg(target_os = "macos")]
        {
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
        let mut glyph_cache = GlyphCache::with_variants(
            &shared.device,
            font_variants.clone(),
            scaled_font_size,
            line_height_multiplier,
        )
        .expect("Failed to create glyph cache");
        glyph_cache.precache_ascii();
        glyph_cache.flush(&shared.queue);

        let mut grid_renderer = GridRenderer::new(&shared.device, format);
        grid_renderer.set_glyph_cache(&shared.device, &glyph_cache);
        grid_renderer.update_screen_size(&shared.queue, size.width as f32, size.height as f32);

        // Separate renderer for output text (rendered flat, no glow)
        let mut output_grid_renderer = GridRenderer::new(&shared.device, format);
        output_grid_renderer.set_glyph_cache(&shared.device, &glyph_cache);
        output_grid_renderer.update_screen_size(
            &shared.queue,
            size.width as f32,
            size.height as f32,
        );

        // Tab bar uses same font at smaller size with fixed line height
        let tab_font_size = 12.0 * scale_factor;
        let mut tab_glyph_cache =
            GlyphCache::with_variants(&shared.device, font_variants, tab_font_size, 1.3)
                .expect("Failed to create tab glyph cache");
        tab_glyph_cache.precache_ascii();
        tab_glyph_cache.flush(&shared.queue);

        let mut tab_title_renderer = GridRenderer::new(&shared.device, format);
        tab_title_renderer.set_glyph_cache(&shared.device, &tab_glyph_cache);
        tab_title_renderer.update_screen_size(&shared.queue, size.width as f32, size.height as f32);

        // Effect pipeline for background rendering - get theme from registry
        let (theme_name, theme) = self.theme_registry.get_default_theme();
        let mut effect_pipeline = EffectPipeline::new(&shared.device, format);
        effect_pipeline.set_theme(theme.clone());

        // Backdrop effects renderer (grid, starfield, rain, particles, etc.)
        let mut effects_renderer =
            EffectsRenderer::new(&shared.device, shared.vello_renderer_arc(), format);
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
        terminal_vello.set_cursor_color([
            theme.cursor_color.r,
            theme.cursor_color.g,
            theme.cursor_color.b,
            theme.cursor_color.a,
        ]);
        terminal_vello.set_cursor_glow(theme.cursor_glow.map(|g| {
            (
                [g.color.r, g.color.g, g.color.b, g.color.a],
                g.radius,
                g.intensity,
            )
        }));

        // Rect renderer for cell backgrounds and tab bar
        let rect_renderer = RectRenderer::new(&shared.device, format);

        // Separate rect renderer for overlays (cursor, selection, underlines)
        // to avoid buffer conflicts with tab bar rendering
        let overlay_rect_renderer = RectRenderer::new(&shared.device, format);

        // Create instance buffers for renderers (external for future pooling)
        let grid_instance_buffer = GridRenderer::create_instance_buffer(&shared.device);
        let output_grid_instance_buffer = GridRenderer::create_instance_buffer(&shared.device);
        let tab_title_instance_buffer = GridRenderer::create_instance_buffer(&shared.device);
        let overlay_text_instance_buffer = GridRenderer::create_instance_buffer(&shared.device);
        let rect_instance_buffer = RectRenderer::create_instance_buffer(&shared.device);
        let overlay_rect_instance_buffer = RectRenderer::create_instance_buffer(&shared.device);

        // Background image pipeline (always created, state only if theme has background image)
        let background_image_pipeline = BackgroundImagePipeline::new(&shared.device, format);
        let (background_image_state, background_image_bind_group) =
            if let Some(ref bg_image) = theme.background_image {
                match BackgroundImageState::new(&shared.device, &shared.queue, bg_image) {
                    Ok(state) => {
                        let bind_group = background_image_pipeline
                            .create_bind_group(&shared.device, &state.texture.view);
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
        log::info!(
            "Checking sprite config: {:?}",
            theme.sprite.as_ref().map(|s| (s.enabled, &s.path))
        );
        let sprite_state = if let Some(ref sprite) = theme.sprite {
            if sprite.enabled {
                if let Some(ref path_str) = sprite.path {
                    // Resolve path relative to theme base directory
                    let path = std::path::PathBuf::from(path_str);
                    let resolved_path = if let Some(ref base_dir) = sprite.base_dir {
                        if path.is_relative() {
                            base_dir.join(&path)
                        } else {
                            path.clone()
                        }
                    } else {
                        path.clone()
                    };

                    log::info!("Resolved sprite path: {:?}", resolved_path);
                    let base_dir = sprite.base_dir.clone().unwrap_or_else(|| {
                        resolved_path
                            .parent()
                            .map(|p| p.to_path_buf())
                            .unwrap_or_default()
                    });
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
                        base_dir,
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

        // Create intermediate text texture for glow effect (from pool for memory reuse)
        let text_texture = shared
            .texture_pool
            .checkout(size.width, size.height, format)
            .expect("Texture pool checkout failed - pool lock poisoned");
        let composite_bind_group = effect_pipeline
            .composite
            .create_bind_group(&shared.device, text_texture.view());

        // CRT post-processing pipeline
        let mut crt_pipeline = CrtPipeline::new(&shared.device, format);
        crt_pipeline.set_effect(theme.crt);
        let (crt_texture, crt_bind_group) = if crt_pipeline.is_enabled() {
            log::info!("CRT effect enabled - creating intermediate texture from pool");
            let texture = shared
                .texture_pool
                .checkout(size.width, size.height, format)
                .expect("Texture pool checkout failed - pool lock poisoned");
            let bind_group = crt_pipeline.create_bind_group(&shared.device, texture.view());
            (Some(texture), Some(bind_group))
        } else {
            (None, None)
        };

        let gpu = WindowGpuState {
            surface,
            config: surface_config,
            glyph_cache,
            grid_renderer,
            output_grid_renderer,
            tab_glyph_cache,
            tab_title_renderer,
            grid_instance_buffer,
            output_grid_instance_buffer,
            tab_title_instance_buffer,
            overlay_text_instance_buffer,
            effect_pipeline,
            effects_renderer,
            tab_bar,
            terminal_vello,
            rect_renderer,
            overlay_rect_renderer,
            rect_instance_buffer,
            overlay_rect_instance_buffer,
            background_image_pipeline,
            background_image_state,
            background_image_bind_group,
            sprite_state,
            text_texture,
            composite_bind_group,
            crt_pipeline,
            crt_texture,
            crt_bind_group,
        };

        // Create initial shell with semantic prompts if enabled
        let mut shells = HashMap::new();
        let mut content_hashes = HashMap::new();

        // Inherit CWD from focused window if available, otherwise use config default
        let cwd = self
            .focused_window
            .and_then(|id| self.windows.get(&id))
            .and_then(|state| state.active_shell_cwd())
            .or_else(|| self.config.shell.working_directory.clone());

        let spawn_options = SpawnOptions {
            shell: self.config.shell.program.clone(),
            cwd,
            semantic_prompts: self.config.shell.semantic_prompts,
            shell_assets_dir: Config::shell_assets_dir(),
        };
        if let Ok(shell) = ShellTerminal::with_options(Size::new(cols, rows), spawn_options) {
            log::info!(
                "Shell spawned for initial tab (semantic_prompts={})",
                self.config.shell.semantic_prompts
            );
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
            render: window::RenderState {
                dirty: true,
                frame_count: 0,
                occluded: false,
                focused: true,
                cached: Default::default(),
                paste_pending: false,
            },
            interaction: Default::default(),
            ui: window::UiState {
                search: Default::default(),
                bell: window::BellState::from_config(&self.config.bell),
                context_menu: window::ContextMenu {
                    themes: self
                        .theme_registry
                        .list_themes()
                        .iter()
                        .map(|s| s.to_string())
                        .collect(),
                    current_theme: theme_name.to_string(),
                    ..Default::default()
                },
                zoom_indicator: Default::default(),
                copy_indicator: Default::default(),
                toast: Default::default(),
                window_rename: Default::default(),
                overrides: Default::default(),
                pending_theme: None,
            },
            custom_title: None,
            theme: theme.clone(),
            theme_name: theme_name.to_string(),
        };

        self.windows.insert(window_id, window_state);
        self.focused_window = Some(window_id);
        log::info!(
            "Created window {:?}, total: {}",
            window_id,
            self.windows.len()
        );
        window_id
    }

    fn focused_window_mut(&mut self) -> Option<&mut WindowState> {
        self.focused_window.and_then(|id| self.windows.get_mut(&id))
    }

    /// Update CRT pipeline and textures for new theme
    fn update_crt_pipeline(state: &mut WindowState, shared: &SharedGpuState, theme: &Theme) {
        // Update CRT effect settings
        state.gpu.crt_pipeline.set_effect(theme.crt);

        // Create or destroy CRT texture based on whether effect is enabled
        if state.gpu.crt_pipeline.is_enabled() {
            if state.gpu.crt_texture.is_none() {
                log::info!("CRT effect enabled - creating intermediate texture");
                let width = state.gpu.config.width;
                let height = state.gpu.config.height;
                let format = state.gpu.config.format;
                let texture = shared
                    .texture_pool
                    .checkout(width, height, format)
                    .expect("Texture pool checkout failed");
                let bind_group = state
                    .gpu
                    .crt_pipeline
                    .create_bind_group(&shared.device, texture.view());
                state.gpu.crt_texture = Some(texture);
                state.gpu.crt_bind_group = Some(bind_group);
            }
        } else {
            // Disable CRT - release texture back to pool (dropped automatically)
            if state.gpu.crt_texture.take().is_some() {
                log::info!("CRT effect disabled - releasing texture");
            }
            state.gpu.crt_bind_group = None;
        }
    }

    /// Update background image state for new theme
    fn update_background_image(
        state: &mut WindowState,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        theme: &Theme,
    ) {
        // Clear existing background image
        state.gpu.background_image_state = None;
        state.gpu.background_image_bind_group = None;

        // Create new background image if theme has one
        if let Some(ref bg_image) = theme.background_image {
            match BackgroundImageState::new(device, queue, bg_image) {
                Ok(bg_state) => {
                    let bind_group = state
                        .gpu
                        .background_image_pipeline
                        .create_bind_group(device, &bg_state.texture.view);
                    log::info!("Loaded background image: {:?}", bg_image.path);
                    state.gpu.background_image_state = Some(bg_state);
                    state.gpu.background_image_bind_group = Some(bind_group);
                }
                Err(e) => {
                    log::warn!("Failed to load background image: {}", e);
                }
            }
        }
    }

    /// Create sprite animation state from theme configuration
    fn create_sprite_state(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        theme: &Theme,
        format: wgpu::TextureFormat,
    ) -> Option<SpriteAnimationState> {
        let sprite = theme.sprite.as_ref()?;
        if !sprite.enabled {
            return None;
        }
        let path_str = sprite.path.as_ref()?;

        // Resolve path relative to theme base directory
        let path = std::path::PathBuf::from(path_str);
        let resolved_path = if let Some(ref base_dir) = sprite.base_dir {
            if path.is_relative() {
                base_dir.join(&path)
            } else {
                path.clone()
            }
        } else {
            path.clone()
        };

        log::info!("Creating sprite state from: {:?}", resolved_path);
        let base_dir = sprite.base_dir.clone().unwrap_or_else(|| {
            resolved_path
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_default()
        });

        let config = SpriteConfig {
            path: resolved_path,
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
            base_dir,
        };

        match SpriteAnimationState::new(device, queue, config, format) {
            Ok(state) => {
                log::info!("Loaded sprite animation");
                Some(state)
            }
            Err(e) => {
                log::warn!("Failed to load sprite animation: {}", e);
                None
            }
        }
    }

    fn close_window(&mut self, window_id: WindowId) {
        if let Some(ref shared) = self.shared_gpu {
            // Poll GPU to complete any pending work for this window before cleanup
            let _ = shared.device.poll(wgpu::PollType::Wait);

            // Explicitly cleanup GPU resources before dropping
            // This unconfigures the surface to release IOSurface buffers
            if let Some(state) = self.windows.get_mut(&window_id) {
                state.gpu.cleanup(&shared.device);
            }
        }

        // Now remove and drop the window state (triggers Drop impls)
        if self.windows.remove(&window_id).is_some() {
            log::info!(
                "Closed window {:?}, remaining: {}",
                window_id,
                self.windows.len()
            );

            // Poll again after Drop to ensure destroyed resources are freed
            if let Some(ref shared) = self.shared_gpu {
                let _ = shared.device.poll(wgpu::PollType::Wait);

                // Shrink texture pool to release excess pooled textures
                // This frees GPU memory from closed windows while keeping
                // one texture per bucket for quick reuse on next window
                shared.texture_pool.shrink();

                // Reset Vello renderer to free accumulated texture atlas memory
                // This prevents unbounded growth from windows being opened/closed
                shared.reset_vello_renderer();
            }

            if self.focused_window == Some(window_id) {
                self.focused_window = self.windows.keys().next().copied();
            }
        }
    }

    #[cfg(target_os = "macos")]
    fn handle_menu_action(&mut self, action: MenuAction, event_loop: &ActiveEventLoop) {
        match action {
            MenuAction::NewTab => {
                // Extract config values before borrowing state mutably
                let shell_program = self.config.shell.program.clone();
                let semantic_prompts = self.config.shell.semantic_prompts;
                let shell_assets_dir = Config::shell_assets_dir();

                if let Some(state) = self.focused_window_mut() {
                    // Get current shell's working directory for the new tab
                    let cwd = state.active_shell_cwd();
                    let tab_num = state.gpu.tab_bar.tab_count() + 1;
                    let tab_id = state.gpu.tab_bar.add_tab(format!("Terminal {}", tab_num));
                    state
                        .gpu
                        .tab_bar
                        .select_tab_index(state.gpu.tab_bar.tab_count() - 1);
                    let spawn_options = SpawnOptions {
                        shell: shell_program,
                        cwd,
                        semantic_prompts,
                        shell_assets_dir,
                    };
                    state.create_shell_for_tab(tab_id, spawn_options);
                    state.render.dirty = true;
                    state.window.request_redraw();
                }
            }
            MenuAction::NewWindow => {
                self.pending_new_window = true;
            }
            MenuAction::RenameWindow => {
                if let Some(state) = self.focused_window_mut() {
                    let current_title = state
                        .custom_title
                        .clone()
                        .unwrap_or_else(|| "CRT Terminal".to_string());
                    // Cancel tab editing if active
                    if state.gpu.tab_bar.is_editing() {
                        state.gpu.tab_bar.cancel_editing();
                    }
                    state.ui.window_rename.start(&current_title);
                    state.render.dirty = true;
                    state.window.request_redraw();
                }
            }
            MenuAction::CloseTab => {
                let should_close = if let Some(state) = self.focused_window_mut() {
                    if state.gpu.tab_bar.tab_count() > 1 {
                        if let Some(id) = state.gpu.tab_bar.active_tab_id() {
                            state.gpu.tab_bar.close_tab(id);
                            state.remove_shell_for_tab(id);
                            // Force redraw of new active tab to clear stale cached render state
                            state.force_active_tab_redraw();
                            state.window.request_redraw();
                        }
                        false
                    } else {
                        true
                    }
                } else {
                    false
                };
                if should_close && let Some(id) = self.focused_window {
                    self.close_window(id);
                }
            }
            MenuAction::CloseWindow => {
                if let Some(id) = self.focused_window {
                    self.close_window(id);
                }
            }
            MenuAction::Quit => event_loop.exit(),
            MenuAction::ToggleFullScreen => {
                if let Some(state) = self.focused_window_mut() {
                    let fs = state.window.fullscreen().is_some();
                    state.window.set_fullscreen(if fs {
                        None
                    } else {
                        Some(winit::window::Fullscreen::Borderless(None))
                    });
                }
            }
            MenuAction::IncreaseFontSize => self.adjust_font_scale(FONT_SCALE_STEP),
            MenuAction::DecreaseFontSize => self.adjust_font_scale(-FONT_SCALE_STEP),
            MenuAction::ResetFontSize => {
                // Compute delta to get back to scale 1.0
                if let Some(state) = self
                    .windows
                    .get(&self.focused_window.unwrap_or(WindowId::dummy()))
                {
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
                if let Some(state) = self.focused_window_mut()
                    && let Some(content) = get_clipboard_content()
                {
                    paste_to_terminal(state, &content);
                }
            }
            MenuAction::Copy => {
                if let Some(state) = self.focused_window_mut()
                    && let Some(text) = get_terminal_selection_text(state)
                {
                    set_clipboard_content(&text);
                    state.ui.copy_indicator.trigger();
                }
            }
            MenuAction::Find => {
                if let Some(state) = self.focused_window_mut() {
                    // Toggle search mode
                    state.ui.search.active = !state.ui.search.active;
                    if !state.ui.search.active {
                        // Clear search when closing
                        state.ui.search.query.clear();
                        state.ui.search.matches.clear();
                        state.ui.search.current_match = 0;
                    }
                    state.force_active_tab_redraw();
                    state.window.request_redraw();
                }
            }
            MenuAction::ToggleProfiling => {
                let (enabled, path) = profiling::toggle();
                if enabled {
                    if let Some(p) = path {
                        log::info!("Profiling started: {}", p.display());
                    }
                } else if let Some(p) = path {
                    log::info!("Profiling stopped. Log: {}", p.display());
                }
            }
            MenuAction::SetTheme(ref theme_name) => {
                if let Some(theme) = self.theme_registry.get_theme(theme_name).cloned() {
                    // Get window ID first to avoid borrow issues
                    if let Some(window_id) = self.focused_window
                        && let Some(state) = self.windows.get_mut(&window_id)
                    {
                        log::info!("Switching theme to: {}", theme_name);
                        state.set_theme(theme_name, theme.clone());
                        // Update backdrop effects
                        configure_effects_from_theme(&mut state.gpu.effects_renderer, &theme);
                        // Update GPU resources for new theme
                        if let Some(shared) = self.shared_gpu.as_ref() {
                            // Update sprite state
                            state.gpu.sprite_state = Self::create_sprite_state(
                                &shared.device,
                                &shared.queue,
                                &theme,
                                state.gpu.config.format,
                            );
                            // Update CRT pipeline (scanlines)
                            Self::update_crt_pipeline(state, shared, &theme);
                            // Update background image
                            Self::update_background_image(
                                state,
                                &shared.device,
                                &shared.queue,
                                &theme,
                            );
                        }
                        // Update context menu current theme
                        state.ui.context_menu.current_theme = theme_name.clone();
                        // Force full redraw
                        for hash in state.content_hashes.values_mut() {
                            *hash = 0;
                        }
                    }
                } else {
                    log::warn!("Theme '{}' not found in registry", theme_name);
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
            state
                .gpu
                .glyph_cache
                .set_font_size(&shared.queue, new_font_size);
            state.gpu.glyph_cache.precache_ascii();
            state.gpu.glyph_cache.flush(&shared.queue);

            // Update grid renderers with new glyph cache
            state
                .gpu
                .grid_renderer
                .set_glyph_cache(&shared.device, &state.gpu.glyph_cache);
            state
                .gpu
                .output_grid_renderer
                .set_glyph_cache(&shared.device, &state.gpu.glyph_cache);

            // Recalculate terminal grid size (like resize does)
            let cell_width = state.gpu.glyph_cache.cell_width();
            let line_height = state.gpu.glyph_cache.line_height();
            let tab_bar_height = state.gpu.tab_bar.height();

            let padding_physical = 20.0 * state.scale_factor;
            let tab_bar_physical = tab_bar_height * state.scale_factor;

            let content_width = (state.gpu.config.width as f32 - padding_physical).max(60.0);
            let content_height =
                (state.gpu.config.height as f32 - padding_physical - tab_bar_physical).max(40.0);

            let new_cols = ((content_width / cell_width) as usize).max(10);
            let new_rows = ((content_height / line_height) as usize).max(4);

            state.cols = new_cols;
            state.rows = new_rows;

            // Resize all shells to match new grid size
            for shell in state.shells.values_mut() {
                shell.resize(Size::new(new_cols, new_rows));
            }

            // Trigger zoom indicator
            state.ui.zoom_indicator.trigger(new_scale);

            // Force full redraw
            state.render.dirty = true;
            for hash in state.content_hashes.values_mut() {
                *hash = 0;
            }
            state.window.request_redraw();
        }
    }

    #[cfg(target_os = "macos")]
    fn navigate_tab(&mut self, next: bool) {
        if let Some(state) = self.focused_window_mut() {
            if next {
                state.gpu.tab_bar.next_tab();
            } else {
                state.gpu.tab_bar.prev_tab();
            }
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
        log::debug!(
            "Current theme: {}, font: {:?} @ {}pt",
            self.config.theme.name,
            self.config.font.family,
            self.config.font.size
        );
        let (new_config, config_error) = Config::load_with_error();

        // Show toast if there was a config error
        if let Some(error) = config_error
            && let Some(state) = self.focused_window_mut()
        {
            state.ui.toast.show(error, window::ToastType::Error);
        }

        // Check if theme changed
        let theme_changed = new_config.theme.name != self.config.theme.name;
        log::debug!(
            "New theme: {}, theme_changed: {}",
            new_config.theme.name,
            theme_changed
        );

        self.config = new_config;

        // Reload theme if it changed
        if theme_changed {
            self.reload_theme();
        }

        // Apply other config changes to all windows
        log::debug!("Applying config to {} windows", self.windows.len());
        for state in self.windows.values_mut() {
            // Force redraw
            state.render.dirty = true;
            for hash in state.content_hashes.values_mut() {
                *hash = 0;
            }
        }
    }

    /// Reload themes from disk and apply to all windows
    fn reload_theme(&mut self) {
        log::info!("Reloading themes from disk...");

        // Reload all themes in the registry
        self.theme_registry.reload_all();

        // Collect window updates to avoid borrow issues
        let window_themes: Vec<_> = self
            .windows
            .iter()
            .map(|(id, state)| (*id, state.theme_name.clone()))
            .collect();

        // Update each window with its reloaded theme
        for (window_id, theme_name) in window_themes {
            let theme = self
                .theme_registry
                .get_theme(&theme_name)
                .cloned()
                .unwrap_or_else(|| {
                    log::warn!(
                        "Theme '{}' not found after reload, using default",
                        theme_name
                    );
                    self.theme_registry.get_default_theme().1
                });

            if let Some(state) = self.windows.get_mut(&window_id) {
                // Update window theme
                state.set_theme(&theme_name, theme.clone());

                // Update backdrop effects from theme
                configure_effects_from_theme(&mut state.gpu.effects_renderer, &theme);

                // Force full redraw
                for hash in state.content_hashes.values_mut() {
                    *hash = 0;
                }

                log::debug!("Theme '{}' reloaded for window {:?}", theme_name, window_id);
            }
        }

        // Update App.theme for backward compatibility (event overrides)
        let (_, default_theme) = self.theme_registry.get_default_theme();
        self.theme = default_theme;

        log::debug!("Themes reloaded for {} windows", self.windows.len());
    }
}

/// Configure backdrop effects from theme settings
fn configure_effects_from_theme(effects_renderer: &mut EffectsRenderer, theme: &Theme) {
    let mut config = EffectConfig::new();

    // First, explicitly disable ALL effects to ensure clean state when switching themes.
    // This prevents effects from the previous theme persisting when the new theme
    // doesn't define them.
    config.insert("grid-enabled", "false");
    config.insert("starfield-enabled", "false");
    config.insert("rain-enabled", "false");
    config.insert("particles-enabled", "false");
    config.insert("matrix-enabled", "false");
    config.insert("shape-enabled", "false");
    config.insert("sprite-enabled", "false");

    // Now configure effects that are defined in the new theme (will override the disables above)

    // Grid effect configuration from theme
    if let Some(ref grid) = theme.grid {
        config.insert("grid-enabled", if grid.enabled { "true" } else { "false" });
        // Convert Color to rgba() string
        let c = grid.color;
        config.insert(
            "grid-color",
            format!(
                "rgba({}, {}, {}, {})",
                (c.r * 255.0) as u8,
                (c.g * 255.0) as u8,
                (c.b * 255.0) as u8,
                c.a
            ),
        );
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
        config.insert(
            "starfield-enabled",
            if starfield.enabled { "true" } else { "false" },
        );
        // Convert Color to rgba() string
        let c = starfield.color;
        config.insert(
            "starfield-color",
            format!(
                "rgba({}, {}, {}, {})",
                (c.r * 255.0) as u8,
                (c.g * 255.0) as u8,
                (c.b * 255.0) as u8,
                c.a
            ),
        );
        config.insert("starfield-density", starfield.density.to_string());
        config.insert("starfield-layers", starfield.layers.to_string());
        config.insert("starfield-speed", starfield.speed.to_string());
        config.insert(
            "starfield-direction",
            starfield.direction.as_str().to_string(),
        );
        config.insert("starfield-glow-radius", starfield.glow_radius.to_string());
        config.insert(
            "starfield-glow-intensity",
            starfield.glow_intensity.to_string(),
        );
        config.insert(
            "starfield-twinkle",
            if starfield.twinkle { "true" } else { "false" },
        );
        config.insert(
            "starfield-twinkle-speed",
            starfield.twinkle_speed.to_string(),
        );
        config.insert("starfield-min-size", starfield.min_size.to_string());
        config.insert("starfield-max-size", starfield.max_size.to_string());
    }

    // Rain effect configuration from theme
    if let Some(ref rain) = theme.rain {
        config.insert("rain-enabled", if rain.enabled { "true" } else { "false" });
        let c = rain.color;
        config.insert(
            "rain-color",
            format!(
                "rgba({}, {}, {}, {})",
                (c.r * 255.0) as u8,
                (c.g * 255.0) as u8,
                (c.b * 255.0) as u8,
                c.a
            ),
        );
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
        config.insert(
            "particles-enabled",
            if particles.enabled { "true" } else { "false" },
        );
        let c = particles.color;
        config.insert(
            "particles-color",
            format!(
                "rgba({}, {}, {}, {})",
                (c.r * 255.0) as u8,
                (c.g * 255.0) as u8,
                (c.b * 255.0) as u8,
                c.a
            ),
        );
        config.insert("particles-count", particles.count.to_string());
        config.insert("particles-shape", particles.shape.as_str().to_string());
        config.insert(
            "particles-behavior",
            particles.behavior.as_str().to_string(),
        );
        config.insert("particles-size", particles.size.to_string());
        config.insert("particles-speed", particles.speed.to_string());
        config.insert("particles-glow-radius", particles.glow_radius.to_string());
        config.insert(
            "particles-glow-intensity",
            particles.glow_intensity.to_string(),
        );
    }

    // Matrix effect configuration from theme
    if let Some(ref matrix) = theme.matrix {
        config.insert(
            "matrix-enabled",
            if matrix.enabled { "true" } else { "false" },
        );
        let c = matrix.color;
        config.insert(
            "matrix-color",
            format!(
                "rgba({}, {}, {}, {})",
                (c.r * 255.0) as u8,
                (c.g * 255.0) as u8,
                (c.b * 255.0) as u8,
                c.a
            ),
        );
        config.insert("matrix-density", matrix.density.to_string());
        config.insert("matrix-speed", matrix.speed.to_string());
        config.insert("matrix-font-size", matrix.font_size.to_string());
        config.insert("matrix-charset", matrix.charset.clone());
    }

    // Shape effect configuration from theme
    if let Some(ref shape) = theme.shape {
        config.insert(
            "shape-enabled",
            if shape.enabled { "true" } else { "false" },
        );
        config.insert("shape-type", shape.shape_type.as_str().to_string());
        config.insert("shape-size", shape.size.to_string());
        if let Some(ref fill) = shape.fill {
            config.insert(
                "shape-fill",
                format!(
                    "rgba({}, {}, {}, {})",
                    (fill.r * 255.0) as u8,
                    (fill.g * 255.0) as u8,
                    (fill.b * 255.0) as u8,
                    fill.a
                ),
            );
        } else {
            config.insert("shape-fill", "none".to_string());
        }
        if let Some(ref stroke) = shape.stroke {
            config.insert(
                "shape-stroke",
                format!(
                    "rgba({}, {}, {}, {})",
                    (stroke.r * 255.0) as u8,
                    (stroke.g * 255.0) as u8,
                    (stroke.b * 255.0) as u8,
                    stroke.a
                ),
            );
        } else {
            config.insert("shape-stroke", "none".to_string());
        }
        config.insert("shape-stroke-width", shape.stroke_width.to_string());
        config.insert("shape-glow-radius", shape.glow_radius.to_string());
        if let Some(ref glow_color) = shape.glow_color {
            config.insert(
                "shape-glow-color",
                format!(
                    "rgba({}, {}, {}, {})",
                    (glow_color.r * 255.0) as u8,
                    (glow_color.g * 255.0) as u8,
                    (glow_color.b * 255.0) as u8,
                    glow_color.a
                ),
            );
        }
        config.insert("shape-rotation", shape.rotation.as_str().to_string());
        config.insert("shape-rotation-speed", shape.rotation_speed.to_string());
        config.insert("shape-motion", shape.motion.as_str().to_string());
        config.insert("shape-motion-speed", shape.motion_speed.to_string());
        config.insert("shape-polygon-sides", shape.polygon_sides.to_string());
    }

    // Note: Sprite effect is disabled at the top of this function.
    // Sprite rendering uses raw wgpu SpriteRenderer (in render.rs) to avoid vello memory issues.

    effects_renderer.configure(&config);
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.windows.is_empty() {
            self.create_window(event_loop);

            #[cfg(target_os = "macos")]
            if self.menu.is_none() {
                let theme_names = self.theme_registry.list_themes();
                let current_theme = self.theme_registry.default_theme_name();
                let (menu, ids, window_submenu) = build_menu_bar(&theme_names, current_theme);
                menu.init_for_nsapp();
                // Register the Window menu with macOS so it automatically lists windows
                set_windows_menu(&window_submenu);
                self.menu = Some(menu);
                self.menu_ids = Some(ids);
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let Some(state) = self.windows.get_mut(&id) else {
            return;
        };

        match event {
            WindowEvent::CloseRequested => {
                self.windows.remove(&id);
                if self.focused_window == Some(id) {
                    self.focused_window = self.windows.keys().next().copied();
                }
            }

            WindowEvent::Focused(focused) => {
                state.render.focused = focused;

                // Trigger theme override for focus events
                if focused {
                    self.focused_window = Some(id);
                    // Apply on_focus override if theme defines it
                    if let Some(ref override_props) = self.theme.on_focus {
                        state.ui.overrides.add(
                            window::OverrideEventType::FocusGained,
                            override_props.clone(),
                        );
                        log::debug!("Focus gained - applied theme override");
                    }
                    // Redraw immediately when gaining focus to resume effects
                    state.window.request_redraw();
                } else {
                    // Apply on_blur override if theme defines it
                    if let Some(ref override_props) = self.theme.on_blur {
                        state
                            .ui
                            .overrides
                            .add(window::OverrideEventType::FocusLost, override_props.clone());
                        log::debug!("Focus lost - applied theme override");
                    }
                }
            }

            WindowEvent::Occluded(occluded) => {
                if let Some(state) = self.windows.get_mut(&id) {
                    state.render.occluded = occluded;
                    log::debug!("Window {:?} occluded: {}", id, occluded);
                }
            }

            WindowEvent::ModifiersChanged(m) => {
                self.modifiers = m;
            }

            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                let Some(state) = self.windows.get_mut(&id) else {
                    return;
                };

                // Delegate to keyboard handler
                let action = handle_keyboard_input(
                    state,
                    &event.logical_key,
                    event.text.as_ref().map(|s| s.as_str()),
                    &self.modifiers,
                );

                // Handle actions that require App-level access
                match action {
                    KeyboardAction::Quit => {
                        event_loop.exit();
                    }
                    KeyboardAction::CloseWindow => {
                        self.windows.remove(&id);
                        if self.focused_window == Some(id) {
                            self.focused_window = self.windows.keys().next().copied();
                        }
                    }
                    KeyboardAction::NewWindow => {
                        self.pending_new_window = true;
                    }
                    KeyboardAction::NewTab => {
                        // Extract config values before borrowing state mutably
                        let shell_program = self.config.shell.program.clone();
                        let semantic_prompts = self.config.shell.semantic_prompts;
                        let shell_assets_dir = Config::shell_assets_dir();

                        if let Some(state) = self.windows.get_mut(&id) {
                            let cwd = state.active_shell_cwd();
                            let tab_num = state.gpu.tab_bar.tab_count() + 1;
                            let tab_id = state.gpu.tab_bar.add_tab(format!("Terminal {}", tab_num));
                            state
                                .gpu
                                .tab_bar
                                .select_tab_index(state.gpu.tab_bar.tab_count() - 1);
                            let spawn_options = SpawnOptions {
                                shell: shell_program,
                                cwd,
                                semantic_prompts,
                                shell_assets_dir,
                            };
                            state.create_shell_for_tab(tab_id, spawn_options);
                            state.render.dirty = true;
                            state.window.request_redraw();
                        }
                    }
                    KeyboardAction::Handled
                    | KeyboardAction::NotHandled
                    | KeyboardAction::Scroll(_)
                    | KeyboardAction::CloseTab(_) => {
                        // Already handled by keyboard module or no action needed
                    }
                }
            }

            WindowEvent::Resized(size) => {
                let shared = self.shared_gpu.as_ref().unwrap();
                handle_resize(state, shared, size.width, size.height);
            }

            WindowEvent::ScaleFactorChanged {
                scale_factor,
                inner_size_writer: _,
            } => {
                let new_scale = scale_factor as f32;
                let old_scale = state.scale_factor;

                if (new_scale - old_scale).abs() > 0.001 {
                    log::debug!(
                        "Scale factor changed: {} -> {} (window {:?})",
                        old_scale,
                        new_scale,
                        id
                    );

                    let shared = self.shared_gpu.as_ref().unwrap();
                    handle_scale_factor_change(state, shared, &self.config, new_scale);
                }
            }

            WindowEvent::CursorMoved { position, .. } => {
                handle_cursor_moved(state, position.x as f32, position.y as f32);
            }

            WindowEvent::MouseInput {
                state: button_state,
                button,
                ..
            } => {
                handle_mouse_input(state, button, button_state, &self.modifiers);
                // Check for pending theme change from context menu
                if let Some(theme_name) = state.ui.pending_theme.take() {
                    if let Some(theme) = self.theme_registry.get_theme(&theme_name).cloned() {
                        log::info!("Switching theme via context menu to: {}", theme_name);
                        state.set_theme(&theme_name, theme.clone());
                        // Update effect pipeline theme (needed for event overrides like on_bell)
                        state.gpu.effect_pipeline.set_theme(theme.clone());
                        configure_effects_from_theme(&mut state.gpu.effects_renderer, &theme);
                        // Update GPU resources for new theme
                        if let Some(shared) = self.shared_gpu.as_ref() {
                            // Update sprite state
                            state.gpu.sprite_state = Self::create_sprite_state(
                                &shared.device,
                                &shared.queue,
                                &theme,
                                state.gpu.config.format,
                            );
                            // Update CRT pipeline (scanlines)
                            Self::update_crt_pipeline(state, shared, &theme);
                            // Update background image
                            Self::update_background_image(
                                state,
                                &shared.device,
                                &shared.queue,
                                &theme,
                            );
                        }
                        // Update context menu current theme
                        state.ui.context_menu.current_theme = theme_name;
                        // Force full redraw
                        for hash in state.content_hashes.values_mut() {
                            *hash = 0;
                        }
                    } else {
                        log::warn!("Theme '{}' not found in registry", theme_name);
                    }
                }
            }

            WindowEvent::MouseWheel { delta, .. } => {
                handle_mouse_wheel(state, delta);
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
        if let Some(ids) = &self.menu_ids
            && let Ok(event) = MenuEvent::receiver().try_recv()
            && let Some(action) = menu_id_to_action(event.id(), ids)
        {
            self.handle_menu_action(action, event_loop);
        }

        // Check for config/theme file changes - collect events first to avoid borrow issues
        let events: Vec<_> = self
            .config_watcher
            .as_mut()
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

        // Focused window: 60fps for smooth effects
        if elapsed >= TARGET_FRAME_TIME {
            self.last_frame_time = Instant::now();

            if let Some(focused_id) = self.focused_window
                && let Some(state) = self.windows.get(&focused_id)
                && !state.render.occluded
            {
                state.window.request_redraw();
            }
        }

        // Unfocused windows: 10fps for PTY output updates (saves GPU work)
        const UNFOCUSED_FRAME_TIME: std::time::Duration = std::time::Duration::from_millis(100);
        let unfocused_elapsed = self.last_unfocused_frame_time.elapsed();

        if unfocused_elapsed >= UNFOCUSED_FRAME_TIME {
            self.last_unfocused_frame_time = Instant::now();

            for (id, state) in self.windows.iter() {
                // Skip focused window (handled above) and occluded windows
                if Some(*id) != self.focused_window && !state.render.occluded {
                    state.window.request_redraw();
                }
            }
        }
    }
}

/// Handle scale factor change (display DPI change)
///
/// When a window moves between displays with different DPI (e.g., Retina to external monitor),
/// we need to recreate glyph caches with the new scaled font sizes and update all renderers.
fn handle_scale_factor_change(
    state: &mut WindowState,
    shared: &SharedGpuState,
    config: &Config,
    new_scale: f32,
) {
    use crt_core::Size;

    // Update scale factor
    state.scale_factor = new_scale;

    // Recreate glyph cache with new scaled font size
    let scaled_font_size = config.font.size * new_scale * state.font_scale;
    let line_height_multiplier = config.font.line_height;
    let font_variants = font::load_font_variants(&config.font);

    let mut glyph_cache = GlyphCache::with_variants(
        &shared.device,
        font_variants.clone(),
        scaled_font_size,
        line_height_multiplier,
    )
    .expect("Failed to create glyph cache");
    glyph_cache.precache_ascii();
    glyph_cache.flush(&shared.queue);

    // Update grid renderers with new glyph cache
    state
        .gpu
        .grid_renderer
        .set_glyph_cache(&shared.device, &glyph_cache);
    state
        .gpu
        .output_grid_renderer
        .set_glyph_cache(&shared.device, &glyph_cache);

    state.gpu.glyph_cache = glyph_cache;

    // Recreate tab glyph cache with new scaled tab font size
    let tab_font_size = 12.0 * new_scale;
    let mut tab_glyph_cache =
        GlyphCache::with_variants(&shared.device, font_variants, tab_font_size, 1.3)
            .expect("Failed to create tab glyph cache");
    tab_glyph_cache.precache_ascii();
    tab_glyph_cache.flush(&shared.queue);

    // Update tab title renderer with new glyph cache
    state
        .gpu
        .tab_title_renderer
        .set_glyph_cache(&shared.device, &tab_glyph_cache);

    state.gpu.tab_glyph_cache = tab_glyph_cache;

    // Update tab bar scale factor
    state.gpu.tab_bar.set_scale_factor(new_scale);

    // Recalculate terminal dimensions with new cell sizes
    let size = state.window.inner_size();
    let cell_width = state.gpu.glyph_cache.cell_width();
    let line_height = state.gpu.glyph_cache.line_height();
    let tab_bar_height = state.gpu.tab_bar.height();

    let padding_physical = 20.0 * new_scale;
    let tab_bar_physical = tab_bar_height * new_scale;

    let content_width = (size.width as f32 - padding_physical).max(60.0);
    let content_height = (size.height as f32 - padding_physical - tab_bar_physical).max(40.0);

    let new_cols = ((content_width / cell_width) as usize).max(10);
    let new_rows = ((content_height / line_height) as usize).max(4);

    state.cols = new_cols;
    state.rows = new_rows;

    // Resize all shells
    for shell in state.shells.values_mut() {
        shell.resize(Size::new(new_cols, new_rows));
    }

    // Mark as dirty and invalidate content hashes
    state.render.dirty = true;
    for hash in state.content_hashes.values_mut() {
        *hash = 0;
    }
    state.window.request_redraw();

    log::info!(
        "Scale factor updated to {}: font size {}px, grid {}x{}",
        new_scale,
        scaled_font_size,
        new_cols,
        new_rows
    );
}

fn main() {
    // Enable debug logging when profiling is enabled
    let profiling_enabled = std::env::var("CRT_PROFILE").is_ok();
    let default_filter = if profiling_enabled {
        "warn,crt=debug,crt_renderer=debug,crt_theme=debug,crt_core=debug"
    } else {
        "warn,crt=info"
    };

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(default_filter))
        .init();

    if profiling_enabled {
        log::info!("CRT Terminal starting (profiling mode - debug logging enabled)");
    } else {
        log::info!("CRT Terminal starting");
    }

    // Initialize profiling (enabled via CRT_PROFILE=1)
    profiling::init();

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);
    event_loop.run_app(&mut App::new()).unwrap();

    // Flush profiling data on exit
    profiling::shutdown();
}
