// Background shader - renders gradient + animated grid
// No texture samples needed, just math - very fast

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

fn perspective_grid(uv: vec2<f32>, time: f32) -> f32 {
    let horizon = params.grid_horizon;
    if uv.y < horizon {
        return 0.0;
    }

    let grid_y = (uv.y - horizon) / (1.0 - horizon);
    let perspective = pow(grid_y, params.grid_perspective);
    let horizon_fade = smoothstep(0.0, 0.15, grid_y);

    let x_centered = uv.x - 0.5;
    let x_perspective = x_centered / (perspective + 0.01);
    let x_grid = abs(fract(x_perspective * params.grid_spacing + 0.5) - 0.5);
    let line_width = params.grid_line_width / (perspective + 0.2);
    let x_line = 1.0 - smoothstep(0.0, line_width, x_grid);

    let y_scroll = perspective * params.grid_spacing * 2.0 - time * 0.5;
    let y_grid = abs(fract(y_scroll + 0.5) - 0.5);
    let y_line = 1.0 - smoothstep(0.0, params.grid_line_width * 3.0, y_grid);

    let grid = max(x_line, y_line);
    let distance_fade = 1.0 - perspective * 0.3;

    return grid * horizon_fade * distance_fade * params.grid_intensity;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var color = gradient(in.uv, params.gradient_top.rgb, params.gradient_bottom.rgb);

    if params.grid_intensity > 0.0 {
        let grid = perspective_grid(in.uv, params.time);
        color = mix(color, params.grid_color.rgb, grid * params.grid_color.a);
    }

    return vec4<f32>(color, 1.0);
}
