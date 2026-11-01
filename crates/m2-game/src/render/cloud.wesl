// Cloud layer of the map environment (`CSkyBox::RenderCloud`).
//
// The client's stage is D3DTOP_MODULATEINVALPHA_ADDCOLOR with Arg1 = texture
// and Arg2 = diffuse (the cloud colour): rgb = tex + colour * (1 - tex.a),
// alpha = tex.a. The material blends ONE / INVSRCCOLOR, so the returned rgb is
// added and the destination is scaled by its inverse.

import bevy_pbr::render::{
    mesh_functions,
    forward_io::{Vertex, decompress_vertex},
    view_transformations::position_world_to_clip,
};

struct CloudUniform {
    color: vec4<f32>,
    offset: vec2<f32>,
    scale: vec2<f32>,
    _pad: vec4<f32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> cloud: CloudUniform;
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var image: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(2) var image_sampler: sampler;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vertex(input: Vertex) -> VertexOutput {
    let v = decompress_vertex(input, input.instance_index);
    let world_from_local = mesh_functions::get_world_from_local(input.instance_index);
    let position = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(v.position, 1.0));
    var out: VertexOutput;
    out.position = position_world_to_clip(position.xyz);
    out.uv = v.uv;
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex = textureSample(image, image_sampler, in.uv * cloud.scale + cloud.offset);
    return vec4<f32>(tex.rgb + cloud.color.rgb * (1.0 - tex.a), tex.a);
}
