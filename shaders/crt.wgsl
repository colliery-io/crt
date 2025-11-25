// CRT Effect Shader - Prototype A
// Static shader with uniform-based configuration

// Uniforms matching CSS-like style properties
struct CrtStyle {
    // Display properties
    resolution: vec2<f32>,      // Virtual CRT resolution
    screen_size: vec2<f32>,     // Actual screen size in pixels

    // Scanline effect
    scanline_intensity: f32,    // 0.0 = off, 1.0 = full strength
    scanline_count: f32,        // Number of scanlines

    // Phosphor/subpixel mask
    phosphor_intensity: f32,    // RGB subpixel mask strength

    // Glow/bloom
    glow_intensity: f32,        // Bloom strength
    glow_radius: f32,           // Bloom spread

    // Screen curvature
    curvature: f32,             // Barrel distortion amount

    // Color adjustments
    brightness: f32,
    contrast: f32,
    saturation: f32,

    _padding: f32,              // Alignment padding
}

@group(0) @binding(0) var<uniform> style: CrtStyle;
@group(0) @binding(1) var input_texture: texture_2d<f32>;
@group(0) @binding(2) var input_sampler: sampler;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

// Full-screen triangle vertices (oversized triangle technique)
@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;

    // Generate oversized triangle that covers full viewport when clipped
    // vertex 0: (-1, -1), vertex 1: (3, -1), vertex 2: (-1, 3)
    let x = f32(i32(vertex_index & 1u)) * 4.0 - 1.0;
    let y = f32(i32(vertex_index >> 1u)) * 4.0 - 1.0;

    out.position = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);

    return out;
}

// Apply barrel distortion for screen curvature
fn apply_curvature(uv: vec2<f32>, amount: f32) -> vec2<f32> {
    let centered = uv - 0.5;
    let dist = dot(centered, centered);
    let curved = centered * (1.0 + dist * amount);
    return curved + 0.5;
}

// Scanline effect
fn apply_scanlines(color: vec3<f32>, uv: vec2<f32>, intensity: f32, count: f32) -> vec3<f32> {
    let scanline = sin(uv.y * count * 3.14159) * 0.5 + 0.5;
    let scanline_factor = mix(1.0, scanline, intensity);
    return color * scanline_factor;
}

// RGB phosphor mask (simplified aperture grille)
fn apply_phosphor_mask(color: vec3<f32>, screen_pos: vec2<f32>, intensity: f32) -> vec3<f32> {
    let pixel_x = i32(screen_pos.x) % 3;
    var mask = vec3<f32>(1.0);

    if pixel_x == 0 {
        mask = vec3<f32>(1.0, 1.0 - intensity * 0.5, 1.0 - intensity * 0.5);
    } else if pixel_x == 1 {
        mask = vec3<f32>(1.0 - intensity * 0.5, 1.0, 1.0 - intensity * 0.5);
    } else {
        mask = vec3<f32>(1.0 - intensity * 0.5, 1.0 - intensity * 0.5, 1.0);
    }

    return color * mask;
}

// Simplified glow (box blur approximation)
fn sample_with_glow(uv: vec2<f32>, radius: f32, intensity: f32) -> vec3<f32> {
    let texel_size = 1.0 / style.screen_size;
    var color = textureSample(input_texture, input_sampler, uv).rgb;

    if intensity > 0.0 && radius > 0.0 {
        var glow = vec3<f32>(0.0);
        let samples = 4.0;

        for (var i = -2.0; i <= 2.0; i += 1.0) {
            for (var j = -2.0; j <= 2.0; j += 1.0) {
                let offset = vec2<f32>(i, j) * texel_size * radius;
                glow += textureSample(input_texture, input_sampler, uv + offset).rgb;
            }
        }
        glow /= 25.0;
        color = mix(color, max(color, glow), intensity);
    }

    return color;
}

// Brightness/contrast/saturation adjustments
fn apply_color_adjustments(color: vec3<f32>, brightness: f32, contrast: f32, saturation: f32) -> vec3<f32> {
    // Brightness
    var c = color * brightness;

    // Contrast
    c = (c - 0.5) * contrast + 0.5;

    // Saturation
    let gray = dot(c, vec3<f32>(0.299, 0.587, 0.114));
    c = mix(vec3<f32>(gray), c, saturation);

    return clamp(c, vec3<f32>(0.0), vec3<f32>(1.0));
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Apply screen curvature
    var uv = apply_curvature(in.uv, style.curvature);

    // Discard pixels outside curved screen
    if uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0 {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }

    // Sample with glow
    var color = sample_with_glow(uv, style.glow_radius, style.glow_intensity);

    // Apply scanlines
    color = apply_scanlines(color, uv, style.scanline_intensity, style.scanline_count);

    // Apply phosphor mask
    let screen_pos = in.position.xy;
    color = apply_phosphor_mask(color, screen_pos, style.phosphor_intensity);

    // Apply color adjustments
    color = apply_color_adjustments(color, style.brightness, style.contrast, style.saturation);

    return vec4<f32>(color, 1.0);
}
