// Synthwave Theme Shader
// Combines: gradient background, perspective grid, text glow

struct Params {
    screen_size: vec2<f32>,
    time: f32,
    grid_intensity: f32,

    // Gradient colors (top to bottom)
    gradient_top: vec4<f32>,
    gradient_bottom: vec4<f32>,

    // Grid settings
    grid_color: vec4<f32>,
    grid_spacing: f32,
    grid_line_width: f32,
    grid_perspective: f32,
    grid_horizon: f32,  // 0.0 = bottom, 1.0 = top

    // Glow settings
    glow_color: vec4<f32>,
    glow_radius: f32,
    glow_intensity: f32,

    text_color: vec4<f32>,

    _pad: vec4<f32>,  // Align to 144 bytes
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

// Linear gradient (like CSS linear-gradient(to bottom, ...))
fn gradient(uv: vec2<f32>, top: vec3<f32>, bottom: vec3<f32>) -> vec3<f32> {
    return mix(top, bottom, uv.y);
}

// Perspective grid (synthwave floor effect)
fn perspective_grid(uv: vec2<f32>, time: f32) -> f32 {
    // Only draw grid below horizon
    let horizon = params.grid_horizon;
    if uv.y < horizon {
        return 0.0;
    }

    // Map y from [horizon, 1.0] to [0.0, 1.0] for grid space
    let grid_y = (uv.y - horizon) / (1.0 - horizon);

    // Apply perspective transformation
    // As y increases (toward bottom), lines get further apart
    let perspective = pow(grid_y, params.grid_perspective);

    // Vertical lines (get closer together toward horizon)
    let x_centered = uv.x - 0.5;
    let x_perspective = x_centered / (perspective + 0.001);
    let x_grid = abs(fract(x_perspective * params.grid_spacing + 0.5) - 0.5);
    let x_line = 1.0 - smoothstep(0.0, params.grid_line_width / (perspective + 0.1), x_grid);

    // Horizontal lines (scroll with time for animation)
    let y_scroll = perspective * params.grid_spacing * 2.0 - time * 0.5;
    let y_grid = abs(fract(y_scroll + 0.5) - 0.5);
    let y_line = 1.0 - smoothstep(0.0, params.grid_line_width * 2.0, y_grid);

    // Combine lines, fade with distance from camera
    let grid = max(x_line, y_line);
    let fade = 1.0 - perspective * 0.5;

    return grid * fade * params.grid_intensity;
}

// Gaussian blur for glow
fn sample_blur(uv: vec2<f32>, radius: f32) -> f32 {
    let texel_size = 1.0 / params.screen_size;
    let sigma = radius / 2.0;

    var total = 0.0;
    var weight_sum = 0.0;

    let samples = 4i;
    for (var x = -samples; x <= samples; x++) {
        for (var y = -samples; y <= samples; y++) {
            let offset = vec2<f32>(f32(x), f32(y)) * texel_size * (radius / 4.0);
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
    // Layer 1: Gradient background
    var color = gradient(in.uv, params.gradient_top.rgb, params.gradient_bottom.rgb);

    // Layer 2: Perspective grid
    let grid = perspective_grid(in.uv, params.time);
    color = mix(color, params.grid_color.rgb, grid * params.grid_color.a);

    // Layer 3: Text glow
    let blur = sample_blur(in.uv, params.glow_radius);
    color = mix(color, params.glow_color.rgb, blur * params.glow_intensity);

    // Layer 4: Text
    let text = textureSample(text_texture, text_sampler, in.uv);
    let text_luminance = dot(text.rgb, vec3<f32>(0.299, 0.587, 0.114));
    color = mix(color, params.text_color.rgb, text_luminance);

    return vec4<f32>(color, 1.0);
}
