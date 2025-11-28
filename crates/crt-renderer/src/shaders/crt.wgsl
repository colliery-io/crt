// CRT post-processing shader
// Applies scanlines, screen curvature, vignette, and other CRT effects

// Must match CrtUniforms in lib.rs (64 bytes total, 16-byte aligned)
struct CrtParams {
    screen_size: vec2<f32>,       // 8 bytes
    time: f32,                     // 4 bytes
    scanline_intensity: f32,       // 4 bytes = 16 bytes
    scanline_frequency: f32,       // 4 bytes
    curvature: f32,                // 4 bytes
    vignette: f32,                 // 4 bytes
    chromatic_aberration: f32,     // 4 bytes = 32 bytes
    bloom: f32,                    // 4 bytes
    flicker: f32,                  // 4 bytes
    _pad0: f32,                    // 4 bytes
    _pad1: f32,                    // 4 bytes = 48 bytes
    _pad2: vec4<f32>,              // 16 bytes = 64 bytes total
}

@group(0) @binding(0) var<uniform> params: CrtParams;
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

// Apply barrel distortion (CRT screen curvature)
fn apply_curvature(uv: vec2<f32>, curvature: f32) -> vec2<f32> {
    let centered = uv * 2.0 - 1.0;
    let offset = centered.yx * centered.yx * centered.xy * curvature;
    let distorted = centered + offset;
    return distorted * 0.5 + 0.5;
}

// Check if UV is within screen bounds
fn is_in_bounds(uv: vec2<f32>) -> bool {
    return uv.x >= 0.0 && uv.x <= 1.0 && uv.y >= 0.0 && uv.y <= 1.0;
}

// Calculate vignette darkening
fn calc_vignette(uv: vec2<f32>, intensity: f32) -> f32 {
    let centered = uv * 2.0 - 1.0;
    let dist = length(centered);
    // Smooth falloff from center to edges
    return 1.0 - smoothstep(0.5, 1.5, dist) * intensity;
}

// Calculate scanline darkening
fn calc_scanlines(uv: vec2<f32>, screen_height: f32, intensity: f32, frequency: f32) -> f32 {
    let line = uv.y * screen_height * frequency;
    // Sine wave creates smooth scanline pattern
    let scanline = sin(line * 3.14159) * 0.5 + 0.5;
    // Return multiplier (1.0 = no darkening, lower = darker)
    return 1.0 - (1.0 - scanline) * intensity;
}

// Simple bloom/glow sampling
fn sample_bloom(uv: vec2<f32>, radius: f32) -> vec3<f32> {
    let texel = 1.0 / params.screen_size;
    var bloom = vec3<f32>(0.0);
    let samples = 4;

    for (var x = -samples; x <= samples; x++) {
        for (var y = -samples; y <= samples; y++) {
            let offset = vec2<f32>(f32(x), f32(y)) * texel * radius;
            let sample_uv = uv + offset;
            if is_in_bounds(sample_uv) {
                bloom += textureSample(input_texture, input_sampler, sample_uv).rgb;
            }
        }
    }

    let total_samples = f32((2 * samples + 1) * (2 * samples + 1));
    return bloom / total_samples;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var uv = in.uv;

    // Apply screen curvature (barrel distortion)
    if params.curvature > 0.001 {
        uv = apply_curvature(uv, params.curvature);
    }

    // Check bounds after curvature - show black outside screen
    if !is_in_bounds(uv) {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }

    var color: vec3<f32>;

    // Chromatic aberration - separate RGB channels slightly
    if params.chromatic_aberration > 0.001 {
        let offset = params.chromatic_aberration * 0.01;
        let r = textureSample(input_texture, input_sampler, uv + vec2<f32>(offset, 0.0)).r;
        let g = textureSample(input_texture, input_sampler, uv).g;
        let b = textureSample(input_texture, input_sampler, uv - vec2<f32>(offset, 0.0)).b;
        color = vec3<f32>(r, g, b);
    } else {
        color = textureSample(input_texture, input_sampler, uv).rgb;
    }

    // Add bloom/glow
    if params.bloom > 0.001 {
        let bloom = sample_bloom(uv, 3.0);
        color = mix(color, bloom, params.bloom * 0.3);
        // Also brighten based on bloom
        color += bloom * params.bloom * 0.2;
    }

    // Apply scanlines
    if params.scanline_intensity > 0.001 {
        let scanline = calc_scanlines(uv, params.screen_size.y, params.scanline_intensity, params.scanline_frequency);
        color *= scanline;
    }

    // Apply vignette (edge darkening)
    if params.vignette > 0.001 {
        let vignette = calc_vignette(uv, params.vignette);
        color *= vignette;
    }

    // Apply flicker (subtle brightness variation)
    if params.flicker > 0.001 {
        let flicker = 1.0 + sin(params.time * 60.0) * params.flicker * 0.02;
        color *= flicker;
    }

    return vec4<f32>(color, 1.0);
}
