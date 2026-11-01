import bevy_pbr::render::{
    mesh_functions,
    forward_io::{Vertex, decompress_vertex},
    view_transformations::position_world_to_clip,
};
struct TerrainUniform {
    transforms: array<vec4<f32>, 256>,
    layers: array<vec4<u32>, 64>,
    count: vec4<u32>,
}
@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> terrain: TerrainUniform;
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var ground: texture_2d_array<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(2) var ground_sampler: sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(3) var masks: texture_2d_array<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(4) var mask_sampler: sampler;
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) alpha_uv: vec2<f32>,
    @location(2) light: vec3<f32>,
}
@vertex
fn vertex(input: Vertex) -> VertexOutput {
    let v = decompress_vertex(input, input.instance_index);
    let world_from_local = mesh_functions::get_world_from_local(input.instance_index);
    let position = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(v.position, 1.0));
    let normal = mesh_functions::mesh_normal_local_to_world(v.normal, input.instance_index);
    var out: VertexOutput;
    out.position = position_world_to_clip(position.xyz);
    out.uv = v.uv;
    out.alpha_uv = v.uv_b;
    let intensity = max(dot(normalize(normal), -normalize(vec3<f32>(0.35, -0.85, 0.4))), 0.0);
    out.light = clamp(vec3<f32>(0.35) + intensity * vec3<f32>(0.95, 0.93, 0.88), vec3<f32>(0.0), vec3<f32>(1.0));
    return out;
}
@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    var color = vec3<f32>(0.0);
    for (var i = 0u; i < terrain.count.x; i++) {
        let id = terrain.layers[i / 4u][i % 4u];
        let transform = terrain.transforms[id];
        let texel = textureSample(ground, ground_sampler, in.uv * transform.xy + transform.zw, i32(id)).rgb;
        let mask = textureSample(masks, mask_sampler, in.alpha_uv, i32(id)).r;
        // First used texture is the opaque base, just as in the original terrain pass.
        color = mix(color, texel, select(mask, 1.0, i == 0u));
    }
    return vec4<f32>(color * in.light, 1.0);
}
