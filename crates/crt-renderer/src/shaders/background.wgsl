// Background shader - renders gradient only
// Grid effects are now handled by the Vello-based effects system
// No texture samples needed, just math - very fast

struct Params {
    screen_size: vec2<f32>,
    time: f32,
    grid_intensity: f32,  // Kept for uniform layout compatibility, ignored
    gradient_top: vec4<f32>,
    gradient_bottom: vec4<f32>,
    grid_color: vec4<f32>,
    grid_spacing: f32,
    grid_line_width: f32,
    grid_perspective: f32,
    grid_horizon: f32,
    glow_color: vec4<f32>,
    glow_radius: f32,
    glow_intensity: f32,
    text_color: vec4<f32>,
    _pad: vec4<f32>,
}

@group(0) @binding(0) var<uniform> params: Params;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    let x = f32(i32(vertex_index & 1u)) * 4.0 - 1.0;
    let y = f32(i32(vertex_index >> 1u)) * 4.0 - 1.0;
    out.position = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);
    return out;
}

fn gradient(uv: vec2<f32>, top: vec3<f32>, bottom: vec3<f32>) -> vec3<f32> {
    return mix(top, bottom, uv.y);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Render gradient only - grid is handled by Vello effects system
    let color = gradient(in.uv, params.gradient_top.rgb, params.gradient_bottom.rgb);
    return vec4<f32>(color, 1.0);
}
