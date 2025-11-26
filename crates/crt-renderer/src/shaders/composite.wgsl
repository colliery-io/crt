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

// Tighter 25-sample (5x5) Gaussian blur for subtle glow
fn sample_blur(uv: vec2<f32>, radius: f32) -> f32 {
    let texel_size = 1.0 / params.screen_size;
    // Clamp radius to prevent overly spread blur
    let effective_radius = min(radius, 8.0);
    let sigma = effective_radius / 3.0;

    var total = 0.0;
    var weight_sum = 0.0;

    // 5x5 grid: -2 to 2
    let samples = 2i;
    for (var x = -samples; x <= samples; x++) {
        for (var y = -samples; y <= samples; y++) {
            // Tighter sampling - divide by 8 instead of 4
            let offset = vec2<f32>(f32(x), f32(y)) * texel_size * (effective_radius / 8.0);
            let dist = length(vec2<f32>(f32(x), f32(y)));
            let w = exp(-(dist * dist) / (2.0 * sigma * sigma));

            let sample_color = textureSample(text_texture, text_sampler, uv + offset);
            let luminance = dot(sample_color.rgb, vec3<f32>(0.299, 0.587, 0.114));
            total += luminance * w;
            weight_sum += w;
        }
    }

    return total / weight_sum;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Start with transparent - we're blending onto the background
    var color = vec3<f32>(0.0, 0.0, 0.0);
    var alpha = 0.0;

    // Glow effect (if enabled)
    if params.glow_intensity > 0.0 {
        let blur = sample_blur(in.uv, params.glow_radius);
        // Boost intensity to compensate for tighter blur
        let glow_alpha = blur * params.glow_intensity * 1.5;
        color = mix(color, params.glow_color.rgb, min(glow_alpha, 1.0));
        alpha = max(alpha, min(glow_alpha, 0.9));
    }

    // Text
    let text = textureSample(text_texture, text_sampler, in.uv);
    let text_luminance = dot(text.rgb, vec3<f32>(0.299, 0.587, 0.114));
    color = mix(color, params.text_color.rgb, text_luminance);
    alpha = max(alpha, text_luminance);

    return vec4<f32>(color, alpha);
}
