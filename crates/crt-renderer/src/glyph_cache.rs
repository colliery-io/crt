//! Glyph cache with texture atlas for fast terminal rendering
//!
//! Pre-rasterizes glyphs using swash and stores them in a GPU texture atlas.
//! Uses fixed-width grid positioning for terminal rendering.

use std::collections::HashMap;
use swash::{
    scale::{Render, ScaleContext, Source, StrikeWith},
    zeno::Format,
    FontRef,
};

/// Key for glyph lookup - character + size
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct GlyphKey {
    pub character: char,
    pub size_tenths: u16,
}

impl GlyphKey {
    pub fn new(character: char, size: f32) -> Self {
        Self {
            character,
            size_tenths: (size * 10.0) as u16,
        }
    }
}

/// Cached glyph data - position in atlas + metrics for positioning
#[derive(Clone, Copy, Debug)]
pub struct CachedGlyph {
    /// UV coordinates in atlas (normalized 0.0-1.0)
    pub uv_min: [f32; 2],
    pub uv_max: [f32; 2],
    /// Glyph bitmap dimensions
    pub width: f32,
    pub height: f32,
    /// Placement offset from cell origin
    pub offset_x: f32,
    pub offset_y: f32,
}

/// Atlas packing state
struct AtlasPacker {
    width: u32,
    height: u32,
    row_x: u32,
    row_y: u32,
    row_height: u32,
}

impl AtlasPacker {
    fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            row_x: 1,
            row_y: 1,
            row_height: 0,
        }
    }

    fn allocate(&mut self, glyph_width: u32, glyph_height: u32) -> Option<(u32, u32)> {
        let padded_width = glyph_width + 1;
        let padded_height = glyph_height + 1;

        if self.row_x + padded_width > self.width {
            self.row_x = 1;
            self.row_y += self.row_height;
            self.row_height = 0;
        }

        if self.row_y + padded_height > self.height {
            return None;
        }

        let x = self.row_x;
        let y = self.row_y;

        self.row_x += padded_width;
        self.row_height = self.row_height.max(padded_height);

        Some((x, y))
    }
}

/// Positioned glyph ready for rendering
#[derive(Clone, Copy, Debug)]
pub struct PositionedGlyph {
    /// Screen position (top-left of glyph bitmap)
    pub x: f32,
    pub y: f32,
    /// Glyph bitmap dimensions
    pub width: f32,
    pub height: f32,
    /// UV coordinates in atlas
    pub uv_min: [f32; 2],
    pub uv_max: [f32; 2],
}

/// Glyph cache with GPU texture atlas
pub struct GlyphCache {
    /// Font data
    font_data: Vec<u8>,
    /// Swash scale context
    scale_context: ScaleContext,
    /// Font size in pixels
    font_size: f32,
    /// Cached cell width (monospace advance)
    cached_cell_width: f32,
    /// Cached line height
    cached_line_height: f32,
    /// Baseline position within cell (from top)
    baseline_offset: f32,
    /// Cached glyphs (bitmap data in atlas)
    glyphs: HashMap<GlyphKey, CachedGlyph>,
    /// Atlas texture
    pub atlas_texture: wgpu::Texture,
    pub atlas_view: wgpu::TextureView,
    /// Atlas dimensions
    atlas_width: u32,
    atlas_height: u32,
    /// Atlas packer
    packer: AtlasPacker,
    /// Staging buffer for atlas updates
    staging_data: Vec<u8>,
    /// Pending uploads
    pending_uploads: Vec<(u32, u32, u32, u32, usize)>,
}

impl GlyphCache {
    pub fn new(
        device: &wgpu::Device,
        font_data: &[u8],
        font_size: f32,
    ) -> Result<Self, &'static str> {
        let font_data = font_data.to_vec();
        let font = FontRef::from_index(&font_data, 0).ok_or("Failed to load font")?;

        let atlas_width = 1024;
        let atlas_height = 1024;

        let atlas_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Glyph Atlas"),
            size: wgpu::Extent3d {
                width: atlas_width,
                height: atlas_height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let atlas_view = atlas_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Get font metrics
        let metrics = font.metrics(&[]);
        let units_per_em = metrics.units_per_em as f32;
        let scale = font_size / units_per_em;

        let ascent = metrics.ascent as f32 * scale;
        let descent = metrics.descent as f32 * scale;
        let line_gap = metrics.leading as f32 * scale;

        // Line height includes ascent + |descent| + line gap + extra vertical padding
        // Add vertical breathing room - Nerd Fonts with icons need more space
        let vertical_padding = font_size * 0.7; // 70% extra vertical space for icon glyphs
        let cached_line_height = ascent - descent + line_gap + vertical_padding;

        // Baseline is where text sits - ascent from top of cell (with half the padding above)
        let baseline_offset = ascent + (vertical_padding * 0.5);

        // Get cell width using 'M' as reference
        // For monospace fonts, we need proper horizontal spacing
        let mut scale_context = ScaleContext::new();
        let cached_cell_width = {
            let mut scaler = scale_context
                .builder(font)
                .size(font_size)
                .hint(true)
                .build();

            let glyph_id = font.charmap().map('M');
            if glyph_id != 0 {
                // Render to get dimensions
                if let Some(image) = Render::new(&[
                    Source::ColorOutline(0),
                    Source::ColorBitmap(StrikeWith::BestFit),
                    Source::Outline,
                ])
                .format(Format::Alpha)
                .render(&mut scaler, glyph_id)
                {
                    // For monospace terminals, cell width should match glyph advance
                    // - left bearing (offset from cell origin)
                    // - glyph width (no extra padding to avoid gaps in ligatures)
                    let visual_width = image.placement.width as f32;
                    let left_bearing = image.placement.left as f32;

                    left_bearing + visual_width
                } else {
                    font_size * 0.6
                }
            } else {
                font_size * 0.6
            }
        };

        Ok(Self {
            font_data,
            scale_context,
            font_size,
            cached_cell_width,
            cached_line_height,
            baseline_offset,
            glyphs: HashMap::new(),
            atlas_texture,
            atlas_view,
            atlas_width,
            atlas_height,
            packer: AtlasPacker::new(atlas_width, atlas_height),
            staging_data: Vec::new(),
            pending_uploads: Vec::new(),
        })
    }

    /// Get or create a cached glyph
    pub fn get_or_insert(&mut self, character: char) -> Option<CachedGlyph> {
        let key = GlyphKey::new(character, self.font_size);

        if let Some(&glyph) = self.glyphs.get(&key) {
            return Some(glyph);
        }

        // Get font reference and glyph ID
        // TODO: Investigate caching FontRef to avoid recreating on each glyph insertion.
        // Would require handling self-referential struct (FontRef borrows from font_data).
        // Current approach is likely fine since glyph insertion is infrequent.
        let font = FontRef::from_index(&self.font_data, 0)?;
        let glyph_id = font.charmap().map(character);

        // Return None for unmapped characters
        if glyph_id == 0 {
            return None;
        }

        // Build scaler
        let mut scaler = self
            .scale_context
            .builder(font)
            .size(self.font_size)
            .hint(true)
            .build();

        // Render the glyph
        let image = Render::new(&[
            Source::ColorOutline(0),
            Source::ColorBitmap(StrikeWith::BestFit),
            Source::Outline,
        ])
        .format(Format::Alpha)
        .render(&mut scaler, glyph_id)?;

        // Handle zero-size glyphs (spaces, etc.)
        if image.placement.width == 0 || image.placement.height == 0 {
            let glyph = CachedGlyph {
                uv_min: [0.0, 0.0],
                uv_max: [0.0, 0.0],
                width: 0.0,
                height: 0.0,
                offset_x: 0.0,
                offset_y: 0.0,
            };
            self.glyphs.insert(key, glyph);
            return Some(glyph);
        }

        // Allocate space in atlas
        let (x, y) = self
            .packer
            .allocate(image.placement.width, image.placement.height)?;

        // Store bitmap for upload
        let data_offset = self.staging_data.len();
        self.staging_data.extend_from_slice(&image.data);
        self.pending_uploads.push((
            x,
            y,
            image.placement.width,
            image.placement.height,
            data_offset,
        ));

        let uv_min = [
            x as f32 / self.atlas_width as f32,
            y as f32 / self.atlas_height as f32,
        ];
        let uv_max = [
            (x + image.placement.width) as f32 / self.atlas_width as f32,
            (y + image.placement.height) as f32 / self.atlas_height as f32,
        ];

        // For terminal rendering, we want consistent baseline alignment
        // offset_x: horizontal placement (typically left-align in cell)
        // offset_y: vertical placement from baseline
        let glyph = CachedGlyph {
            uv_min,
            uv_max,
            width: image.placement.width as f32,
            height: image.placement.height as f32,
            offset_x: image.placement.left as f32,
            offset_y: -image.placement.top as f32, // Negative because placement.top is baseline-relative
        };

        self.glyphs.insert(key, glyph);
        Some(glyph)
    }

    /// Position a character at a fixed grid cell
    /// cell_x, cell_y: grid cell coordinates (in pixels, top-left of cell)
    pub fn position_char(&mut self, character: char, cell_x: f32, cell_y: f32) -> Option<PositionedGlyph> {
        let glyph = self.get_or_insert(character)?;

        // Skip zero-size glyphs
        if glyph.width == 0.0 || glyph.height == 0.0 {
            return None;
        }

        // Position within cell with consistent baseline alignment:
        // - x: cell left + horizontal offset (centers glyph in monospace cell)
        // - y: cell top + baseline + vertical offset (aligns to baseline)
        let x = cell_x + glyph.offset_x;
        let y = cell_y + self.baseline_offset + glyph.offset_y;

        Some(PositionedGlyph {
            x,
            y,
            width: glyph.width,
            height: glyph.height,
            uv_min: glyph.uv_min,
            uv_max: glyph.uv_max,
        })
    }

    /// Upload pending glyphs to GPU
    pub fn flush(&mut self, queue: &wgpu::Queue) {
        for (x, y, width, height, data_offset) in self.pending_uploads.drain(..) {
            let data_end = data_offset + (width * height) as usize;
            let data = &self.staging_data[data_offset..data_end];

            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.atlas_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d { x, y, z: 0 },
                    aspect: wgpu::TextureAspect::All,
                },
                data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(width),
                    rows_per_image: Some(height),
                },
                wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
            );
        }
        self.staging_data.clear();
    }

    /// Pre-cache ASCII characters
    pub fn precache_ascii(&mut self) {
        for c in 32u8..=126u8 {
            self.get_or_insert(c as char);
        }
        // Also cache the block cursor character
        self.get_or_insert('\u{2588}');
    }

    pub fn font_size(&self) -> f32 {
        self.font_size
    }

    pub fn line_height(&self) -> f32 {
        self.cached_line_height
    }

    pub fn cell_width(&self) -> f32 {
        self.cached_cell_width
    }
}
