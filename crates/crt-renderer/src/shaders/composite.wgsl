// Composite shader - applies glow blur to text texture
// This is expensive (25 texture samples) so only runs when text changes

struct Params {
    screen_size: vec2<f32>,
    time: f32,
    grid_intensity: f32,
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
@group(0) @binding(1) var text_texture: texture_2d<f32>;
@group(0) @binding(2) var text_sampler: sampler;

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

// 324-sample (18x18) Gaussian blur for smooth glow effect
fn sample_blur(uv: vec2<f32>, radius: f32) -> f32 {
    let texel_size = 1.0 / params.screen_size;
    let effective_radius = min(radius, 50.0);
    let sigma = effective_radius / 3.0;

    var total = 0.0;
    var weight_sum = 0.0;

    // 18x18 grid: -8 to 9 (using 9 for 18 total)
    let samples = 8i;
    for (var x = -samples; x <= samples; x++) {
        for (var y = -samples; y <= samples; y++) {
            // Spread samples evenly across the radius
            let offset = vec2<f32>(f32(x), f32(y)) * texel_size * (effective_radius / 8.0);
            let dist = length(vec2<f32>(f32(x), f32(y)));
            let w = exp(-(dist * dist) / (2.0 * sigma * sigma));

            let sample_color = textureSample(text_texture, text_sampler, uv + offset);
            // Use alpha channel for glow source (text has alpha where glyphs are)
            total += sample_color.a * w;
            weight_sum += w;
        }
    }

    return total / weight_sum;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample the text texture (contains colored glyphs)
    let text = textureSample(text_texture, text_sampler, in.uv);
    let text_alpha = text.a;

    // Start with transparent
    var color = vec3<f32>(0.0, 0.0, 0.0);
    var alpha = 0.0;

    // Glow effect (if enabled) - render glow behind text
    if params.glow_intensity > 0.0 {
        let blur = sample_blur(in.uv, params.glow_radius);
        let glow_alpha = blur * params.glow_intensity * 2.0;
        color = params.glow_color.rgb;
        alpha = min(glow_alpha, 0.8);
    }

    // Blend text on top (preserving original colors from texture)
    if text_alpha > 0.01 {
        color = mix(color, text.rgb, text_alpha);
        alpha = max(alpha, text_alpha);
    }

    return vec4<f32>(color, alpha);
}
