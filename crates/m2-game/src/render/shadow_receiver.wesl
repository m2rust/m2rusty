// The character shadow on walkable objects (bridges, platforms):
// `CMapOutdoor`'s shadow receivers, drawn again with the shadow texture on
// stage 1. Here a multiply-blended copy of the receiver's mesh outputs the
// shadow texel, white outside the shadow box (`D3DTADDRESS_BORDER`).
import bevy_pbr::render::forward_io::VertexOutput;

struct ShadowView {
    light_from_world: mat4x4<f32>,
    params: vec4<f32>,
}
@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<storage, read> character_shadow_view: ShadowView;
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var character_shadow: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(2) var character_shadow_sampler: sampler;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let light = character_shadow_view.light_from_world * vec4<f32>(in.world_position.xyz, 1.0);
    let clip = light.xy * character_shadow_view.params.x;
    let uv = vec2<f32>(clip.x * 0.5 + 0.5, 0.5 - clip.y * 0.5);
    let texel = textureSample(character_shadow, character_shadow_sampler, uv).rgb;
    let inside = all(abs(clip) < vec2<f32>(1.0));
    if character_shadow_view.params.y > 0.5 && inside {
        return vec4<f32>(texel, 1.0);
    }
    return vec4<f32>(1.0);
}
