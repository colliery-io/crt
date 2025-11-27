// Sprite rendering shader - samples from sprite sheet with frame offset
// Renders a single sprite frame with position, scale, and opacity

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

struct SpriteUniforms {
    // Transform: position (xy) and scale (zw) in normalized device coords
    transform: vec4<f32>,
    // Frame UV offset (xy) and frame UV size (zw)
    frame_uv: vec4<f32>,
    // Opacity (x), unused (yzw)
    params: vec4<f32>,
};

@group(0) @binding(0) var sprite_texture: texture_2d<f32>;
@group(0) @binding(1) var sprite_sampler: sampler;
@group(0) @binding(2) var<uniform> uniforms: SpriteUniforms;

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    // Generate a quad from vertex index (0-3)
    // 0: bottom-left, 1: bottom-right, 2: top-left, 3: top-right
    let x = f32(vertex_index & 1u);
    let y = f32((vertex_index >> 1u) & 1u);

    // Position in normalized device coords (-1 to 1)
    // transform.xy = center position, transform.zw = half-size
    let pos = uniforms.transform.xy + vec2<f32>(x * 2.0 - 1.0, y * 2.0 - 1.0) * uniforms.transform.zw;

    // UV coordinates for this vertex within the frame
    // frame_uv.xy = offset, frame_uv.zw = size
    let uv = uniforms.frame_uv.xy + vec2<f32>(x, 1.0 - y) * uniforms.frame_uv.zw;

    var output: VertexOutput;
    output.position = vec4<f32>(pos, 0.0, 1.0);
    output.uv = uv;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(sprite_texture, sprite_sampler, input.uv);
    return vec4<f32>(color.rgb, color.a * uniforms.params.x);
}
