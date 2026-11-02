import bevy_pbr::render::{
    mesh_functions,
    mesh_view_bindings::view,
    view_transformations::position_world_to_clip,
};
struct TerrainUniform {
    transforms: array<vec4<f32>, 256>,
    layers: array<vec4<u32>, 64>,
    count: vec4<u32>,
    fog_color: vec4<f32>,
    // x = fog start, y = fog end, z = density.
    fog_params: vec4<f32>,
    // x = fog mode (0 off, 1 linear, 2 exp, 3 exp2).
    fog_mode: vec4<u32>,
    // xyz = direction the light travels (Bevy world space), w unused.
    light_direction: vec4<f32>,
    // material.diffuse * light.diffuse.
    light_diffuse: vec4<f32>,
    // material.ambient * light.ambient + material.emissive.
    light_ambient: vec4<f32>,
}
@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> terrain: TerrainUniform;
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var ground: texture_2d_array<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(2) var ground_sampler: sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(3) var masks: texture_2d_array<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(4) var mask_sampler: sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(5) var shadow: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(6) var shadow_sampler: sampler;
// The character shadow (render/shadow.rs): the light view of its
// orthographic box, `params.x = 2 / box size`, `params.y = enabled`.
struct ShadowView {
    light_from_world: mat4x4<f32>,
    params: vec4<f32>,
}
@group(constants::MATERIAL_BIND_GROUP) @binding(7) var<storage, read> character_shadow_view: ShadowView;
@group(constants::MATERIAL_BIND_GROUP) @binding(8) var character_shadow: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(9) var character_shadow_sampler: sampler;
struct TerrainVertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) uv_b: vec2<f32>,
    @location(8) shadow_uv: vec2<f32>,
}
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) alpha_uv: vec2<f32>,
    @location(2) light: vec3<f32>,
    // Distance along the view axis, the fog coordinate of the GL reference.
    @location(3) view_depth: f32,
    @location(4) shadow_uv: vec2<f32>,
    @location(5) world_position: vec3<f32>,
}
@vertex
fn vertex(v: TerrainVertex) -> VertexOutput {
    let world_from_local = mesh_functions::get_world_from_local(v.instance_index);
    let position = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(v.position, 1.0));
    let normal = mesh_functions::mesh_normal_local_to_world(v.normal, v.instance_index);
    var out: VertexOutput;
    out.position = position_world_to_clip(position.xyz);
    out.view_depth = -(view.view_from_world * position).z;
    out.uv = v.uv;
    out.alpha_uv = v.uv_b;
    out.shadow_uv = v.shadow_uv;
    out.world_position = position.xyz;
    // D3D9 vertex lighting of the hardware T&L shadow pass: one directional
    // light, material colours, Gouraud, the sum clamped per channel.
    var ndl = 0.0;
    if length(terrain.light_direction.xyz) > 1e-6 {
        ndl = max(dot(normalize(normal), -normalize(terrain.light_direction.xyz)), 0.0);
    }
    let ambient = clamp(terrain.light_ambient.rgb, vec3<f32>(0.0), vec3<f32>(1.0));
    out.light = clamp(ambient + ndl * terrain.light_diffuse.rgb, vec3<f32>(0.0), vec3<f32>(1.0));
    return out;
}
@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    var color = vec3<f32>(0.0);
    for (var i = 0u; i < terrain.count.x; i++) {
        let id = terrain.layers[i / 4u][i % 4u];
        let transform = terrain.transforms[id];
        let texel = textureSample(ground, ground_sampler, in.uv * transform.xy + transform.zw, i32(id)).rgb;
        // The base is opaque; only later layers need a mask sample.
        if i == 0u {
            color = texel;
        } else {
            let mask = textureSample(masks, mask_sampler, in.alpha_uv, i32(id)).r;
            color = mix(color, texel, mask);
        }
    }
    // The shadow pass (`D3DBLEND_ZERO, D3DBLEND_SRCCOLOR`) multiplies the
    // splats by lighting times the static shadow map. Its fog colour is white,
    // so the multiplier fades out with the fog while the splats fade to the fog
    // colour.
    var shade = in.light * textureSample(shadow, shadow_sampler, in.shadow_uv).rgb;
    // Stage 1 of the same pass: `m_matDynamicShadow` into the character shadow
    // texture, white outside its box (`D3DTADDRESS_BORDER`).
    let light = character_shadow_view.light_from_world * vec4<f32>(in.world_position, 1.0);
    let clip = light.xy * character_shadow_view.params.x;
    let character_uv = vec2<f32>(clip.x * 0.5 + 0.5, 0.5 - clip.y * 0.5);
    let character = textureSample(character_shadow, character_shadow_sampler, character_uv).rgb;
    let inside = all(abs(clip) < vec2<f32>(1.0));
    if character_shadow_view.params.y > 0.5 && inside {
        shade *= character;
    }
    // Fog on the view-axis distance, exactly as fixed_function.wesl does it.
    var fog_factor = 1.0;
    switch terrain.fog_mode.x {
        case 1u: { fog_factor = (terrain.fog_params.y - in.view_depth) / (terrain.fog_params.y - terrain.fog_params.x); }
        case 2u: { fog_factor = exp(-terrain.fog_params.z * in.view_depth); }
        case 3u: {
            let d = terrain.fog_params.z * in.view_depth;
            fog_factor = exp(-d * d);
        }
        default: {}
    }
    if terrain.fog_mode.x == 0u {
        return vec4<f32>(color * shade, 1.0);
    }
    let f = clamp(fog_factor, 0.0, 1.0);
    return vec4<f32>(mix(terrain.fog_color.rgb, color, f) * mix(vec3<f32>(1.0), shade, f), 1.0);
}
