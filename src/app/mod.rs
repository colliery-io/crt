//! Application state and lifecycle management.
//!
//! Contains the `App` struct and core methods for managing windows,
//! GPU state, config, and theme resources.

mod effects;
mod handler;
mod initialization;
mod menu_actions;

use std::collections::HashMap;
use std::time::Instant;

use crate::config::{Config, ConfigPaths};
use crate::gpu::SharedGpuState;
use crate::theme_registry::ThemeRegistry;
use crate::watcher;
use crate::window::WindowState;
use crt_renderer::{
    BackgroundImageState, SpriteAnimationState, SpriteConfig, SpriteMotion, SpritePosition,
};
use crt_theme::Theme;
use winit::window::WindowId;

#[cfg(target_os = "macos")]
use muda::Menu;

#[cfg(target_os = "macos")]
use crate::menu::MenuIds;

// Font scale bounds
const MIN_FONT_SCALE: f32 = 0.5;
const MAX_FONT_SCALE: f32 = 3.0;
const FONT_SCALE_STEP: f32 = 0.1;

pub(crate) struct App {
    pub(crate) windows: HashMap<WindowId, WindowState>,
    pub(crate) shared_gpu: Option<SharedGpuState>,
    pub(crate) focused_window: Option<WindowId>,
    pub(crate) config: Config,
    /// Current theme (stored for event override access)
    pub(crate) theme: Theme,
    /// Registry of available themes for runtime switching
    pub(crate) theme_registry: ThemeRegistry,
    pub(crate) modifiers: winit::event::Modifiers,
    pub(crate) pending_new_window: bool,
    pub(crate) config_watcher: Option<watcher::ConfigWatcher>,
    /// Last frame time for throttling focused window redraws (~60fps)
    pub(crate) last_frame_time: Instant,
    /// Last frame time for unfocused windows (~1fps for PTY updates)
    pub(crate) last_unfocused_frame_time: Instant,
    #[cfg(target_os = "macos")]
    pub(crate) menu: Option<Menu>,
    #[cfg(target_os = "macos")]
    pub(crate) menu_ids: Option<MenuIds>,
}

impl App {
    pub(crate) fn new() -> Self {
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
            modifiers: winit::event::Modifiers::default(),
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

    pub(crate) fn init_shared_gpu(&mut self) {
        if self.shared_gpu.is_none() {
            self.shared_gpu = Some(SharedGpuState::new());
        }
    }

    pub(crate) fn focused_window_mut(&mut self) -> Option<&mut WindowState> {
        self.focused_window.and_then(|id| self.windows.get_mut(&id))
    }

    /// Update CRT pipeline and textures for new theme
    pub(crate) fn update_crt_pipeline(
        state: &mut WindowState,
        shared: &SharedGpuState,
        theme: &Theme,
    ) {
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
    pub(crate) fn update_background_image(
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
    pub(crate) fn create_sprite_state(
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

    pub(crate) fn close_window(&mut self, window_id: WindowId) {
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
                shared.buffer_pool.shrink();

                // Reset Vello renderer to free accumulated texture atlas memory
                // This prevents unbounded growth from windows being opened/closed
                shared.reset_vello_renderer();
            }

            if self.focused_window == Some(window_id) {
                self.focused_window = self.windows.keys().next().copied();
            }
        }
    }

    /// Reload config from disk and apply changes
    pub(crate) fn reload_config(&mut self) {
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
            state
                .ui
                .toast
                .show(error, crate::window::ToastType::Error);
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
    pub(crate) fn reload_theme(&mut self) {
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
                effects::configure_effects_from_theme(&mut state.gpu.effects_renderer, &theme);

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

/// Apply a theme switch to a specific window state.
///
/// Shared helper that updates the window theme, effects, sprite, CRT pipeline,
/// background image, and context menu. Used by menu action, context menu,
/// and theme reload paths.
pub(crate) fn apply_theme_to_window(
    state: &mut WindowState,
    shared_gpu: Option<&SharedGpuState>,
    theme_name: &str,
    theme: &Theme,
) {
    log::info!("Switching theme to: {}", theme_name);
    state.set_theme(theme_name, theme.clone());
    effects::configure_effects_from_theme(&mut state.gpu.effects_renderer, theme);
    if let Some(shared) = shared_gpu {
        state.gpu.sprite_state = App::create_sprite_state(
            &shared.device,
            &shared.queue,
            theme,
            state.gpu.config.format,
        );
        App::update_crt_pipeline(state, shared, theme);
        App::update_background_image(state, &shared.device, &shared.queue, theme);
    }
    state.ui.context_menu.current_theme = theme_name.to_string();
    for hash in state.content_hashes.values_mut() {
        *hash = 0;
    }
}
