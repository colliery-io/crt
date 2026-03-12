//! Headless offscreen renderer for visual regression testing.
//!
//! Creates a wgpu device without a window surface, renders to an offscreen
//! texture, and captures the result as raw RGBA pixels or PNG bytes.

use image::{ImageBuffer, Rgba};

/// Errors from headless rendering operations.
#[derive(Debug, thiserror::Error)]
pub enum HeadlessError {
    #[error("no suitable GPU adapter found (tried software fallback)")]
    NoAdapter,
    #[error("failed to create GPU device: {0}")]
    DeviceCreation(#[from] wgpu::RequestDeviceError),
    #[error("failed to map staging buffer")]
    BufferMap,
    #[error("PNG encoding failed: {0}")]
    PngEncode(String),
}

/// Offscreen wgpu renderer for visual regression tests.
///
/// Creates a headless GPU context with software rendering support,
/// renders to an offscreen texture, and captures frames as pixels or PNG.
pub struct HeadlessRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    render_texture: wgpu::Texture,
    render_view: wgpu::TextureView,
    staging_buffer: wgpu::Buffer,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
    /// Padded bytes per row (wgpu requires 256-byte row alignment).
    padded_bytes_per_row: u32,
}

/// Minimum alignment for buffer copy rows required by wgpu.
const COPY_BYTES_PER_ROW_ALIGNMENT: u32 = 256;

impl HeadlessRenderer {
    /// Create a new headless renderer with the given dimensions.
    ///
    /// Uses `force_fallback_adapter: true` so this works in CI environments
    /// without GPU drivers (falls back to CPU-based software rendering).
    pub fn new(width: u32, height: u32) -> Result<Self, HeadlessError> {
        Self::with_options(width, height, true)
    }

    /// Create a headless renderer, optionally forcing software fallback.
    ///
    /// Set `force_fallback` to `false` when you want to test with real GPU
    /// hardware (e.g., local development).
    pub fn with_options(
        width: u32,
        height: u32,
        force_fallback: bool,
    ) -> Result<Self, HeadlessError> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());

        let adapter = pollster::block_on(async {
            instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::LowPower,
                    compatible_surface: None,
                    force_fallback_adapter: force_fallback,
                })
                .await
        })
        .map_err(|_| HeadlessError::NoAdapter)?;

        log::info!(
            "Headless renderer using adapter: {:?} (backend: {:?})",
            adapter.get_info().name,
            adapter.get_info().backend
        );

        let (device, queue): (wgpu::Device, wgpu::Queue) = pollster::block_on(async {
            adapter
                .request_device(&wgpu::DeviceDescriptor::default())
                .await
        })?;

        // Use Rgba8UnormSrgb — a universally supported format for offscreen rendering.
        // This avoids platform-dependent surface format issues (macOS prefers Bgra8,
        // Linux often uses Rgba8). For visual tests we always work in RGBA order.
        let format = wgpu::TextureFormat::Rgba8UnormSrgb;

        let render_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Headless Render Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let render_view = render_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // wgpu requires buffer copy rows to be aligned to 256 bytes.
        let unpadded_bytes_per_row = width * 4; // 4 bytes per RGBA pixel
        let padded_bytes_per_row = (unpadded_bytes_per_row + COPY_BYTES_PER_ROW_ALIGNMENT - 1)
            / COPY_BYTES_PER_ROW_ALIGNMENT
            * COPY_BYTES_PER_ROW_ALIGNMENT;

        let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Headless Staging Buffer"),
            size: (padded_bytes_per_row * height) as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Ok(Self {
            device,
            queue,
            render_texture,
            render_view,
            staging_buffer,
            width,
            height,
            format,
            padded_bytes_per_row,
        })
    }

    /// The GPU device.
    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    /// The GPU command queue.
    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    /// The render target texture view (use as a color attachment).
    pub fn texture_view(&self) -> &wgpu::TextureView {
        &self.render_view
    }

    /// The texture format used by this renderer.
    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    /// Width in pixels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Height in pixels.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Capture the current render texture contents as raw RGBA pixels.
    ///
    /// Returns a `width * height * 4` byte vector in row-major RGBA order,
    /// with no padding between rows.
    pub fn capture_frame(&self) -> Result<Vec<u8>, HeadlessError> {
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Headless Capture Encoder"),
            });

        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.render_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &self.staging_buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(self.padded_bytes_per_row),
                    rows_per_image: Some(self.height),
                },
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );

        self.queue.submit(std::iter::once(encoder.finish()));

        // Map the staging buffer and read back pixels.
        let buffer_slice = self.staging_buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            tx.send(result).ok();
        });
        let _ = self.device.poll(wgpu::PollType::Wait);

        rx.recv()
            .map_err(|_| HeadlessError::BufferMap)?
            .map_err(|_| HeadlessError::BufferMap)?;

        let data = buffer_slice.get_mapped_range();
        let unpadded_bytes_per_row = self.width as usize * 4;

        // Strip row padding to produce a tightly packed pixel buffer.
        let mut pixels = Vec::with_capacity(unpadded_bytes_per_row * self.height as usize);
        for row in 0..self.height as usize {
            let start = row * self.padded_bytes_per_row as usize;
            pixels.extend_from_slice(&data[start..start + unpadded_bytes_per_row]);
        }

        drop(data);
        self.staging_buffer.unmap();

        Ok(pixels)
    }

    /// Capture the current render texture as PNG-encoded bytes.
    pub fn capture_png(&self) -> Result<Vec<u8>, HeadlessError> {
        let pixels = self.capture_frame()?;

        let img: ImageBuffer<Rgba<u8>, _> =
            ImageBuffer::from_raw(self.width, self.height, pixels)
                .ok_or_else(|| HeadlessError::PngEncode("pixel buffer size mismatch".into()))?;

        let mut png_bytes = Vec::new();
        let mut cursor = std::io::Cursor::new(&mut png_bytes);
        img.write_to(&mut cursor, image::ImageFormat::Png)
            .map_err(|e| HeadlessError::PngEncode(e.to_string()))?;

        Ok(png_bytes)
    }

    /// Submit a command encoder and immediately capture the frame as raw RGBA.
    ///
    /// Convenience method that submits the encoder, then reads back pixels.
    pub fn submit_and_capture(&self, encoder: wgpu::CommandEncoder) -> Result<Vec<u8>, HeadlessError> {
        self.queue.submit(std::iter::once(encoder.finish()));
        self.capture_frame()
    }

    /// Clear the render texture to a solid color, then capture as raw RGBA.
    ///
    /// Useful for testing the capture pipeline itself.
    pub fn clear_and_capture(&self, color: wgpu::Color) -> Result<Vec<u8>, HeadlessError> {
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Headless Clear Encoder"),
            });

        {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Headless Clear Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.render_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(color),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            // Pass drops here, ending the render pass.
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        self.capture_frame()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_headless_renderer() {
        // Try with fallback first, fall back to hardware if not available
        let renderer = HeadlessRenderer::new(64, 64)
            .or_else(|_| HeadlessRenderer::with_options(64, 64, false));
        // Skip test if no adapter available at all (e.g., bare CI without any GPU)
        let renderer = match renderer {
            Ok(r) => r,
            Err(_) => {
                eprintln!("Skipping headless test: no GPU adapter available");
                return;
            }
        };

        assert_eq!(renderer.width(), 64);
        assert_eq!(renderer.height(), 64);
        assert_eq!(renderer.format(), wgpu::TextureFormat::Rgba8UnormSrgb);
    }

    #[test]
    fn test_clear_and_capture_solid_color() {
        let renderer = HeadlessRenderer::new(16, 16)
            .or_else(|_| HeadlessRenderer::with_options(16, 16, false));
        let renderer = match renderer {
            Ok(r) => r,
            Err(_) => {
                eprintln!("Skipping headless test: no GPU adapter available");
                return;
            }
        };

        // Clear to red (in linear color space, sRGB conversion happens in texture)
        let pixels = renderer
            .clear_and_capture(wgpu::Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            })
            .expect("capture failed");

        assert_eq!(pixels.len(), 16 * 16 * 4);

        // Every pixel should be red. In sRGB, linear 1.0 → 255.
        for y in 0..16 {
            for x in 0..16 {
                let idx = (y * 16 + x) * 4;
                assert_eq!(pixels[idx], 255, "R at ({x},{y})");
                assert_eq!(pixels[idx + 1], 0, "G at ({x},{y})");
                assert_eq!(pixels[idx + 2], 0, "B at ({x},{y})");
                assert_eq!(pixels[idx + 3], 255, "A at ({x},{y})");
            }
        }
    }

    #[test]
    fn test_capture_png_produces_valid_image() {
        let renderer = HeadlessRenderer::new(32, 24)
            .or_else(|_| HeadlessRenderer::with_options(32, 24, false));
        let renderer = match renderer {
            Ok(r) => r,
            Err(_) => {
                eprintln!("Skipping headless test: no GPU adapter available");
                return;
            }
        };

        // Clear to green
        let _ = renderer
            .clear_and_capture(wgpu::Color {
                r: 0.0,
                g: 1.0,
                b: 0.0,
                a: 1.0,
            })
            .expect("capture failed");

        let png_bytes = renderer.capture_png().expect("PNG capture failed");

        // Verify it's valid PNG by decoding it
        let decoded = image::load_from_memory(&png_bytes).expect("failed to decode PNG");
        assert_eq!(decoded.width(), 32);
        assert_eq!(decoded.height(), 24);
    }
}
