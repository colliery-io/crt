//! Terminal with Effects Example
//!
//! A terminal emulator with the full synthwave effect pipeline:
//! - Gradient background
//! - Perspective grid
//! - Text glow
//! - Themed colors

use std::sync::Arc;

use crt_core::{ShellTerminal, Size};
use crt_renderer::{EffectPipeline, TextRenderTarget};
use glyphon::{
    Attrs, Buffer, Cache, Color, Family, FontSystem, Metrics, Resolution, Shaping, SwashCache,
    TextArea, TextBounds, TextRenderer, Viewport,
};
use wgpu::MultisampleState;
use winit::{
    application::ApplicationHandler,
    event::{ElementState, WindowEvent},
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
    viewport: Viewport,
    text_atlas: glyphon::TextAtlas,
    text_renderer: TextRenderer,
    text_buffer: Buffer,

    // Offscreen text target
    text_target: TextRenderTarget,

    // Effect pipeline
    effect_pipeline: EffectPipeline,
}

struct App {
    window: Option<Arc<Window>>,
    gpu: Option<GpuState>,
    shell: Option<ShellTerminal>,
    dirty: bool,
}

impl App {
    fn new() -> Self {
        Self {
            window: None,
            gpu: None,
            shell: None,
            dirty: true,
        }
    }

    fn update_text_buffer(&mut self) {
        let Some(gpu) = &mut self.gpu else { return };
        let Some(shell) = &self.shell else { return };

        let content = shell.terminal().renderable_content();
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

            if point.column.0 == COLS - 1 {
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

        // Create effect pipeline with synthwave theme
        let effect_pipeline = EffectPipeline::new(&device, format);

        self.gpu = Some(GpuState {
            device,
            queue,
            surface,
            config,
            format,
            font_system,
            swash_cache,
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

        log::info!("Terminal with effects initialized: {}x{}", COLS, ROWS);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),

            WindowEvent::KeyboardInput { event, .. } => {
                if event.state != ElementState::Pressed {
                    return;
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
                            shell.send_input(c.as_bytes());
                            self.dirty = true;
                        }
                        _ => {}
                    }
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                }
            }

            WindowEvent::Resized(new_size) => {
                if let Some(gpu) = &mut self.gpu {
                    gpu.config.width = new_size.width.max(1);
                    gpu.config.height = new_size.height.max(1);
                    gpu.surface.configure(&gpu.device, &gpu.config);

                    gpu.text_buffer.set_size(
                        &mut gpu.font_system,
                        Some(new_size.width as f32 - 40.0),
                        Some(new_size.height as f32 - 40.0),
                    );

                    // Resize text render target
                    gpu.text_target.resize(
                        &gpu.device,
                        new_size.width,
                        new_size.height,
                        gpu.format,
                    );

                    self.dirty = true;
                }
            }

            WindowEvent::RedrawRequested => {
                // Process PTY output
                if let Some(shell) = &mut self.shell {
                    if shell.process_pty_output() {
                        self.dirty = true;
                    }
                }

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

                    // Pass 1: Render text to offscreen target
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
                                default_color: Color::rgb(200, 200, 200),
                                custom_glyphs: &[],
                            }],
                            &mut gpu.swash_cache,
                        )
                        .unwrap();

                    let frame = gpu.surface.get_current_texture().unwrap();
                    let frame_view = frame.texture.create_view(&Default::default());

                    let mut encoder = gpu.device.create_command_encoder(&Default::default());

                    // Pass 1: Render text to offscreen texture
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
                        // Update effect uniforms
                        gpu.effect_pipeline.update_uniforms(
                            &gpu.queue,
                            gpu.config.width as f32,
                            gpu.config.height as f32,
                        );

                        // Create bind group with text texture
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

    log::info!("CRT Terminal - Synthwave Edition");
    log::info!("Press ESC to exit");

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::wait_duration(std::time::Duration::from_millis(16)));

    let mut app = App::new();
    event_loop.run_app(&mut app).unwrap();
}
