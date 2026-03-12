//! Window creation and GPU initialization.
//!
//! Contains the heavy `create_window()` function and scale factor handling.

use std::collections::HashMap;
use std::sync::Arc;

use crate::config::Config;
use crate::font;
use crate::gpu::{SharedGpuState, WindowGpuState};
use crate::window::{self, WindowState};
use crt_core::{ShellTerminal, Size, SpawnOptions};
use crt_renderer::{
    BackgroundImagePipeline, BackgroundImageState, CrtPipeline, EffectsRenderer, GlyphCache,
    GridEffect, GridRenderer, MatrixEffect, ParticleEffect, RainEffect, RectRenderer, ShapeEffect,
    SpriteEffect, StarfieldEffect, TabBar,
};
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowId};

use super::effects::configure_effects_from_theme;
use super::App;

#[cfg(target_os = "macos")]
use winit::platform::macos::WindowAttributesExtMacOS;

impl App {
    pub(crate) fn create_window(&mut self, event_loop: &ActiveEventLoop) -> WindowId {
        log::debug!("Creating new window");
        self.init_shared_gpu();
        let shared = self.shared_gpu.as_mut().unwrap();

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

        // Initialize shared render pipelines (created once, shared across all windows)
        shared.ensure_shared_pipelines(format);
        let pipelines = shared.shared_pipelines.as_ref().unwrap();

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

        let mut grid_renderer = GridRenderer::new_with_shared(&shared.device, &pipelines.grid);
        grid_renderer.set_glyph_cache(&shared.device, &glyph_cache);
        grid_renderer.update_screen_size(&shared.queue, size.width as f32, size.height as f32);

        // Separate renderer for output text (rendered flat, no glow)
        let mut output_grid_renderer = GridRenderer::new_with_shared(&shared.device, &pipelines.grid);
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

        let mut tab_title_renderer = GridRenderer::new_with_shared(&shared.device, &pipelines.grid);
        tab_title_renderer.set_glyph_cache(&shared.device, &tab_glyph_cache);
        tab_title_renderer
            .update_screen_size(&shared.queue, size.width as f32, size.height as f32);

        // Effect pipeline for background rendering - get theme from registry
        let (theme_name, theme) = self.theme_registry.get_default_theme();
        let mut effect_pipeline = crt_renderer::EffectPipeline::new_with_shared(
            &shared.device,
            &pipelines.background,
            &pipelines.composite,
        );
        effect_pipeline.set_theme(theme.clone());

        // Backdrop effects renderer (grid, starfield, rain, particles, etc.)
        let mut effects_renderer =
            EffectsRenderer::new_with_shared(shared.vello_renderer_arc(), &pipelines.effects_blit);
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
            crate::config::CursorStyle::Block => crt_renderer::CursorShape::Block,
            crate::config::CursorStyle::Bar => crt_renderer::CursorShape::Bar,
            crate::config::CursorStyle::Underline => crt_renderer::CursorShape::Underline,
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
        let rect_renderer = RectRenderer::new_with_shared(&shared.device, &pipelines.rect);

        // Separate rect renderer for overlays (cursor, selection, underlines)
        // to avoid buffer conflicts with tab bar rendering
        let overlay_rect_renderer = RectRenderer::new_with_shared(&shared.device, &pipelines.rect);

        // Checkout instance buffers from pool (reused across window lifecycles)
        use crate::gpu::BufferClass;
        let grid_instance_buffer = shared
            .buffer_pool
            .checkout(BufferClass::GridInstance)
            .expect("Buffer pool checkout failed");
        let output_grid_instance_buffer = shared
            .buffer_pool
            .checkout(BufferClass::GridInstance)
            .expect("Buffer pool checkout failed");
        let tab_title_instance_buffer = shared
            .buffer_pool
            .checkout(BufferClass::GridInstance)
            .expect("Buffer pool checkout failed");
        let overlay_text_instance_buffer = shared
            .buffer_pool
            .checkout(BufferClass::GridInstance)
            .expect("Buffer pool checkout failed");
        let rect_instance_buffer = shared
            .buffer_pool
            .checkout(BufferClass::RectInstance)
            .expect("Buffer pool checkout failed");
        let overlay_rect_instance_buffer = shared
            .buffer_pool
            .checkout(BufferClass::RectInstance)
            .expect("Buffer pool checkout failed");

        // Background image pipeline (always created, state only if theme has background image)
        let background_image_pipeline = BackgroundImagePipeline::new_with_shared(&shared.device, &pipelines.background_image);
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
        let sprite_state = Self::create_sprite_state(&shared.device, &shared.queue, &theme, format);

        // Create intermediate text texture for glow effect (from pool for memory reuse)
        let text_texture = shared
            .texture_pool
            .checkout(size.width, size.height, format)
            .expect("Texture pool checkout failed - pool lock poisoned");
        let composite_bind_group = effect_pipeline
            .composite
            .create_bind_group(&shared.device, text_texture.view());

        // CRT post-processing pipeline
        let mut crt_pipeline = CrtPipeline::new_with_shared(&shared.device, &pipelines.crt);
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
}

/// Handle scale factor change (display DPI change)
///
/// When a window moves between displays with different DPI (e.g., Retina to external monitor),
/// we need to recreate glyph caches with the new scaled font sizes and update all renderers.
pub(crate) fn handle_scale_factor_change(
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
