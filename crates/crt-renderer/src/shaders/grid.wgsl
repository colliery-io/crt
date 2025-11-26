// Grid shader - GPU-accelerated text glyph rendering using instanced quads

struct Globals {
    screen_size: vec2<f32>,
    atlas_size: vec2<f32>,
}

struct GlyphInstance {
    // Screen position (top-left of glyph)
    @location(0) pos: vec2<f32>,
    // UV min (atlas coordinates)
    @location(1) uv_min: vec2<f32>,
    // UV max (atlas coordinates)
    @location(2) uv_max: vec2<f32>,
    // Glyph size in pixels
    @location(3) size: vec2<f32>,
    // RGBA color
    @location(4) color: vec4<f32>,
}

@group(0) @binding(0) var<uniform> globals: Globals;
@group(0) @binding(1) var atlas_texture: texture_2d<f32>;
@group(0) @binding(2) var atlas_sampler: sampler;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
}

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    instance: GlyphInstance,
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

    // Interpolate UV coordinates
    out.uv = mix(instance.uv_min, instance.uv_max, vec2<f32>(x, y));
    out.color = instance.color;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample alpha from R8 atlas texture
    let alpha = textureSample(atlas_texture, atlas_sampler, in.uv).r;

    // Discard fully transparent pixels
    if alpha < 0.01 {
        discard;
    }

    return vec4<f32>(in.color.rgb, in.color.a * alpha);
}
