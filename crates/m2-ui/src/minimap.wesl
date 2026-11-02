// The minimap disc (`CPythonMiniMap::Render`): the 3x3 terrains around the
// player, each terrain's `minimap.dds` covering 25600 units, centred on the
// player at `units_per_px` and cut to a circle (`minimap_image_filter.dds`).

import bevy_ui_render::ui_vertex_output::UiVertexOutput;

struct MiniMapUniform {
    // Map-local units: the player, then the top-left corner of the 3x3 block.
    center: vec2<f32>,
    origin: vec2<f32>,
    // Map units per screen pixel, and the disc's radius in pixels.
    units_per_px: f32,
    radius: f32,
    _pad: vec2<f32>,
    // 1 where a terrain's texture is loaded, row-major (x fastest).
    loaded_a: vec4<f32>,
    loaded_b: vec4<f32>,
    loaded_c: vec4<f32>,
}

@group(1) @binding(0) var<uniform> m: MiniMapUniform;
@group(1) @binding(1) var t0: texture_2d<f32>;
@group(1) @binding(2) var s0: sampler;
@group(1) @binding(3) var t1: texture_2d<f32>;
@group(1) @binding(4) var s1: sampler;
@group(1) @binding(5) var t2: texture_2d<f32>;
@group(1) @binding(6) var s2: sampler;
@group(1) @binding(7) var t3: texture_2d<f32>;
@group(1) @binding(8) var s3: sampler;
@group(1) @binding(9) var t4: texture_2d<f32>;
@group(1) @binding(10) var s4: sampler;
@group(1) @binding(11) var t5: texture_2d<f32>;
@group(1) @binding(12) var s5: sampler;
@group(1) @binding(13) var t6: texture_2d<f32>;
@group(1) @binding(14) var s6: sampler;
@group(1) @binding(15) var t7: texture_2d<f32>;
@group(1) @binding(16) var s7: sampler;
@group(1) @binding(17) var t8: texture_2d<f32>;
@group(1) @binding(18) var s8: sampler;

fn loaded(i: i32) -> f32 {
    if i < 4 { return m.loaded_a[i]; }
    if i < 8 { return m.loaded_b[i - 4]; }
    return m.loaded_c[0];
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let px = (in.uv - vec2(0.5)) * in.size;
    let distance = length(px);
    // A one-pixel soft rim, as the filter image fades the edge.
    let alpha = clamp(m.radius - distance, 0.0, 1.0);
    if alpha <= 0.0 {
        discard;
    }
    let rel = (m.center + px * m.units_per_px - m.origin) / 25600.0;
    let cell = vec2<i32>(floor(rel));
    let uv = fract(rel);
    let black = vec4(0.0, 0.0, 0.0, 1.0);
    if cell.x < 0 || cell.y < 0 || cell.x > 2 || cell.y > 2 {
        return vec4(black.rgb, alpha);
    }
    let i = cell.y * 3 + cell.x;
    // Every texture is sampled (uniform control flow), the right one picked.
    let c0 = textureSample(t0, s0, uv);
    let c1 = textureSample(t1, s1, uv);
    let c2 = textureSample(t2, s2, uv);
    let c3 = textureSample(t3, s3, uv);
    let c4 = textureSample(t4, s4, uv);
    let c5 = textureSample(t5, s5, uv);
    let c6 = textureSample(t6, s6, uv);
    let c7 = textureSample(t7, s7, uv);
    let c8 = textureSample(t8, s8, uv);
    var c = c0;
    switch i {
        case 1: { c = c1; }
        case 2: { c = c2; }
        case 3: { c = c3; }
        case 4: { c = c4; }
        case 5: { c = c5; }
        case 6: { c = c6; }
        case 7: { c = c7; }
        case 8: { c = c8; }
        default: {}
    }
    if loaded(i) < 0.5 {
        c = black;
    }
    return vec4(c.rgb, alpha);
}
