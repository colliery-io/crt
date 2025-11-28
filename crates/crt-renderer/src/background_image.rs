//! Background Image Loading and Rendering
//!
//! Supports loading static images (PNG, JPEG) and animated GIFs with frame timing.

use std::io::BufReader;
use std::path::Path;
use std::time::{Duration, Instant};

use crt_theme::{BackgroundImage, BackgroundPosition, BackgroundRepeat, BackgroundSize};
use image::AnimationDecoder;
use wgpu::util::DeviceExt;

/// A frame of an animated image
#[derive(Debug, Clone)]
pub struct ImageFrame {
    /// RGBA pixel data
    pub data: Vec<u8>,
    /// Width in pixels
    pub width: u32,
    /// Height in pixels
    pub height: u32,
    /// Duration to display this frame
    pub delay: Duration,
}

/// Loaded background image data (static or animated)
#[derive(Debug)]
pub enum LoadedImage {
    /// Single static image
    Static {
        data: Vec<u8>,
        width: u32,
        height: u32,
    },
    /// Animated image with multiple frames
    Animated {
        frames: Vec<ImageFrame>,
        current_frame: usize,
        last_frame_time: Instant,
    },
}

impl LoadedImage {
    /// Load an image from file path
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref();

        // Check if it's a GIF (may be animated)
        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_lowercase());

        if extension.as_deref() == Some("gif") {
            Self::load_gif(path)
        } else {
            Self::load_static(path)
        }
    }

    /// Load a static image (PNG, JPEG, etc.)
    fn load_static(path: &Path) -> Result<Self, String> {
        let img =
            image::open(path).map_err(|e| format!("Failed to load image {:?}: {}", path, e))?;

        let rgba = img.to_rgba8();
        let (width, height) = rgba.dimensions();

        Ok(LoadedImage::Static {
            data: rgba.into_raw(),
            width,
            height,
        })
    }

    /// Load a GIF (may be animated)
    fn load_gif(path: &Path) -> Result<Self, String> {
        let file = std::fs::File::open(path)
            .map_err(|e| format!("Failed to open GIF {:?}: {}", path, e))?;
        let reader = BufReader::new(file);

        let decoder = image::codecs::gif::GifDecoder::new(reader)
            .map_err(|e| format!("Failed to decode GIF {:?}: {}", path, e))?;

        let frames_result: Result<Vec<_>, _> = decoder.into_frames().collect();
        let frames =
            frames_result.map_err(|e| format!("Failed to decode GIF frames {:?}: {}", path, e))?;

        if frames.is_empty() {
            return Err(format!("GIF has no frames: {:?}", path));
        }

        // If only one frame, treat as static
        if frames.len() == 1 {
            let frame = frames.into_iter().next().unwrap();
            let rgba = frame.buffer().clone();
            let (width, height) = rgba.dimensions();
            return Ok(LoadedImage::Static {
                data: rgba.into_raw(),
                width,
                height,
            });
        }

        // Multiple frames - animated
        let image_frames: Vec<ImageFrame> = frames
            .into_iter()
            .map(|frame| {
                let delay = Duration::from(frame.delay());
                let rgba = frame.into_buffer();
                let (width, height) = rgba.dimensions();
                ImageFrame {
                    data: rgba.into_raw(),
                    width,
                    height,
                    delay,
                }
            })
            .collect();

        Ok(LoadedImage::Animated {
            frames: image_frames,
            current_frame: 0,
            last_frame_time: Instant::now(),
        })
    }

    /// Get current frame dimensions
    pub fn dimensions(&self) -> (u32, u32) {
        match self {
            LoadedImage::Static { width, height, .. } => (*width, *height),
            LoadedImage::Animated {
                frames,
                current_frame,
                ..
            } => {
                let frame = &frames[*current_frame];
                (frame.width, frame.height)
            }
        }
    }

    /// Get current frame data
    pub fn current_data(&self) -> &[u8] {
        match self {
            LoadedImage::Static { data, .. } => data,
            LoadedImage::Animated {
                frames,
                current_frame,
                ..
            } => &frames[*current_frame].data,
        }
    }

    /// Check if animation frame needs update, returns true if texture should be updated
    pub fn update_animation(&mut self) -> bool {
        match self {
            LoadedImage::Static { .. } => false,
            LoadedImage::Animated {
                frames,
                current_frame,
                last_frame_time,
            } => {
                let frame = &frames[*current_frame];
                if last_frame_time.elapsed() >= frame.delay {
                    *current_frame = (*current_frame + 1) % frames.len();
                    *last_frame_time = Instant::now();
                    true
                } else {
                    false
                }
            }
        }
    }

    /// Check if this is an animated image
    pub fn is_animated(&self) -> bool {
        matches!(self, LoadedImage::Animated { .. })
    }
}

/// GPU texture for background image
pub struct BackgroundTexture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    pub width: u32,
    pub height: u32,
}

impl BackgroundTexture {
    /// Create a new texture from loaded image data
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, image: &LoadedImage) -> Self {
        let (width, height) = image.dimensions();
        let data = image.current_data();

        let texture = device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some("Background Image Texture"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            data,
        );

        let view = texture.create_view(&Default::default());

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Background Image Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        Self {
            texture,
            view,
            sampler,
            width,
            height,
        }
    }

    /// Update texture data (for animated images)
    pub fn update(&self, queue: &wgpu::Queue, image: &LoadedImage) {
        let data = image.current_data();
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(self.width * 4),
                rows_per_image: Some(self.height),
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );
    }

    /// Create sampler with specific repeat mode
    pub fn create_sampler_with_repeat(
        device: &wgpu::Device,
        repeat: BackgroundRepeat,
    ) -> wgpu::Sampler {
        let (address_u, address_v) = match repeat {
            BackgroundRepeat::NoRepeat => (
                wgpu::AddressMode::ClampToEdge,
                wgpu::AddressMode::ClampToEdge,
            ),
            BackgroundRepeat::Repeat => (wgpu::AddressMode::Repeat, wgpu::AddressMode::Repeat),
            BackgroundRepeat::RepeatX => {
                (wgpu::AddressMode::Repeat, wgpu::AddressMode::ClampToEdge)
            }
            BackgroundRepeat::RepeatY => {
                (wgpu::AddressMode::ClampToEdge, wgpu::AddressMode::Repeat)
            }
        };

        device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Background Image Sampler (Custom Repeat)"),
            address_mode_u: address_u,
            address_mode_v: address_v,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        })
    }
}

/// Complete background image state for rendering
pub struct BackgroundImageState {
    pub image: LoadedImage,
    pub texture: BackgroundTexture,
    pub config: BackgroundImage,
}

impl BackgroundImageState {
    /// Load and create background image state
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        config: &BackgroundImage,
    ) -> Result<Self, String> {
        // Use resolved_path() to handle relative paths against theme directory
        let path = config
            .resolved_path()
            .ok_or_else(|| "No background image path specified".to_string())?;

        log::debug!("Loading background image from: {:?}", path);
        let image = LoadedImage::from_path(&path)?;
        let texture = BackgroundTexture::new(device, queue, &image);

        Ok(Self {
            image,
            texture,
            config: config.clone(),
        })
    }

    /// Update animation state, returns true if texture was updated
    pub fn update(&mut self, queue: &wgpu::Queue) -> bool {
        if self.image.update_animation() {
            self.texture.update(queue, &self.image);
            true
        } else {
            false
        }
    }

    /// Calculate UV transform for background sizing and positioning
    /// Returns (scale_x, scale_y, offset_x, offset_y)
    pub fn calculate_uv_transform(&self, screen_width: f32, screen_height: f32) -> [f32; 4] {
        let img_width = self.texture.width as f32;
        let img_height = self.texture.height as f32;

        let screen_aspect = screen_width / screen_height;
        let img_aspect = img_width / img_height;

        // Calculate the display size of the image in normalized screen coordinates (0-1)
        // norm_w/norm_h represent what fraction of the screen the image should occupy
        let (norm_w, norm_h) = match self.config.size {
            BackgroundSize::Cover => {
                // Fill screen completely, may crop
                (1.0, 1.0)
            }
            BackgroundSize::Contain => {
                // Fit within screen, maintaining aspect ratio
                if screen_aspect > img_aspect {
                    // Screen is wider than image - fit height
                    let h = 1.0;
                    let w = img_aspect / screen_aspect;
                    (w, h)
                } else {
                    // Screen is taller than image - fit width
                    let w = 1.0;
                    let h = screen_aspect / img_aspect;
                    (w, h)
                }
            }
            BackgroundSize::Auto => {
                // Original pixel size, doesn't scale with window
                let w = img_width / screen_width;
                let h = img_height / screen_height;
                (w, h)
            }
            BackgroundSize::Fixed(fw, fh) => {
                // Fixed pixel dimensions
                let w = fw as f32 / screen_width;
                let h = fh as f32 / screen_height;
                (w, h)
            }
            BackgroundSize::CanvasPercent(pct) => {
                // Percentage of canvas width, maintain aspect ratio
                let w = pct / 100.0;
                let h = w * screen_aspect / img_aspect;
                (w, h)
            }
            BackgroundSize::ImageScale(scale) => {
                // Scale relative to original image size
                let w = (img_width * scale) / screen_width;
                let h = (img_height * scale) / screen_height;
                (w, h)
            }
        };

        // Calculate position anchor (0-1 range)
        let (anchor_x, anchor_y) = match self.config.position {
            BackgroundPosition::Center => (0.5, 0.5),
            BackgroundPosition::TopLeft => (0.0, 0.0),
            BackgroundPosition::Top => (0.5, 0.0),
            BackgroundPosition::TopRight => (1.0, 0.0),
            BackgroundPosition::Left => (0.0, 0.5),
            BackgroundPosition::Right => (1.0, 0.5),
            BackgroundPosition::BottomLeft => (0.0, 1.0),
            BackgroundPosition::Bottom => (0.5, 1.0),
            BackgroundPosition::BottomRight => (1.0, 1.0),
            BackgroundPosition::Percent(x, y) => (x, y),
        };

        // For Cover mode, use the original UV scaling logic
        if matches!(self.config.size, BackgroundSize::Cover) {
            let (scale_x, scale_y) = if screen_aspect > img_aspect {
                (1.0, img_aspect / screen_aspect)
            } else {
                (screen_aspect / img_aspect, 1.0)
            };
            let uv_offset_x = anchor_x * (1.0 - 1.0 / scale_x).max(0.0);
            let uv_offset_y = anchor_y * (1.0 - 1.0 / scale_y).max(0.0);
            return [1.0 / scale_x, 1.0 / scale_y, uv_offset_x, uv_offset_y];
        }

        // For other modes: UV scale is inverse of normalized size
        // If image takes up 30% of screen (norm_w = 0.3), UV scale = 1/0.3 = 3.33
        // This means UV goes from 0 to 3.33, but we only show 0-1 portion
        let uv_scale_x = 1.0 / norm_w;
        let uv_scale_y = 1.0 / norm_h;

        // Offset to position the image
        // At anchor (0,0) = top-left: offset = 0
        // At anchor (0.5,0.5) = center: offset centers the visible portion
        // At anchor (1,1) = bottom-right: offset moves to end
        let uv_offset_x = -anchor_x * (uv_scale_x - 1.0);
        let uv_offset_y = -anchor_y * (uv_scale_y - 1.0);

        [uv_scale_x, uv_scale_y, uv_offset_x, uv_offset_y]
    }

    /// Get opacity
    pub fn opacity(&self) -> f32 {
        self.config.opacity
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uv_transform_cover_wide_screen() {
        // Create a mock config for testing
        let _config = BackgroundImage {
            path: None,
            base_dir: None,
            size: BackgroundSize::Cover,
            position: BackgroundPosition::Center,
            repeat: BackgroundRepeat::NoRepeat,
            opacity: 1.0,
        };

        // For a 1000x500 (2:1) screen with 500x500 (1:1) image
        // Cover should scale to fill, cropping height
        // scale_x = 1.0, scale_y = 0.5 (image needs to be scaled up 2x to cover width)
    }
}
