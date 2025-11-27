// Background image shader - renders textured background with sizing/positioning
// Supports Cover, Contain, and other CSS-like sizing modes

struct Params {
    // UV transform: (scale_x, scale_y, offset_x, offset_y)
    uv_transform: vec4<f32>,
    // Opacity (0-1)
    opacity: f32,
    // Padding for alignment (use individual floats to avoid vec3 alignment issues)
    _pad1: f32,
    _pad2: f32,
    _pad3: f32,
}

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var image_texture: texture_2d<f32>;
@group(0) @binding(2) var image_sampler: sampler;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    // Full-screen quad using triangle strip (4 vertices)
    // Same pattern as working background.wgsl
    let x = f32(i32(vertex_index & 1u)) * 4.0 - 1.0;
    let y = f32(i32(vertex_index >> 1u)) * 4.0 - 1.0;
    out.position = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Apply UV transform for sizing and positioning
    let scale = params.uv_transform.xy;
    let offset = params.uv_transform.zw;

    // Transform UV: scale then offset
    let tex_uv = in.uv * scale + offset;

    // Sample the texture
    let color = textureSample(image_texture, image_sampler, tex_uv);

    // Apply opacity
    return vec4<f32>(color.rgb, color.a * params.opacity);
}
