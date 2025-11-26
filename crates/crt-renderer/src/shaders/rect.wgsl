// Rect shader - solid color rectangle rendering using instanced quads

struct Globals {
    screen_size: vec2<f32>,
    _pad: vec2<f32>,
}

struct RectInstance {
    // Screen position (top-left of rect)
    @location(0) pos: vec2<f32>,
    // Rect size in pixels
    @location(1) size: vec2<f32>,
    // RGBA color
    @location(2) color: vec4<f32>,
}

@group(0) @binding(0) var<uniform> globals: Globals;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
}

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    instance: RectInstance,
) -> VertexOutput {
    var out: VertexOutput;

    // Generate quad vertices (0,1,2,3 -> triangle strip)
    let x = f32(vertex_index & 1u);
    let y = f32(vertex_index >> 1u);

    // Calculate pixel position
    let pixel_pos = instance.pos + vec2<f32>(x * instance.size.x, y * instance.size.y);

    // Convert to clip space (-1 to 1)
    let clip_x = (pixel_pos.x / globals.screen_size.x) * 2.0 - 1.0;
    let clip_y = 1.0 - (pixel_pos.y / globals.screen_size.y) * 2.0;

    out.position = vec4<f32>(clip_x, clip_y, 0.0, 1.0);
    out.color = instance.color;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}
