//! Terminal with Effects Example
//!
//! A terminal emulator with the full synthwave effect pipeline:
//! - Gradient background
//! - Perspective grid
//! - Text glow
//! - Themed colors
//!
//! Usage:
//!   cargo run --example terminal_effects [theme.css]
//!
//! If no theme file is provided, defaults to themes/synthwave.css
//!
//! Hot-reload: Edit the CSS file while the terminal is running to see changes live!

use std::path::PathBuf;
use std::sync::mpsc::channel;
use std::sync::Arc;

use crt_core::{ShellTerminal, Size};
use crt_renderer::{EffectPipeline, TextRenderTarget};
use crt_theme::Theme;
use glyphon::{
    Attrs, Buffer, Cache, Color, Family, FontSystem, Metrics, Resolution, Shaping, SwashCache,
    TextArea, TextBounds, TextRenderer, Viewport,
};
use muda::{accelerator::{Accelerator, Code, Modifiers as MenuModifiers}, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem, Submenu};
use notify::{Event as NotifyEvent, RecommendedWatcher, RecursiveMode, Watcher};
use wgpu::MultisampleState;
use winit::{
    application::ApplicationHandler,
    event::{ElementState, Modifiers, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::{Window, WindowId},
};

const FONT_SIZE: f32 = 14.0;
const LINE_HEIGHT: f32 = 18.0;
const COLS: usize = 80;
const ROWS: usize = 24;

struct GpuState {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    format: wgpu::TextureFormat,

    // Text rendering (to offscreen target)
    font_system: FontSystem,
    swash_cache: SwashCache,
    #[allow(dead_code)]
    cache: Cache, // Kept for potential atlas recreation on resize
    viewport: Viewport,
    text_atlas: glyphon::TextAtlas,
    text_renderer: TextRenderer,
    text_buffer: Buffer,

    // Offscreen text target
    text_target: TextRenderTarget,

    // Effect pipeline
    effect_pipeline: EffectPipeline,
}

// Menu item IDs
const MENU_CLEAR: &str = "clear";
const MENU_RELOAD_THEME: &str = "reload_theme";

struct App {
    window: Option<Arc<Window>>,
    gpu: Option<GpuState>,
    shell: Option<ShellTerminal>,
    theme: Theme,
    theme_path: PathBuf,
    theme_watcher: Option<RecommendedWatcher>,
    theme_rx: Option<std::sync::mpsc::Receiver<Result<NotifyEvent, notify::Error>>>,
    modifiers: Modifiers,
    dirty: bool,
}

impl App {
    fn new(theme: Theme, theme_path: PathBuf) -> Self {
        Self {
            window: None,
            gpu: None,
            shell: None,
            theme,
            theme_path,
            theme_watcher: None,
            theme_rx: None,
            modifiers: Modifiers::default(),
            dirty: true,
        }
    }

    #[cfg(target_os = "macos")]
    fn setup_menu(&mut self) {
        // Create menu bar
        let menu_bar = Menu::new();

        // App menu (macOS standard)
        let app_menu = Submenu::new("CRT Terminal", true);
        app_menu.append(&PredefinedMenuItem::about(None, None)).ok();
        app_menu.append(&PredefinedMenuItem::separator()).ok();
        app_menu.append(&PredefinedMenuItem::services(None)).ok();
        app_menu.append(&PredefinedMenuItem::separator()).ok();
        app_menu.append(&PredefinedMenuItem::hide(None)).ok();
        app_menu.append(&PredefinedMenuItem::hide_others(None)).ok();
        app_menu.append(&PredefinedMenuItem::show_all(None)).ok();
        app_menu.append(&PredefinedMenuItem::separator()).ok();
        app_menu.append(&PredefinedMenuItem::quit(None)).ok();
        menu_bar.append(&app_menu).ok();

        // Edit menu
        let edit_menu = Submenu::new("Edit", true);
        edit_menu.append(&PredefinedMenuItem::undo(None)).ok();
        edit_menu.append(&PredefinedMenuItem::redo(None)).ok();
        edit_menu.append(&PredefinedMenuItem::separator()).ok();
        edit_menu.append(&PredefinedMenuItem::cut(None)).ok();
        edit_menu.append(&PredefinedMenuItem::copy(None)).ok();
        edit_menu.append(&PredefinedMenuItem::paste(None)).ok();
        edit_menu.append(&PredefinedMenuItem::select_all(None)).ok();
        edit_menu.append(&PredefinedMenuItem::separator()).ok();
        edit_menu.append(&MenuItem::with_id(
            MenuId::new(MENU_CLEAR),
            "Clear",
            true,
            Some(Accelerator::new(Some(MenuModifiers::SUPER), Code::KeyK)),
        )).ok();
        menu_bar.append(&edit_menu).ok();

        // View menu
        let view_menu = Submenu::new("View", true);
        view_menu.append(&MenuItem::with_id(
            MenuId::new(MENU_RELOAD_THEME),
            "Reload Theme",
            true,
            Some(Accelerator::new(Some(MenuModifiers::SUPER.union(MenuModifiers::SHIFT)), Code::KeyR)),
        )).ok();
        view_menu.append(&PredefinedMenuItem::separator()).ok();
        view_menu.append(&PredefinedMenuItem::fullscreen(None)).ok();
        menu_bar.append(&view_menu).ok();

        // Window menu
        let window_menu = Submenu::new("Window", true);
        window_menu.append(&PredefinedMenuItem::minimize(None)).ok();
        window_menu.append(&PredefinedMenuItem::maximize(None)).ok();
        window_menu.append(&PredefinedMenuItem::separator()).ok();
        window_menu.append(&PredefinedMenuItem::close_window(None)).ok();
        menu_bar.append(&window_menu).ok();

        // Initialize menu bar for macOS
        menu_bar.init_for_nsapp();

        log::info!("macOS menu bar initialized");
    }

    #[cfg(not(target_os = "macos"))]
    fn setup_menu(&mut self) {
        // Menu bar not implemented for other platforms yet
        log::info!("Menu bar not available on this platform");
    }

    fn handle_menu_events(&mut self, event_loop: &ActiveEventLoop) {
        let rx = MenuEvent::receiver();

        while let Ok(event) = rx.try_recv() {
            let id = event.id().0.as_str();
            match id {
                MENU_CLEAR => {
                    // Send clear screen escape sequence
                    if let Some(shell) = &self.shell {
                        shell.send_input(b"\x1b[2J\x1b[H");
                        self.dirty = true;
                    }
                }
                MENU_RELOAD_THEME => {
                    // Force theme reload
                    match Theme::from_css_file(&self.theme_path) {
                        Ok(new_theme) => {
                            self.theme = new_theme;
                            if let Some(gpu) = &mut self.gpu {
                                gpu.effect_pipeline.set_theme(self.theme.clone());
                            }
                            log::info!("Theme reloaded from menu");
                        }
                        Err(e) => {
                            log::error!("Failed to reload theme: {}", e);
                        }
                    }
                }
                _ => {
                    // Handle predefined menu items (quit, etc.)
                    if id.contains("quit") {
                        event_loop.exit();
                    }
                }
            }
        }
    }

    fn setup_file_watcher(&mut self) {
        let (tx, rx) = channel();
        let watcher = notify::recommended_watcher(tx);

        match watcher {
            Ok(mut w) => {
                if let Err(e) = w.watch(&self.theme_path, RecursiveMode::NonRecursive) {
                    log::warn!("Failed to watch theme file: {}", e);
                } else {
                    log::info!("Watching theme file for changes: {:?}", self.theme_path);
                    self.theme_watcher = Some(w);
                    self.theme_rx = Some(rx);
                }
            }
            Err(e) => {
                log::warn!("Failed to create file watcher: {}", e);
            }
        }
    }

    fn check_theme_reload(&mut self) {
        let Some(rx) = &self.theme_rx else { return };

        // Drain all pending events
        let mut should_reload = false;
        while let Ok(event) = rx.try_recv() {
            if let Ok(event) = event {
                // Check for modify or create events
                if matches!(
                    event.kind,
                    notify::EventKind::Modify(_) | notify::EventKind::Create(_)
                ) {
                    should_reload = true;
                }
            }
        }

        if should_reload {
            log::info!("Theme file changed, reloading...");
            match Theme::from_css_file(&self.theme_path) {
                Ok(new_theme) => {
                    self.theme = new_theme;
                    if let Some(gpu) = &mut self.gpu {
                        gpu.effect_pipeline.set_theme(self.theme.clone());
                    }
                    log::info!("Theme reloaded successfully");
                }
                Err(e) => {
                    log::error!("Failed to reload theme: {}", e);
                }
            }
        }
    }

    fn update_text_buffer(&mut self) {
        let Some(gpu) = &mut self.gpu else { return };
        let Some(shell) = &self.shell else { return };

        let term = shell.terminal();
        let cols = term.columns();
        let content = term.renderable_content();
        let cursor = content.cursor;
        let cursor_point = cursor.point;
        let mut text = String::new();

        for indexed in content.display_iter {
            let cell = &indexed.cell;
            let point = indexed.point;

            if point.line == cursor_point.line && point.column == cursor_point.column {
                text.push('\u{2588}');
            } else {
                text.push(cell.c);
            }

            if point.column.0 == cols - 1 {
                text.push('\n');
            }
        }

        gpu.text_buffer.set_text(
            &mut gpu.font_system,
            &text,
            &Attrs::new().family(Family::Monospace),
            Shaping::Advanced,
        );
        gpu.text_buffer.shape_until_scroll(&mut gpu.font_system, false);
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let width = (COLS as f32 * FONT_SIZE * 0.6) as u32 + 40;
        let height = (ROWS as f32 * LINE_HEIGHT) as u32 + 40;

        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title("CRT Terminal - Synthwave Edition")
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
        let caps = surface.get_capabilities(&adapter);
        let format = caps.formats[0];

        // Use AutoNoVsync for faster frame presentation during resize
        // Fall back to Fifo if not supported
        let present_mode = if caps.present_modes.contains(&wgpu::PresentMode::AutoNoVsync) {
            wgpu::PresentMode::AutoNoVsync
        } else {
            wgpu::PresentMode::Fifo
        };

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width,
            height: size.height,
            present_mode,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        // Initialize font system
        let mut font_system = FontSystem::new();
        let swash_cache = SwashCache::new();
        let cache = Cache::new(&device);
        let viewport = Viewport::new(&device, &cache);

        let mut text_atlas = glyphon::TextAtlas::new(&device, &queue, &cache, format);
        let text_renderer = TextRenderer::new(
            &mut text_atlas,
            &device,
            MultisampleState::default(),
            None,
        );

        let mut text_buffer = Buffer::new(&mut font_system, Metrics::new(FONT_SIZE, LINE_HEIGHT));
        text_buffer.set_size(
            &mut font_system,
            Some(size.width as f32 - 40.0),
            Some(size.height as f32 - 40.0),
        );

        // Create offscreen text render target
        let text_target = TextRenderTarget::new(&device, size.width, size.height, format);

        // Create effect pipeline and apply loaded theme
        let mut effect_pipeline = EffectPipeline::new(&device, format);
        effect_pipeline.set_theme(self.theme.clone());

        self.gpu = Some(GpuState {
            device,
            queue,
            surface,
            config,
            format,
            font_system,
            swash_cache,
            cache,
            viewport,
            text_atlas,
            text_renderer,
            text_buffer,
            text_target,
            effect_pipeline,
        });

        // Create shell terminal
        match ShellTerminal::new(Size::new(COLS, ROWS)) {
            Ok(shell) => {
                log::info!("Shell spawned successfully");
                self.shell = Some(shell);
            }
            Err(e) => {
                log::error!("Failed to spawn shell: {}", e);
            }
        }

        self.window = Some(window);

        // Set up file watcher for hot-reload
        self.setup_file_watcher();
        self.setup_menu();

        log::info!("Terminal with effects initialized: {}x{}", COLS, ROWS);
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

                // Check for Cmd/Super modifier (macOS shortcuts)
                let super_pressed = self.modifiers.state().super_key();

                // Handle system shortcuts first
                if super_pressed {
                    match &event.logical_key {
                        Key::Character(c) if c.as_str() == "q" => {
                            event_loop.exit();
                            return;
                        }
                        Key::Character(c) if c.as_str() == "w" => {
                            event_loop.exit();
                            return;
                        }
                        Key::Character(c) if c.as_str() == "k" => {
                            // Clear screen (Cmd+K)
                            if let Some(shell) = &self.shell {
                                shell.send_input(b"\x1b[2J\x1b[H");
                                self.dirty = true;
                            }
                            return;
                        }
                        _ => {}
                    }
                }

                if let Some(shell) = &self.shell {
                    match &event.logical_key {
                        Key::Named(NamedKey::Escape) => {
                            event_loop.exit();
                        }
                        Key::Named(NamedKey::Enter) => {
                            shell.send_input(b"\r");
                            self.dirty = true;
                        }
                        Key::Named(NamedKey::Backspace) => {
                            shell.send_input(b"\x7f");
                            self.dirty = true;
                        }
                        Key::Named(NamedKey::Tab) => {
                            shell.send_input(b"\t");
                            self.dirty = true;
                        }
                        Key::Named(NamedKey::ArrowUp) => {
                            shell.send_input(b"\x1b[A");
                            self.dirty = true;
                        }
                        Key::Named(NamedKey::ArrowDown) => {
                            shell.send_input(b"\x1b[B");
                            self.dirty = true;
                        }
                        Key::Named(NamedKey::ArrowRight) => {
                            shell.send_input(b"\x1b[C");
                            self.dirty = true;
                        }
                        Key::Named(NamedKey::ArrowLeft) => {
                            shell.send_input(b"\x1b[D");
                            self.dirty = true;
                        }
                        Key::Named(NamedKey::Space) => {
                            shell.send_input(b" ");
                            self.dirty = true;
                        }
                        Key::Character(c) => {
                            // Don't send to shell if super key is pressed
                            if !super_pressed {
                                shell.send_input(c.as_bytes());
                                self.dirty = true;
                            }
                        }
                        _ => {}
                    }
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                }
            }

            WindowEvent::Resized(new_size) => {
                // Require minimum window size to avoid artifacts
                let min_width = 100;
                let min_height = 80;
                if new_size.width < min_width || new_size.height < min_height {
                    return;
                }

                // Calculate new terminal size based on window size
                let content_width = (new_size.width as f32 - 40.0).max(60.0);
                let content_height = (new_size.height as f32 - 40.0).max(40.0);
                let new_cols = (content_width / (FONT_SIZE * 0.6)) as usize;
                let new_rows = (content_height / LINE_HEIGHT) as usize;
                let new_cols = new_cols.max(10);
                let new_rows = new_rows.max(4);

                // Resize shell terminal
                if let Some(shell) = &mut self.shell {
                    shell.resize(Size::new(new_cols, new_rows));
                }

                if let Some(gpu) = &mut self.gpu {
                    gpu.config.width = new_size.width;
                    gpu.config.height = new_size.height;
                    gpu.surface.configure(&gpu.device, &gpu.config);

                    // Update viewport immediately on resize
                    gpu.viewport.update(
                        &gpu.queue,
                        Resolution {
                            width: new_size.width,
                            height: new_size.height,
                        },
                    );

                    gpu.text_buffer.set_size(
                        &mut gpu.font_system,
                        Some(content_width),
                        Some(content_height),
                    );

                    // Resize text render target
                    gpu.text_target.resize(
                        &gpu.device,
                        new_size.width,
                        new_size.height,
                        gpu.format,
                    );
                }
                self.dirty = true;

                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }

            WindowEvent::RedrawRequested => {
                // Check for theme hot-reload
                self.check_theme_reload();

                // Handle menu events
                self.handle_menu_events(event_loop);

                // Process PTY output
                if let Some(shell) = &mut self.shell {
                    if shell.process_pty_output() {
                        self.dirty = true;
                    }
                }

                // Always update text buffer when dirty
                if self.dirty {
                    self.update_text_buffer();
                    self.dirty = false;
                }

                if let Some(gpu) = &mut self.gpu {
                    // Update viewport
                    gpu.viewport.update(
                        &gpu.queue,
                        Resolution {
                            width: gpu.config.width,
                            height: gpu.config.height,
                        },
                    );

                    let frame = gpu.surface.get_current_texture().unwrap();
                    let frame_view = frame.texture.create_view(&Default::default());

                    let mut encoder = gpu.device.create_command_encoder(&Default::default());

                    // Pass 1: Render text to offscreen texture
                    gpu.text_renderer
                        .prepare(
                            &gpu.device,
                            &gpu.queue,
                            &mut gpu.font_system,
                            &mut gpu.text_atlas,
                            &gpu.viewport,
                            [TextArea {
                                buffer: &gpu.text_buffer,
                                left: 20.0,
                                top: 20.0,
                                scale: 1.0,
                                bounds: TextBounds {
                                    left: 0,
                                    top: 0,
                                    right: gpu.config.width as i32,
                                    bottom: gpu.config.height as i32,
                                },
                                default_color: Color::rgb(255, 255, 255),
                                custom_glyphs: &[],
                            }],
                            &mut gpu.swash_cache,
                        )
                        .unwrap();

                    {
                        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: Some("Text Render Pass"),
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                view: &gpu.text_target.view,
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

                        gpu.text_renderer
                            .render(&gpu.text_atlas, &gpu.viewport, &mut pass)
                            .unwrap();
                    }

                    // Pass 2: Apply effects and render to screen
                    {
                        gpu.effect_pipeline.update_uniforms(
                            &gpu.queue,
                            gpu.config.width as f32,
                            gpu.config.height as f32,
                        );

                        let bind_group = gpu.effect_pipeline.create_bind_group(
                            &gpu.device,
                            &gpu.text_target.view,
                        );

                        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: Some("Effect Render Pass"),
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

                        gpu.effect_pipeline.render(&mut pass, &bind_group);
                    }

                    gpu.queue.submit(std::iter::once(encoder.finish()));
                    frame.present();

                    gpu.text_atlas.trim();
                }

                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

fn main() {
    env_logger::init();

    // Get theme file from command line or use default
    let args: Vec<String> = std::env::args().collect();
    let theme_path = if args.len() > 1 {
        PathBuf::from(&args[1])
    } else {
        PathBuf::from("themes/synthwave.css")
    };

    // Load theme
    let theme = match Theme::from_css_file(&theme_path) {
        Ok(t) => {
            log::info!("Loaded theme from: {:?}", theme_path);
            t
        }
        Err(e) => {
            log::warn!("Failed to load theme from {:?}: {}. Using default.", theme_path, e);
            Theme::synthwave()
        }
    };

    log::info!("CRT Terminal - Synthwave Edition");
    log::info!("Press ESC to exit");
    log::info!("Hot-reload enabled: edit {:?} to update theme live", theme_path);

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::wait_duration(std::time::Duration::from_millis(16)));

    let mut app = App::new(theme, theme_path);
    event_loop.run_app(&mut app).unwrap();
}
