//! Tab Bar Rendering
//!
//! GPU-accelerated tab bar with theme support.

use crt_theme::{TabTheme, Color};
use wgpu::util::DeviceExt;
use bytemuck::{Pod, Zeroable};

/// Tab bar position on the screen
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TabPosition {
    #[default]
    Top,
    Bottom,
    Left,
    Right,
}

/// A single tab in the tab bar
#[derive(Debug, Clone)]
pub struct Tab {
    pub id: u64,
    pub title: String,
    pub is_active: bool,
    /// Whether this tab has a user-set custom title (prevents OSC overwrite)
    pub has_custom_title: bool,
}

impl Tab {
    pub fn new(id: u64, title: impl Into<String>) -> Self {
        Self {
            id,
            title: title.into(),
            is_active: false,
            has_custom_title: false,
        }
    }
}

/// State for inline tab title editing
#[derive(Debug, Clone, Default)]
pub struct EditState {
    /// Tab ID being edited (None if not editing)
    pub tab_id: Option<u64>,
    /// Current edit text
    pub text: String,
    /// Cursor position in the text
    pub cursor: usize,
}

/// Tab bar state and rendering
pub struct TabBar {
    tabs: Vec<Tab>,
    active_tab: usize,
    next_id: u64,
    theme: TabTheme,
    position: TabPosition,

    // Rendering state
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,

    // Computed layout
    tab_rects: Vec<TabRect>,
    bar_height: f32,
    bar_width: f32, // For left/right positioning
    screen_width: f32,
    screen_height: f32,
    scale_factor: f32,
    dirty: bool,
    vertex_count: usize,

    // Inline editing state
    edit_state: EditState,
}

/// Rectangle for a tab (for hit testing and rendering)
#[derive(Debug, Clone, Copy, Default)]
pub struct TabRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub close_x: f32,
    pub close_width: f32,
}

impl TabRect {
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.x + self.width &&
        y >= self.y && y < self.y + self.height
    }

    pub fn close_contains(&self, x: f32, y: f32) -> bool {
        x >= self.close_x && x < self.close_x + self.close_width &&
        y >= self.y && y < self.y + self.height
    }
}

/// Vertex for tab bar quads
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct TabVertex {
    position: [f32; 2],
    color: [f32; 4],
}

/// Uniforms for tab bar rendering
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct TabUniforms {
    screen_size: [f32; 2],
    _pad: [f32; 2],
}

const TAB_BAR_SHADER: &str = r#"
struct Uniforms {
    screen_size: vec2<f32>,
    _pad: vec2<f32>,
}

@group(0) @binding(0) var<uniform> uniforms: Uniforms;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    // Convert pixel coords to NDC (-1 to 1)
    let ndc_x = (in.position.x / uniforms.screen_size.x) * 2.0 - 1.0;
    let ndc_y = 1.0 - (in.position.y / uniforms.screen_size.y) * 2.0;
    out.position = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
    out.color = in.color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}
"#;

// Maximum vertices for tab bar (enough for many tabs)
const MAX_VERTICES: usize = 1024;

impl TabBar {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Tab Bar Shader"),
            source: wgpu::ShaderSource::Wgsl(TAB_BAR_SHADER.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Tab Bar Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Tab Bar Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Tab Bar Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<TabVertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: 8,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                    ],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Tab Bar Vertex Buffer"),
            size: (MAX_VERTICES * std::mem::size_of::<TabVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Tab Bar Uniform Buffer"),
            contents: bytemuck::cast_slice(&[TabUniforms {
                screen_size: [800.0, 600.0],
                _pad: [0.0; 2],
            }]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Tab Bar Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
            ],
        });

        // Start with one default tab
        let initial_tab = Tab::new(0, "Terminal");

        Self {
            tabs: vec![initial_tab],
            active_tab: 0,
            next_id: 1,
            theme: TabTheme::default(),
            position: TabPosition::Top,
            pipeline,
            vertex_buffer,
            uniform_buffer,
            bind_group,
            tab_rects: Vec::new(),
            bar_height: 36.0,
            bar_width: 180.0, // Default width for left/right positioning
            screen_width: 800.0,
            screen_height: 600.0,
            scale_factor: 1.0,
            dirty: true,
            vertex_count: 0,
            edit_state: EditState::default(),
        }
    }

    /// Set the tab theme
    pub fn set_theme(&mut self, theme: TabTheme) {
        self.theme = theme;
        self.bar_height = theme.bar.height;
        self.dirty = true;
    }

    /// Set scale factor for HiDPI displays
    pub fn set_scale_factor(&mut self, scale_factor: f32) {
        self.scale_factor = scale_factor;
        self.dirty = true;
    }

    /// Set tab bar position
    pub fn set_position(&mut self, position: TabPosition) {
        self.position = position;
        self.dirty = true;
    }

    /// Get current tab bar position
    pub fn position(&self) -> TabPosition {
        self.position
    }

    /// Check if tab bar is horizontal (top/bottom)
    pub fn is_horizontal(&self) -> bool {
        matches!(self.position, TabPosition::Top | TabPosition::Bottom)
    }

    /// Get current tab bar height (in logical pixels) - for top/bottom positioning
    pub fn height(&self) -> f32 {
        self.bar_height
    }

    /// Get current tab bar width (in logical pixels) - for left/right positioning
    pub fn width(&self) -> f32 {
        self.bar_width
    }

    /// Get the dimension that affects content layout (height for top/bottom, width for left/right)
    pub fn size(&self) -> f32 {
        if self.is_horizontal() {
            self.bar_height
        } else {
            self.bar_width
        }
    }

    /// Get the content offset (x, y) in logical pixels based on tab bar position
    /// This tells where the terminal content should start rendering
    pub fn content_offset(&self) -> (f32, f32) {
        match self.position {
            TabPosition::Top => (0.0, self.bar_height),
            TabPosition::Bottom => (0.0, 0.0),
            TabPosition::Left => (self.bar_width, 0.0),
            TabPosition::Right => (0.0, 0.0),
        }
    }

    /// Update screen size (in physical pixels)
    pub fn resize(&mut self, width: f32, height: f32) {
        self.screen_width = width;
        self.screen_height = height;
        self.dirty = true;
    }

    /// Add a new tab
    pub fn add_tab(&mut self, title: impl Into<String>) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.tabs.push(Tab::new(id, title));
        self.dirty = true;
        id
    }

    /// Close a tab by ID
    pub fn close_tab(&mut self, id: u64) -> bool {
        if self.tabs.len() <= 1 {
            return false; // Don't close last tab
        }

        if let Some(idx) = self.tabs.iter().position(|t| t.id == id) {
            self.tabs.remove(idx);
            if self.active_tab >= self.tabs.len() {
                self.active_tab = self.tabs.len() - 1;
            }
            self.dirty = true;
            return true;
        }
        false
    }

    /// Select a tab by ID
    pub fn select_tab(&mut self, id: u64) -> bool {
        if let Some(idx) = self.tabs.iter().position(|t| t.id == id) {
            self.active_tab = idx;
            self.dirty = true;
            return true;
        }
        false
    }

    /// Select tab by index (0-based)
    pub fn select_tab_index(&mut self, index: usize) -> bool {
        if index < self.tabs.len() {
            self.active_tab = index;
            self.dirty = true;
            return true;
        }
        false
    }

    /// Select next tab
    pub fn next_tab(&mut self) {
        if !self.tabs.is_empty() {
            self.active_tab = (self.active_tab + 1) % self.tabs.len();
            self.dirty = true;
        }
    }

    /// Select previous tab
    pub fn prev_tab(&mut self) {
        if !self.tabs.is_empty() {
            self.active_tab = if self.active_tab == 0 {
                self.tabs.len() - 1
            } else {
                self.active_tab - 1
            };
            self.dirty = true;
        }
    }

    /// Get active tab ID
    pub fn active_tab_id(&self) -> Option<u64> {
        self.tabs.get(self.active_tab).map(|t| t.id)
    }

    /// Get number of tabs
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    /// Hit test - returns (tab_id, is_close_button) if hit
    pub fn hit_test(&self, x: f32, y: f32) -> Option<(u64, bool)> {
        for (i, rect) in self.tab_rects.iter().enumerate() {
            if rect.contains(x, y) {
                let is_close = rect.close_contains(x, y);
                return Some((self.tabs[i].id, is_close));
            }
        }
        None
    }

    /// Build vertices for the tab bar
    fn build_vertices(&mut self) -> Vec<TabVertex> {
        let mut vertices = Vec::new();
        self.tab_rects.clear();

        let bar_bg = color_to_array(&self.theme.bar.background);
        let tab_bg = color_to_array(&self.theme.tab.background);
        let active_bg = color_to_array(&self.theme.active.background);
        let border_color = color_to_array(&self.theme.bar.border_color);

        // Scale all logical dimensions to physical pixels
        let s = self.scale_factor;
        let padding = self.theme.bar.padding * s;
        let tab_padding_x = self.theme.tab.padding_x * s;
        let bar_height = self.bar_height * s;
        let bar_width = self.bar_width * s;
        let tab_gap = 4.0 * s;
        let border_width = s; // 1 logical pixel border

        match self.position {
            TabPosition::Top => {
                let tab_height = bar_height - padding * 2.0;

                // Tab bar background
                add_quad(&mut vertices, 0.0, 0.0, self.screen_width, bar_height, bar_bg);
                // Bottom border
                add_quad(&mut vertices, 0.0, bar_height - s, self.screen_width, s, border_color);

                // Calculate tab widths
                let available_width = self.screen_width - padding * 2.0;
                let tab_count = self.tabs.len();
                let total_gap = tab_gap * (tab_count.saturating_sub(1)) as f32;
                let min_width = self.theme.tab.min_width * s;
                let max_width = self.theme.tab.max_width * s;
                let width_per_tab = ((available_width - total_gap) / tab_count as f32)
                    .clamp(min_width, max_width);

                let mut x = padding;
                for (i, _tab) in self.tabs.iter().enumerate() {
                    let is_active = i == self.active_tab;
                    let bg_color = if is_active { active_bg } else { tab_bg };

                    let tab_x = x;
                    let tab_y = padding;
                    let tab_width = width_per_tab;

                    add_quad(&mut vertices, tab_x, tab_y, tab_width, tab_height, bg_color);
                    add_quad(&mut vertices, tab_x, tab_y, tab_width, border_width, border_color);
                    add_quad(&mut vertices, tab_x, tab_y + tab_height - border_width, tab_width, border_width, border_color);
                    add_quad(&mut vertices, tab_x, tab_y, border_width, tab_height, border_color);
                    add_quad(&mut vertices, tab_x + tab_width - border_width, tab_y, border_width, tab_height, border_color);

                    if is_active {
                        let accent = color_to_array(&self.theme.active.accent);
                        add_quad(&mut vertices, tab_x, tab_y + tab_height - 2.0 * s, tab_width, 2.0 * s, accent);
                    }

                    let close_width = self.theme.close.size * s;
                    self.tab_rects.push(TabRect {
                        x: tab_x, y: tab_y, width: tab_width, height: tab_height,
                        close_x: tab_x + tab_width - close_width - tab_padding_x, close_width,
                    });

                    x += tab_width + tab_gap;
                }
            }

            TabPosition::Bottom => {
                let tab_height = bar_height - padding * 2.0;
                let bar_y = self.screen_height - bar_height;

                // Tab bar background
                add_quad(&mut vertices, 0.0, bar_y, self.screen_width, bar_height, bar_bg);
                // Top border
                add_quad(&mut vertices, 0.0, bar_y, self.screen_width, s, border_color);

                // Calculate tab widths
                let available_width = self.screen_width - padding * 2.0;
                let tab_count = self.tabs.len();
                let total_gap = tab_gap * (tab_count.saturating_sub(1)) as f32;
                let min_width = self.theme.tab.min_width * s;
                let max_width = self.theme.tab.max_width * s;
                let width_per_tab = ((available_width - total_gap) / tab_count as f32)
                    .clamp(min_width, max_width);

                let mut x = padding;
                for (i, _tab) in self.tabs.iter().enumerate() {
                    let is_active = i == self.active_tab;
                    let bg_color = if is_active { active_bg } else { tab_bg };

                    let tab_x = x;
                    let tab_y = bar_y + padding;
                    let tab_width = width_per_tab;

                    add_quad(&mut vertices, tab_x, tab_y, tab_width, tab_height, bg_color);
                    add_quad(&mut vertices, tab_x, tab_y, tab_width, border_width, border_color);
                    add_quad(&mut vertices, tab_x, tab_y + tab_height - border_width, tab_width, border_width, border_color);
                    add_quad(&mut vertices, tab_x, tab_y, border_width, tab_height, border_color);
                    add_quad(&mut vertices, tab_x + tab_width - border_width, tab_y, border_width, tab_height, border_color);

                    if is_active {
                        let accent = color_to_array(&self.theme.active.accent);
                        add_quad(&mut vertices, tab_x, tab_y, tab_width, 2.0 * s, accent);
                    }

                    let close_width = self.theme.close.size * s;
                    self.tab_rects.push(TabRect {
                        x: tab_x, y: tab_y, width: tab_width, height: tab_height,
                        close_x: tab_x + tab_width - close_width - tab_padding_x, close_width,
                    });

                    x += tab_width + tab_gap;
                }
            }

            TabPosition::Left => {
                let tab_width = bar_width - padding * 2.0;

                // Tab bar background
                add_quad(&mut vertices, 0.0, 0.0, bar_width, self.screen_height, bar_bg);
                // Right border
                add_quad(&mut vertices, bar_width - s, 0.0, s, self.screen_height, border_color);

                // Calculate tab heights for vertical tabs
                let available_height = self.screen_height - padding * 2.0;
                let tab_count = self.tabs.len();
                let total_gap = tab_gap * (tab_count.saturating_sub(1)) as f32;
                let min_height = 24.0 * s;
                let max_height = 32.0 * s;
                let height_per_tab = ((available_height - total_gap) / tab_count as f32)
                    .clamp(min_height, max_height);

                let mut y = padding;
                for (i, _tab) in self.tabs.iter().enumerate() {
                    let is_active = i == self.active_tab;
                    let bg_color = if is_active { active_bg } else { tab_bg };

                    let tab_x = padding;
                    let tab_y = y;
                    let tab_height = height_per_tab;

                    add_quad(&mut vertices, tab_x, tab_y, tab_width, tab_height, bg_color);
                    add_quad(&mut vertices, tab_x, tab_y, tab_width, border_width, border_color);
                    add_quad(&mut vertices, tab_x, tab_y + tab_height - border_width, tab_width, border_width, border_color);
                    add_quad(&mut vertices, tab_x, tab_y, border_width, tab_height, border_color);
                    add_quad(&mut vertices, tab_x + tab_width - border_width, tab_y, border_width, tab_height, border_color);

                    if is_active {
                        let accent = color_to_array(&self.theme.active.accent);
                        add_quad(&mut vertices, tab_x + tab_width - 2.0 * s, tab_y, 2.0 * s, tab_height, accent);
                    }

                    let close_width = self.theme.close.size * s;
                    self.tab_rects.push(TabRect {
                        x: tab_x, y: tab_y, width: tab_width, height: tab_height,
                        close_x: tab_x + tab_width - close_width - tab_padding_x, close_width,
                    });

                    y += tab_height + tab_gap;
                }
            }

            TabPosition::Right => {
                let tab_width = bar_width - padding * 2.0;
                let bar_x = self.screen_width - bar_width;

                // Tab bar background
                add_quad(&mut vertices, bar_x, 0.0, bar_width, self.screen_height, bar_bg);
                // Left border
                add_quad(&mut vertices, bar_x, 0.0, s, self.screen_height, border_color);

                // Calculate tab heights for vertical tabs
                let available_height = self.screen_height - padding * 2.0;
                let tab_count = self.tabs.len();
                let total_gap = tab_gap * (tab_count.saturating_sub(1)) as f32;
                let min_height = 24.0 * s;
                let max_height = 32.0 * s;
                let height_per_tab = ((available_height - total_gap) / tab_count as f32)
                    .clamp(min_height, max_height);

                let mut y = padding;
                for (i, _tab) in self.tabs.iter().enumerate() {
                    let is_active = i == self.active_tab;
                    let bg_color = if is_active { active_bg } else { tab_bg };

                    let tab_x = bar_x + padding;
                    let tab_y = y;
                    let tab_height = height_per_tab;

                    add_quad(&mut vertices, tab_x, tab_y, tab_width, tab_height, bg_color);
                    add_quad(&mut vertices, tab_x, tab_y, tab_width, border_width, border_color);
                    add_quad(&mut vertices, tab_x, tab_y + tab_height - border_width, tab_width, border_width, border_color);
                    add_quad(&mut vertices, tab_x, tab_y, border_width, tab_height, border_color);
                    add_quad(&mut vertices, tab_x + tab_width - border_width, tab_y, border_width, tab_height, border_color);

                    if is_active {
                        let accent = color_to_array(&self.theme.active.accent);
                        add_quad(&mut vertices, tab_x, tab_y, 2.0 * s, tab_height, accent);
                    }

                    let close_width = self.theme.close.size * s;
                    self.tab_rects.push(TabRect {
                        x: tab_x, y: tab_y, width: tab_width, height: tab_height,
                        close_x: tab_x + tab_width - close_width - tab_padding_x, close_width,
                    });

                    y += tab_height + tab_gap;
                }
            }
        }

        vertices
    }

    /// Update uniforms and vertex buffer
    pub fn prepare(&mut self, queue: &wgpu::Queue) {
        // Update uniforms
        let uniforms = TabUniforms {
            screen_size: [self.screen_width, self.screen_height],
            _pad: [0.0; 2],
        };
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));

        // Rebuild vertices if dirty
        if self.dirty {
            let vertices = self.build_vertices();
            self.vertex_count = vertices.len();
            if !vertices.is_empty() {
                queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&vertices));
            }
            self.dirty = false;
        }
    }

    /// Render the tab bar
    pub fn render<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
        if self.vertex_count == 0 {
            return;
        }

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        pass.draw(0..self.vertex_count as u32, 0..1);
    }

    /// Get tab titles for text rendering (returns position and title in physical pixels)
    /// When a tab is being edited, returns the edit text with cursor indicator
    pub fn get_tab_labels(&self) -> Vec<(f32, f32, String, bool, bool)> {
        let s = self.scale_factor;
        let tab_padding_x = self.theme.tab.padding_x * s;
        let font_height = 14.0 * s;

        self.tab_rects.iter().zip(self.tabs.iter()).enumerate().map(|(i, (rect, tab))| {
            let text_x = rect.x + tab_padding_x;
            // Center text vertically in tab rectangle (works for all positions)
            let text_y = rect.y + (rect.height - font_height) / 2.0;
            let is_active = i == self.active_tab;

            // Check if this tab is being edited
            let (display_text, is_editing) = if self.edit_state.tab_id == Some(tab.id) {
                // Show edit text with cursor
                let mut text = self.edit_state.text.clone();
                // Insert a pipe character as cursor indicator at cursor position
                text.insert(self.edit_state.cursor, '|');
                (text, true)
            } else {
                (tab.title.clone(), false)
            };

            (text_x, text_y, display_text, is_active, is_editing)
        }).collect()
    }

    /// Update a tab's title by ID (from OSC escape sequences)
    /// Cleans the title by stripping control characters and truncating
    /// Will NOT update if the tab has a user-set custom title
    pub fn set_tab_title(&mut self, id: u64, title: impl Into<String>) -> bool {
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) {
            // Don't overwrite user-set custom titles with OSC updates
            if tab.has_custom_title {
                return false;
            }

            let raw_title = title.into();
            // Clean the title: strip control characters
            let cleaned: String = raw_title
                .chars()
                .filter(|c| !c.is_control() && *c != '\x1b')
                .collect();
            let cleaned = cleaned.trim();

            // Only update if we have meaningful content
            if !cleaned.is_empty() {
                // Truncate to 20 chars with ellipsis
                let final_title = if cleaned.chars().count() > 20 {
                    format!("{}...", cleaned.chars().take(17).collect::<String>())
                } else {
                    cleaned.to_string()
                };
                tab.title = final_title;
                self.dirty = true;
                return true;
            }
        }
        false
    }

    /// Set a custom title for a tab (user-initiated)
    /// This prevents OSC escape sequences from overwriting the title
    pub fn set_custom_tab_title(&mut self, id: u64, title: impl Into<String>) -> bool {
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) {
            let raw_title = title.into();
            let trimmed = raw_title.trim();

            if !trimmed.is_empty() {
                // Truncate to 20 chars with ellipsis
                let final_title = if trimmed.chars().count() > 20 {
                    format!("{}...", trimmed.chars().take(17).collect::<String>())
                } else {
                    trimmed.to_string()
                };
                tab.title = final_title;
                tab.has_custom_title = true;
                self.dirty = true;
                return true;
            }
        }
        false
    }

    /// Clear custom title flag (allows OSC to update title again)
    pub fn clear_custom_title(&mut self, id: u64) {
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) {
            tab.has_custom_title = false;
        }
    }

    /// Check if a tab has a custom title
    pub fn has_custom_title(&self, id: u64) -> bool {
        self.tabs.iter().find(|t| t.id == id).map(|t| t.has_custom_title).unwrap_or(false)
    }

    // ---- Inline Editing Methods ----

    /// Check if currently editing a tab
    pub fn is_editing(&self) -> bool {
        self.edit_state.tab_id.is_some()
    }

    /// Get the tab ID being edited (if any)
    pub fn editing_tab_id(&self) -> Option<u64> {
        self.edit_state.tab_id
    }

    /// Start editing a tab's title
    pub fn start_editing(&mut self, id: u64) -> bool {
        if let Some(tab) = self.tabs.iter().find(|t| t.id == id) {
            self.edit_state = EditState {
                tab_id: Some(id),
                text: tab.title.clone(),
                cursor: tab.title.len(),
            };
            self.dirty = true;
            return true;
        }
        false
    }

    /// Cancel editing without saving
    pub fn cancel_editing(&mut self) {
        self.edit_state = EditState::default();
        self.dirty = true;
    }

    /// Confirm editing and save the new title
    pub fn confirm_editing(&mut self) -> bool {
        if let Some(id) = self.edit_state.tab_id {
            let text = self.edit_state.text.clone();
            self.edit_state = EditState::default();
            self.dirty = true;
            return self.set_custom_tab_title(id, text);
        }
        false
    }

    /// Handle a character input during editing
    pub fn edit_insert_char(&mut self, c: char) {
        if self.edit_state.tab_id.is_some() {
            // Limit to reasonable length
            if self.edit_state.text.len() < 50 {
                self.edit_state.text.insert(self.edit_state.cursor, c);
                self.edit_state.cursor += 1;
                self.dirty = true;
            }
        }
    }

    /// Handle backspace during editing
    pub fn edit_backspace(&mut self) {
        if self.edit_state.tab_id.is_some() && self.edit_state.cursor > 0 {
            self.edit_state.cursor -= 1;
            self.edit_state.text.remove(self.edit_state.cursor);
            self.dirty = true;
        }
    }

    /// Handle delete during editing
    pub fn edit_delete(&mut self) {
        if self.edit_state.tab_id.is_some() && self.edit_state.cursor < self.edit_state.text.len() {
            self.edit_state.text.remove(self.edit_state.cursor);
            self.dirty = true;
        }
    }

    /// Move cursor left during editing
    pub fn edit_cursor_left(&mut self) {
        if self.edit_state.tab_id.is_some() && self.edit_state.cursor > 0 {
            self.edit_state.cursor -= 1;
            self.dirty = true;
        }
    }

    /// Move cursor right during editing
    pub fn edit_cursor_right(&mut self) {
        if self.edit_state.tab_id.is_some() && self.edit_state.cursor < self.edit_state.text.len() {
            self.edit_state.cursor += 1;
            self.dirty = true;
        }
    }

    /// Move cursor to start during editing
    pub fn edit_cursor_home(&mut self) {
        if self.edit_state.tab_id.is_some() {
            self.edit_state.cursor = 0;
            self.dirty = true;
        }
    }

    /// Move cursor to end during editing
    pub fn edit_cursor_end(&mut self) {
        if self.edit_state.tab_id.is_some() {
            self.edit_state.cursor = self.edit_state.text.len();
            self.dirty = true;
        }
    }

    /// Get a tab's title by ID
    pub fn get_tab_title(&self, id: u64) -> Option<&str> {
        self.tabs.iter().find(|t| t.id == id).map(|t| t.title.as_str())
    }

    /// Get the foreground color for inactive tabs (from theme)
    pub fn inactive_tab_color(&self) -> [f32; 4] {
        color_to_array(&self.theme.tab.foreground)
    }

    /// Get the foreground color for active tabs (from theme)
    pub fn active_tab_color(&self) -> [f32; 4] {
        color_to_array(&self.theme.active.foreground)
    }

    /// Get text shadow for inactive tabs (if any)
    pub fn inactive_tab_text_shadow(&self) -> Option<(f32, [f32; 4])> {
        self.theme.tab.text_shadow.map(|s| {
            (s.radius, color_to_array(&s.color))
        })
    }

    /// Get text shadow for active tabs (if any)
    pub fn active_tab_text_shadow(&self) -> Option<(f32, [f32; 4])> {
        self.theme.active.text_shadow.map(|s| {
            (s.radius, color_to_array(&s.color))
        })
    }
}

fn color_to_array(color: &Color) -> [f32; 4] {
    [color.r, color.g, color.b, color.a]
}

fn add_quad(vertices: &mut Vec<TabVertex>, x: f32, y: f32, width: f32, height: f32, color: [f32; 4]) {
    // Two triangles for a quad
    // Triangle 1: top-left, top-right, bottom-left
    vertices.push(TabVertex { position: [x, y], color });
    vertices.push(TabVertex { position: [x + width, y], color });
    vertices.push(TabVertex { position: [x, y + height], color });

    // Triangle 2: top-right, bottom-right, bottom-left
    vertices.push(TabVertex { position: [x + width, y], color });
    vertices.push(TabVertex { position: [x + width, y + height], color });
    vertices.push(TabVertex { position: [x, y + height], color });
}
