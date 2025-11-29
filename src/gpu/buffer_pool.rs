//! GPU buffer pooling with RAII semantics
//!
//! Pools instance and uniform buffers for reuse across window lifecycles.
//! Buffers are checked out when windows are created and automatically
//! returned to the pool when dropped.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};

/// Buffer size classes for pooling
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BufferClass {
    /// Grid instance buffer: 32K instances * 48 bytes = 1.5 MB
    GridInstance,
    /// Rect instance buffer: 16K instances * 32 bytes = 512 KB
    RectInstance,
    /// Small uniform buffer: 256 bytes (for globals)
    SmallUniform,
}

impl BufferClass {
    /// Get the size in bytes for this buffer class
    pub fn size_bytes(&self) -> u64 {
        match self {
            Self::GridInstance => 32 * 1024 * 48,  // 1.5 MB
            Self::RectInstance => 16 * 1024 * 32,  // 512 KB
            Self::SmallUniform => 256,
        }
    }

    /// Get the buffer usage flags for this class
    pub fn usages(&self) -> wgpu::BufferUsages {
        match self {
            Self::GridInstance | Self::RectInstance => {
                wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST
            }
            Self::SmallUniform => {
                wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST
            }
        }
    }

    /// Get a descriptive label for this class
    pub fn label(&self) -> &'static str {
        match self {
            Self::GridInstance => "Pooled Grid Instance Buffer",
            Self::RectInstance => "Pooled Rect Instance Buffer",
            Self::SmallUniform => "Pooled Small Uniform Buffer",
        }
    }
}

/// Internal pool state
struct BufferPoolInner {
    device: Arc<wgpu::Device>,
    pools: HashMap<BufferClass, Vec<wgpu::Buffer>>,
    max_per_class: usize,
    stats: PoolStats,
}

/// Pool statistics for monitoring
#[derive(Debug, Default, Clone)]
pub struct PoolStats {
    pub allocations: u64,
    pub reuses: u64,
    pub returns: u64,
}

impl BufferPoolInner {
    fn new(device: Arc<wgpu::Device>, max_per_class: usize) -> Self {
        Self {
            device,
            pools: HashMap::new(),
            max_per_class,
            stats: PoolStats::default(),
        }
    }

    fn checkout(&mut self, class: BufferClass) -> wgpu::Buffer {
        if let Some(buffer) = self.pools.get_mut(&class).and_then(|v| v.pop()) {
            self.stats.reuses += 1;
            log::debug!("Reusing pooled {:?} buffer (reuses: {})", class, self.stats.reuses);
            buffer
        } else {
            self.stats.allocations += 1;
            log::debug!("Allocating new {:?} buffer (allocations: {})", class, self.stats.allocations);
            self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(class.label()),
                size: class.size_bytes(),
                usage: class.usages(),
                mapped_at_creation: false,
            })
        }
    }

    fn return_buffer(&mut self, class: BufferClass, buffer: wgpu::Buffer) {
        let pool = self.pools.entry(class).or_insert_with(Vec::new);
        if pool.len() < self.max_per_class {
            self.stats.returns += 1;
            log::debug!("Returning {:?} buffer to pool (size: {})", class, pool.len() + 1);
            pool.push(buffer);
        } else {
            log::debug!("Pool full for {:?}, destroying buffer", class);
            buffer.destroy();
        }
    }

    fn shrink(&mut self) {
        for (class, pool) in &mut self.pools {
            // Keep at most 1 buffer per class when shrinking
            while pool.len() > 1 {
                if let Some(buffer) = pool.pop() {
                    log::debug!("Shrinking pool: destroying {:?} buffer", class);
                    buffer.destroy();
                }
            }
        }
    }
}

/// Buffer pool for reusing GPU buffers across window lifecycles
///
/// # Usage
/// ```ignore
/// let pool = BufferPool::new(device.clone(), 4);
/// let buffer = pool.checkout(BufferClass::GridInstance);
/// // Use buffer...
/// // Buffer is automatically returned to pool when dropped
/// ```
pub struct BufferPool {
    inner: Arc<Mutex<BufferPoolInner>>,
}

impl BufferPool {
    /// Create a new buffer pool
    ///
    /// # Arguments
    /// * `device` - The wgpu device for creating buffers
    /// * `max_per_class` - Maximum buffers to keep pooled per class
    pub fn new(device: Arc<wgpu::Device>, max_per_class: usize) -> Self {
        Self {
            inner: Arc::new(Mutex::new(BufferPoolInner::new(device, max_per_class))),
        }
    }

    /// Check out a buffer from the pool
    ///
    /// Returns a pooled buffer if available, otherwise allocates a new one.
    /// The buffer is automatically returned to the pool when the PooledBuffer is dropped.
    pub fn checkout(&self, class: BufferClass) -> PooledBuffer {
        let buffer = self.inner.lock().unwrap().checkout(class);
        PooledBuffer {
            buffer: Some(buffer),
            class,
            pool: Arc::downgrade(&self.inner),
        }
    }

    /// Get current pool statistics
    pub fn stats(&self) -> PoolStats {
        self.inner.lock().unwrap().stats.clone()
    }

    /// Shrink the pool by releasing excess buffers
    ///
    /// Call this after closing windows to free up GPU memory.
    pub fn shrink(&self) {
        self.inner.lock().unwrap().shrink();
    }
}

/// A buffer checked out from the pool with RAII semantics
///
/// The buffer is automatically returned to the pool when dropped.
pub struct PooledBuffer {
    buffer: Option<wgpu::Buffer>,
    class: BufferClass,
    pool: Weak<Mutex<BufferPoolInner>>,
}

impl PooledBuffer {
    /// Get a reference to the underlying buffer
    pub fn buffer(&self) -> &wgpu::Buffer {
        self.buffer.as_ref().expect("PooledBuffer already returned")
    }

    /// Get the buffer class
    pub fn class(&self) -> BufferClass {
        self.class
    }
}

impl Drop for PooledBuffer {
    fn drop(&mut self) {
        if let Some(buffer) = self.buffer.take() {
            if let Some(pool) = self.pool.upgrade() {
                if let Ok(mut inner) = pool.lock() {
                    inner.return_buffer(self.class, buffer);
                    return;
                }
            }
            // Pool is gone, destroy the buffer
            buffer.destroy();
        }
    }
}

impl std::ops::Deref for PooledBuffer {
    type Target = wgpu::Buffer;

    fn deref(&self) -> &Self::Target {
        self.buffer()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_class_sizes() {
        assert_eq!(BufferClass::GridInstance.size_bytes(), 32 * 1024 * 48);
        assert_eq!(BufferClass::RectInstance.size_bytes(), 16 * 1024 * 32);
        assert_eq!(BufferClass::SmallUniform.size_bytes(), 256);
    }
}
