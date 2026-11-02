// Fixed-function (Direct3D 9 style) material for Metin2.
//
// One shader for every FixedFunctionMaterial. Discrete state that needs a
// pipeline change (blend, cull, depth) lives in the specialisation key on the
// Rust side; everything here is driven by the uniform. All colour maths is in
// gamma space with no sRGB conversion, as in the original renderer.
//
// Vertex contract (Bevy mesh attributes):
//   POSITION  location 0   required
//   NORMAL    location 1   only read when lighting is enabled
//   UV_0      location 2   repeating texture coordinates
//   UV_1      location 3   optional stage 1 coordinates (falls back to UV_0)
//   COLOR     location 5   Float32x4 gamma-space diffuse, white when absent
//   JOINT_INDEX / JOINT_WEIGHT (6, 7) with a SkinnedMesh component: GPU skinning
//   BILLBOARD location 8   camera-facing cards, drawn by `billboard_vertex`

import bevy_pbr::render::{
    mesh_functions,
    skinning,
    forward_io::{Vertex, UncompressedVertex, decompress_vertex},
    view_transformations::position_world_to_clip,
    mesh_view_bindings::view,
};

struct FixedFunction {
    fog_color: vec4<f32>,
    texture_factor: vec4<f32>,
    material_diffuse: vec4<f32>,
    material_ambient: vec4<f32>,
    material_emissive: vec4<f32>,
    global_ambient: vec4<f32>,
    // Direction the light travels (world space), w unused.
    light_direction: vec4<f32>,
    light_diffuse: vec4<f32>,
    light_ambient: vec4<f32>,
    // Per stage: x = op, y = arg1, z = arg2 (D3DTOP_* / packed D3DTA_*).
    stage_color: array<vec4<u32>, 2>,
    stage_alpha: array<vec4<u32>, 2>,
    // x = fog start, y = fog end, z = fog density, w = unused.
    fog_params: vec4<f32>,
    // x = fog mode (0 off, 1 linear, 2 exp, 3 exp2), y = alpha func (D3DCMP_*,
    // 0 = alpha test off), z = lighting on, w = texture mask (bit i: stage i has a texture).
    flags: vec4<u32>,
    // x = alpha reference in 0..255.
    alpha_ref: vec4<u32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> ff: FixedFunction;
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var texture0: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(2) var sampler0: sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(3) var texture1: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(4) var sampler1: sampler;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(3) uv1: vec2<f32>,
    @location(1) color: vec4<f32>,
    // Distance along the view axis, the fog coordinate of the GL reference.
    @location(2) view_depth: f32,
}

@vertex
fn vertex(vertex_no_morph: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let vertex = decompress_vertex(vertex_no_morph, vertex_no_morph.instance_index);

@if(SKINNED)
    let world_from_local = skinning::skin_model(
        vertex.joint_indices,
        vertex.joint_weights,
        vertex_no_morph.instance_index
    );
@else
    let world_from_local = mesh_functions::get_world_from_local(vertex_no_morph.instance_index);

    let world_position = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(vertex.position, 1.0));
    out.position = position_world_to_clip(world_position.xyz);
    out.view_depth = -(view.view_from_world * world_position).z;

@if(VERTEX_UVS_A)
    out.uv = vertex.uv;
@else
    out.uv = vec2<f32>(0.0);

@if(VERTEX_UVS_B)
    out.uv1 = vertex.uv_b;
@else
    out.uv1 = out.uv;

    // Primary colour: vertex colour when the mesh has one, else the material colour.
@if(VERTEX_COLORS)
    var diffuse = vertex.color;
@else
    var diffuse = ff.material_diffuse;

@if(VERTEX_NORMALS)
    if ff.flags.z != 0u {
@if(SKINNED)
        let world_normal = skinning::skin_normals(world_from_local, vertex.normal);
@else
        let world_normal = mesh_functions::mesh_normal_local_to_world(vertex.normal, vertex_no_morph.instance_index);
        // D3D9 fixed-function lighting, one directional light, Gouraud, material colours
        // taken from the material (D3DMCS_MATERIAL, as the client's StateManager sets).
        let n_dot_l = max(dot(normalize(world_normal), -normalize(ff.light_direction.xyz)), 0.0);
        let lit = ff.material_emissive.rgb
            + ff.global_ambient.rgb * ff.material_ambient.rgb
            + ff.light_ambient.rgb * ff.material_ambient.rgb
            + n_dot_l * ff.light_diffuse.rgb * ff.material_diffuse.rgb;
        diffuse = vec4<f32>(clamp(lit, vec3<f32>(0.0), vec3<f32>(1.0)), ff.material_diffuse.a);
    }

    out.color = diffuse;
    return out;
}


// Camera-facing cards (SpeedTree leaves, `CSpeedTreeWrapper`'s per-frame
// billboarding moved to the GPU). POSITION is the card centre in mesh space and
// BILLBOARD (location 8) the corner as coefficients of the camera basis
// `right * x + up * y`, the card's own rotation already applied. The basis is
// the one `leaf_billboards` builds in source space (z up), from the camera
// towards the mesh origin, so every instance faces the camera with one shared
// mesh. Unlit: cards have no normals.
struct BillboardVertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(5) color: vec4<f32>,
    @location(8) corner: vec2<f32>,
}

@vertex
fn billboard_vertex(vertex: BillboardVertex) -> VertexOutput {
    var out: VertexOutput;
    let world_from_local = mesh_functions::get_world_from_local(vertex.instance_index);
    let local_from_world = mesh_functions::get_local_from_world(vertex.instance_index);
    // Bevy space is source space with y and z swapped.
    let camera = (local_from_world * vec4<f32>(view.world_position, 1.0)).xzy;
    var v = -camera;
    let vl = length(v);
    if vl < 1e-4 {
        v = vec3<f32>(0.0, 1.0, 0.0);
    } else {
        v = v / vl;
    }
    // right = normalize(cross(world up, view)), up = cross(view, right).
    var right = vec3<f32>(-v.y, v.x, 0.0);
    let rl = length(right);
    if rl < 1e-4 {
        right = vec3<f32>(1.0, 0.0, 0.0);
    } else {
        right = right / rl;
    }
    let up = cross(v, right);
    let offset = right * vertex.corner.x + up * vertex.corner.y;
    let local = vertex.position + offset.xzy;

    let world_position = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(local, 1.0));
    out.position = position_world_to_clip(world_position.xyz);
    out.view_depth = -(view.view_from_world * world_position).z;
    out.uv = vertex.uv;
    out.uv1 = vertex.uv;
    out.color = vertex.color;
    return out;
}

fn stage_arg(arg: u32, diffuse: vec4<f32>, current: vec4<f32>, tex: vec4<f32>, tfactor: vec4<f32>) -> vec4<f32> {
    var v: vec4<f32>;
    switch arg & 0xffu {
        // DIFFUSE, SPECULAR (no specular channel: the GL backend maps it to the primary colour)
        case 0u, 4u: { v = diffuse; }
        // CURRENT, TEMP
        case 1u, 5u: { v = current; }
        case 2u: { v = tex; }
        // TFACTOR, CONSTANT
        case 3u, 6u: { v = tfactor; }
        default: { v = tex; }
    }
    if (arg & 0x100u) != 0u {
        v = vec4<f32>(1.0) - v;
    }
    return v;
}

// Evaluates one D3DTOP_* on four channels; the caller keeps rgb or a.
fn stage_op(op: u32, a1: vec4<f32>, a2: vec4<f32>, w_diffuse: f32, w_tex: f32, w_factor: f32, w_current: f32) -> vec4<f32> {
    switch op {
        case 2u: { return a1; }
        case 3u: { return a2; }
        case 4u: { return a1 * a2; }
        case 5u: { return a1 * a2 * 2.0; }
        case 6u: { return a1 * a2 * 4.0; }
        case 7u: { return a1 + a2; }
        case 8u: { return a1 + a2 - vec4<f32>(0.5); }
        case 9u: { return (a1 + a2 - vec4<f32>(0.5)) * 2.0; }
        case 10u: { return a1 - a2; }
        case 11u: { return a1 + a2 * (vec4<f32>(1.0) - a1); }
        case 12u: { return a1 * w_diffuse + a2 * (1.0 - w_diffuse); }
        case 13u: { return a1 * w_tex + a2 * (1.0 - w_tex); }
        case 14u: { return a1 * w_factor + a2 * (1.0 - w_factor); }
        case 15u: { return a1 + a2 * (1.0 - w_tex); }
        case 16u: { return a1 * w_current + a2 * (1.0 - w_current); }
        // Unknown ops behave as the GL backend's default: MODULATE.
        default: { return a1 * a2; }
    }
}

fn compare_alpha(func: u32, a: u32, r: u32) -> bool {
    switch func {
        case 1u: { return false; }
        case 2u: { return a < r; }
        case 3u: { return a == r; }
        case 4u: { return a <= r; }
        case 5u: { return a > r; }
        case 6u: { return a != r; }
        case 7u: { return a >= r; }
        default: { return true; }
    }
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample unconditionally to stay in uniform control flow.
    let tex0 = textureSample(texture0, sampler0, in.uv);
    let tex1 = textureSample(texture1, sampler1, in.uv1);

    let diffuse = in.color;
    let tfactor = ff.texture_factor;
    var current = diffuse;

    for (var i = 0u; i < 2u; i++) {
        let textured = ((ff.flags.w >> i) & 1u) != 0u;
        let cop = ff.stage_color[i].x;
        let aop = ff.stage_alpha[i].x;
        // As in the GL backend: a stage without a texture, or with both ops disabled, passes through.
        if !textured || (cop == 1u && aop == 1u) {
            continue;
        }
        var tex = tex0;
        if i == 1u {
            tex = tex1;
        }
        var rgb = current.rgb;
        var alpha = current.a;
        if cop != 1u {
            let c1 = stage_arg(ff.stage_color[i].y, diffuse, current, tex, tfactor);
            let c2 = stage_arg(ff.stage_color[i].z, diffuse, current, tex, tfactor);
            var a1 = c1.rgb;
            var a2 = c2.rgb;
            // D3DTA_ALPHAREPLICATE
            if (ff.stage_color[i].y & 0x200u) != 0u { a1 = vec3<f32>(c1.a); }
            if (ff.stage_color[i].z & 0x200u) != 0u { a2 = vec3<f32>(c2.a); }
            rgb = stage_op(cop, vec4<f32>(a1, 0.0), vec4<f32>(a2, 0.0), diffuse.a, tex.a, tfactor.a, current.a).rgb;
        }
        if aop != 1u {
            let c1 = stage_arg(ff.stage_alpha[i].y, diffuse, current, tex, tfactor);
            let c2 = stage_arg(ff.stage_alpha[i].z, diffuse, current, tex, tfactor);
            alpha = stage_op(aop, c1, c2, diffuse.a, tex.a, tfactor.a, current.a).a;
        }
        current = clamp(vec4<f32>(rgb, alpha), vec4<f32>(0.0), vec4<f32>(1.0));
    }

    var color = current;

    // Fog on the view-axis distance, applied per fragment.
    var fog_factor = 1.0;
    switch ff.flags.x {
        case 1u: { fog_factor = (ff.fog_params.y - in.view_depth) / (ff.fog_params.y - ff.fog_params.x); }
        case 2u: { fog_factor = exp(-ff.fog_params.z * in.view_depth); }
        case 3u: {
            let d = ff.fog_params.z * in.view_depth;
            fog_factor = exp(-d * d);
        }
        default: {}
    }
    if ff.flags.x != 0u {
        color = vec4<f32>(mix(ff.fog_color.rgb, color.rgb, clamp(fog_factor, 0.0, 1.0)), color.a);
    }

    // Alpha test on the 8-bit alpha, as D3D compares integers.
    if ff.flags.y != 0u {
        let a8 = u32(clamp(color.a, 0.0, 1.0) * 255.0 + 0.5);
        if !compare_alpha(ff.flags.y, a8, ff.alpha_ref.x) {
            discard;
        }
    }

    // A view with zero exposure is the character shadow pass (m2-game's
    // `render::shadow` marks its camera so; colours here never use exposure):
    // `CMapOutdoor::BeginRenderCharacterShadowToTexture` turns lighting off and
    // draws stage 0 `SELECTARG1(TFACTOR)` with `TFACTOR = 0xFF808080`, on white.
    if view.exposure == 0.0 {
        return vec4<f32>(vec3<f32>(128.0 / 255.0), 1.0);
    }

    return color;
}
