//! Per-vertex data the C++ software terrain pipeline computes besides the
//! projection: vertex lighting (`CTerrainPatch::
//! SoftwareTransformPatch_UpdateTerrainLighting`) and the three texture
//! coordinate sets (`CMapOutdoor::__SoftwareTransformPatch_SetTransform`).
//! A renderer using a GPU pipeline can use these as vertex attributes.

use crate::mesh::PatchVertex;

/// `float` to `DWORD` as gcc does on x86-64 (`cvttss2si`, low 32 bits).
fn float_to_dword(f: f32) -> u32 {
    let wide = if f.is_nan() || f.abs() >= 9.223_372e18 { i64::MIN } else { f as i64 };
    wide as u32
}

/// The fields of `D3DLIGHT9` the lighting uses.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Light {
    pub direction: [f32; 3],
    pub ambient: [f32; 3],
    pub diffuse: [f32; 3],
}

/// The fields of `D3DMATERIAL9` the lighting uses.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Material {
    pub ambient: [f32; 3],
    pub diffuse: [f32; 3],
    pub emissive: [f32; 3],
}

/// `SoftwareTransformPatch_UpdateTerrainLighting`: the `dwDiffuse` (ARGB)
/// of each vertex.
pub fn vertex_diffuse(vertices: &[PatchVertex], light: &Light, mtrl: &Material) -> Vec<u32> {
    let ambient = |c: usize| float_to_dword((mtrl.ambient[c] * light.ambient[c] + mtrl.emissive[c]) * 255.0f32);
    let diffuse = |c: usize| float_to_dword((mtrl.diffuse[c] * light.diffuse[c]) * 255.0f32);
    let (a_r, a_g, a_b) = (ambient(0), ambient(1), ambient(2));
    let (mut d_r, mut d_g, mut d_b) = (diffuse(0), diffuse(1), diffuse(2));
    d_r = d_r.min(255u32.wrapping_sub(a_r));
    if d_g.wrapping_add(a_g) > 255 {
        d_g = 255u32.wrapping_sub(a_g);
    }
    if d_b.wrapping_add(a_b) > 255 {
        d_b = 255u32.wrapping_sub(a_b);
    }
    const N: f32 = 0xff_ffff as f32;
    vertices
        .iter()
        .map(|v| {
            let l = light.direction;
            let dot = (v.normal[0] * l[0]) + (v.normal[1] * l[1]) + (v.normal[2] * l[2]);
            let dw_dot = if dot < 0.0 { float_to_dword(N * -dot) } else { float_to_dword(N * dot) };
            0xff00_0000
                | (((d_r.wrapping_mul(dw_dot) >> 24).wrapping_add(a_r)) << 16)
                | (((d_g.wrapping_mul(dw_dot) >> 24).wrapping_add(a_g)) << 8)
                | (d_b.wrapping_mul(dw_dot) >> 24).wrapping_add(a_b)
        })
        .collect()
}

/// Texture coordinates of one terrain vertex.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VertexUv {
    /// Repeating ground texture (`kTexTile`).
    pub tile: [f32; 2],
    /// Splat alpha map (`kTexAlpha`).
    pub alpha: [f32; 2],
    /// Static shadow map (`kTexStaticShadow`).
    pub static_shadow: [f32; 2],
}

/// The coordinates `__SoftwareTransformPatch_SetTransform` writes for a
/// patch of the terrain at `(terrain_x, terrain_y)`.
/// `splat_alpha` is `m_matSplatAlpha` ([`m2_formats::map::splat_alpha_matrix`]),
/// `tex_coord_base` is `m_fTerrainTexCoordBase`.
pub fn vertex_uvs(
    vertices: &[PatchVertex],
    terrain_x: u32,
    terrain_y: u32,
    tex_coord_base: f32,
    splat_alpha: &[f32; 16],
) -> Vec<VertexUv> {
    let tile_x = 1.0f32 / 640.0f32;
    let tile_y = -1.0f32 / 640.0f32;
    let base_x = -((terrain_x.wrapping_mul(25600)) as f32) + (tex_coord_base * 12.30769f32);
    let base_y = ((terrain_y.wrapping_mul(25600)) as f32) + (tex_coord_base * 12.30769f32);
    let (a_x, a_y, bias_x, bias_y) = (splat_alpha[0], splat_alpha[5], splat_alpha[12], splat_alpha[13]);
    let shadow_x = tex_coord_base * (16.0f32 / 128.0);
    let shadow_y = -tex_coord_base * (16.0f32 / 128.0);
    vertices
        .iter()
        .map(|v| {
            let p = v.position;
            let (lx, ly) = (p[0] + base_x, p[1] + base_y);
            VertexUv {
                tile: [p[0] * tile_x, p[1] * tile_y],
                alpha: [(lx * a_x) + bias_x, (ly * a_y) + bias_y],
                static_shadow: [lx * shadow_x, ly * shadow_y],
            }
        })
        .collect()
}
