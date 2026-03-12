//! Rect renderer for solid color rectangles using instanced quads
//!
//! Renders colored rectangles for cell backgrounds.
//! All rectangles render in a single draw call.

use std::sync::Arc;

use crate::shared_pipelines::SharedRectPipeline;
use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

/// Per-instance data for a rectangle
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct RectInstance {
    /// Screen position (top-left of rect)
    pub pos: [f32; 2],
    /// Rect size in pixels
    pub size: [f32; 2],
    /// RGBA color
    pub color: [f32; 4],
}

/// Global uniforms for the rect shader
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct Globals {
    screen_size: [f32; 2],
    _pad: [f32; 2],
}

/// Rect renderer using instanced quads
///
/// The renderer does not own its instance buffer - this allows for buffer pooling
/// across window lifecycles. Use `create_instance_buffer()` to create a buffer,
/// or provide one from a buffer pool.
///
/// Pipeline objects can be shared across windows via `new_with_shared()` to
/// avoid duplicating Metal shader caches.
pub struct RectRenderer {
    /// Shared pipeline objects (pipeline, bind group layout).
    shared: Arc<SharedRectPipeline>,
    globals_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    instance_capacity: usize,
    /// Pending instances to render
    instances: Vec<RectInstance>,
    /// Cached screen size to avoid redundant uniform updates
    cached_screen_size: (f32, f32),
}

impl RectRenderer {
    /// Maximum number of rect instances per render call
    pub const MAX_INSTANCES: usize = 16 * 1024;

    /// Size of instance buffer in bytes (16K instances * 32 bytes = 512 KB)
    pub const INSTANCE_BUFFER_SIZE: u64 =
        (Self::MAX_INSTANCES * std::mem::size_of::<RectInstance>()) as u64;

    /// Create an instance buffer for use with this renderer
    ///
    /// Call this to create a buffer if not using a buffer pool.
    /// The buffer can be reused across renderer instances.
    pub fn create_instance_buffer(device: &wgpu::Device) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Rect Instance Buffer"),
            size: Self::INSTANCE_BUFFER_SIZE,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    /// Create a rect renderer using shared pipeline objects.
    pub fn new_with_shared(device: &wgpu::Device, shared: &Arc<SharedRectPipeline>) -> Self {
        let (globals_buffer, bind_group) = Self::create_per_window_resources(device, &shared.bind_group_layout);

        Self {
            shared: shared.clone(),
            globals_buffer,
            bind_group,
            instance_capacity: Self::MAX_INSTANCES,
            instances: Vec::with_capacity(Self::MAX_INSTANCES),
            cached_screen_size: (0.0, 0.0),
        }
    }

    /// Create a rect renderer with its own pipeline objects.
    pub fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let shared = Arc::new(SharedRectPipeline::new(device, target_format));
        let (globals_buffer, bind_group) = Self::create_per_window_resources(device, &shared.bind_group_layout);

        Self {
            shared,
            globals_buffer,
            bind_group,
            instance_capacity: Self::MAX_INSTANCES,
            instances: Vec::with_capacity(Self::MAX_INSTANCES),
            cached_screen_size: (0.0, 0.0),
        }
    }

    fn create_per_window_resources(
        device: &wgpu::Device,
        bind_group_layout: &wgpu::BindGroupLayout,
    ) -> (wgpu::Buffer, wgpu::BindGroup) {
        let globals = Globals {
            screen_size: [1.0, 1.0],
            _pad: [0.0, 0.0],
        };

        let globals_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Rect Globals Buffer"),
            contents: bytemuck::cast_slice(&[globals]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Rect Bind Group"),
            layout: bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: globals_buffer.as_entire_binding(),
            }],
        });

        (globals_buffer, bind_group)
    }

    /// Clear pending instances
    pub fn clear(&mut self) {
        self.instances.clear();
    }

    /// Add a rectangle
    pub fn push_rect(&mut self, x: f32, y: f32, width: f32, height: f32, color: [f32; 4]) {
        if self.instances.len() < self.instance_capacity {
            self.instances.push(RectInstance {
                pos: [x, y],
                size: [width, height],
                color,
            });
        }
    }

    /// Get the number of pending instances
    pub fn instance_count(&self) -> usize {
        self.instances.len()
    }

    /// Update screen size uniform (only writes if size changed)
    pub fn update_screen_size(&mut self, queue: &wgpu::Queue, width: f32, height: f32) {
        // Skip if size hasn't changed
        if self.cached_screen_size == (width, height) {
            return;
        }
        self.cached_screen_size = (width, height);

        let globals = Globals {
            screen_size: [width, height],
            _pad: [0.0, 0.0],
        };
        queue.write_buffer(&self.globals_buffer, 0, bytemuck::cast_slice(&[globals]));
    }

    /// Upload instances and render
    ///
    /// The instance buffer must be created with `create_instance_buffer()` or
    /// be at least `INSTANCE_BUFFER_SIZE` bytes with VERTEX | COPY_DST usage.
    pub fn render<'a>(
        &'a self,
        queue: &wgpu::Queue,
        render_pass: &mut wgpu::RenderPass<'a>,
        instance_buffer: &'a wgpu::Buffer,
    ) {
        if self.instances.is_empty() {
            return;
        }

        // Upload instance data
        queue.write_buffer(instance_buffer, 0, bytemuck::cast_slice(&self.instances));

        render_pass.set_pipeline(&self.shared.pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.set_vertex_buffer(0, instance_buffer.slice(..));

        // Draw 4 vertices per instance (triangle strip quad)
        render_pass.draw(0..4, 0..self.instances.len() as u32);
    }
}

impl Drop for RectRenderer {
    fn drop(&mut self) {
        // Destroy globals buffer to release GPU memory immediately
        // Note: instance buffer is external (for pooling) and not owned by renderer
        self.globals_buffer.destroy();
    }
}
