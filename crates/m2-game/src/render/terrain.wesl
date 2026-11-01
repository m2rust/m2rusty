import bevy_pbr::render::{
    mesh_functions,
    forward_io::{Vertex, decompress_vertex},
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
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) alpha_uv: vec2<f32>,
    @location(2) light: vec3<f32>,
    // Distance along the view axis, the fog coordinate of the GL reference.
    @location(3) view_depth: f32,
}
@vertex
fn vertex(input: Vertex) -> VertexOutput {
    let v = decompress_vertex(input, input.instance_index);
    let world_from_local = mesh_functions::get_world_from_local(input.instance_index);
    let position = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(v.position, 1.0));
    let normal = mesh_functions::mesh_normal_local_to_world(v.normal, input.instance_index);
    var out: VertexOutput;
    out.position = position_world_to_clip(position.xyz);
    out.view_depth = -(view.view_from_world * position).z;
    out.uv = v.uv;
    out.alpha_uv = v.uv_b;
    // CTerrainPatch::SoftwareTransformPatch_UpdateTerrainLighting: the
    // absolute dot (the client does not clamp to the lit hemisphere), with the
    // diffuse capped so that ambient + diffuse never overflows one channel.
    var ndl = 0.0;
    if length(terrain.light_direction.xyz) > 1e-6 {
        ndl = abs(dot(normalize(normal), normalize(terrain.light_direction.xyz)));
    }
    let ambient = clamp(terrain.light_ambient.rgb, vec3<f32>(0.0), vec3<f32>(1.0));
    let diffuse = min(terrain.light_diffuse.rgb, vec3<f32>(1.0) - ambient);
    out.light = clamp(ambient + ndl * diffuse, vec3<f32>(0.0), vec3<f32>(1.0));
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
    color *= in.light;
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
    if terrain.fog_mode.x != 0u {
        color = mix(terrain.fog_color.rgb, color, clamp(fog_factor, 0.0, 1.0));
    }
    return vec4<f32>(color, 1.0);
}
