//! Font Rendering Prototype
//!
//! Real font rendering with ligatures using glyphon (cosmic-text + wgpu).
//! This demonstrates proper text shaping for programming fonts.

use glyphon::{
    Attrs, Buffer, Cache, Color, Family, FontSystem, Metrics, Resolution, Shaping, SwashCache,
    TextArea, TextBounds, TextRenderer, Viewport,
};
use std::sync::Arc;
use wgpu::MultisampleState;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

struct GpuState {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,

    // Text rendering
    font_system: FontSystem,
    swash_cache: SwashCache,
    viewport: Viewport,
    text_atlas: glyphon::TextAtlas,
    text_renderer: TextRenderer,
    text_buffer: Buffer,
}

struct App {
    window: Option<Arc<Window>>,
    gpu: Option<GpuState>,
}

impl App {
    fn new() -> Self {
        Self {
            window: None,
            gpu: None,
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title("Font Rendering - Ligatures with cosmic-text")
                        .with_inner_size(winit::dpi::LogicalSize::new(900, 600)),
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

        // Create swash cache for rasterization
        let swash_cache = SwashCache::new();

        // Create viewport
        let viewport = Viewport::new(&device, &Cache::new(&device));

        // Create text atlas and renderer
        let mut text_atlas = glyphon::TextAtlas::new(&device, &queue, &Cache::new(&device), format);
        let text_renderer = TextRenderer::new(
            &mut text_atlas,
            &device,
            MultisampleState::default(),
            None,
        );

        // Create text buffer with sample text showing ligatures
        let mut text_buffer = Buffer::new(&mut font_system, Metrics::new(24.0, 28.0));

        text_buffer.set_size(&mut font_system, Some(850.0), Some(550.0));

        // Text with common programming ligatures
        let sample_text = r#"Font Rendering with Ligatures

Common programming ligatures:
  -> => != == === <= >= |> <| :: ...

Code sample (ligatures depend on font):
  fn main() -> Result<(), Error> {
      let value = items.iter()
          .filter(|x| x.is_valid())
          .map(|x| x.value * 2)
          .collect::<Vec<_>>();

      if value != expected && value >= threshold {
          println!("Check: {} == {}", value, expected);
      }
      Ok(())
  }

Special characters: <!-- --> ==> <=> |=> www

Press ESC to exit"#;

        text_buffer.set_text(
            &mut font_system,
            sample_text,
            &Attrs::new().family(Family::Monospace),
            Shaping::Advanced, // Enable ligature shaping
        );

        text_buffer.shape_until_scroll(&mut font_system, false);

        self.window = Some(window);
        self.gpu = Some(GpuState {
            device,
            queue,
            surface,
            config,
            font_system,
            swash_cache,
            viewport,
            text_atlas,
            text_renderer,
            text_buffer,
        });

        log::info!("Initialized with glyphon (cosmic-text)");
        log::info!("Ligatures depend on system fonts - try with a coding font like Fira Code");
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state.is_pressed() {
                    use winit::keyboard::{Key, NamedKey};
                    if let Key::Named(NamedKey::Escape) = event.logical_key {
                        event_loop.exit();
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
                        Some(new_size.width as f32 - 50.0),
                        Some(new_size.height as f32 - 50.0),
                    );
                    gpu.text_buffer.shape_until_scroll(&mut gpu.font_system, false);
                }
            }
            WindowEvent::RedrawRequested => {
                if let Some(gpu) = &mut self.gpu {
                    // Update viewport
                    gpu.viewport.update(
                        &gpu.queue,
                        Resolution {
                            width: gpu.config.width,
                            height: gpu.config.height,
                        },
                    );

                    // Prepare text for rendering
                    gpu.text_renderer
                        .prepare(
                            &gpu.device,
                            &gpu.queue,
                            &mut gpu.font_system,
                            &mut gpu.text_atlas,
                            &gpu.viewport,
                            [TextArea {
                                buffer: &gpu.text_buffer,
                                left: 25.0,
                                top: 25.0,
                                scale: 1.0,
                                bounds: TextBounds {
                                    left: 0,
                                    top: 0,
                                    right: gpu.config.width as i32,
                                    bottom: gpu.config.height as i32,
                                },
                                default_color: Color::rgb(97, 226, 254), // Teal
                                custom_glyphs: &[],
                            }],
                            &mut gpu.swash_cache,
                        )
                        .unwrap();

                    let frame = gpu.surface.get_current_texture().unwrap();
                    let view = frame.texture.create_view(&Default::default());

                    let mut encoder = gpu.device.create_command_encoder(&Default::default());
                    {
                        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: Some("Text Render Pass"),
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                view: &view,
                                resolve_target: None,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Clear(wgpu::Color {
                                        r: 0.125,
                                        g: 0.035,
                                        b: 0.2,
                                        a: 1.0,
                                    }),
                                    store: wgpu::StoreOp::Store,
                                },
                            })],
                            depth_stencil_attachment: None,
                            timestamp_writes: None,
                            occlusion_query_set: None,
                        });

                        gpu.text_renderer.render(&gpu.text_atlas, &gpu.viewport, &mut pass).unwrap();
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
}

fn main() {
    env_logger::init();

    log::info!("Font Rendering Prototype");
    log::info!("Uses glyphon (cosmic-text + wgpu) for proper text shaping");
    log::info!("Ligatures like -> => != will render if your system has a ligature font");

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App::new();
    event_loop.run_app(&mut app).unwrap();
}
