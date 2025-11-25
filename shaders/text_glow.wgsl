// Text Glow Shader
// Implements CSS text-shadow style glow effects
// text-shadow: 0px 0px 10px color1, 0px 0px 20px color2;

struct GlowParams {
    screen_size: vec2<f32>,
    glow1_radius: f32,
    glow1_intensity: f32,

    glow1_color: vec4<f32>,  // rgb + unused alpha

    glow2_radius: f32,
    glow2_intensity: f32,
    _pad1: f32,
    _pad2: f32,

    glow2_color: vec4<f32>,  // rgb + unused alpha

    text_color: vec4<f32>,   // rgb + unused alpha
}

@group(0) @binding(0) var<uniform> params: GlowParams;
@group(0) @binding(1) var input_texture: texture_2d<f32>;
@group(0) @binding(2) var input_sampler: sampler;

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

// Gaussian blur kernel weights (approximation)
fn gaussian_weight(offset: f32, sigma: f32) -> f32 {
    return exp(-(offset * offset) / (2.0 * sigma * sigma));
}

// Sample with blur at given radius
fn sample_blur(uv: vec2<f32>, radius: f32) -> f32 {
    let texel_size = 1.0 / params.screen_size;
    let sigma = radius / 2.0;

    var total = 0.0;
    var weight_sum = 0.0;

    // 9-tap blur in both directions
    let samples = 4i;
    for (var x = -samples; x <= samples; x++) {
        for (var y = -samples; y <= samples; y++) {
            let offset = vec2<f32>(f32(x), f32(y)) * texel_size * (radius / 4.0);
            let dist = length(vec2<f32>(f32(x), f32(y)));
            let w = gaussian_weight(dist, sigma);

            let sample_color = textureSample(input_texture, input_sampler, uv + offset);
            // Use luminance as the "text" signal
            let luminance = dot(sample_color.rgb, vec3<f32>(0.299, 0.587, 0.114));
            total += luminance * w;
            weight_sum += w;
        }
    }

    return total / weight_sum;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample original text
    let original = textureSample(input_texture, input_sampler, in.uv);
    let text_luminance = dot(original.rgb, vec3<f32>(0.299, 0.587, 0.114));

    // Sample blurred versions for glow layers
    let blur1 = sample_blur(in.uv, params.glow1_radius);
    let blur2 = sample_blur(in.uv, params.glow2_radius);

    // Build up the glow layers (outer first, then inner)
    var color = vec3<f32>(0.0);

    // Background (could be configurable)
    let bg_color = vec3<f32>(0.125, 0.035, 0.2); // Deep purple like the theme
    color = bg_color;

    // Outer glow (larger radius, more diffuse)
    color = mix(color, params.glow2_color.rgb, blur2 * params.glow2_intensity);

    // Inner glow (smaller radius, more concentrated)
    color = mix(color, params.glow1_color.rgb, blur1 * params.glow1_intensity);

    // Original text on top (colorized)
    color = mix(color, params.text_color.rgb, text_luminance);

    return vec4<f32>(color, 1.0);
}
